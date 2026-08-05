//! Тесты ручек на уровне соединения. Каждый работает в транзакции, которая
//! откатывается при выходе, поэтому тесты не видят друг друга и не оставляют
//! мусора. Хендлеры как HTTP проверяются отдельно, в `tests/http.rs`.

mod support;

use backend::api::entries as api_entries;
use backend::api::error::ApiError;
use backend::api::meetings;
use backend::api::participants;
use backend::db;
use backend::db::meetings::NewMeeting;
use chrono::{NaiveDate, TimeZone, Utc};
use support::test_pool;
use uuid::Uuid;

/// Имя поля из ошибки валидации.
///
/// Отдельная функция, а не `matches!` с guard'ом в каждом тесте: поле в ошибке
/// объявлено как `&'static str`, и режимы связывания в паттерне делают из него
/// то `&str`, то `&&str` — сравнение приходилось бы подгонять под компилятор.
/// Заодно при ошибке другого рода в сообщении видно, что пришло вместо неё.
fn validation_field(error: &ApiError) -> &str {
    match error {
        ApiError::Validation { field, .. } => field,
        other => panic!("ожидалась ошибка валидации, получено: {other:?}"),
    }
}

/// Встреча-заготовка с четырьмя участниками — «Дача у Влада» из спеки.
/// Возвращает идентификатор встречи и идентификаторы участников по порядку.
async fn seed_dacha(conn: &mut sqlx::PgConnection) -> (Uuid, Vec<Uuid>) {
    let meeting = db::meetings::insert(
        &mut *conn,
        NewMeeting {
            title: "Дача у Влада".to_owned(),
            description: "Три дня на природе".to_owned(),
            emoji: "🏡".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 7, 23).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи");

    let mut people = Vec::new();
    for name in ["Настя", "Влад", "Егор", "Марина"] {
        let row = db::participants::insert(&mut *conn, meeting.id, name, "🦊")
            .await
            .expect("вставка участника");
        people.push(row.id);
    }

    (meeting.id, people)
}

#[tokio::test]
async fn blank_title_becomes_the_default_one() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    // В дизайне встреча создаётся одной кнопкой, названия может не быть вовсе.
    let view = meetings::create_meeting(
        &mut tx,
        meetings::CreateMeeting {
            title: "   ".to_owned(),
            description: String::new(),
            emoji: None,
            held_on: None,
        },
    )
    .await
    .expect("создание встречи");

    assert_eq!(view.title, "Новая встреча");
    assert_eq!(view.emoji, "✨");
    assert_eq!(view.held_on, Utc::now().date_naive());
    // У `MeetingStatusView` нет `Display` — статус сверяется в том виде,
    // в каком уйдёт клиенту.
    assert_eq!(
        serde_json::to_value(&view.status).expect("сериализация"),
        "no-participants"
    );
}

#[tokio::test]
async fn creation_writes_a_log_line() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let view = meetings::create_meeting(
        &mut tx,
        meetings::CreateMeeting {
            title: "Дача у Влада".to_owned(),
            description: "Три дня".to_owned(),
            emoji: Some("🏡".to_owned()),
            held_on: NaiveDate::from_ymd_opt(2026, 7, 23),
        },
    )
    .await
    .expect("создание встречи");

    assert_eq!(view.title, "Дача у Влада");
    assert_eq!(view.emoji, "🏡");
    assert_eq!(
        view.held_on,
        NaiveDate::from_ymd_opt(2026, 7, 23).expect("дата")
    );
    // Лог пишется в той же транзакции, что и сама встреча, поэтому он уже виден
    // в ответе — второго запроса клиенту не нужно.
    assert_eq!(view.log.len(), 1);
    assert_eq!(view.log[0].text, "Встреча создана");
}

#[tokio::test]
async fn missing_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let error = meetings::load_view(&mut tx, Uuid::nil())
        .await
        .expect_err("несуществующая встреча");

    assert!(
        matches!(error, ApiError::NotFound),
        "ожидался NotFound, получено: {error:?}"
    );
}

