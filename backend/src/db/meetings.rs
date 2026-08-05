use chrono::NaiveDate;
use sqlx::PgConnection;
use uuid::Uuid;

use super::records::MeetingRow;

/// Поля новой встречи. Значения по умолчанию подставляет слой API, а не база:
/// пустое название превращается в «Новая встреча» там, где есть тексты.
#[derive(Debug, Clone)]
pub struct NewMeeting {
    pub title: String,
    pub description: String,
    pub emoji: String,
    pub held_on: NaiveDate,
}

// Список колонок выписан в каждом запросе, а не собран через `format!` из общей
// константы: в sqlx 0.9 запрос обязан быть `&'static str`, иначе нужна обёртка
// `AssertSqlSafe`. Обёртку не используем сознательно — она снимает защиту от
// склейки запросов там, где позже появятся фильтры с пользовательским вводом.
// Байты обложки не читаются ни одним из запросов: они не нужны ни списку,
// ни странице встречи, и возить их в каждом ответе значило бы тратить трафик.

pub async fn insert(
    conn: &mut PgConnection,
    meeting: NewMeeting,
) -> Result<MeetingRow, sqlx::Error> {
    sqlx::query_as(
        "insert into meetings (title, description, emoji, held_on) \
         values ($1, $2, $3, $4) \
         returning id, title, description, emoji, held_on, cover_mime, \
                   cover_version, created_at, updated_at",
    )
    .bind(meeting.title)
    .bind(meeting.description)
    .bind(meeting.emoji)
    .bind(meeting.held_on)
    .fetch_one(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<MeetingRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, title, description, emoji, held_on, cover_mime, \
                cover_version, created_at, updated_at \
         from meetings where id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
}
