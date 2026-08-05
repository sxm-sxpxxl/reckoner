mod support;

use backend::db::records::EntryKindRow;
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
