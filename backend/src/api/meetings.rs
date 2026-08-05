//! Ручки встречи: список, чтение, создание, правка, удаление.

use axum::Json;
use axum::extract::{Path, State};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::db;
use crate::db::records::MeetingRow;

use super::LOG_LIMIT;
use super::error::ApiError;
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

pub async fn show(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut conn = pool.acquire().await?;

    Ok(Json(load_view(&mut conn, id).await?))
}
