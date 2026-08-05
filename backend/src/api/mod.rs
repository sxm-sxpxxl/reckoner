//! Слой HTTP: извлечение параметров, валидация, значения по умолчанию,
//! тексты лога и форма JSON.
//!
//! Ниже лежат `db` (только запросы) и `domain` (только расчёт), и ни один из них
//! этот слой не меняет. Если ручке не хватает данных — это повод вернуться
//! к спеке, а не дописать запрос здесь.

pub mod error;
pub mod money;
pub mod texts;

use axum::extract::{DefaultBodyLimit, FromRequest, Request};
use axum::routing::get;
use axum::{Json, Router};
use serde::de::DeserializeOwned;
use serde_json::{Value, json};
use sqlx::PgPool;
use tower_http::cors::CorsLayer;

use error::ApiError;

/// Потолок JSON-тела. Самое большое из них — правка записи с долями на всех
/// участников встречи, это единицы килобайт. У обложек будет свой роутер и свой
/// лимит: там мегабайт — норма, и путать эти два лимита нельзя.
const JSON_BODY_LIMIT: usize = 64 * 1024;

/// CORS передаётся снаружи, а не читается здесь из окружения: роутер должен
/// собираться в тесте без переменных среды, а решение «падать или нет при кривом
/// `ALLOWED_ORIGIN`» принимает `main`.
pub fn router(pool: PgPool, cors: CorsLayer) -> Router {
    Router::new()
        .route("/api/health", get(health))
        .layer(DefaultBodyLimit::max(JSON_BODY_LIMIT))
        .layer(cors)
        .with_state(pool)
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}

/// Обёртка над `Json`, приводящая отказ разбора к нашей форме ошибки.
///
/// Штатный `Json` при кривом теле отвечает `text/plain`, а клиент разбирает тело
/// как JSON и упал бы сам, потеряв настоящую причину. Поле в ошибке одно —
/// `body`: чего именно не хватает, серде сообщает в тексте.
///
/// Тело сверх `JSON_BODY_LIMIT` попадает сюда же и превращается в `422`.
/// Для JSON это нормально: 64 КБ невозможно перебрать честным запросом.
/// У обложек будет свой путь и свой `413`.
#[derive(Debug)]
pub struct JsonBody<T>(pub T);

#[axum::async_trait]
impl<S, T> FromRequest<S> for JsonBody<T>
where
    T: DeserializeOwned,
    S: Send + Sync,
{
    type Rejection = ApiError;

    async fn from_request(request: Request, state: &S) -> Result<Self, Self::Rejection> {
        let Json(value) = Json::<T>::from_request(request, state)
            .await
            .map_err(|rejection| ApiError::validation("body", rejection.body_text()))?;

        Ok(Self(value))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::http::StatusCode;
    use axum::response::IntoResponse;
    use http_body_util::BodyExt;

    #[tokio::test]
    async fn malformed_json_becomes_our_validation_error() {
        let request = Request::builder()
            .method("POST")
            .uri("/")
            .header("content-type", "application/json")
            .body(axum::body::Body::from("{нет закрывающей"))
            .expect("запрос");

        let rejection = JsonBody::<Value>::from_request(request, &())
            .await
            .expect_err("кривой JSON обязан отказать");

        let response = rejection.into_response();
        assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);

        let bytes = response
            .into_body()
            .collect()
            .await
            .expect("тело")
            .to_bytes();
        let body: Value = serde_json::from_slice(&bytes).expect("тело — JSON");

        assert_eq!(body["error"], "validation");
        assert_eq!(body["field"], "body");
    }
}
