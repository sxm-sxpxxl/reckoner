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
