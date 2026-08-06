//! Слой доступа к данным: запросы и отображение строк в доменные типы.
//!
//! Ни валидации, ни бизнес-правил здесь нет — они выше, в слое API. Функции
//! принимают `&mut PgConnection`, поэтому вызывающий сам решает, нужна ли
//! транзакция.

pub mod covers;
pub mod entries;
pub mod facts;
pub mod log;
pub mod meetings;
pub mod participants;
pub mod records;

use sqlx::PgPool;
use sqlx::postgres::PgPoolOptions;

/// Ошибка на старте приложения: без базы поднимать сервер бессмысленно.
#[derive(Debug, thiserror::Error)]
pub enum StartupError {
    #[error("переменная окружения {0} не задана")]
    MissingEnv(&'static str),
    #[error("не удалось подключиться к базе: {0}")]
    Connect(#[source] sqlx::Error),
    #[error("не удалось применить миграции: {0}")]
    Migrate(#[source] sqlx::migrate::MigrateError),
}

/// Читает `DATABASE_URL` из окружения. `.env` подхватывается вызывающим.
pub fn database_url() -> Result<String, StartupError> {
    std::env::var("DATABASE_URL").map_err(|_| StartupError::MissingEnv("DATABASE_URL"))
}

pub async fn connect(url: &str) -> Result<PgPool, StartupError> {
    PgPoolOptions::new()
        .max_connections(5)
        .connect(url)
        .await
        .map_err(StartupError::Connect)
}

/// Применяет миграции при старте. Инстанс один, поэтому отдельный шаг в деплое
/// не нужен. `migrate!` читает каталог при компиляции — база для сборки не
/// требуется, в отличие от макросов `query!`.
pub async fn run_migrations(pool: &PgPool) -> Result<(), StartupError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(StartupError::Migrate)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reports_which_variable_is_missing() {
        // Проверяем текст ошибки, а не факт её наличия: сообщение читает
        // человек, у которого не поднялся сервер.
        let error = StartupError::MissingEnv("DATABASE_URL");
        assert_eq!(
            error.to_string(),
            "переменная окружения DATABASE_URL не задана"
        );
    }
}
