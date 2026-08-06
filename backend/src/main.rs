use axum::http::{HeaderValue, Method, header};
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

    let app = backend::api::router(pool, cors_layer());
    let address = format!("0.0.0.0:{}", port());

    let listener = tokio::net::TcpListener::bind(&address)
        .await
        .unwrap_or_else(|error| {
            eprintln!("не удалось занять {address}: {error}");
            std::process::exit(1);
        });

    println!("listening on {}", listener.local_addr().unwrap());

    axum::serve(listener, app).await.expect("server error");
}

/// Порт из окружения.
///
/// Render выдаёт его в `PORT` и считает сервис живым, только если тот на нём
/// отвечает: с захардкоженным портом деплой висел бы в «in progress» до
/// таймаута. Локально переменной нет, и 3000 — тот же порт, которого ждёт
/// дев-прокси Vite.
fn port() -> u16 {
    let Ok(value) = std::env::var("PORT") else {
        return 3000;
    };

    value.parse().unwrap_or_else(|_| {
        eprintln!("не удалось запустить сервер: PORT не число: {value}");
        std::process::exit(1);
    })
}

/// CORS: в проде — единственный origin из `ALLOWED_ORIGIN`, локально — любой.
///
/// Правило одностороннее: если переменная **задана**, она обязана разбираться,
/// иначе выходим. Молча вернуться к открытому CORS при опечатке в панели Render
/// — худший из возможных исходов: прод остался бы нараспашку, а в логе была бы
/// строчка, которую никто не читает.
fn cors_layer() -> CorsLayer {
    let base = CorsLayer::new()
        .allow_methods([
            Method::GET,
            Method::POST,
            Method::PATCH,
            Method::PUT,
            Method::DELETE,
        ])
        .allow_headers([header::CONTENT_TYPE]);

    let Ok(origin) = std::env::var("ALLOWED_ORIGIN") else {
        // Локально фронт живёт на 5173, бэкенд на 3000 — это разные origin,
        // и без открытого CORS дев-режим бы не работал.
        eprintln!(
            "ALLOWED_ORIGIN не задана: CORS открыт для любого origin. \
             Для прода переменную надо задать."
        );

        return base.allow_origin(Any);
    };

    match parse_origin(&origin) {
        Ok(value) => base.allow_origin(value),
        Err(reason) => {
            eprintln!("не удалось запустить сервер: ALLOWED_ORIGIN {reason}: {origin}");
            std::process::exit(1);
        }
    }
}

/// Разбирает `ALLOWED_ORIGIN` в значение заголовка.
///
/// Одного `HeaderValue::from_str` недостаточно, и это проверено: он запрещает
/// только управляющие символы, а байты ≥ 128 разрешает как obs-text — строка
/// кириллицей проходила насквозь. Опечатка вроде `htps://…` прошла бы тоже,
/// и браузер просто не нашёл бы совпадения: фронтенд получал бы отказ CORS,
/// а в логе не было бы ни слова.
///
/// Origin — это схема, хост и порт, без пути и без слеша на конце: сравнение
/// в CORS строковое, и `https://x.io/` никогда не совпадёт с `https://x.io`.
fn parse_origin(raw: &str) -> Result<HeaderValue, &'static str> {
    let rest = raw
        .strip_prefix("https://")
        .or_else(|| raw.strip_prefix("http://"))
        .ok_or("должна начинаться с http:// или https://")?;

    if rest.is_empty() {
        return Err("не содержит хоста");
    }

    if rest.contains('/') {
        return Err("не должна содержать путь или слеш на конце");
    }

    if !rest.is_ascii() {
        return Err("содержит не-ASCII символы");
    }

    HeaderValue::from_str(raw).map_err(|_| "не годится в значение заголовка")
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
