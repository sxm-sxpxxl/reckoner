mod support;

use backend::db::entries::{self, NewEntry};
use backend::db::facts;
use backend::db::log;
use backend::db::meetings::{self, MeetingPatch, NewMeeting};
use backend::db::participants;
use backend::db::records::EntryKindRow;
use backend::domain::{MeetingStatus, ParticipantId, reckon};
use chrono::NaiveDate;
use support::test_pool;

/// Встреча-заготовка: почти каждому тесту нужна встреча и ничего больше.
async fn seed_meeting(tx: &mut sqlx::PgConnection) -> uuid::Uuid {
    meetings::insert(
        tx,
        NewMeeting {
            title: "Тестовая".to_owned(),
            description: String::new(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи")
    .id
}

#[tokio::test]
async fn migrations_apply_and_schema_is_queryable() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    // Пять таблиц схемы должны существовать после миграций.
    let tables: Vec<String> = sqlx::query_scalar(
        "select table_name from information_schema.tables \
         where table_schema = 'public' and table_type = 'BASE TABLE' \
         order by table_name",
    )
    .fetch_all(&mut *tx)
    .await
    .expect("список таблиц");

    for expected in [
        "entries",
        "entry_shares",
        "meeting_log",
        "meetings",
        "participants",
    ] {
        assert!(
            tables.iter().any(|name| name == expected),
            "нет таблицы {expected}; есть: {tables:?}"
        );
    }
}

#[tokio::test]
async fn entry_kind_round_trips_through_a_text_column() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    // Отображение перечисления на колонку `text` — то место, где компиляция
    // ничего не доказывает. Строки обязаны совпадать со значениями в CHECK
    // схемы, иначе вставка записи упадёт в рантайме, а не при сборке.
    for (kind, expected) in [
        (EntryKindRow::Expense, "expense"),
        (EntryKindRow::Transfer, "transfer"),
    ] {
        let encoded: String = sqlx::query_scalar("select $1::text")
            .bind(kind)
            .fetch_one(&mut *tx)
            .await
            .expect("кодирование в text");
        assert_eq!(encoded, expected, "kind {kind:?} закодировался не так");

        let decoded: EntryKindRow = sqlx::query_scalar("select $1::text")
            .bind(expected)
            .fetch_one(&mut *tx)
            .await
            .expect("декодирование из text");
        assert_eq!(decoded, kind, "строка {expected} раскодировалась не так");
    }
}

#[tokio::test]
async fn inserts_and_reads_back_a_meeting() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "Дача у Влада".to_owned(),
            description: "Три дня, баня и продукты из «Ленты».".to_owned(),
            emoji: "🏡".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 7, 23).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи");

    assert_eq!(created.title, "Дача у Влада");
    assert_eq!(created.emoji, "🏡");
    assert_eq!(created.cover_version, 0);
    assert!(created.cover_mime.is_none());

    let found = meetings::find(&mut tx, created.id)
        .await
        .expect("чтение встречи")
        .expect("встреча существует");

    assert_eq!(found.id, created.id);
    assert_eq!(found.held_on, created.held_on);
}

#[tokio::test]
async fn returns_none_for_a_missing_meeting() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let found = meetings::find(&mut tx, uuid::Uuid::nil())
        .await
        .expect("запрос выполнен");

    assert!(found.is_none());
}

#[tokio::test]
async fn patches_only_provided_fields() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "Новая встреча".to_owned(),
            description: "черновик".to_owned(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка");

    let patched = meetings::update(
        &mut tx,
        created.id,
        MeetingPatch {
            title: Some("Солевые шашлыки".to_owned()),
            emoji: Some("🔥".to_owned()),
            ..MeetingPatch::default()
        },
    )
    .await
    .expect("правка")
    .expect("встреча существует");

    assert_eq!(patched.title, "Солевые шашлыки");
    assert_eq!(patched.emoji, "🔥");
    // Не переданные поля остаются как были.
    assert_eq!(patched.description, "черновик");
    assert_eq!(patched.held_on, created.held_on);
    // Правка двигает updated_at.
    assert!(patched.updated_at >= created.updated_at);
}

#[tokio::test]
async fn deletes_a_meeting_and_reports_whether_it_existed() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "На удаление".to_owned(),
            description: String::new(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка");

    assert!(
        meetings::delete(&mut tx, created.id)
            .await
            .expect("удаление")
    );
    assert!(
        !meetings::delete(&mut tx, created.id)
            .await
            .expect("повторное удаление")
    );
    assert!(
        meetings::find(&mut tx, created.id)
            .await
            .expect("чтение")
            .is_none()
    );
}

