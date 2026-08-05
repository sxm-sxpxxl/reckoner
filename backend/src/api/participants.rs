//! Ручки участника. `colorIndex` и `position` назначает база — см.
//! `db::participants::insert`.

use axum::Json;
use axum::extract::{Path, State};
use serde::Deserialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::db;

use super::JsonBody;
use super::error::ApiError;
use super::meetings;
use super::texts;
use super::validate;
use super::view::MeetingView;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateParticipant {
    pub name: String,
    pub emoji: Option<String>,
}

pub async fn add_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    body: CreateParticipant,
) -> Result<MeetingView, ApiError> {
    meetings::require_meeting(&mut *conn, meeting_id).await?;

    let name = validate::participant_name(&body.name)?;
    let emoji = validate::emoji_or_default(body.emoji.as_deref(), texts::DEFAULT_PARTICIPANT_EMOJI);

    let participant = db::participants::insert(&mut *conn, meeting_id, &name, &emoji).await?;
    db::log::append(
        &mut *conn,
        meeting_id,
        &texts::participant_joined(&participant.name),
    )
    .await?;

    meetings::load_view(conn, meeting_id).await
}

/// `None` в поле означает «не менять». Позиция и цвет не меняются никогда:
/// они закреплены за участником с момента добавления.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateParticipant {
    pub name: Option<String>,
    pub emoji: Option<String>,
}

pub async fn update_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    participant_id: Uuid,
    body: UpdateParticipant,
) -> Result<MeetingView, ApiError> {
    let current = require_participant(&mut *conn, meeting_id, participant_id).await?;

    // Запрос в слое данных требует оба значения: позиция и цвет остаются, а имя
    // с эмодзи перезаписываются целиком. Незаданное поле берём из текущей
    // строки — так частичная правка получается без второго запроса.
    let name = match body.name.as_deref() {
        Some(raw) => validate::participant_name(raw)?,
        None => current.name.clone(),
    };
    let emoji = validate::emoji_or_default(body.emoji.as_deref(), &current.emoji);

    let updated = db::participants::update(&mut *conn, participant_id, &name, &emoji)
        .await?
        .ok_or(ApiError::NotFound)?;

    db::log::append(
        &mut *conn,
        meeting_id,
        &texts::participant_updated(&updated.name),
    )
    .await?;

    meetings::load_view(conn, meeting_id).await
}

pub async fn remove_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    participant_id: Uuid,
) -> Result<MeetingView, ApiError> {
    // Имя нужно для строки лога, поэтому читается до удаления: после каскада
    // взять его негде.
    let current = require_participant(&mut *conn, meeting_id, participant_id).await?;
    let name = current.name.clone();

    db::participants::delete(&mut *conn, participant_id).await?;
    db::log::append(&mut *conn, meeting_id, &texts::participant_deleted(&name)).await?;

    meetings::load_view(conn, meeting_id).await
}

/// Участник, принадлежащий этой встрече. Чужой — `404`, а не `422`: по этому
/// адресу его не существует, и различать «нет» и «есть, но не здесь» клиенту
/// незачем.
async fn require_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    participant_id: Uuid,
) -> Result<db::records::ParticipantRow, ApiError> {
    meetings::require_meeting(&mut *conn, meeting_id).await?;

    db::participants::list_for_meeting(&mut *conn, meeting_id)
        .await?
        .into_iter()
        .find(|row| row.id == participant_id)
        .ok_or(ApiError::NotFound)
}

/// Ответ — `200`, а не `201`: тело описывает встречу, а не созданного участника,
/// и `Location` указывать некуда.
pub async fn create(
    State(pool): State<PgPool>,
    Path(meeting_id): Path<Uuid>,
    JsonBody(body): JsonBody<CreateParticipant>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = add_participant(&mut tx, meeting_id, body).await?;
    tx.commit().await?;

    Ok(Json(view))
}

pub async fn update(
    State(pool): State<PgPool>,
    Path((meeting_id, participant_id)): Path<(Uuid, Uuid)>,
    JsonBody(body): JsonBody<UpdateParticipant>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = update_participant(&mut tx, meeting_id, participant_id, body).await?;
    tx.commit().await?;

    Ok(Json(view))
}

pub async fn destroy(
    State(pool): State<PgPool>,
    Path((meeting_id, participant_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = remove_participant(&mut tx, meeting_id, participant_id).await?;
    tx.commit().await?;

    Ok(Json(view))
}
