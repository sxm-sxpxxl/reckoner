mod support;

use backend::db::meetings::{self, MeetingPatch, NewMeeting};
use backend::db::records::EntryKindRow;
use chrono::NaiveDate;
use support::test_pool;

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