#[tokio::test]
async fn assigns_position_and_color_by_order_of_addition() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let first = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("первый участник");
    let second = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй участник");

    assert_eq!(first.position, 0);
    assert_eq!(second.position, 1);
    // Палитра аватаров на 8 цветов; индекс идёт по кругу.
    assert_eq!(first.color_index, 0);
    assert_eq!(second.color_index, 1);
}

#[tokio::test]
async fn lists_participants_in_position_order() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    for (name, emoji) in [("Настя", "🦊"), ("Влад", "🦉"), ("Егор", "🐸")] {
        participants::insert(&mut tx, meeting_id, name, emoji)
            .await
            .expect("участник");
    }

    let listed = participants::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("список");

    let names: Vec<&str> = listed.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["Настя", "Влад", "Егор"]);
}

#[tokio::test]
async fn updates_and_deletes_a_participant() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let person = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("участник");

    let renamed = participants::update(&mut tx, person.id, "Анастасия", "🦩")
        .await
        .expect("правка")
        .expect("участник существует");
    assert_eq!(renamed.name, "Анастасия");
    assert_eq!(renamed.emoji, "🦩");
    // Позиция и цвет при правке не меняются.
    assert_eq!(renamed.position, person.position);
    assert_eq!(renamed.color_index, person.color_index);

    assert!(
        participants::delete(&mut tx, person.id)
            .await
            .expect("удаление")
    );
    assert!(
        participants::list_for_meeting(&mut tx, meeting_id)
            .await
            .expect("список")
            .is_empty()
    );
}

#[tokio::test]
async fn position_never_reuses_a_number_after_a_deletion() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let first = participants::insert(&mut tx, meeting_id, "Раз", "🐻")
        .await
        .expect("первый");
    participants::insert(&mut tx, meeting_id, "Два", "🦊")
        .await
        .expect("второй");
    participants::delete(&mut tx, first.id)
        .await
        .expect("удаление");

    // Номер берётся как max(position) + 1, поэтому третий получает 2, а не 1.
    // Счётчик по числу существующих участников выдал бы здесь 1 — уже занятый
    // «Два» номер — и вставка упала бы на уникальном индексе. Дырка на месте
    // удалённого остаётся навсегда, и это правильно.
    let third = participants::insert(&mut tx, meeting_id, "Три", "🐸")
        .await
        .expect("третий");
    assert_eq!(third.position, 2);

    let positions: Vec<i32> = participants::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("список")
        .iter()
        .map(|row| row.position)
        .collect();
    assert_eq!(positions, [1, 2]);
}

#[tokio::test]
async fn stores_an_expense_with_partial_shares() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");
    let other = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй");

    let entry = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 8400,
            description: "Продукты на все дни".to_owned(),
            occurred_at: None,
            shares: vec![(other.id, 2)],
        },
    )
    .await
    .expect("вставка расхода");

    assert_eq!(entry.amount_rubles, 8400);
    assert_eq!(entry.kind, EntryKindRow::Expense);
    assert!(entry.recipient_id.is_none());

    let shares = entries::shares_for_meeting(&mut tx, meeting_id)
        .await
        .expect("доли");
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].participant_id, other.id);
    assert_eq!(shares[0].weight_quarters, 2);
}

#[tokio::test]
async fn stores_a_transfer() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let from = participants::insert(&mut tx, meeting_id, "Егор", "🐸")
        .await
        .expect("отправитель");
    let to = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("получатель");

    let transfer = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Transfer,
            payer_id: from.id,
            recipient_id: Some(to.id),
            amount_rubles: 3425,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("вставка перевода");

    assert_eq!(transfer.recipient_id, Some(to.id));
    assert_eq!(transfer.kind, EntryKindRow::Transfer);
}

#[tokio::test]
async fn rejects_a_self_transfer() {
    let pool = test_pool().await;
    // Отдельная транзакция: нарушение CHECK переводит транзакцию Postgres
    // в сбойное состояние, и все последующие запросы в ней тоже упали бы.
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let person = participants::insert(&mut tx, meeting_id, "Егор", "🐸")
        .await
        .expect("участник");

    // Слой API проверит это раньше и вернёт 422, но защита в базе — последняя
    // линия, и она должна работать.
    let rejected = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Transfer,
            payer_id: person.id,
            recipient_id: Some(person.id),
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await;

    assert!(rejected.is_err(), "перевод самому себе должен быть отбит");
}

#[tokio::test]
async fn deleting_a_participant_removes_their_entries() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");
    let other = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй");

    entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 1000,
            description: "Его расход".to_owned(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("расход плательщика");

    entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Transfer,
            payer_id: other.id,
            recipient_id: Some(payer.id),
            amount_rubles: 500,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("перевод в его адрес");

    participants::delete(&mut tx, payer.id)
        .await
        .expect("удаление участника");

    // Требование дизайна: удаление участника уносит и записи, где он
    // плательщик, и записи, где он получатель.
    let left = entries::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("записи");
    assert!(left.is_empty(), "остались записи: {left:?}");
}