#[tokio::test]
async fn meeting_view_reports_computed_money() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    for (payer, amount) in [(people[1], 8400), (people[0], 3200), (people[3], 2100)] {
        db::entries::insert(
            &mut tx,
            meeting_id,
            db::entries::NewEntry {
                kind: db::records::EntryKindRow::Expense,
                payer_id: payer,
                recipient_id: None,
                amount_rubles: amount,
                description: "Продукты".to_owned(),
                occurred_at: None,
                shares: Vec::new(),
            },
        )
        .await
        .expect("вставка расхода");
    }

    let view = meetings::load_view(&mut tx, meeting_id)
        .await
        .expect("чтение встречи");

    // Те же числа, что в доменном тесте фикстуры: считает домен, ручка их
    // только раскладывает по полям.
    assert_eq!(view.totals.spent_rubles, 13700);
    assert_eq!(view.totals.per_person_rubles, 3425);
    assert_eq!(view.totals.pending_transfers, 3);
    assert_eq!(view.participants[1].contributed_rubles, 8400);
    assert_eq!(view.participants[1].net_rubles, 4975);
    assert_eq!(view.participants[2].contributed_rubles, 0);
    assert_eq!(view.participants[2].net_rubles, -3425);
    assert_eq!(view.settlement.len(), 3);
    assert_eq!(view.settlement[0].from_id, people[2]);
    assert_eq!(view.settlement[0].to_id, people[1]);
    assert_eq!(view.settlement[0].amount_rubles, 3425);
}

#[tokio::test]
async fn patch_changes_only_the_given_fields() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, _) = seed_dacha(&mut tx).await;

    let view = meetings::update_meeting(
        &mut tx,
        meeting_id,
        meetings::UpdateMeeting {
            title: None,
            description: Some("Новое описание".to_owned()),
            emoji: None,
            held_on: None,
        },
    )
    .await
    .expect("правка встречи");

    assert_eq!(view.title, "Дача у Влада");
    assert_eq!(view.description, "Новое описание");
    assert_eq!(view.emoji, "🏡");
    assert_eq!(view.log[0].text, "Встреча отредактирована");
}

#[tokio::test]
async fn patch_can_clear_the_description() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, _) = seed_dacha(&mut tx).await;

    // Пустая строка — это значение, а не «не менять»: `coalesce` в запросе
    // отличает NULL от пустой строки, и описание можно убрать.
    let view = meetings::update_meeting(
        &mut tx,
        meeting_id,
        meetings::UpdateMeeting {
            title: None,
            description: Some(String::new()),
            emoji: None,
            held_on: None,
        },
    )
    .await
    .expect("правка встречи");

    assert_eq!(view.description, "");
}

#[tokio::test]
async fn blank_title_in_patch_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, _) = seed_dacha(&mut tx).await;

    // На создании пустое название допустимо — там формы может не быть вовсе.
    // На правке пользователь смотрит в это поле, и молча подставить
    // «Новая встреча» вместо его текста значило бы соврать.
    let error = meetings::update_meeting(
        &mut tx,
        meeting_id,
        meetings::UpdateMeeting {
            title: Some("  ".to_owned()),
            description: None,
            emoji: None,
            held_on: None,
        },
    )
    .await
    .expect_err("пустое название");

    assert_eq!(validation_field(&error), "title");
}

#[tokio::test]
async fn patch_of_missing_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let error = meetings::update_meeting(
        &mut tx,
        Uuid::nil(),
        meetings::UpdateMeeting {
            title: Some("Что-нибудь".to_owned()),
            description: None,
            emoji: None,
            held_on: None,
        },
    )
    .await
    .expect_err("несуществующая встреча");

    assert!(matches!(error, ApiError::NotFound), "получено: {error:?}");
}

#[tokio::test]
async fn delete_takes_the_meeting_and_everything_under_it() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    db::entries::insert(
        &mut tx,
        meeting_id,
        db::entries::NewEntry {
            kind: db::records::EntryKindRow::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 500,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("вставка расхода");

    meetings::delete_meeting(&mut tx, meeting_id)
        .await
        .expect("удаление встречи");

    let entries: i64 = sqlx::query_scalar("select count(*) from entries where meeting_id = $1")
        .bind(meeting_id)
        .fetch_one(&mut *tx)
        .await
        .expect("подсчёт записей");
    let participants: i64 =
        sqlx::query_scalar("select count(*) from participants where meeting_id = $1")
            .bind(meeting_id)
            .fetch_one(&mut *tx)
            .await
            .expect("подсчёт участников");

    assert_eq!(entries, 0);
    assert_eq!(participants, 0);
    assert!(
        db::meetings::find(&mut tx, meeting_id)
            .await
            .expect("чтение встречи")
            .is_none()
    );
}

#[tokio::test]
async fn delete_of_missing_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let error = meetings::delete_meeting(&mut tx, Uuid::nil())
        .await
        .expect_err("несуществующая встреча");

    assert!(matches!(error, ApiError::NotFound), "получено: {error:?}");
}

