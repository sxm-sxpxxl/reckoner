use axum::{Json, Router, routing::get};
use serde_json::{Value, json};
use tower_http::cors::{Any, CorsLayer};

#[tokio::main]
async fn main() {
    // `.env` нужен только локально; на Render переменные задаются в панели.
    let _ = dotenvy::dotenv();

    // Падаем на старте, а не отвечаем 500 на каждый запрос: сервер без базы
    // бесполезен, и лучше это увидеть сразу в логе.
    let url = or_exit(backend::db::database_url());
    let pool = or_exit(backend::db::connect(&url).await);
    or_exit(backend::db::run_migrations(&pool).await);

    let cors = CorsLayer::new().allow_origin(Any);

    let app = Router::new()
        .route("/api/health", get(health))
        .layer(cors)
        .with_state(pool);

    let listener = tokio::net::TcpListener::bind("0.0.0.0:3000")
        .await
        .expect("failed to bind to 0.0.0.0:3000");

    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.expect("server error");
}

/// Печатает причину и выходит с ненулевым кодом.
///
/// Не `expect`: тот выводит ошибку через `Debug`, то есть человек увидел бы
/// `MissingEnv("DATABASE_URL")` вместо написанного для него текста. А читать это
/// будут именно в логе — локально или в панели Render.
fn or_exit<T>(result: Result<T, backend::db::StartupError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            eprintln!("не удалось запустить сервер: {error}");
            std::process::exit(1);
        }
    }
}

async fn health() -> Json<Value> {
    Json(json!({ "status": "ok" }))
}
