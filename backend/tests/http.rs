//! Тесты роутера целиком: маршруты, коды ответов, форма тел, заголовки.
//!
//! В отличие от `tests/api.rs`, транзакцию с откатом здесь применить нельзя:
//! каждый запрос берёт из пула своё соединение. Поэтому тесты создают встречу
//! и удаляют её в конце. Упавший тест может оставить встречу в бранче `test` —
//! это допустимо, база черновая.
//!
//! В URI только ASCII: `http::Uri` не разбирает кириллицу, и русская строка
//! в пути или query уронила бы не ручку, а сам сборщик запроса.

mod support;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, StatusCode};
use http_body_util::BodyExt;
use serde_json::{Value, json};
use tower::ServiceExt;
use tower_http::cors::CorsLayer;

use support::test_pool;

/// Приложение на живом пуле. `Router` клонируется дёшево, поэтому один тест
/// может слать сколько нужно запросов, не пересоздавая пул: `test_pool`
/// подключается к Neon, и делать это на каждый запрос значило бы добавить
/// к тесту лишние round-trip'ы.
async fn app() -> Router {
    Router::clone(&backend::api::router(
        test_pool().await,
        CorsLayer::permissive(),
    ))
}

async fn read(response: axum::response::Response) -> (StatusCode, Value) {
    let status = response.status();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("тело ответа")
        .to_bytes();

    // У `204` тела нет — отдаём `null`, чтобы вызывающий не разбирал пустоту.
    let value = if bytes.is_empty() {
        Value::Null
    } else {
        serde_json::from_slice(&bytes).expect("тело — JSON")
    };

    (status, value)
}

/// Запрос с JSON-телом (или без тела).
async fn call_json(
    app: &Router,
    method: &str,
    uri: &str,
    body: Option<Value>,
) -> (StatusCode, Value) {
    let builder = Request::builder().method(method).uri(uri);

    let request = match body {
        Some(value) => builder
            .header("content-type", "application/json")
            .body(Body::from(serde_json::to_vec(&value).expect("тело")))
            .expect("запрос"),
        None => builder.body(Body::empty()).expect("запрос"),
    };

    read(app.clone().oneshot(request).await.expect("ответ")).await
}

/// Запрос с сырым телом — для обложек.
async fn call_bytes(app: &Router, method: &str, uri: &str, bytes: &[u8]) -> (StatusCode, Value) {
    let request = Request::builder()
        .method(method)
        .uri(uri)
        .body(Body::from(bytes.to_vec()))
        .expect("запрос");

    read(app.clone().oneshot(request).await.expect("ответ")).await
}

#[tokio::test]
async fn health_answers_ok() {
    let app = app().await;
    let (status, body) = call_json(&app, "GET", "/api/health", None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, json!({ "status": "ok" }));
}

#[tokio::test]
async fn missing_meeting_answers_404_in_our_shape() {
    let app = app().await;
    let (status, body) = call_json(
        &app,
        "GET",
        "/api/meetings/00000000-0000-0000-0000-000000000000",
        None,
    )
    .await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body, json!({ "error": "not-found" }));
}

#[tokio::test]
async fn malformed_uuid_in_path_does_not_reach_the_database() {
    let app = app().await;
    let request = Request::builder()
        .method("GET")
        .uri("/api/meetings/not-a-uuid")
        .body(Body::empty())
        .expect("запрос");
    let response = app.oneshot(request).await.expect("ответ");

    // Отказ извлекателя `Path` — единственный, который мы не переопределяем:
    // путь формирует наш собственный роутер, а не человек.
    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
}