#[tokio::test]
async fn entries_come_newest_first() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let early = Utc
        .with_ymd_and_hms(2026, 7, 23, 18, 0, 0)
        .single()
        .expect("время");
    let late = Utc
        .with_ymd_and_hms(2026, 7, 23, 21, 0, 0)
        .single()
        .expect("время");

    for (amount, occurred_at) in [(100, early), (200, late)] {
        db::entries::insert(
            &mut tx,
            meeting_id,
            db::entries::NewEntry {
                kind: db::records::EntryKindRow::Expense,
                payer_id: people[0],
                recipient_id: None,
                amount_rubles: amount,
                description: String::new(),
                occurred_at: Some(occurred_at),
                shares: Vec::new(),
            },
        )
        .await
        .expect("вставка расхода");
    }

    let view = meetings::load_view(&mut tx, meeting_id)
        .await
        .expect("чтение встречи");

    assert_eq!(view.entries[0].amount_rubles, 200);
    assert_eq!(view.entries[1].amount_rubles, 100);
}

#[tokio::test]
async fn log_returns_twelve_newest_records() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, _) = seed_dacha(&mut tx).await;

    for index in 0..15 {
        db::log::append(&mut tx, meeting_id, &format!("запись {index}"))
            .await
            .expect("запись лога");
    }

    let view = meetings::load_view(&mut tx, meeting_id)
        .await
        .expect("чтение встречи");

    // Спека: последние 12, новые сверху. `now()` в Postgres — время начала
    // транзакции, поэтому у всех записей одинаковый `created_at` и порядок
    // держится на `id`.
    assert_eq!(view.log.len(), 12);
    assert_eq!(view.log[0].text, "запись 14");
    assert_eq!(view.log[11].text, "запись 3");
}

#[tokio::test]
async fn server_assigns_position_and_colour() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting = meetings::create_meeting(
        &mut tx,
        meetings::CreateMeeting {
            title: "Пустая".to_owned(),
            description: String::new(),
            emoji: None,
            held_on: None,
        },
    )
    .await
    .expect("создание встречи");

    let view = participants::add_participant(
        &mut tx,
        meeting.id,
        participants::CreateParticipant {
            name: "  Настя  ".to_owned(),
            emoji: Some("🦊".to_owned()),
        },
    )
    .await
    .expect("добавление участника");

    // Имя обрезается по краям, позицию и цвет назначает база.
    assert_eq!(view.participants[0].name, "Настя");
    assert_eq!(view.participants[0].emoji, "🦊");
    assert_eq!(view.participants[0].position, 0);
    assert_eq!(view.participants[0].color_index, 0);
    assert_eq!(view.log[0].text, "Настя присоединяется к встрече");
}

#[tokio::test]
async fn participant_without_emoji_gets_the_default_one() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, _) = seed_dacha(&mut tx).await;

    let view = participants::add_participant(
        &mut tx,
        meeting_id,
        participants::CreateParticipant {
            name: "Лёша".to_owned(),
            emoji: None,
        },
    )
    .await
    .expect("добавление участника");

    let added = view
        .participants
        .iter()
        .find(|row| row.name == "Лёша")
        .expect("новый участник в ответе");

    assert_eq!(added.emoji, "🐻");
}

#[tokio::test]
async fn blank_participant_name_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, _) = seed_dacha(&mut tx).await;

    let error = participants::add_participant(
        &mut tx,
        meeting_id,
        participants::CreateParticipant {
            name: "   ".to_owned(),
            emoji: None,
        },
    )
    .await
    .expect_err("пустое имя");

    assert_eq!(validation_field(&error), "name");
}

#[tokio::test]
async fn participant_in_a_missing_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    // Без проверки существования встречи здесь была бы ошибка внешнего ключа,
    // то есть `500` вместо `404`.
    let error = participants::add_participant(
        &mut tx,
        Uuid::nil(),
        participants::CreateParticipant {
            name: "Настя".to_owned(),
            emoji: None,
        },
    )
    .await
    .expect_err("несуществующая встреча");

    assert!(matches!(error, ApiError::NotFound), "получено: {error:?}");
}

