//! Тесты ручек на уровне соединения. Каждый работает в транзакции, которая
//! откатывается при выходе, поэтому тесты не видят друг друга и не оставляют
//! мусора. Хендлеры как HTTP проверяются отдельно, в `tests/http.rs`.

mod support;

use backend::api::error::ApiError;
use backend::api::meetings;
use backend::db;
use backend::db::meetings::NewMeeting;
use chrono::{NaiveDate, TimeZone, Utc};
use support::test_pool;
use uuid::Uuid;

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
