//! Тесты ручек на уровне соединения. Каждый работает в транзакции, которая
//! откатывается при выходе, поэтому тесты не видят друг друга и не оставляют
//! мусора. Хендлеры как HTTP проверяются отдельно, в `tests/http.rs`.

mod support;

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