#[tokio::test]
async fn patch_keeps_the_untouched_field() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let view = participants::update_participant(
        &mut tx,
        meeting_id,
        people[0],
        participants::UpdateParticipant {
            name: Some("Анастасия".to_owned()),
            emoji: None,
        },
    )
    .await
    .expect("правка участника");

    assert_eq!(view.participants[0].name, "Анастасия");
    assert_eq!(view.participants[0].emoji, "🦊");
    // Позиция и цвет закреплены за участником с момента добавления.
    assert_eq!(view.participants[0].position, 0);
    assert_eq!(view.log[0].text, "Профиль участника обновлён: Анастасия");
}

#[tokio::test]
async fn participant_of_another_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (first_meeting, first_people) = seed_dacha(&mut tx).await;
    let (second_meeting, _) = seed_dacha(&mut tx).await;
    assert_ne!(first_meeting, second_meeting);

    // Участник существует, но не в этой встрече — по этому адресу его нет.
    // Без проверки правка прошла бы и изменила чужую встречу.
    let error = participants::update_participant(
        &mut tx,
        second_meeting,
        first_people[0],
        participants::UpdateParticipant {
            name: Some("Кто-то".to_owned()),
            emoji: None,
        },
    )
    .await
    .expect_err("чужой участник");

    assert!(matches!(error, ApiError::NotFound), "получено: {error:?}");
}

#[tokio::test]
async fn deleting_a_participant_takes_their_entries() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    for payer in [people[0], people[1]] {
        db::entries::insert(
            &mut tx,
            meeting_id,
            db::entries::NewEntry {
                kind: db::records::EntryKindRow::Expense,
                payer_id: payer,
                recipient_id: None,
                amount_rubles: 400,
                description: String::new(),
                occurred_at: None,
                shares: Vec::new(),
            },
        )
        .await
        .expect("вставка расхода");
    }

    let view = participants::remove_participant(&mut tx, meeting_id, people[0])
        .await
        .expect("удаление участника");

    // Требование дизайна: вместе с участником уходят все записи, где он
    // плательщик или получатель. Значит и сумма встречи падает.
    assert_eq!(view.participants.len(), 3);
    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.totals.spent_rubles, 400);
    // Имя взято до удаления — после него взять его уже негде.
    assert_eq!(view.log[0].text, "Участник удалён: Настя");
}

#[tokio::test]
async fn deleting_a_participant_of_another_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (_, people) = seed_dacha(&mut tx).await;
    let (other_meeting, _) = seed_dacha(&mut tx).await;

    let error = participants::remove_participant(&mut tx, other_meeting, people[0])
        .await
        .expect_err("чужой участник");

    assert!(matches!(error, ApiError::NotFound), "получено: {error:?}");
}

#[tokio::test]
async fn expense_is_split_evenly_by_default() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let view = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 400,
            description: "Продукты".to_owned(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("расход");

    assert_eq!(view.totals.spent_rubles, 400);
    assert!(view.entries[0].shared_by_all);
    assert_eq!(view.participants[0].net_rubles, 300);
    assert_eq!(view.participants[1].net_rubles, -100);
    assert_eq!(
        view.log[0].text,
        "Настя добавляет расход «Продукты» — 400\u{a0}₽"
    );
}

#[tokio::test]
async fn blank_expense_description_gets_the_default_one() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let view = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 400,
            description: "   ".to_owned(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("расход");

    assert_eq!(view.entries[0].description, "Без описания");
    assert_eq!(
        view.log[0].text,
        "Настя добавляет расход «Без описания» — 400\u{a0}₽"
    );
}

#[tokio::test]
async fn transfer_points_at_its_recipient() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let view = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Transfer,
            payer_id: people[0],
            recipient_id: Some(people[1]),
            amount_rubles: 225,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("перевод");

    // Перевод не увеличивает сумму встречи — он только двигает балансы.
    assert_eq!(view.totals.spent_rubles, 0);
    assert_eq!(view.participants[0].net_rubles, 225);
    assert_eq!(view.participants[1].net_rubles, -225);
    // «внёс» — только оплаченные расходы, перевод сюда не входит.
    assert_eq!(view.participants[0].contributed_rubles, 0);
    assert_eq!(view.log[0].text, "Настя переводит 225\u{a0}₽ → Влад");
}

