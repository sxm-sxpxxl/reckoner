//! Единственная форма ошибки для всех ручек. Клиент разбирает тело как JSON,
//! поэтому текстовых ответов быть не должно ни при какой ошибке.

use axum::Json;
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use serde_json::json;

#[derive(Debug, thiserror::Error)]
pub enum ApiError {
    /// Нарушено правило из спеки. `field` — имя поля в теле запроса
    /// (`camelCase`, как его прислал клиент), чтобы UI подсветил нужный инпут.
    #[error("{field}: {message}")]
    Validation {
        field: &'static str,
        message: String,
    },
    /// Нет встречи, участника или записи по этому адресу. Участник чужой
    /// встречи — тоже «нет»: по этому URL его не существует.
    #[error("не найдено")]
    NotFound,
    /// Тело больше допустимого. Отдельный вариант, а не `Validation`: поля,
    /// которое надо подсветить, здесь нет — виноват файл целиком.
    #[error("файл слишком большой")]
    TooLarge,
    /// Не картинка. Тип определяется по байтам, а не по заголовку.
    #[error("неподдерживаемый тип файла")]
    UnsupportedMedia,
    /// Всё прочее. `#[from]` даёт `?` на любом вызове слоя `db`.
    #[error("ошибка базы: {0}")]
    Database(#[from] sqlx::Error),
}

impl ApiError {
    pub fn validation(field: &'static str, message: impl Into<String>) -> Self {
        Self::Validation {
            field,
            message: message.into(),
        }
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        match self {
            Self::Validation { field, message } => (
                StatusCode::UNPROCESSABLE_ENTITY,
                Json(json!({ "error": "validation", "field": field, "message": message })),
            )
                .into_response(),
            Self::NotFound => {
                (StatusCode::NOT_FOUND, Json(json!({ "error": "not-found" }))).into_response()
            }
            Self::TooLarge => (
                StatusCode::PAYLOAD_TOO_LARGE,
                Json(json!({ "error": "too-large" })),
            )
                .into_response(),
            Self::UnsupportedMedia => (
                StatusCode::UNSUPPORTED_MEDIA_TYPE,
                Json(json!({ "error": "unsupported-media" })),
            )
                .into_response(),
            Self::Database(error) => {
                // Единственное место, где ошибка базы вообще видна человеку.
                // Локально это консоль `cargo run`, на Render — панель логов.
                eprintln!("ошибка базы данных: {error}");
                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(json!({ "error": "internal" })),
                )
                    .into_response()
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use http_body_util::BodyExt;

    /// Читает код и тело ответа. Тело в axum 0.7 — поток, поэтому его надо
    /// собрать целиком, прежде чем разбирать.
    async fn parts(error: ApiError) -> (StatusCode, serde_json::Value) {
        let response = error.into_response();
        let status = response.status();
        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("тело ответа")
            .to_bytes();

        (status, serde_json::from_slice(&bytes).expect("тело — JSON"))
    }

    #[tokio::test]
    async fn validation_names_the_field() {
        // Поле в теле — то, что клиент подсветит красным. Без него сообщение
        // некуда показать, поэтому проверяется именно форма, а не только код.
        let (status, body) = parts(ApiError::validation(
            "amountRubles",
            "сумма должна быть больше нуля",
        ))
        .await;

        assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
        assert_eq!(
            body,
            serde_json::json!({
                "error": "validation",
                "field": "amountRubles",
                "message": "сумма должна быть больше нуля"
            })
        );
    }

    #[tokio::test]
    async fn not_found_says_nothing_else() {
        let (status, body) = parts(ApiError::NotFound).await;

        assert_eq!(status, StatusCode::NOT_FOUND);
        assert_eq!(body, serde_json::json!({ "error": "not-found" }));
    }

    #[tokio::test]
    async fn oversized_and_wrong_type_answer_in_our_shape() {
        // Оба кода клиент разбирает как JSON, поэтому текстовых тел здесь быть
        // не должно так же, как у остальных ошибок.
        let (status, body) = parts(ApiError::TooLarge).await;

        assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
        assert_eq!(body, serde_json::json!({ "error": "too-large" }));

        let (status, body) = parts(ApiError::UnsupportedMedia).await;

        assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
        assert_eq!(body, serde_json::json!({ "error": "unsupported-media" }));
    }

    #[tokio::test]
    async fn database_error_does_not_leak_into_the_body() {
        // Текст ошибки sqlx называет таблицы и колонки. Посетителю он бесполезен,
        // а тому, кто ищет форму базы, полезен — поэтому в тело он не попадает.
        let (status, body) = parts(ApiError::Database(sqlx::Error::RowNotFound)).await;

        assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
        assert_eq!(body, serde_json::json!({ "error": "internal" }));
    }
}
