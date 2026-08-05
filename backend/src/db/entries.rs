use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::records::{EntryKindRow, EntryRow, ShareRow};

/// Новая запись вместе с неполными долями. Полная доля (4/4) в `shares` не
/// передаётся: её отсутствие и есть полная доля.
#[derive(Debug, Clone)]
pub struct NewEntry {
    pub kind: EntryKindRow,
    pub payer_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: i64,
    pub description: String,
    /// `None` — «сейчас». Явное значение нужно тестам и импорту.
    pub occurred_at: Option<DateTime<Utc>>,
    /// Пары `(участник, четверти)`, только для неполных долей.
    pub shares: Vec<(Uuid, i16)>,
}

/// Вставляет запись и её доли. Вызывающий обязан передать транзакцию: запись
/// без своих долей — это неверный расчёт, а не просто неполные данные.
pub async fn insert(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    entry: NewEntry,
) -> Result<EntryRow, sqlx::Error> {
    let row: EntryRow = sqlx::query_as(
        "insert into entries \
             (meeting_id, kind, payer_id, recipient_id, amount_rubles, description, occurred_at) \
         values ($1, $2, $3, $4, $5, $6, coalesce($7, now())) \
         returning id, meeting_id, kind, payer_id, recipient_id, \
                   amount_rubles, description, occurred_at, created_at",
    )
    .bind(meeting_id)
    .bind(entry.kind)
    .bind(entry.payer_id)
    .bind(entry.recipient_id)
    .bind(entry.amount_rubles)
    .bind(entry.description)
    .bind(entry.occurred_at)
    .fetch_one(&mut *conn)
    .await?;

    for (participant_id, weight_quarters) in entry.shares {
        sqlx::query(
            "insert into entry_shares (entry_id, participant_id, weight_quarters) \
             values ($1, $2, $3)",
        )
        .bind(row.id)
        .bind(participant_id)
        .bind(weight_quarters)
        .execute(&mut *conn)
        .await?;
    }

    Ok(row)
}

/// Новые сверху — так история показана в интерфейсе.
///
/// Второй ключ сортировки не украшение: две записи с одинаковым `occurred_at`
/// без него шли бы в произвольном порядке, и один и тот же запрос мог бы
/// вернуть историю в разном виде.
pub async fn list_for_meeting(
    conn: &mut PgConnection,
    meeting_id: Uuid,
) -> Result<Vec<EntryRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, meeting_id, kind, payer_id, recipient_id, \
                amount_rubles, description, occurred_at, created_at \
         from entries where meeting_id = $1 \
         order by occurred_at desc, id",
    )
    .bind(meeting_id)
    .fetch_all(conn)
    .await
}

/// Доли всех записей встречи одним запросом: домену они нужны сразу все,
/// и запрос на каждую запись отдельно был бы N+1.
pub async fn shares_for_meeting(
    conn: &mut PgConnection,
    meeting_id: Uuid,
) -> Result<Vec<ShareRow>, sqlx::Error> {
    sqlx::query_as(
        "select s.entry_id, s.participant_id, s.weight_quarters \
         from entry_shares s join entries e on e.id = s.entry_id \
         where e.meeting_id = $1",
    )
    .bind(meeting_id)
    .fetch_all(conn)
    .await
}

pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("delete from entries where id = $1")
        .bind(id)
        .execute(conn)
        .await?;

    Ok(result.rows_affected() > 0)
}