#[tokio::test]
async fn partial_shares_are_stored_without_the_full_ones() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let view = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 100,
            description: "Пиво".to_owned(),
            occurred_at: None,
            shares: vec![
                // Полная доля не хранится: её отсутствие и есть полная доля.
                api_entries::ShareInput {
                    participant_id: people[0],
                    weight_quarters: 4,
                },
                api_entries::ShareInput {
                    participant_id: people[1],
                    weight_quarters: 2,
                },
                api_entries::ShareInput {
                    participant_id: people[2],
                    weight_quarters: 0,
                },
            ],
        },
    )
    .await
    .expect("расход с долями");

    assert!(!view.entries[0].shared_by_all);
    assert_eq!(view.entries[0].shares.len(), 2);
    assert!(
        view.entries[0]
            .shares
            .iter()
            .all(|share| share.participant_id != people[0]),
        "полная доля не должна попадать в базу"
    );
    // Веса 1, ½, 0, 1 на 100 ₽ → 40, 20, 0, 40.
    assert_eq!(view.participants[2].net_rubles, 0);
    assert_eq!(view.participants[1].net_rubles, -20);
}

#[tokio::test]
async fn zero_amount_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 0,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect_err("нулевая сумма");

    assert_eq!(validation_field(&error), "amountRubles");
}

#[tokio::test]
async fn expense_with_a_recipient_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    // CHECK в схеме поймал бы это тоже, но ответом был бы `500`.
    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: people[0],
            recipient_id: Some(people[1]),
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect_err("у расхода нет получателя");

    assert_eq!(validation_field(&error), "recipientId");
}

#[tokio::test]
async fn transfer_without_a_recipient_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Transfer,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect_err("перевод без получателя");

    assert_eq!(validation_field(&error), "recipientId");
}

#[tokio::test]
async fn transfer_to_self_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Transfer,
            payer_id: people[0],
            recipient_id: Some(people[0]),
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect_err("перевод себе");

    assert_eq!(validation_field(&error), "recipientId");
}

#[tokio::test]
async fn transfer_with_shares_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    // Перевод не делится: доли у него бессмысленны, и домен их игнорирует.
    // Принять и выбросить — значит соврать клиенту, что он что-то настроил.
    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Transfer,
            payer_id: people[0],
            recipient_id: Some(people[1]),
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: vec![api_entries::ShareInput {
                participant_id: people[2],
                weight_quarters: 2,
            }],
        },
    )
    .await
    .expect_err("доли у перевода");

    assert_eq!(validation_field(&error), "shares");
}

#[tokio::test]
async fn payer_from_another_meeting_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, _) = seed_dacha(&mut tx).await;
    let (_, strangers) = seed_dacha(&mut tx).await;

    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: strangers[0],
            recipient_id: None,
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect_err("чужой плательщик");

    assert_eq!(validation_field(&error), "payerId");
}

#[tokio::test]
async fn duplicate_share_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    // Первичный ключ `(entry_id, participant_id)` поймал бы это сам, но ответом
    // был бы `500`, а причина осталась бы в логе сервера.
    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: vec![
                api_entries::ShareInput {
                    participant_id: people[1],
                    weight_quarters: 2,
                },
                api_entries::ShareInput {
                    participant_id: people[1],
                    weight_quarters: 1,
                },
            ],
        },
    )
    .await
    .expect_err("участник дважды в долях");

    assert_eq!(validation_field(&error), "shares");
}

#[tokio::test]
async fn share_out_of_range_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: vec![api_entries::ShareInput {
                participant_id: people[1],
                weight_quarters: 5,
            }],
        },
    )
    .await
    .expect_err("доля больше полной");

    assert_eq!(validation_field(&error), "shares");
}