#[tokio::test]
async fn create_read_and_delete_a_meeting_over_http() {
    let app = app().await;
    // Название латиницей: оно же уходит в query-параметр поиска ниже, а туда
    // кириллицу пришлось бы кодировать процентами вручную.
    let (status, created) = call_json(
        &app,
        "POST",
        "/api/meetings",
        Some(json!({ "title": "HTTP check", "heldOn": "2026-08-05" })),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED);

    let id = created["id"].as_str().expect("id встречи").to_owned();

    // Ключи — `camelCase`: это контракт с фронтендом, и на уровне соединения
    // его не видно, потому что там нет сериализации.
    assert_eq!(created["heldOn"], "2026-08-05");
    assert_eq!(created["hasCover"], false);
    assert_eq!(created["coverVersion"], 0);
    assert_eq!(created["totals"]["perPersonRubles"], 0);
    assert_eq!(created["status"], "no-participants");

    let (status, added) = call_json(
        &app,
        "POST",
        &format!("/api/meetings/{id}/participants"),
        Some(json!({ "name": "Настя", "emoji": "🦊" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(added["participants"][0]["colorIndex"], 0);
    assert_eq!(added["participants"][0]["contributedRubles"], 0);

    let (status, listed) = call_json(&app, "GET", "/api/meetings?q=HTTP%20check", None).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        listed
            .as_array()
            .expect("массив карточек")
            .iter()
            .any(|card| card["id"] == id.as_str()),
        "созданной встречи нет в списке"
    );

    let (status, body) = call_json(&app, "DELETE", &format!("/api/meetings/{id}"), None).await;
    assert_eq!(status, StatusCode::NO_CONTENT);
    assert_eq!(body, Value::Null);

    let (status, _) = call_json(&app, "GET", &format!("/api/meetings/{id}"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn validation_error_answers_422_and_names_the_field() {
    let app = app().await;
    let (_, created) = call_json(
        &app,
        "POST",
        "/api/meetings",
        Some(json!({ "title": "Validation" })),
    )
    .await;
    let id = created["id"].as_str().expect("id встречи").to_owned();

    let (status, body) = call_json(
        &app,
        "POST",
        &format!("/api/meetings/{id}/participants"),
        Some(json!({ "name": "   " })),
    )
    .await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
    assert_eq!(body["field"], "name");
    assert!(body["message"].is_string());

    call_json(&app, "DELETE", &format!("/api/meetings/{id}"), None).await;
}

#[tokio::test]
async fn malformed_body_answers_our_shape_too() {
    let app = app().await;
    let request = Request::builder()
        .method("POST")
        .uri("/api/meetings")
        .header("content-type", "application/json")
        .body(Body::from("{no closing brace"))
        .expect("запрос");
    let (status, body) = read(app.oneshot(request).await.expect("ответ")).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["error"], "validation");
    assert_eq!(body["field"], "body");
}

#[tokio::test]
async fn unknown_sort_answers_422() {
    let app = app().await;
    let (status, body) = call_json(&app, "GET", "/api/meetings?sort=whatever", None).await;

    assert_eq!(status, StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(body["field"], "sort");
}

/// Минимальные подписи форматов. Ими же сервер определяет тип: содержимое
/// дальше подписи он не разбирает, потому что картинку не декодирует.
const PNG: &[u8] = b"\x89PNG\r\n\x1a\nfake-bytes";
const JPEG: &[u8] = b"\xFF\xD8\xFFfake-bytes";

#[tokio::test]
async fn cover_upload_marks_the_meeting_and_bumps_the_version() {
    let app = app().await;
    let (_, created) = call_json(
        &app,
        "POST",
        "/api/meetings",
        Some(json!({ "title": "Cover" })),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_owned();

    let (status, view) = call_bytes(&app, "PUT", &format!("/api/meetings/{id}/cover"), PNG).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["hasCover"], true);
    assert_eq!(view["coverVersion"], 1);
    assert_eq!(view["log"][0]["text"], "Обновлена обложка встречи");

    // Замена поднимает версию ещё раз — на этом держится сброс кэша картинки.
    let (status, view) = call_bytes(&app, "PUT", &format!("/api/meetings/{id}/cover"), JPEG).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["coverVersion"], 2);

    call_json(&app, "DELETE", &format!("/api/meetings/{id}"), None).await;
}

#[tokio::test]
async fn cover_is_served_with_etag_and_answers_304() {
    let app = app().await;
    let (_, created) = call_json(
        &app,
        "POST",
        "/api/meetings",
        Some(json!({ "title": "Cover" })),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_owned();
    call_bytes(&app, "PUT", &format!("/api/meetings/{id}/cover"), PNG).await;

    let request = Request::builder()
        .method("GET")
        .uri(format!("/api/meetings/{id}/cover"))
        .body(Body::empty())
        .expect("запрос");
    let response = app.clone().oneshot(request).await.expect("ответ");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response.headers().get("content-type").expect("тип"),
        "image/png"
    );
    assert_eq!(
        response.headers().get("cache-control").expect("кэш"),
        "public, max-age=31536000, immutable"
    );

    let etag = response
        .headers()
        .get("etag")
        .expect("etag")
        .to_str()
        .expect("строка")
        .to_owned();
    let bytes = response
        .into_body()
        .collect()
        .await
        .expect("тело")
        .to_bytes();

    assert_eq!(bytes.as_ref(), PNG);

    // Повторный запрос с той же версией не должен возить байты снова.
    let conditional = Request::builder()
        .method("GET")
        .uri(format!("/api/meetings/{id}/cover"))
        .header("if-none-match", &etag)
        .body(Body::empty())
        .expect("запрос");
    let response = app.clone().oneshot(conditional).await.expect("ответ");

    assert_eq!(response.status(), StatusCode::NOT_MODIFIED);

    call_json(&app, "DELETE", &format!("/api/meetings/{id}"), None).await;
}

#[tokio::test]
async fn non_image_body_is_rejected_as_unsupported_media() {
    let app = app().await;
    let (_, created) = call_json(
        &app,
        "POST",
        "/api/meetings",
        Some(json!({ "title": "Cover" })),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_owned();

    // Заголовок врёт, что это картинка — тип определяется по байтам.
    let request = Request::builder()
        .method("PUT")
        .uri(format!("/api/meetings/{id}/cover"))
        .header("content-type", "image/png")
        .body(Body::from("just text, not an image"))
        .expect("запрос");
    let (status, body) = read(app.clone().oneshot(request).await.expect("ответ")).await;

    assert_eq!(status, StatusCode::UNSUPPORTED_MEDIA_TYPE);
    assert_eq!(body["error"], "unsupported-media");

    call_json(&app, "DELETE", &format!("/api/meetings/{id}"), None).await;
}

#[tokio::test]
async fn oversized_cover_is_rejected() {
    let app = app().await;
    let (_, created) = call_json(
        &app,
        "POST",
        "/api/meetings",
        Some(json!({ "title": "Cover" })),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_owned();

    // Больше мегабайта, но с правильной подписью — чтобы проверялся размер,
    // а не тип.
    let mut huge = PNG.to_vec();
    huge.resize(1024 * 1024 + 1, 0);

    let (status, body) = call_bytes(&app, "PUT", &format!("/api/meetings/{id}/cover"), &huge).await;

    assert_eq!(status, StatusCode::PAYLOAD_TOO_LARGE);
    assert_eq!(body["error"], "too-large");

    call_json(&app, "DELETE", &format!("/api/meetings/{id}"), None).await;
}

#[tokio::test]
async fn cover_of_a_missing_meeting_is_not_found() {
    let app = app().await;
    let missing = "/api/meetings/00000000-0000-0000-0000-000000000000/cover";

    let (status, _) = call_json(&app, "GET", missing, None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let (status, body) = call_bytes(&app, "PUT", missing, PNG).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(body["error"], "not-found");
}

#[tokio::test]
async fn deleting_a_cover_returns_the_meeting_without_it() {
    let app = app().await;
    let (_, created) = call_json(
        &app,
        "POST",
        "/api/meetings",
        Some(json!({ "title": "Cover" })),
    )
    .await;
    let id = created["id"].as_str().expect("id").to_owned();
    call_bytes(&app, "PUT", &format!("/api/meetings/{id}/cover"), PNG).await;

    let (status, view) =
        call_json(&app, "DELETE", &format!("/api/meetings/{id}/cover"), None).await;

    assert_eq!(status, StatusCode::OK);
    assert_eq!(view["hasCover"], false);
    // Версия остаётся: иначе прежний URL `?v=1` снова стал бы действительным.
    assert_eq!(view["coverVersion"], 1);
    assert_eq!(view["log"][0]["text"], "Обложка удалена");

    let (status, _) = call_json(&app, "GET", &format!("/api/meetings/{id}/cover"), None).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    call_json(&app, "DELETE", &format!("/api/meetings/{id}"), None).await;
}
