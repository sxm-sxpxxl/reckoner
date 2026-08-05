//! Ручки записи: расход и перевод. Здесь же вся валидация записи — база ловит
//! те же нарушения CHECK-ограничениями, но ответом было бы `500` вместо `422`,
//! и клиент не узнал бы, какое поле поправить.

use axum::Json;
use axum::extract::{Path, State};
use chrono::{DateTime, Utc};
use serde::Deserialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::db;
use crate::db::records::{EntryKindRow, ParticipantRow};

use super::JsonBody;
use super::error::ApiError;
use super::meetings;
use super::texts;
use super::validate;
use super::view::MeetingView;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKindInput {
    Expense,
    Transfer,
}

impl From<EntryKindInput> for EntryKindRow {
    fn from(kind: EntryKindInput) -> Self {
        match kind {
            EntryKindInput::Expense => Self::Expense,
            EntryKindInput::Transfer => Self::Transfer,
        }
    }
}

/// Доля участника в расходе, как её присылает клиент: `0..=4` четвертей.
/// Четыре — полная доля; в базу она не попадает.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInput {
    pub participant_id: Uuid,
    pub weight_quarters: i16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateEntry {
    pub kind: EntryKindInput,
    pub payer_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: i64,
    #[serde(default)]
    pub description: String,
    /// `None` — «сейчас». Клиент присылает своё время, когда вводит запись
    /// задним числом.
    pub occurred_at: Option<DateTime<Utc>>,
    #[serde(default)]
    pub shares: Vec<ShareInput>,
}

pub async fn add_entry(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    body: CreateEntry,
) -> Result<MeetingView, ApiError> {
    meetings::require_meeting(&mut *conn, meeting_id).await?;
    let participants = db::participants::list_for_meeting(&mut *conn, meeting_id).await?;

    let amount = validate::amount(body.amount_rubles)?;
    let payer = validate::belongs_to_meeting(&participants, body.payer_id, "payerId")?.clone();
    let recipient = check_recipient(&participants, body.kind, body.payer_id, body.recipient_id)?;
    let shares = check_shares(&participants, body.kind, &body.shares)?;
    let description = normalize_description(body.kind, &body.description);

    db::entries::insert(
        &mut *conn,
        meeting_id,
        db::entries::NewEntry {
            kind: body.kind.into(),
            payer_id: payer.id,
            recipient_id: recipient.as_ref().map(|row| row.id),
            amount_rubles: amount,
            description: description.clone(),
            occurred_at: body.occurred_at,
            shares,
        },
    )
    .await?;

    let line = match recipient {
        Some(recipient) => texts::transfer_added(&payer.name, &recipient.name, amount),
        None => texts::expense_added(&payer.name, &description, amount),
    };
    db::log::append(&mut *conn, meeting_id, &line).await?;

    meetings::load_view(conn, meeting_id).await
}

/// У перевода получатель обязателен и не равен плательщику; у расхода его нет.
/// Вид записи неизменен, поэтому дальше по коду `Option` получателя однозначно
/// отличает перевод от расхода.
fn check_recipient(
    participants: &[ParticipantRow],
    kind: EntryKindInput,
    payer_id: Uuid,
    recipient_id: Option<Uuid>,
) -> Result<Option<ParticipantRow>, ApiError> {
    match (kind, recipient_id) {
        (EntryKindInput::Expense, None) => Ok(None),
        (EntryKindInput::Expense, Some(_)) => Err(ApiError::validation(
            "recipientId",
            "у расхода не бывает получателя",
        )),
        (EntryKindInput::Transfer, None) => Err(ApiError::validation(
            "recipientId",
            "у перевода получатель обязателен",
        )),
        (EntryKindInput::Transfer, Some(recipient_id)) => {
            if recipient_id == payer_id {
                return Err(ApiError::validation(
                    "recipientId",
                    "перевод себе ничего не меняет",
                ));
            }

            Ok(Some(
                validate::belongs_to_meeting(participants, recipient_id, "recipientId")?.clone(),
            ))
        }
    }
}

/// Доли бывают только у расхода. Принять их у перевода и молча выбросить —
/// значит показать клиенту, что он настроил разбивку, которой нет.
fn check_shares(
    participants: &[ParticipantRow],
    kind: EntryKindInput,
    raw: &[ShareInput],
) -> Result<Vec<(Uuid, i16)>, ApiError> {
    if kind == EntryKindInput::Transfer && !raw.is_empty() {
        return Err(ApiError::validation("shares", "перевод не делится на доли"));
    }

    validate::shares(raw, participants)
}

/// Пустое описание расхода превращается в «Без описания» — так же делает
/// прототип при вводе, но источник истины здесь. У перевода описания в дизайне
/// нет вовсе, и подставлять ему заглушку не нужно.
fn normalize_description(kind: EntryKindInput, raw: &str) -> String {
    let trimmed = raw.trim();

    match (kind, trimmed.is_empty()) {
        (EntryKindInput::Expense, true) => texts::DEFAULT_EXPENSE_DESCRIPTION.to_owned(),
        _ => trimmed.to_owned(),
    }
}

