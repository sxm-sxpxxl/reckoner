//! Строки таблиц: только данные, никакой логики. Отображение в доменные типы
//! живёт в `facts.rs`, запросы — в модулях по таблицам.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct MeetingRow {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub emoji: String,
    pub held_on: NaiveDate,
    /// Заполнен, если обложка загружена. Сами байты читаются отдельным запросом:
    /// они не нужны ни на списке встреч, ни на странице встречи.
    pub cover_mime: Option<String>,
    pub cover_version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ParticipantRow {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub name: String,
    pub emoji: String,
    pub color_index: i16,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum EntryKindRow {
    Expense,
    Transfer,
}

#[derive(Debug, Clone, FromRow)]
pub struct EntryRow {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub kind: EntryKindRow,
    pub payer_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: i64,
    pub description: String,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, FromRow)]
pub struct ShareRow {
    pub entry_id: Uuid,
    pub participant_id: Uuid,
    /// `0` — участник исключён из расхода, больше нуля — вписанная сумма.
    pub rubles: i64,
}

#[derive(Debug, Clone, FromRow)]
pub struct LogRow {
    pub id: i64,
    pub text: String,
    pub created_at: DateTime<Utc>,
}
