//! Обложка встречи: раздача, загрузка, снятие.
//!
//! У этих маршрутов свой лимит тела: JSON-роутер ограничен 64 КБ, и картинка
//! в него не пролезет. Путать лимиты нельзя — поэтому обложки живут на
//! отдельном под-роутере со своим слоем.

use axum::Json;
use axum::body::Bytes;
use axum::extract::{FromRequest, Path, Request, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{IntoResponse, Response};
use sqlx::PgPool;
use uuid::Uuid;

use crate::db;

use super::error::ApiError;
use super::meetings;
use super::texts;
use super::view::MeetingView;

/// Потолок тела обложки. Клиент сжимает до ~500 КБ; мегабайт — запас на
/// неудачное сжатие. Не путать с `JSON_BODY_LIMIT`: там 64 КБ.
pub const COVER_BODY_LIMIT: usize = 1024 * 1024;

/// Сырое тело запроса с отказом в нашей форме.
///
/// Штатный экстрактор `Bytes` при превышении лимита отвечает `text/plain`,
/// а клиент разбирает тело как JSON и упал бы сам, потеряв причину.
pub struct RawBody(pub Bytes);

#[axum::async_trait]
impl<S> FromRequest<S> for RawBody
where
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        // Единственная причина отказа здесь — превышенный лимит: слой
        // `DefaultBodyLimit` срабатывает раньше, чем тело успевает прочитаться.
        let bytes = Bytes::from_request(request, state)
            .await
            .map_err(|_| ApiError::TooLarge)?;

        Ok(Self(bytes))
    }
}

/// Тип по первым байтам, а не по заголовку `Content-Type`: заголовок присылает
/// клиент, и доверять ему — значит не проверять вовсе.
fn sniff_mime(bytes: &[u8]) -> Option<&'static str> {
    if bytes.starts_with(b"\x89PNG\r\n\x1a\n") {
        return Some("image/png");
    }

    if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        return Some("image/jpeg");
    }

    None
}

/// Байты обложки с кэшированием «навсегда».
///
/// Так можно, потому что URL картинки содержит `?v=<coverVersion>`: при замене
/// версия растёт, адрес меняется, и старый ответ из кэша больше не запрашивают.
pub async fn show(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    headers: HeaderMap,
) -> Result<Response, ApiError> {
    let mut conn = pool.acquire().await?;
    let cover = db::covers::load(&mut conn, id)
        .await?
        .ok_or(ApiError::NotFound)?;

    // `ETag` от версии, а не от хеша байтов: хеш пришлось бы считать на каждый
    // запрос, а версия меняется ровно тогда, когда меняется картинка.
    let etag = format!("\"{}\"", cover.version);

    if headers
        .get(header::IF_NONE_MATCH)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|value| value == etag)
    {
        return Ok(StatusCode::NOT_MODIFIED.into_response());
    }

    Ok((
        [
            (header::CONTENT_TYPE, cover.mime),
            (header::ETAG, etag),
            (
                header::CACHE_CONTROL,
                "public, max-age=31536000, immutable".to_owned(),
            ),
        ],
        cover.bytes,
    )
        .into_response())
}

pub async fn upload(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
    RawBody(bytes): RawBody,
) -> Result<Json<MeetingView>, ApiError> {
    if bytes.len() > COVER_BODY_LIMIT {
        return Err(ApiError::TooLarge);
    }

    let mime = sniff_mime(&bytes).ok_or(ApiError::UnsupportedMedia)?;

    let mut tx = pool.begin().await?;

    db::covers::store(&mut tx, id, mime, &bytes)
        .await?
        .ok_or(ApiError::NotFound)?;
    db::log::append(&mut tx, id, texts::COVER_UPDATED).await?;

    let view = meetings::load_view(&mut tx, id).await?;
    tx.commit().await?;

    Ok(Json(view))
}

pub async fn destroy(
    State(pool): State<PgPool>,
    Path(id): Path<Uuid>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;

    if !db::covers::clear(&mut tx, id).await? {
        return Err(ApiError::NotFound);
    }

    db::log::append(&mut tx, id, texts::COVER_REMOVED).await?;

    let view = meetings::load_view(&mut tx, id).await?;
    tx.commit().await?;

    Ok(Json(view))
}