#[tokio::test]
async fn lists_entries_newest_first() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;
    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");

    let early = chrono::DateTime::parse_from_rfc3339("2026-07-23T20:00:00Z")
        .expect("дата")
        .to_utc();
    let late = chrono::DateTime::parse_from_rfc3339("2026-07-24T23:00:00Z")
        .expect("дата")
        .to_utc();

    for (amount, when) in [(100, early), (200, late)] {
        entries::insert(
            &mut tx,
            meeting_id,
            NewEntry {
                kind: EntryKindRow::Expense,
                payer_id: payer.id,
                recipient_id: None,
                amount_rubles: amount,
                description: String::new(),
                occurred_at: Some(when),
                shares: Vec::new(),
            },
        )
        .await
        .expect("расход");
    }

    let listed = entries::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("записи");

    // История в интерфейсе идёт новыми сверху.
    assert_eq!(listed[0].amount_rubles, 200);
    assert_eq!(listed[1].amount_rubles, 100);
}

#[tokio::test]
async fn keeps_the_newest_log_records_within_the_limit() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    for index in 0..5 {
        log::append(&mut tx, meeting_id, &format!("событие {index}"))
            .await
            .expect("запись в лог");
    }

    let recent = log::recent(&mut tx, meeting_id, 3).await.expect("лог");

    // Новые сверху, лишние отброшены. Все пять событий получили одинаковый
    // created_at — now() даёт время начала транзакции, — поэтому порядок здесь
    // держится целиком на тай-брейке по id.
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].text, "событие 4");
    assert_eq!(recent[1].text, "событие 3");
    assert_eq!(recent[2].text, "событие 2");
}

#[tokio::test]
async fn reckons_the_dacha_meeting_from_stored_rows() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let nastya = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("Настя");
    let vlad = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("Влад");
    let egor = participants::insert(&mut tx, meeting_id, "Егор", "🐸")
        .await
        .expect("Егор");
    let marina = participants::insert(&mut tx, meeting_id, "Марина", "🦩")
        .await
        .expect("Марина");

    for (payer, amount) in [(vlad.id, 8400), (nastya.id, 3200), (marina.id, 2100)] {
        entries::insert(
            &mut tx,
            meeting_id,
            NewEntry {
                kind: EntryKindRow::Expense,
                payer_id: payer,
                recipient_id: None,
                amount_rubles: amount,
                description: String::new(),
                occurred_at: None,
                shares: Vec::new(),
            },
        )
        .await
        .expect("расход");
    }

    let stored = facts::load(&mut tx, meeting_id)
        .await
        .expect("чтение фактов");
    let reckoning = reckon(stored.as_facts());

    // Те же числа, что в юнит-тестах домена и на скриншотах дизайна —
    // но приехавшие через Postgres.
    assert_eq!(reckoning.spent, 13700);
    assert_eq!(reckoning.per_person, 3425);
    assert_eq!(reckoning.net[&ParticipantId(nastya.id)], -225);
    assert_eq!(reckoning.net[&ParticipantId(vlad.id)], 4975);
    assert_eq!(reckoning.net[&ParticipantId(egor.id)], -3425);
    assert_eq!(reckoning.net[&ParticipantId(marina.id)], -1325);
    assert_eq!(reckoning.status, MeetingStatus::Alarm(3));
}

#[tokio::test]
async fn maps_partial_shares_into_the_domain() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Раз", "🐻")
        .await
        .expect("первый");
    let half = participants::insert(&mut tx, meeting_id, "Два", "🦊")
        .await
        .expect("второй");
    let excluded = participants::insert(&mut tx, meeting_id, "Три", "🐸")
        .await
        .expect("третий");

    // Веса 4, 2, 0 из 6: целые части долей 66 и 33, распределено 99, остаток
    // рубля уходит плательщику — у него дробная часть больше (4/6 против 2/6).
    // Итоговые доли 67 / 33 / 0.
    entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: vec![(half.id, 2), (excluded.id, 0)],
        },
    )
    .await
    .expect("расход с долями");

    let stored = facts::load(&mut tx, meeting_id)
        .await
        .expect("чтение фактов");
    let reckoning = reckon(stored.as_facts());

    // Заплатил 100, своя доля 67.
    assert_eq!(reckoning.net[&ParticipantId(payer.id)], 33);
    assert_eq!(reckoning.net[&ParticipantId(half.id)], -33);
    // Исключённый не платит ничего, в том числе не получает рубль остатка.
    assert_eq!(reckoning.net[&ParticipantId(excluded.id)], 0);
}