/// `None` в поле означает «не менять». Вид записи не меняется: превращение
/// расхода в перевод — это другая запись, в интерфейсе это делается удалением
/// и повторным вводом.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateEntry {
    pub payer_id: Option<Uuid>,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: Option<i64>,
    pub description: Option<String>,
    pub occurred_at: Option<DateTime<Utc>>,
    /// `None` — доли не трогать. `Some(vec![])` — снять все неполные доли,
    /// то есть вернуть расход к делению поровну.
    pub shares: Option<Vec<ShareInput>>,
}

pub async fn update_entry(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    entry_id: Uuid,
    body: UpdateEntry,
) -> Result<MeetingView, ApiError> {
    let current = require_entry(&mut *conn, meeting_id, entry_id).await?;
    let participants = db::participants::list_for_meeting(&mut *conn, meeting_id).await?;
    let is_transfer = current.kind == EntryKindRow::Transfer;

    let amount = match body.amount_rubles {
        Some(value) => Some(validate::amount(value)?),
        None => None,
    };

    if let Some(payer_id) = body.payer_id {
        validate::belongs_to_meeting(&participants, payer_id, "payerId")?;
    }

    // Сверяются действующие значения, а не присланные: поменять можно одну
    // сторону перевода, а совпасть она может с той, которой в теле нет.
    let payer_id = body.payer_id.unwrap_or(current.payer_id);
    let recipient_id = match (is_transfer, body.recipient_id) {
        (false, Some(_)) => {
            return Err(ApiError::validation(
                "recipientId",
                "у расхода не бывает получателя",
            ));
        }
        (false, None) => None,
        (true, Some(id)) => {
            validate::belongs_to_meeting(&participants, id, "recipientId")?;
            Some(id)
        }
        (true, None) => current.recipient_id,
    };

    if recipient_id == Some(payer_id) {
        // Поле в ошибке — то, которое клиент только что менял: подсветить надо
        // его, а не то, что осталось прежним.
        let field = if body.payer_id.is_some() {
            "payerId"
        } else {
            "recipientId"
        };

        return Err(ApiError::validation(field, "перевод себе ничего не меняет"));
    }

    let shares = match &body.shares {
        Some(raw) => {
            if is_transfer && !raw.is_empty() {
                return Err(ApiError::validation("shares", "перевод не делится на доли"));
            }

            Some(validate::shares(raw, &participants)?)
        }
        None => None,
    };

    let kind = if is_transfer {
        EntryKindInput::Transfer
    } else {
        EntryKindInput::Expense
    };
    let description = body
        .description
        .as_deref()
        .map(|raw| normalize_description(kind, raw));

    let updated = db::entries::update(
        &mut *conn,
        entry_id,
        db::entries::EntryPatch {
            payer_id: body.payer_id,
            recipient_id: body.recipient_id,
            amount_rubles: amount,
            description,
            occurred_at: body.occurred_at,
            shares,
        },
    )
    .await?
    .ok_or(ApiError::NotFound)?;

    db::log::append(
        &mut *conn,
        meeting_id,
        &texts::entry_updated(updated.amount_rubles),
    )
    .await?;

    meetings::load_view(conn, meeting_id).await
}

pub async fn remove_entry(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    entry_id: Uuid,
) -> Result<MeetingView, ApiError> {
    // Сумма нужна для строки лога, поэтому читается до удаления.
    let current = require_entry(&mut *conn, meeting_id, entry_id).await?;
    let amount = current.amount_rubles;

    db::entries::delete(&mut *conn, entry_id).await?;
    db::log::append(&mut *conn, meeting_id, &texts::entry_deleted(amount)).await?;

    meetings::load_view(conn, meeting_id).await
}

/// Запись, принадлежащая этой встрече. Чужая — `404`: по этому адресу её нет.
async fn require_entry(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    entry_id: Uuid,
) -> Result<db::records::EntryRow, ApiError> {
    meetings::require_meeting(&mut *conn, meeting_id).await?;

    db::entries::list_for_meeting(&mut *conn, meeting_id)
        .await?
        .into_iter()
        .find(|row| row.id == entry_id)
        .ok_or(ApiError::NotFound)
}

pub async fn create(
    State(pool): State<PgPool>,
    Path(meeting_id): Path<Uuid>,
    JsonBody(body): JsonBody<CreateEntry>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = add_entry(&mut tx, meeting_id, body).await?;
    tx.commit().await?;

    Ok(Json(view))
}

pub async fn update(
    State(pool): State<PgPool>,
    Path((meeting_id, entry_id)): Path<(Uuid, Uuid)>,
    JsonBody(body): JsonBody<UpdateEntry>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = update_entry(&mut tx, meeting_id, entry_id, body).await?;
    tx.commit().await?;

    Ok(Json(view))
}

pub async fn destroy(
    State(pool): State<PgPool>,
    Path((meeting_id, entry_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = remove_entry(&mut tx, meeting_id, entry_id).await?;
    tx.commit().await?;

    Ok(Json(view))
}