#[tokio::test]
async fn entry_in_a_missing_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (_, people) = seed_dacha(&mut tx).await;

    let error = api_entries::add_entry(
        &mut tx,
        Uuid::nil(),
        api_entries::CreateEntry {
            kind: api_entries::EntryKindInput::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect_err("несуществующая встреча");

    assert!(matches!(error, ApiError::NotFound), "получено: {error:?}");
}

/// Заготовка: расход на 400 ₽ от указанного участника, делится на всех.
async fn seed_expense(conn: &mut sqlx::PgConnection, meeting_id: Uuid, payer: Uuid) -> Uuid {
    db::entries::insert(
        &mut *conn,
        meeting_id,
        db::entries::NewEntry {
            kind: db::records::EntryKindRow::Expense,
            payer_id: payer,
            recipient_id: None,
            amount_rubles: 400,
            description: "Продукты".to_owned(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("вставка расхода")
    .id
}

#[tokio::test]
async fn patch_replaces_shares_wholesale() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    let entry_id = seed_expense(&mut tx, meeting_id, people[0]).await;

    let view = api_entries::update_entry(
        &mut tx,
        meeting_id,
        entry_id,
        api_entries::UpdateEntry {
            payer_id: None,
            recipient_id: None,
            amount_rubles: Some(1000),
            description: Some("Мясо".to_owned()),
            occurred_at: None,
            shares: Some(vec![api_entries::ShareInput {
                participant_id: people[3],
                weight_quarters: 0,
            }]),
        },
    )
    .await
    .expect("правка записи");

    assert_eq!(view.entries[0].amount_rubles, 1000);
    assert_eq!(view.entries[0].description, "Мясо");
    assert!(!view.entries[0].shared_by_all);
    // 1000 на трёх с полной долей → 334 / 333 / 333, четвёртый не платит.
    assert_eq!(view.participants[3].net_rubles, 0);
    assert_eq!(view.log[0].text, "Запись изменена: 1\u{a0}000\u{a0}₽");
}

#[tokio::test]
async fn empty_shares_return_the_expense_to_an_even_split() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    let entry_id = db::entries::insert(
        &mut tx,
        meeting_id,
        db::entries::NewEntry {
            kind: db::records::EntryKindRow::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 400,
            description: "Продукты".to_owned(),
            occurred_at: None,
            shares: vec![(people[3], 0)],
        },
    )
    .await
    .expect("вставка расхода")
    .id;

    // `Some(vec![])` — «снять все неполные доли», в отличие от `None`,
    // означающего «доли не трогать».
    let view = api_entries::update_entry(
        &mut tx,
        meeting_id,
        entry_id,
        api_entries::UpdateEntry {
            payer_id: None,
            recipient_id: None,
            amount_rubles: None,
            description: None,
            occurred_at: None,
            shares: Some(Vec::new()),
        },
    )
    .await
    .expect("правка записи");

    assert!(view.entries[0].shared_by_all);
    assert_eq!(view.participants[3].net_rubles, -100);
}

#[tokio::test]
async fn patch_cannot_add_a_recipient_to_an_expense() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    let entry_id = seed_expense(&mut tx, meeting_id, people[0]).await;

    // Превращение расхода в перевод — это другая запись; в интерфейсе это
    // делается удалением и повторным вводом.
    let error = api_entries::update_entry(
        &mut tx,
        meeting_id,
        entry_id,
        api_entries::UpdateEntry {
            payer_id: None,
            recipient_id: Some(people[1]),
            amount_rubles: None,
            description: None,
            occurred_at: None,
            shares: None,
        },
    )
    .await
    .expect_err("получатель у расхода");

    assert_eq!(validation_field(&error), "recipientId");
}

#[tokio::test]
async fn patch_cannot_make_a_transfer_point_at_its_payer() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    let entry_id = db::entries::insert(
        &mut tx,
        meeting_id,
        db::entries::NewEntry {
            kind: db::records::EntryKindRow::Transfer,
            payer_id: people[0],
            recipient_id: Some(people[1]),
            amount_rubles: 225,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("вставка перевода")
    .id;

    // Проверять надо действующие значения, а не присланные: меняется
    // плательщик, а совпасть он может с получателем, которого в теле нет.
    let error = api_entries::update_entry(
        &mut tx,
        meeting_id,
        entry_id,
        api_entries::UpdateEntry {
            payer_id: Some(people[1]),
            recipient_id: None,
            amount_rubles: None,
            description: None,
            occurred_at: None,
            shares: None,
        },
    )
    .await
    .expect_err("перевод сам себе");

    assert_eq!(validation_field(&error), "payerId");
}

#[tokio::test]
async fn patch_of_an_entry_from_another_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (first_meeting, first_people) = seed_dacha(&mut tx).await;
    let (second_meeting, _) = seed_dacha(&mut tx).await;
    let entry_id = seed_expense(&mut tx, first_meeting, first_people[0]).await;

    let error = api_entries::update_entry(
        &mut tx,
        second_meeting,
        entry_id,
        api_entries::UpdateEntry {
            payer_id: None,
            recipient_id: None,
            amount_rubles: Some(1),
            description: None,
            occurred_at: None,
            shares: None,
        },
    )
    .await
    .expect_err("чужая запись");

    assert!(matches!(error, ApiError::NotFound), "получено: {error:?}");
}

#[tokio::test]
async fn delete_logs_the_amount_and_removes_the_entry() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    let entry_id = seed_expense(&mut tx, meeting_id, people[0]).await;

    let view = api_entries::remove_entry(&mut tx, meeting_id, entry_id)
        .await
        .expect("удаление записи");

    assert!(view.entries.is_empty());
    assert_eq!(view.totals.spent_rubles, 0);
    assert_eq!(view.log[0].text, "Удалена запись на 400\u{a0}₽");
}

