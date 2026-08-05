//! Ручки встречи: список, чтение, создание, правка, удаление.

use axum::Json;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use chrono::{NaiveDate, Utc};
use serde::Deserialize;
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::db;
use crate::db::records::MeetingRow;

use super::JsonBody;
use super::LOG_LIMIT;
use super::error::ApiError;
use super::texts;
use super::view::{self, MeetingView};

/// Встреча целиком: то же тело, что отдаёт любая мутирующая ручка. Пять запросов
/// одним соединением; если вызвано внутри транзакции — видит её незакоммиченные
/// изменения, поэтому ответ мутирующей ручки не может разойтись с тем, что
/// записано.
pub async fn load_view(conn: &mut PgConnection, id: Uuid) -> Result<MeetingView, ApiError> {
    let meeting = require_meeting(&mut *conn, id).await?;
    let participants = db::participants::list_for_meeting(&mut *conn, id).await?;
    let entries = db::entries::list_for_meeting(&mut *conn, id).await?;
    let shares = db::entries::shares_for_meeting(&mut *conn, id).await?;
    let log = db::log::recent(&mut *conn, id, LOG_LIMIT).await?;

    Ok(view::meeting_view(
        &meeting,
        &participants,
        &entries,
        &shares,
        &log,
    ))
}

/// Существование встречи проверяется до правки, а не после: без этой проверки
/// вставка участника упёрлась бы во внешний ключ, и посетитель увидел бы `500`
/// там, где по спеке `404`.
pub async fn require_meeting(conn: &mut PgConnection, id: Uuid) -> Result<MeetingRow, ApiError> {
    db::meetings::find(conn, id)
        .await?
        .ok_or(ApiError::NotFound)
}

/// Все поля необязательные: единственная обязательная часть новой встречи —
/// сам факт её существования.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateMeeting {
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub description: String,
    pub emoji: Option<String>,
    pub held_on: Option<NaiveDate>,
}

pub async fn create_meeting(
    conn: &mut PgConnection,
    body: CreateMeeting,
) -> Result<MeetingView, ApiError> {
    let title = body.title.trim();
    let title = if title.is_empty() {
        texts::DEFAULT_MEETING_TITLE
    } else {
        title
    };

    let emoji = body
        .emoji
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(texts::DEFAULT_MEETING_EMOJI);

    // Дата по умолчанию — сегодня по UTC. Клиент знает свою зону и в норме
    // присылает дату сам; фолбэк нужен запросам без поля, и ночью в Москве
    // он может дать вчерашнее число — поэтому это фолбэк, а не источник истины.
    let held_on = body.held_on.unwrap_or_else(|| Utc::now().date_naive());

    let meeting = db::meetings::insert(
        &mut *conn,
        db::meetings::NewMeeting {
            title: title.to_owned(),
            description: body.description.trim().to_owned(),
            emoji: emoji.to_owned(),
            held_on,
        },
    )
    .await?;

    db::log::append(&mut *conn, meeting.id, texts::MEETING_CREATED).await?;

    load_view(conn, meeting.id).await
}

pub async fn show(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut conn = pool.acquire().await?;

    Ok(Json(load_view(&mut conn, id).await?))
}

pub async fn create(
    State(pool): State<PgPool>,
    JsonBody(body): JsonBody<CreateMeeting>,
) -> Result<(StatusCode, Json<MeetingView>), ApiError> {
    let mut tx = pool.begin().await?;
    let view = create_meeting(&mut tx, body).await?;
    tx.commit().await?;

    Ok((StatusCode::CREATED, Json(view)))
}
