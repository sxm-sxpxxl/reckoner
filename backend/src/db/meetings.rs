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

/// Частичная правка: `None` означает «не менять». Собирать SQL из непустых
/// полей не нужно — `coalesce` делает это на стороне базы одним запросом.
#[derive(Debug, Clone, Default)]
pub struct MeetingPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub emoji: Option<String>,
    pub held_on: Option<NaiveDate>,
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    patch: MeetingPatch,
) -> Result<Option<MeetingRow>, sqlx::Error> {
    sqlx::query_as(
        "update meetings set \
             title = coalesce($2, title), \
             description = coalesce($3, description), \
             emoji = coalesce($4, emoji), \
             held_on = coalesce($5, held_on), \
             updated_at = now() \
         where id = $1 \
         returning id, title, description, emoji, held_on, cover_mime, \
                   cover_version, created_at, updated_at",
    )
    .bind(id)
    .bind(patch.title)
    .bind(patch.description)
    .bind(patch.emoji)
    .bind(patch.held_on)
    .fetch_optional(conn)
    .await
}

/// Список встреч с фильтрами. Порядок по умолчанию — по дате встречи, новые
/// сверху; остальные три режима сортировки применяет слой API, потому что
/// `total-desc` и `open-first` зависят от посчитанных значений, которых в SQL
/// нет. Поэтому и динамического `order by` здесь не бывает — склеивать запрос
/// не из чего.
///
/// Поиск сделан через `position`, а не через `ilike '%' || $1 || '%'`. Причина
/// не в безопасности — параметр привязан в обоих случаях, — а в том, что у
/// `like` есть своя семантика шаблонов: введённый пользователем `%` стал бы
/// подстановочным символом и такой поиск возвращал бы вообще все встречи.
/// Спека просит поиск по подстроке, и `position` выражает ровно это, не требуя
/// экранировать `%`, `_` и обратный слэш.
///
/// Приведение `$1::text` обязательно: без него Postgres не может определить тип
/// параметра, когда тот равен NULL.
///
/// Третий ключ сортировки `m.id` — та же причина, что и в остальных запросах:
/// две встречи с одинаковой датой и одинаковым `created_at` без него шли бы
/// в произвольном порядке.
pub async fn list_filtered(
    conn: &mut PgConnection,
    query: Option<&str>,
    participant: Option<&str>,
) -> Result<Vec<MeetingRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, title, description, emoji, held_on, cover_mime, \
                cover_version, created_at, updated_at \
         from meetings m \
         where ($1::text is null \
                or position(lower($1) in lower(m.title)) > 0 \
                or position(lower($1) in lower(m.description)) > 0) \
           and ($2::text is null or exists ( \
                 select 1 from participants p \
                 where p.meeting_id = m.id and p.name = $2)) \
         order by m.held_on desc, m.created_at desc, m.id",
    )
    .bind(query)
    .bind(participant)
    .fetch_all(conn)
    .await
}

/// `true`, если встреча была и удалена. Каскады уносят участников, записи,
/// доли и лог.
pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("delete from meetings where id = $1")
        .bind(id)
        .execute(conn)
        .await?;

    Ok(result.rows_affected() > 0)
}
