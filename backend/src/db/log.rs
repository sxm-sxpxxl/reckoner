use sqlx::PgConnection;
use uuid::Uuid;

use super::records::LogRow;

/// Пишется в той же транзакции, что и сама правка, — так лог не может
/// разойтись с данными.
pub async fn append(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    text: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("insert into meeting_log (meeting_id, text) values ($1, $2)")
        .bind(meeting_id)
        .bind(text)
        .execute(conn)
        .await?;

    Ok(())
}

/// Последние события, новые сверху. Второй ключ сортировки — `id`, и он здесь
/// главный: `now()` в Postgres возвращает время начала транзакции, поэтому все
/// события одной правки получают одинаковый `created_at`, и без `id` их порядок
/// был бы произвольным.
pub async fn recent(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    limit: i64,
) -> Result<Vec<LogRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, text, created_at from meeting_log where meeting_id = $1 \
         order by created_at desc, id desc limit $2",
    )
    .bind(meeting_id)
    .bind(limit)
    .fetch_all(conn)
    .await
}