#[tokio::test]
async fn delete_of_an_entry_from_another_meeting_is_not_found() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let (first_meeting, first_people) = seed_dacha(&mut tx).await;
    let (second_meeting, _) = seed_dacha(&mut tx).await;
    let entry_id = seed_expense(&mut tx, first_meeting, first_people[0]).await;

    let error = api_entries::remove_entry(&mut tx, second_meeting, entry_id)
        .await
        .expect_err("чужая запись");

    assert!(matches!(error, ApiError::NotFound), "получено: {error:?}");
}

/// Уникальная метка в названии: список читает всю таблицу, и тест не должен
/// зависеть от того, что осталось в бранче `test` от предыдущих прогонов.
fn marker() -> String {
    Uuid::new_v4().to_string()
}

/// Встреча с одной датой, одним расходом и двумя участниками — всё, что нужно
/// тестам списка.
async fn seed_card(
    conn: &mut sqlx::PgConnection,
    marker: &str,
    title: &str,
    held_on: (i32, u32, u32),
    participant: &str,
    amount: i64,
) -> Uuid {
    let meeting = db::meetings::insert(
        &mut *conn,
        NewMeeting {
            title: format!("{title} {marker}"),
            description: String::new(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(held_on.0, held_on.1, held_on.2).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи");

    let person = db::participants::insert(&mut *conn, meeting.id, participant, "🦊")
        .await
        .expect("вставка участника");
    // Второй участник нужен, чтобы расход было между кем делить: на одном
    // участнике баланс всегда нулевой, и открытую встречу не отличить
    // от закрытой.
    let _second = db::participants::insert(&mut *conn, meeting.id, "Второй", "🐸")
        .await
        .expect("вставка участника");

    if amount > 0 {
        db::entries::insert(
            &mut *conn,
            meeting.id,
            db::entries::NewEntry {
                kind: db::records::EntryKindRow::Expense,
                payer_id: person.id,
                recipient_id: None,
                amount_rubles: amount,
                description: String::new(),
                occurred_at: None,
                shares: Vec::new(),
            },
        )
        .await
        .expect("вставка расхода");
    }

    meeting.id
}

#[tokio::test]
async fn list_returns_cards_with_computed_totals() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let marker = marker();
    seed_card(&mut tx, &marker, "Шашлыки", (2026, 7, 20), "Настя", 500).await;

    let cards = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: Some(marker.clone()),
            participant: None,
            sort: None,
        },
    )
    .await
    .expect("список встреч");

    assert_eq!(cards.len(), 1);
    assert_eq!(cards[0].total_rubles, 500);
    assert_eq!(cards[0].participants.len(), 2);
    assert_eq!(cards[0].pending_transfers, 1);
}

#[tokio::test]
async fn query_matches_title_and_description() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let marker = marker();
    let meeting_id = seed_card(&mut tx, &marker, "Шашлыки", (2026, 7, 20), "Настя", 0).await;
    let unique_word = format!("солёные{marker}");
    db::meetings::update(
        &mut tx,
        meeting_id,
        db::meetings::MeetingPatch {
            title: None,
            description: Some(unique_word.clone()),
            emoji: None,
            held_on: None,
        },
    )
    .await
    .expect("правка встречи");

    // Регистр не важен, и совпадение по описанию считается наравне с названием.
    let by_description = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: Some(unique_word.to_uppercase()),
            participant: None,
            sort: None,
        },
    )
    .await
    .expect("список встреч");

    assert!(by_description.iter().any(|card| card.id == meeting_id));
}

