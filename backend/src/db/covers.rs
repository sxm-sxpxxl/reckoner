//! Обложки встреч. Байты лежат в самой таблице `meetings` — внешнего
//! хранилища нет намеренно: на бесплатном Render нет постоянного диска,
//! а поднимать S3 ради нескольких фотографий — лишняя сущность.
//!
//! Ни один запрос слоя `meetings` байты не читает: они не нужны ни списку,
//! ни странице встречи, и возить их в каждом ответе значило бы тратить трафик.

use sqlx::PgConnection;
use uuid::Uuid;

/// Обложка целиком: байты, тип и версия. Версия нужна для `ETag`.
#[derive(Debug, Clone)]
pub struct Cover {
    pub mime: String,
    pub bytes: Vec<u8>,
    pub version: i32,
}

/// `None`, если встречи нет или у неё нет обложки. Различать эти два случая
/// вызывающему незачем: и там и там ответ `404`.
pub async fn load(conn: &mut PgConnection, meeting_id: Uuid) -> Result<Option<Cover>, sqlx::Error> {
    let row: Option<(Option<String>, Option<Vec<u8>>, i32)> =
        sqlx::query_as("select cover_mime, cover_bytes, cover_version from meetings where id = $1")
            .bind(meeting_id)
            .fetch_optional(conn)
            .await?;

    Ok(row.and_then(|(mime, bytes, version)| {
        // Тип и байты пишутся вместе, но схема допускает NULL у обоих —
        // обложкой считаем только полную пару.
        Some(Cover {
            mime: mime?,
            bytes: bytes?,
            version,
        })
    }))
}

/// Записывает обложку и увеличивает версию. Возвращает новую версию —
/// клиенту она нужна, чтобы сбросить кэш картинки. `None`, если встречи нет.
pub async fn store(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    mime: &str,
    bytes: &[u8],
) -> Result<Option<i32>, sqlx::Error> {
    let row: Option<(i32,)> = sqlx::query_as(
        "update meetings set \
             cover_mime = $2, \
             cover_bytes = $3, \
             cover_version = cover_version + 1, \
             updated_at = now() \
         where id = $1 \
         returning cover_version",
    )
    .bind(meeting_id)
    .bind(mime)
    .bind(bytes)
    .fetch_optional(conn)
    .await?;

    Ok(row.map(|(version,)| version))
}

/// Снимает обложку. Версию **не** трогает: иначе прежний URL `?v=N` снова стал
/// бы действительным, и браузер отдал бы из кэша картинку, которой уже нет.
pub async fn clear(conn: &mut PgConnection, meeting_id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query(
        "update meetings set cover_mime = null, cover_bytes = null, updated_at = now() \
         where id = $1",
    )
    .bind(meeting_id)
    .execute(conn)
    .await?;

    Ok(result.rows_affected() > 0)
}
