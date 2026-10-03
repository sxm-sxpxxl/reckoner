use sqlx::PgConnection;
use uuid::Uuid;

use super::records::ParticipantRow;

/// Размер палитры аватаров из дизайна. Цвет не хранится, хранится индекс:
/// сами цвета знает фронтенд. Передаётся в запрос параметром, а не подставляется
/// в текст, чтобы значение осталось единственным.
const PALETTE_SIZE: i64 = 8;

/// `position` и `color_index` присваивает база, а не вызывающий: они зависят от
/// того, что уже есть в таблице, и считать это на стороне приложения значило бы
/// гонку между двумя одновременными добавлениями.
///
/// Номер берётся как `max(position) + 1`, а не как число существующих
/// участников. Разница принципиальная: после удаления кого-то из середины
/// счётчик по количеству выдал бы уже занятый номер и упёрся в уникальный
/// индекс `(meeting_id, position)`. Монотонный счётчик оставляет дырки
/// в нумерации — это безобидно, а вот столкновение сломало бы добавление
/// участника после любого удаления.
pub async fn insert(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    name: &str,
    emoji: &str,
) -> Result<ParticipantRow, sqlx::Error> {
    sqlx::query_as(
        "insert into participants (meeting_id, name, emoji, color_index, position) \
         select $1, $2, $3, (next.value % $4)::smallint, next.value::int \
         from ( \
             select coalesce(max(position), -1) + 1 as value \
             from participants where meeting_id = $1 \
         ) as next \
         returning id, meeting_id, name, emoji, color_index, position, paid_by, created_at",
    )
    .bind(meeting_id)
    .bind(name)
    .bind(emoji)
    .bind(PALETTE_SIZE)
    .fetch_one(conn)
    .await
}

/// Порядок по `position` — тот же, что использует домен для тай-брейка при
/// раздаче остатка рублей.
pub async fn list_for_meeting(
    conn: &mut PgConnection,
    meeting_id: Uuid,
) -> Result<Vec<ParticipantRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, meeting_id, name, emoji, color_index, position, paid_by, created_at \
         from participants where meeting_id = $1 order by position",
    )
    .bind(meeting_id)
    .fetch_all(conn)
    .await
}

/// Участники сразу нескольких встреч. Нужны списку встреч: без этого запроса
/// карточка стоила бы отдельного обращения к базе, и список из двадцати встреч
/// превратился бы в шестьдесят round-trip'ов до Neon.
///
/// `= any($1)` принимает массив параметром, поэтому запрос остаётся литералом
/// и склеивать `in (…)` из идентификаторов не нужно.
pub async fn list_for_meetings(
    conn: &mut PgConnection,
    meeting_ids: &[Uuid],
) -> Result<Vec<ParticipantRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, meeting_id, name, emoji, color_index, position, paid_by, created_at \
         from participants where meeting_id = any($1) \
         order by meeting_id, position",
    )
    .bind(meeting_ids)
    .fetch_all(conn)
    .await
}

/// Меняются только имя и эмодзи: позиция и цвет закреплены за участником
/// с момента добавления.
pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    name: &str,
    emoji: &str,
) -> Result<Option<ParticipantRow>, sqlx::Error> {
    sqlx::query_as(
        "update participants set name = $2, emoji = $3 where id = $1 \
         returning id, meeting_id, name, emoji, color_index, position, paid_by, created_at",
    )
    .bind(id)
    .bind(name)
    .bind(emoji)
    .fetch_optional(conn)
    .await
}

/// Назначает или снимает плательщика. Отдельно от `update`, потому что имя и
/// эмодзи перезаписываются всегда, а плательщик — только когда его поменяли.
pub async fn set_paid_by(
    conn: &mut PgConnection,
    id: Uuid,
    paid_by: Option<Uuid>,
) -> Result<Option<ParticipantRow>, sqlx::Error> {
    sqlx::query_as(
        "update participants set paid_by = $2 where id = $1 \
         returning id, meeting_id, name, emoji, color_index, position, paid_by, created_at",
    )
    .bind(id)
    .bind(paid_by)
    .fetch_optional(conn)
    .await
}

/// Каскады уносят все записи, где участник плательщик или получатель, и его
/// доли — это требование дизайна, а не побочный эффект. Тех, за кого он
/// платил, база отвязывает сама (`on delete set null`): они снова платят сами
/// за себя.
pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("delete from participants where id = $1")
        .bind(id)
        .execute(conn)
        .await?;

    Ok(result.rows_affected() > 0)
}