#[tokio::test]
async fn percent_in_query_is_not_a_wildcard() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let marker = marker();
    seed_card(&mut tx, &marker, "Шашлыки", (2026, 7, 20), "Настя", 0).await;

    // С `like '%' || $1 || '%'` этот поиск вернул бы все встречи вообще.
    let cards = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: Some("%".to_owned()),
            participant: None,
            sort: None,
        },
    )
    .await
    .expect("список встреч");

    assert!(cards.is_empty(), "получено карточек: {}", cards.len());
}

#[tokio::test]
async fn participant_filter_keeps_only_meetings_with_that_name() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let marker = marker();
    let unique_name = format!("Настя {marker}");
    let with_her = seed_card(&mut tx, &marker, "Шашлыки", (2026, 7, 20), &unique_name, 0).await;
    let without_her = seed_card(&mut tx, &marker, "Кино", (2026, 7, 21), "Егор", 0).await;

    let cards = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: None,
            participant: Some(unique_name),
            sort: None,
        },
    )
    .await
    .expect("список встреч");

    let ids: Vec<Uuid> = cards.iter().map(|card| card.id).collect();
    assert!(ids.contains(&with_her));
    assert!(!ids.contains(&without_her));
}

#[tokio::test]
async fn default_sort_is_newest_meeting_first() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let marker = marker();
    let older = seed_card(&mut tx, &marker, "Раньше", (2026, 7, 1), "Настя", 0).await;
    let newer = seed_card(&mut tx, &marker, "Позже", (2026, 7, 30), "Настя", 0).await;

    let cards = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: Some(marker.clone()),
            participant: None,
            sort: None,
        },
    )
    .await
    .expect("список встреч");

    assert_eq!(cards[0].id, newer);
    assert_eq!(cards[1].id, older);

    let reversed = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: Some(marker),
            participant: None,
            sort: Some("date-asc".to_owned()),
        },
    )
    .await
    .expect("список встреч");

    assert_eq!(reversed[0].id, older);
    assert_eq!(reversed[1].id, newer);
}

#[tokio::test]
async fn total_desc_puts_the_expensive_meeting_first() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let marker = marker();
    // Дешёвая встреча позже дорогой, поэтому по умолчанию она была бы первой.
    let cheap = seed_card(&mut tx, &marker, "Дешёвая", (2026, 7, 30), "Настя", 100).await;
    let costly = seed_card(&mut tx, &marker, "Дорогая", (2026, 7, 1), "Настя", 9000).await;

    let cards = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: Some(marker),
            participant: None,
            sort: Some("total-desc".to_owned()),
        },
    )
    .await
    .expect("список встреч");

    assert_eq!(cards[0].id, costly);
    assert_eq!(cards[1].id, cheap);
}

#[tokio::test]
async fn open_first_puts_the_most_open_meeting_first() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let marker = marker();
    // Закрытая встреча позже открытой: без сортировки она была бы первой.
    let settled = seed_card(&mut tx, &marker, "Закрытая", (2026, 7, 30), "Настя", 0).await;
    let open = seed_card(&mut tx, &marker, "Открытая", (2026, 7, 1), "Настя", 100).await;

    let cards = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: Some(marker),
            participant: None,
            sort: Some("open-first".to_owned()),
        },
    )
    .await
    .expect("список встреч");

    assert_eq!(cards[0].id, open);
    assert_eq!(cards[0].pending_transfers, 1);
    assert_eq!(cards[1].id, settled);
    assert_eq!(cards[1].pending_transfers, 0);
}

#[tokio::test]
async fn unknown_sort_is_rejected() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let error = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: None,
            participant: None,
            sort: Some("по-настроению".to_owned()),
        },
    )
    .await
    .expect_err("неизвестная сортировка");

    assert_eq!(validation_field(&error), "sort");
}

#[tokio::test]
async fn blank_filters_are_treated_as_absent() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let marker = marker();
    let meeting_id = seed_card(&mut tx, &marker, "Шашлыки", (2026, 7, 20), "Настя", 0).await;

    // Пустая строка приходит от инпута, который пользователь очистил.
    // Считать её фильтром «название содержит пустоту» значит вернуть всё,
    // но по коду это должно быть «фильтра нет», а не «фильтр пустой».
    let cards = meetings::load_cards(
        &mut tx,
        meetings::ListFilters {
            query: Some("   ".to_owned()),
            participant: Some(String::new()),
            sort: Some(String::new()),
        },
    )
    .await
    .expect("список встреч");

    assert!(cards.iter().any(|card| card.id == meeting_id));
}
