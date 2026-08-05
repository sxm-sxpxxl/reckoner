# Persistence Layer Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Положить факты встречи в Postgres и научиться читать их обратно в доменные типы, чтобы `reckon` считал по реальным данным.

**Architecture:** Слой `db` содержит только запросы и отображение строк в доменные типы — ни валидации, ни HTTP, ни бизнес-правил. Функции принимают `&mut PgConnection`, поэтому вызывающий сам решает, нужна ли транзакция, а тесты оборачивают каждый случай в транзакцию с откатом и не оставляют мусора. Домен из плана 1 не меняется ни на строку: если ему чего-то не хватает, это повод вернуться к спеке, а не дописать логику здесь.

**Tech Stack:** Rust 1.97 (edition 2024), sqlx 0.9 с Postgres и rustls, Neon Postgres (18.4), chrono, dotenvy, uuid.

---

## Предварительные условия

Без них план не выполняется:

1. Проект в Neon с тремя бранчами: `main` (прод), `dev` (разработка), `test` (интеграционные тесты).
2. `backend/.env` со строкой `DATABASE_URL=<unpooled строка подключения бранча dev>`.
3. `backend/.env` со строкой `TEST_DATABASE_URL=<unpooled строка подключения бранча test>`.

Пошаговая инструкция по настройке — [`docs/setup-neon.md`](../../setup-neon.md).

**Строки нужны именно unpooled, без `-pooler` в имени хоста.** Pooled-эндпоинт Neon — это PgBouncer
в transaction mode: он выбрасывает подготовленные запросы между транзакциями, а `sqlx` по умолчанию
их использует, и на случайных запросах полетят ошибки `prepared statement "sqlx_s_N" already
exists`. Пулер нужен при тысячах короткоживущих подключений; у нас один процесс с пулом на пять
соединений. Параметр `channel_binding=require` из строки тоже убирается — TLS даёт
`sslmode=require`.

Две переменные, а не одна, — намеренно. Интеграционные тесты пишут и удаляют данные; перепутать их
базу с рабочей нельзя. Тест, не нашедший `TEST_DATABASE_URL`, обязан падать с внятным сообщением,
а не тихо проходить.

## Решения, принятые до начала

**Запросы проверяются в рантайме.** Используем `sqlx::query_as` с `#[derive(FromRow)]`, а не макросы
`sqlx::query!`. Макросы проверяют SQL при компиляции, но требуют либо живой базы во время сборки,
либо закоммиченного кэша `.sqlx`, который надо перегенерировать при каждой правке запроса — CI без
базы будет падать на забытом `cargo sqlx prepare`. Взамен каждый запрос закрывается интеграционным
тестом; именно там опечатка в SQL и всплывает.

**Запросы пишутся строковыми литералами целиком, без `format!`.** В sqlx 0.9 `query`/`query_as`
принимают `impl SqlSafeStr`, а он реализован только для `&'static str` — склеить запрос в рантайме
без явной обёртки `AssertSqlSafe` нельзя. Обёртка существует, и наш случай (подстановка
compile-time константы со списком колонок) был бы безобиден, но применять её здесь не будем: приём,
появившийся в первом же запросе, повторится во всех следующих, а в Task 13 строится запрос
с фильтрами и сортировкой — ровно то место, куда потом просачивается пользовательский ввод. Лучше
заплатить дублированием списка колонок и оставить каждый запрос читаемым как литерал.

Практически это значит: там, где в блоках кода ниже стоит `sqlx::query_as(&format!("… {COLUMNS} …"))`,
надо писать литерал с выписанными колонками. Динамические части (сортировка в Task 13) собираются
не склейкой, а `match` по перечислению, каждая ветка которого возвращает свой литерал.

**Функции принимают `&mut PgConnection`.** Не `&PgPool`. Так вызывающий решает, нужна ли транзакция,
а тест оборачивает случай в транзакцию и откатывает её — изоляция без очистки. Где нужна атомарность
из нескольких запросов (запись расхода вместе с долями и строкой лога), вызывающий открывает
транзакцию и передаёт `&mut *tx`.

**`position` присваивается как `max(position) + 1` по встрече.** Не как число уже существующих
участников — это была ошибка в первой редакции плана, и она ломала бы приложение. Счётчик по
количеству после удаления кого-то из середины выдаёт уже занятый номер: двое получили 0 и 1,
первого удалили, `count(*)` стал 1, и следующий участник претендует на позицию 1, которая занята.
Проверено на живой базе — Postgres отвечает `duplicate key value violates unique constraint
"participants_meeting_id_position_key"`, то есть добавление участника падало бы после любого
удаления. Монотонный счётчик оставляет дырки в нумерации; они безобидны, потому что важен только
порядок, а не плотность, и тай-брейк при раздаче остатка рублей остаётся однозначным.

## Структура файлов

```
backend/migrations/
  0001_initial_schema.sql        вся схема одной миграцией

backend/src/db/
  mod.rs        пул, миграции, реэкспорты
  records.rs    строки таблиц — только данные, никакой логики
  meetings.rs   запросы к meetings
  participants.rs
  entries.rs    записи вместе с долями
  log.rs        «Что менялось»
  facts.rs      сборка StoredFacts для домена

backend/tests/
  persistence.rs  интеграционные тесты против бранча test
  support/mod.rs  подключение к тестовой базе и хелперы
```

`records.rs` отдельно от запросов, потому что строки нужны всем модулям и `facts.rs`, а запросы —
нет. `facts.rs` отдельно, потому что это единственное место, где слой БД знает про доменные типы:
если отображение переедет, менять придётся один файл.

---

### Task 1: Зависимости, `.env` и пул соединений

**Files:**
- Modify: `backend/Cargo.toml`
- Modify: `.gitignore`
- Create: `backend/src/db/mod.rs`
- Modify: `backend/src/lib.rs`

- [x] **Step 1: Закрыть `.env` от git прежде, чем он появится**

Выполнено в коммите `cca614e`, причём шире, чем здесь было написано. Вместо строки `backend/.env`
в `.gitignore` добавлен шаблон `.env` без ведущей косой черты — он закрывает файлы с таким именем
на любой глубине, а не только в одном каталоге. Рядом появился коммитируемый
`backend/.env.example`: он документирует, какие переменные нужны и какие у строк требования, чтобы
это не приходилось выяснять из кода.

Порядок важен: строка подключения к базе не должна попасть в историю даже одним коммитом.

- [ ] **Step 2: Добавить зависимости**

Run:

```
cd backend && cargo add sqlx --no-default-features --features runtime-tokio,tls-rustls,postgres,uuid,chrono,migrate,macros
```

Run:

```
cd backend && cargo add chrono --features serde
```

Run:

```
cd backend && cargo add dotenvy
```

Ожидается: в `Cargo.toml` появляются `sqlx`, `chrono`, `dotenvy`. Флаг `--no-default-features` у sqlx
не косметика: по умолчанию тянется `native-tls`, а нам нужен `rustls`, иначе на Render понадобится
системный OpenSSL.

- [ ] **Step 3: Написать падающий тест на чтение конфигурации**

`backend/src/db/mod.rs`:

```rust
//! Слой доступа к данным: запросы и отображение строк в доменные типы.
//!
//! Ни валидации, ни бизнес-правил здесь нет — они выше, в слое API. Функции
//! принимают `&mut PgConnection`, поэтому вызывающий сам решает, нужна ли
//! транзакция.

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
```

Дописать в `backend/src/lib.rs`:

```rust
pub mod db;
```

- [ ] **Step 4: Добавить `thiserror`**

Run: `cd backend && cargo add thiserror`

- [ ] **Step 5: Прогнать тест**

Run: `cd backend && cargo test --lib db`

Ожидается: `test result: ok. 1 passed; 0 failed`.

- [ ] **Step 6: Проверить линтером и закоммитить**

Run: `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

Ожидается: без предупреждений, пустой вывод `fmt --check`.

```bash
git add .gitignore backend/Cargo.toml backend/Cargo.lock backend/src/db backend/src/lib.rs
git commit -m "feat(db): add sqlx pool and startup configuration"
```

---

### Task 2: Схема и миграции

**Files:**
- Create: `backend/migrations/0001_initial_schema.sql`
- Modify: `backend/src/db/mod.rs`

- [ ] **Step 1: Написать миграцию**

`backend/migrations/0001_initial_schema.sql`:

```sql
create table meetings (
    id            uuid        primary key default gen_random_uuid(),
    title         text        not null,
    description   text        not null default '',
    emoji         text        not null default '✨',
    held_on       date        not null,
    cover_mime    text,
    cover_bytes   bytea,
    cover_version integer     not null default 0,
    created_at    timestamptz not null default now(),
    updated_at    timestamptz not null default now()
);

create table participants (
    id          uuid        primary key default gen_random_uuid(),
    meeting_id  uuid        not null references meetings (id) on delete cascade,
    name        text        not null,
    emoji       text        not null,
    color_index smallint    not null,
    position    integer     not null,
    created_at  timestamptz not null default now(),
    -- Не косметика: position участвует в тай-брейке при раздаче остатка рублей.
    -- Два одинаковых position в одной встрече сделали бы результат зависимым от
    -- порядка строк, который Postgres при равных ключах не гарантирует.
    unique (meeting_id, position)
);

create table entries (
    id            uuid        primary key default gen_random_uuid(),
    meeting_id    uuid        not null references meetings (id) on delete cascade,
    kind          text        not null check (kind in ('expense', 'transfer')),
    payer_id      uuid        not null references participants (id) on delete cascade,
    recipient_id  uuid                 references participants (id) on delete cascade,
    amount_rubles bigint      not null check (amount_rubles > 0),
    description   text        not null default '',
    occurred_at   timestamptz not null default now(),
    created_at    timestamptz not null default now(),
    -- Получатель есть тогда и только тогда, когда это перевод.
    check ((kind = 'transfer') = (recipient_id is not null)),
    check (recipient_id is null or recipient_id <> payer_id)
);

create index entries_meeting_occurred_idx on entries (meeting_id, occurred_at desc);

create table entry_shares (
    entry_id        uuid     not null references entries (id) on delete cascade,
    participant_id  uuid     not null references participants (id) on delete cascade,
    -- Полная доля (4/4) не хранится: её отсутствие и есть полная доля.
    weight_quarters smallint not null check (weight_quarters between 0 and 3),
    primary key (entry_id, participant_id)
);

create table meeting_log (
    id         bigserial   primary key,
    meeting_id uuid        not null references meetings (id) on delete cascade,
    text       text        not null,
    created_at timestamptz not null default now()
);

create index meeting_log_meeting_created_idx on meeting_log (meeting_id, created_at desc);
```

`gen_random_uuid()` в Postgres 13 и новее встроена, расширение подключать не нужно. Neon работает на
свежих версиях.

- [ ] **Step 2: Встроить миграции в бинарник**

Дописать в `backend/src/db/mod.rs` после `connect`:

```rust
/// Применяет миграции при старте. Инстанс один, поэтому отдельный шаг в деплое
/// не нужен. `migrate!` читает каталог при компиляции — база для сборки не
/// требуется, в отличие от макросов `query!`.
pub async fn run_migrations(pool: &PgPool) -> Result<(), StartupError> {
    sqlx::migrate!("./migrations")
        .run(pool)
        .await
        .map_err(StartupError::Migrate)
}
```

- [ ] **Step 3: Проверить, что миграция компилируется в бинарник**

Run: `cd backend && cargo test --lib db`

Ожидается: `test result: ok. 1 passed`. Если `migrate!` не находит каталог — путь указан
относительно `backend/`, проверьте, что файл лежит в `backend/migrations/`.

Компиляции недостаточно: она проверяет, что каталог найден, но не что SQL валиден. Опечатка в
миграции всплыла бы только на Task 3, когда её начнёт применять тестовый каркас. Поэтому миграция
дополнительно применена живьём к бранчам `dev` и `test` одноразовым тестом (удалён после прогона):
в обоих создались `meetings`, `participants`, `entries`, `entry_shares`, `meeting_log` и служебная
`_sqlx_migrations`. Повторный запуск прошёл без ошибок — идемпотентность подтверждена, значит
применять миграции при каждом старте сервера безопасно.

- [ ] **Step 4: Закоммитить**

```bash
git add backend/migrations backend/src/db/mod.rs
git commit -m "feat(db): add initial schema migration"
```

---

### Task 3: Каркас интеграционных тестов

Без него нельзя проверить ни один запрос, поэтому идёт раньше запросов.

**Files:**
- Create: `backend/tests/support/mod.rs`
- Create: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать хелпер подключения**

`backend/tests/support/mod.rs`:

```rust
//! Подключение к тестовой базе. Каждый тест работает в транзакции, которая
//! откатывается при выходе, поэтому тесты не видят друг друга и не оставляют
//! мусора — чистить ничего не нужно.

use sqlx::{PgPool, postgres::PgPoolOptions};

/// Пул к бранчу `test`. Отдельная переменная, а не `DATABASE_URL`: тесты пишут
/// и удаляют данные, и перепутать их базу с рабочей нельзя.
///
/// Паникует, если переменная не задана. Тихо проходить такой тест не должен —
/// зелёный прогон без базы создаёт ложное чувство покрытия.
pub async fn test_pool() -> PgPool {
    // `cargo test` не читает `.env` сам, в отличие от бинарника, который делает
    // это в `main`. Без этой строки переменная не найдётся, даже если она в файле.
    // Уже заданные в окружении значения `dotenvy` не перетирает.
    dotenvy::dotenv().ok();

    let url = std::env::var("TEST_DATABASE_URL").expect(
        "TEST_DATABASE_URL не задана. Интеграционные тесты требуют бранч `test` в Neon; \
         строка подключения кладётся в backend/.env (см. docs/setup-neon.md). \
         В CI эти тесты не запускаются: там идёт только `cargo test --lib`.",
    );

    let pool = PgPoolOptions::new()
        .max_connections(5)
        .connect(&url)
        .await
        .expect("не удалось подключиться к тестовой базе");

    backend::db::run_migrations(&pool)
        .await
        .expect("не удалось применить миграции к тестовой базе");

    pool
}
```

- [ ] **Step 2: Написать падающий тест**

`backend/tests/persistence.rs`:

```rust
mod support;

use support::test_pool;

#[tokio::test]
async fn migrations_apply_and_schema_is_queryable() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    // Пять таблиц схемы должны существовать после миграций.
    let tables: Vec<String> = sqlx::query_scalar(
        "select table_name from information_schema.tables \
         where table_schema = 'public' and table_type = 'BASE TABLE' \
         order by table_name",
    )
    .fetch_all(&mut *tx)
    .await
    .expect("список таблиц");

    for expected in [
        "entries",
        "entry_shares",
        "meeting_log",
        "meetings",
        "participants",
    ] {
        assert!(
            tables.iter().any(|name| name == expected),
            "нет таблицы {expected}; есть: {tables:?}"
        );
    }
}
```

- [ ] **Step 3: Добавить `tokio` в dev-зависимости для тестов**

`tokio` уже есть в основных зависимостях с features `full`, поэтому `#[tokio::test]` доступен.
Проверьте это, а не добавляйте вторую копию:

Run: `cd backend && cargo test --test persistence`

Ожидается: тест проходит. Если `#[tokio::test]` не найден — в `Cargo.toml` у `tokio` нет feature
`macros`; тогда добавьте её: `cargo add tokio --features full`.

- [ ] **Step 4: Убедиться, что без переменной тест падает понятно**

Снять переменную из окружения недостаточно: `dotenvy` подставит её из `backend/.env`, и тест пройдёт.
Чтобы проверить сообщение, файл надо на время убрать — с гарантированным возвратом, иначе одна
неудачная команда оставит разработчика без строк подключения.

Run (PowerShell):

```
$envPath = "D:\rust-projects\reckoner\backend\.env"; $bak = "$envPath.verify-bak"; Set-Location D:\rust-projects\reckoner\backend; try { Rename-Item $envPath $bak -ErrorAction Stop; if (Test-Path Env:\TEST_DATABASE_URL) { Remove-Item Env:\TEST_DATABASE_URL }; cargo test --test persistence 2>&1 | Select-String "TEST_DATABASE_URL|panicked" } finally { if (Test-Path $bak) { Rename-Item $bak $envPath } }; "env на месте: " + (Test-Path $envPath)
```

Ожидается: в выводе текст про то, что переменная не задана и где взять строку подключения, затем
`env на месте: True`. Это проверка сообщения, а не поведения: его будет читать человек, у которого
тесты не идут. Ненулевой код возврата здесь нормален — тест обязан упасть.

После проверки прогнать тест ещё раз и убедиться, что он снова зелёный.

- [ ] **Step 5: Закоммитить**

```bash
git add backend/tests
git commit -m "test(db): add integration test harness against the test branch"
```

---

### Task 4: Строки таблиц

**Files:**
- Create: `backend/src/db/records.rs`
- Modify: `backend/src/db/mod.rs`

- [ ] **Step 1: Описать строки**

`backend/src/db/records.rs`:

```rust
//! Строки таблиц: только данные, никакой логики. Отображение в доменные типы
//! живёт в `facts.rs`, запросы — в модулях по таблицам.

use chrono::{DateTime, NaiveDate, Utc};
use sqlx::FromRow;
use uuid::Uuid;

#[derive(Debug, Clone, FromRow)]
pub struct MeetingRow {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub emoji: String,
    pub held_on: NaiveDate,
    /// Заполнен, если обложка загружена. Сами байты читаются отдельным запросом:
    /// они не нужны ни на списке встреч, ни на странице встречи.
    pub cover_mime: Option<String>,
    pub cover_version: i32,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Debug, Clone, FromRow)]
pub struct ParticipantRow {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub name: String,
    pub emoji: String,
    pub color_index: i16,
    pub position: i32,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, sqlx::Type)]
#[sqlx(type_name = "text", rename_all = "lowercase")]
pub enum EntryKindRow {
    Expense,
    Transfer,
}

#[derive(Debug, Clone, FromRow)]
pub struct EntryRow {
    pub id: Uuid,
    pub meeting_id: Uuid,
    pub kind: EntryKindRow,
    pub payer_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: i64,
    pub description: String,
    pub occurred_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, FromRow)]
pub struct ShareRow {
    pub entry_id: Uuid,
    pub participant_id: Uuid,
    pub weight_quarters: i16,
}

#[derive(Debug, Clone, FromRow)]
pub struct LogRow {
    pub id: i64,
    pub text: String,
    pub created_at: DateTime<Utc>,
}
```

Дописать в `backend/src/db/mod.rs`:

```rust
pub mod records;
```

**Результат: derive сработал.** На sqlx 0.9 форма `#[sqlx(type_name = "text", rename_all =
"lowercase")]` компилируется, и — что важнее — правильно ходит в Postgres и обратно. Компиляция
здесь ничего не доказывала бы сама по себе, поэтому в `tests/persistence.rs` добавлен постоянный
тест `entry_kind_round_trips_through_a_text_column`: он проверяет, что `Expense` кодируется ровно
в `expense`, а `transfer` раскодируется обратно. Тест не про sqlx, а про то, что значения
перечисления совпадают со строками в CHECK-констрейнте схемы: разойдись они — вставка записи упала
бы в рантайме, а не при сборке. Запасной вариант ниже не понадобился; он оставлен как справка.

**Если `sqlx::Type` на `EntryKindRow` не соберётся** (например, на другой версии sqlx), не
изобретайте: замените derive на явное преобразование, оно работает всегда.

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKindRow {
    Expense,
    Transfer,
}

impl EntryKindRow {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Expense => "expense",
            Self::Transfer => "transfer",
        }
    }

    /// Значения ограничены CHECK в схеме, поэтому чужая строка означает, что
    /// схема и код разошлись — это баг, а не пользовательский ввод.
    pub fn from_column(value: &str) -> Self {
        match value {
            "expense" => Self::Expense,
            "transfer" => Self::Transfer,
            other => panic!("неизвестный kind в базе: {other}"),
        }
    }
}
```

В этом случае в `EntryRow` поле объявляется как `pub kind: String`, в `entries::insert`
привязывается `entry.kind.as_str()`, а в тестах сравнение идёт со строкой (`assert_eq!(entry.kind,
"expense")`). В `facts.rs` вместо `match row.kind` используется
`match EntryKindRow::from_column(&row.kind)`. Отметьте в отчёте, какой вариант получился.

- [ ] **Step 2: Проверить, что типы компилируются**

Run: `cd backend && cargo test --lib db`

Ожидается: `test result: ok. 1 passed`.

- [ ] **Step 3: Закоммитить**

```bash
git add backend/src/db
git commit -m "feat(db): describe table rows"
```

---

### Task 5: Встречи — вставка и чтение

**Files:**
- Create: `backend/src/db/meetings.rs`
- Modify: `backend/src/db/mod.rs`
- Modify: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать падающий тест**

Дописать в `backend/tests/persistence.rs`:

```rust
use backend::db::meetings::{self, NewMeeting};
use chrono::NaiveDate;

#[tokio::test]
async fn inserts_and_reads_back_a_meeting() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "Дача у Влада".to_owned(),
            description: "Три дня, баня и продукты из «Ленты».".to_owned(),
            emoji: "🏡".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 7, 23).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи");

    assert_eq!(created.title, "Дача у Влада");
    assert_eq!(created.emoji, "🏡");
    assert_eq!(created.cover_version, 0);
    assert!(created.cover_mime.is_none());

    let found = meetings::find(&mut tx, created.id)
        .await
        .expect("чтение встречи")
        .expect("встреча существует");

    assert_eq!(found.id, created.id);
    assert_eq!(found.held_on, created.held_on);
}

#[tokio::test]
async fn returns_none_for_a_missing_meeting() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let found = meetings::find(&mut tx, uuid::Uuid::nil())
        .await
        .expect("запрос выполнен");

    assert!(found.is_none());
}
```

- [ ] **Step 2: Запустить и убедиться, что не компилируется**

Run: `cd backend && cargo test --test persistence`

Ожидается: ошибка компиляции — модуля `meetings` ещё нет. Это и есть красный шаг для слоя данных:
запрос нельзя «сначала объявить», не написав его.

- [ ] **Step 3: Реализовать**

`backend/src/db/meetings.rs`:

```rust
use chrono::NaiveDate;
use sqlx::PgConnection;
use uuid::Uuid;

use super::records::MeetingRow;

/// Поля новой встречи. Значения по умолчанию подставляет слой API, а не база:
/// пустое название превращается в «Новая встреча» там, где есть тексты.
#[derive(Debug, Clone)]
pub struct NewMeeting {
    pub title: String,
    pub description: String,
    pub emoji: String,
    pub held_on: NaiveDate,
}

// Список колонок выписан в каждом запросе, а не собран через `format!` из общей
// константы: в sqlx 0.9 запрос обязан быть `&'static str`, иначе нужна обёртка
// `AssertSqlSafe`. Обёртку не используем сознательно — она снимает защиту от
// склейки запросов там, где позже появятся фильтры с пользовательским вводом.
// Байты обложки не читаются ни одним из запросов: они не нужны ни списку,
// ни странице встречи, и возить их в каждом ответе значило бы тратить трафик.

pub async fn insert(
    conn: &mut PgConnection,
    meeting: NewMeeting,
) -> Result<MeetingRow, sqlx::Error> {
    sqlx::query_as(
        "insert into meetings (title, description, emoji, held_on) \
         values ($1, $2, $3, $4) \
         returning id, title, description, emoji, held_on, cover_mime, \
                   cover_version, created_at, updated_at",
    )
    .bind(meeting.title)
    .bind(meeting.description)
    .bind(meeting.emoji)
    .bind(meeting.held_on)
    .fetch_one(conn)
    .await
}

pub async fn find(conn: &mut PgConnection, id: Uuid) -> Result<Option<MeetingRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, title, description, emoji, held_on, cover_mime, \
                cover_version, created_at, updated_at \
         from meetings where id = $1",
    )
    .bind(id)
    .fetch_optional(conn)
    .await
}
```

Дописать в `backend/src/db/mod.rs`:

```rust
pub mod meetings;
```

- [ ] **Step 4: Прогнать тесты**

Run: `cd backend && cargo test --test persistence`

Ожидается: `test result: ok. 3 passed; 0 failed`.

- [ ] **Step 5: Проверить линтером и закоммитить**

Run: `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

```bash
git add backend/src/db backend/tests/persistence.rs
git commit -m "feat(db): insert and read meetings"
```

---

### Task 6: Встречи — правка и удаление

**Files:**
- Modify: `backend/src/db/meetings.rs`
- Modify: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать падающий тест**

Дописать в `backend/tests/persistence.rs`:

```rust
use backend::db::meetings::MeetingPatch;

#[tokio::test]
async fn patches_only_provided_fields() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "Новая встреча".to_owned(),
            description: "черновик".to_owned(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка");

    let patched = meetings::update(
        &mut tx,
        created.id,
        MeetingPatch {
            title: Some("Солевые шашлыки".to_owned()),
            emoji: Some("🔥".to_owned()),
            ..MeetingPatch::default()
        },
    )
    .await
    .expect("правка")
    .expect("встреча существует");

    assert_eq!(patched.title, "Солевые шашлыки");
    assert_eq!(patched.emoji, "🔥");
    // Не переданные поля остаются как были.
    assert_eq!(patched.description, "черновик");
    assert_eq!(patched.held_on, created.held_on);
    // Правка двигает updated_at.
    assert!(patched.updated_at >= created.updated_at);
}

#[tokio::test]
async fn deletes_a_meeting_and_reports_whether_it_existed() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let created = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "На удаление".to_owned(),
            description: String::new(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка");

    assert!(meetings::delete(&mut tx, created.id).await.expect("удаление"));
    assert!(
        !meetings::delete(&mut tx, created.id)
            .await
            .expect("повторное удаление")
    );
    assert!(
        meetings::find(&mut tx, created.id)
            .await
            .expect("чтение")
            .is_none()
    );
}
```

- [ ] **Step 2: Запустить и убедиться, что не компилируется**

Run: `cd backend && cargo test --test persistence`

Ожидается: ошибка компиляции — нет `MeetingPatch`, `update`, `delete`.

- [ ] **Step 3: Реализовать**

Дописать в `backend/src/db/meetings.rs`:

```rust
/// Частичная правка: `None` означает «не менять». Собирать SQL из непустых
/// полей не нужно — `coalesce` делает это на стороне базы одним запросом.
#[derive(Debug, Clone, Default)]
pub struct MeetingPatch {
    pub title: Option<String>,
    pub description: Option<String>,
    pub emoji: Option<String>,
    pub held_on: Option<NaiveDate>,
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    patch: MeetingPatch,
) -> Result<Option<MeetingRow>, sqlx::Error> {
    sqlx::query_as(
        "update meetings set \
             title = coalesce($2, title), \
             description = coalesce($3, description), \
             emoji = coalesce($4, emoji), \
             held_on = coalesce($5, held_on), \
             updated_at = now() \
         where id = $1 \
         returning id, title, description, emoji, held_on, cover_mime, \
                   cover_version, created_at, updated_at",
    )
    .bind(id)
    .bind(patch.title)
    .bind(patch.description)
    .bind(patch.emoji)
    .bind(patch.held_on)
    .fetch_optional(conn)
    .await
}

/// `true`, если встреча была и удалена. Каскады уносят участников, записи,
/// доли и лог.
pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("delete from meetings where id = $1")
        .bind(id)
        .execute(conn)
        .await?;

    Ok(result.rows_affected() > 0)
}
```

- [ ] **Step 4: Прогнать тесты**

Run: `cd backend && cargo test --test persistence`

Ожидается: `test result: ok. 5 passed; 0 failed`.

- [ ] **Step 5: Закоммитить**

```bash
git add backend/src/db/meetings.rs backend/tests/persistence.rs
git commit -m "feat(db): patch and delete meetings"
```

---

### Task 7: Участники

**Files:**
- Create: `backend/src/db/participants.rs`
- Modify: `backend/src/db/mod.rs`
- Modify: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать падающий тест**

Дописать в `backend/tests/persistence.rs`:

```rust
use backend::db::participants;

/// Встреча-заготовка: почти каждому тесту ниже нужна встреча и ничего больше.
async fn seed_meeting(tx: &mut sqlx::PgConnection) -> uuid::Uuid {
    meetings::insert(
        tx,
        NewMeeting {
            title: "Тестовая".to_owned(),
            description: String::new(),
            emoji: "✨".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 5).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи")
    .id
}

#[tokio::test]
async fn assigns_position_and_color_by_order_of_addition() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let first = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("первый участник");
    let second = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй участник");

    assert_eq!(first.position, 0);
    assert_eq!(second.position, 1);
    // Палитра аватаров на 8 цветов; индекс идёт по кругу.
    assert_eq!(first.color_index, 0);
    assert_eq!(second.color_index, 1);
}

#[tokio::test]
async fn lists_participants_in_position_order() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    for (name, emoji) in [("Настя", "🦊"), ("Влад", "🦉"), ("Егор", "🐸")] {
        participants::insert(&mut tx, meeting_id, name, emoji)
            .await
            .expect("участник");
    }

    let listed = participants::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("список");

    let names: Vec<&str> = listed.iter().map(|row| row.name.as_str()).collect();
    assert_eq!(names, ["Настя", "Влад", "Егор"]);
}

#[tokio::test]
async fn updates_and_deletes_a_participant() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let person = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("участник");

    let renamed = participants::update(&mut tx, person.id, "Анастасия", "🦩")
        .await
        .expect("правка")
        .expect("участник существует");
    assert_eq!(renamed.name, "Анастасия");
    assert_eq!(renamed.emoji, "🦩");
    // Позиция и цвет при правке не меняются.
    assert_eq!(renamed.position, person.position);
    assert_eq!(renamed.color_index, person.color_index);

    assert!(
        participants::delete(&mut tx, person.id)
            .await
            .expect("удаление")
    );
    assert!(
        participants::list_for_meeting(&mut tx, meeting_id)
            .await
            .expect("список")
            .is_empty()
    );
}

#[tokio::test]
async fn position_keeps_growing_after_a_deletion() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let first = participants::insert(&mut tx, meeting_id, "Раз", "🐻")
        .await
        .expect("первый");
    participants::insert(&mut tx, meeting_id, "Два", "🦊")
        .await
        .expect("второй");
    participants::delete(&mut tx, first.id)
        .await
        .expect("удаление");

    // Номер берётся как max(position) + 1, поэтому третий получает 2, а не 1.
    // Счётчик по числу существующих участников выдал бы здесь 1 — уже занятый
    // «Два» номер — и вставка упала бы на уникальном индексе. Дырка на месте
    // удалённого остаётся навсегда, и это правильно.
    let third = participants::insert(&mut tx, meeting_id, "Три", "🐸")
        .await
        .expect("третий");
    assert_eq!(third.position, 2);

    let positions: Vec<i32> = participants::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("список")
        .iter()
        .map(|row| row.position)
        .collect();
    assert_eq!(positions, [1, 2]);
}
```

Тест назван `position_never_reuses_a_number_after_a_deletion`: он про то, что номер не переиспользуется,
а не про то, что нумерация плотная.

- [ ] **Step 2: Запустить и убедиться, что не компилируется**

Run: `cd backend && cargo test --test persistence`

Ожидается: ошибка компиляции — модуля `participants` нет.

- [ ] **Step 3: Реализовать**

`backend/src/db/participants.rs`:

```rust
use sqlx::PgConnection;
use uuid::Uuid;

use super::records::ParticipantRow;

/// Размер палитры аватаров из дизайна. Цвет не хранится, хранится индекс:
/// сами цвета знает фронтенд. Передаётся в запрос параметром, а не подставляется
/// в текст, чтобы значение осталось единственным.
const PALETTE_SIZE: i64 = 8;

/// `position` и `color_index` присваивает база, а не вызывающий: они зависят от
/// того, что уже есть в таблице, и считать это на стороне приложения значило бы
/// гонку между двумя одновременными добавлениями.
///
/// Номер берётся как `max(position) + 1`, а не как число существующих
/// участников. Разница принципиальная: после удаления кого-то из середины
/// счётчик по количеству выдал бы уже занятый номер и упёрся в уникальный
/// индекс `(meeting_id, position)`. Монотонный счётчик оставляет дырки
/// в нумерации — это безобидно, а вот столкновение сломало бы добавление
/// участника после любого удаления.
pub async fn insert(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    name: &str,
    emoji: &str,
) -> Result<ParticipantRow, sqlx::Error> {
    sqlx::query_as(
        "insert into participants (meeting_id, name, emoji, color_index, position) \
         select $1, $2, $3, (next.value % $4)::smallint, next.value::int \
         from ( \
             select coalesce(max(position), -1) + 1 as value \
             from participants where meeting_id = $1 \
         ) as next \
         returning id, meeting_id, name, emoji, color_index, position, created_at",
    )
    .bind(meeting_id)
    .bind(name)
    .bind(emoji)
    .bind(PALETTE_SIZE)
    .fetch_one(conn)
    .await
}

/// Порядок по `position` — тот же, что использует домен для тай-брейка при
/// раздаче остатка рублей.
pub async fn list_for_meeting(
    conn: &mut PgConnection,
    meeting_id: Uuid,
) -> Result<Vec<ParticipantRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, meeting_id, name, emoji, color_index, position, created_at \
         from participants where meeting_id = $1 order by position",
    )
    .bind(meeting_id)
    .fetch_all(conn)
    .await
}

/// Меняются только имя и эмодзи: позиция и цвет закреплены за участником
/// с момента добавления.
pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    name: &str,
    emoji: &str,
) -> Result<Option<ParticipantRow>, sqlx::Error> {
    sqlx::query_as(
        "update participants set name = $2, emoji = $3 where id = $1 \
         returning id, meeting_id, name, emoji, color_index, position, created_at",
    )
    .bind(id)
    .bind(name)
    .bind(emoji)
    .fetch_optional(conn)
    .await
}

/// Каскады уносят все записи, где участник плательщик или получатель, и его
/// доли — это требование дизайна, а не побочный эффект.
pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("delete from participants where id = $1")
        .bind(id)
        .execute(conn)
        .await?;

    Ok(result.rows_affected() > 0)
}
```

Дописать в `backend/src/db/mod.rs`:

```rust
pub mod participants;
```

- [ ] **Step 4: Прогнать тесты**

Run: `cd backend && cargo test --test persistence`

Ожидается: `test result: ok. 9 passed; 0 failed`.

- [ ] **Step 5: Закоммитить**

```bash
git add backend/src/db backend/tests/persistence.rs
git commit -m "feat(db): store meeting participants"
```

---

### Task 8: Записи и доли

**Files:**
- Create: `backend/src/db/entries.rs`
- Modify: `backend/src/db/mod.rs`
- Modify: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать падающий тест**

Дописать в `backend/tests/persistence.rs`:

```rust
use backend::db::entries::{self, NewEntry};
use backend::db::records::EntryKindRow;

#[tokio::test]
async fn stores_an_expense_with_partial_shares() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");
    let other = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй");

    let entry = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 8400,
            description: "Продукты на все дни".to_owned(),
            occurred_at: None,
            shares: vec![(other.id, 2)],
        },
    )
    .await
    .expect("вставка расхода");

    assert_eq!(entry.amount_rubles, 8400);
    assert_eq!(entry.kind, EntryKindRow::Expense);
    assert!(entry.recipient_id.is_none());

    let shares = entries::shares_for_meeting(&mut tx, meeting_id)
        .await
        .expect("доли");
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].participant_id, other.id);
    assert_eq!(shares[0].weight_quarters, 2);
}
```

Тест на перевод разделён на два. Проверка перевода самому себе живёт в **отдельной** транзакции:
нарушение CHECK переводит транзакцию Postgres в сбойное состояние, и любой следующий запрос в ней
тоже упал бы. Держать обе проверки в одной транзакции можно, только если отбитая идёт последней, —
а это скрытая зависимость от порядка, которая ломается при добавлении ещё одной проверки.

```rust
#[tokio::test]
async fn stores_a_transfer() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let from = participants::insert(&mut tx, meeting_id, "Егор", "🐸")
        .await
        .expect("отправитель");
    let to = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("получатель");

    let transfer = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Transfer,
            payer_id: from.id,
            recipient_id: Some(to.id),
            amount_rubles: 3425,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("вставка перевода");

    assert_eq!(transfer.recipient_id, Some(to.id));
    assert_eq!(transfer.kind, EntryKindRow::Transfer);
}

#[tokio::test]
async fn rejects_a_self_transfer() {
    let pool = test_pool().await;
    // Отдельная транзакция: нарушение CHECK переводит транзакцию Postgres
    // в сбойное состояние, и все последующие запросы в ней тоже упали бы.
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let person = participants::insert(&mut tx, meeting_id, "Егор", "🐸")
        .await
        .expect("участник");

    // Слой API проверит это раньше и вернёт 422, но защита в базе — последняя
    // линия, и она должна работать.
    let rejected = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Transfer,
            payer_id: person.id,
            recipient_id: Some(person.id),
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await;

    assert!(rejected.is_err(), "перевод самому себе должен быть отбит");
}

#[tokio::test]
async fn deleting_a_participant_removes_their_entries() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");
    let other = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй");

    entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 1000,
            description: "Его расход".to_owned(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("расход плательщика");

    entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Transfer,
            payer_id: other.id,
            recipient_id: Some(payer.id),
            amount_rubles: 500,
            description: String::new(),
            occurred_at: None,
            shares: Vec::new(),
        },
    )
    .await
    .expect("перевод в его адрес");

    participants::delete(&mut tx, payer.id)
        .await
        .expect("удаление участника");

    // Требование дизайна: удаление участника уносит и записи, где он
    // плательщик, и записи, где он получатель.
    let left = entries::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("записи");
    assert!(left.is_empty(), "остались записи: {left:?}");
}

#[tokio::test]
async fn lists_entries_newest_first() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;
    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");

    let early = chrono::DateTime::parse_from_rfc3339("2026-07-23T20:00:00Z")
        .expect("дата")
        .to_utc();
    let late = chrono::DateTime::parse_from_rfc3339("2026-07-24T23:00:00Z")
        .expect("дата")
        .to_utc();

    for (amount, when) in [(100, early), (200, late)] {
        entries::insert(
            &mut tx,
            meeting_id,
            NewEntry {
                kind: EntryKindRow::Expense,
                payer_id: payer.id,
                recipient_id: None,
                amount_rubles: amount,
                description: String::new(),
                occurred_at: Some(when),
                shares: Vec::new(),
            },
        )
        .await
        .expect("расход");
    }

    let listed = entries::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("записи");

    // История в интерфейсе идёт новыми сверху.
    assert_eq!(listed[0].amount_rubles, 200);
    assert_eq!(listed[1].amount_rubles, 100);
}
```

- [ ] **Step 2: Запустить и убедиться, что не компилируется**

Run: `cd backend && cargo test --test persistence`

Ожидается: ошибка компиляции — модуля `entries` нет.

- [ ] **Step 3: Реализовать**

`backend/src/db/entries.rs`:

```rust
use chrono::{DateTime, Utc};
use sqlx::PgConnection;
use uuid::Uuid;

use super::records::{EntryKindRow, EntryRow, ShareRow};

/// Новая запись вместе с неполными долями. Полная доля (4/4) в `shares` не
/// передаётся: её отсутствие и есть полная доля.
#[derive(Debug, Clone)]
pub struct NewEntry {
    pub kind: EntryKindRow,
    pub payer_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: i64,
    pub description: String,
    /// `None` — «сейчас». Явное значение нужно тестам и импорту.
    pub occurred_at: Option<DateTime<Utc>>,
    /// Пары `(участник, четверти)`, только для неполных долей.
    pub shares: Vec<(Uuid, i16)>,
}

/// Вставляет запись и её доли. Вызывающий обязан передать транзакцию: запись
/// без своих долей — это неверный расчёт, а не просто неполные данные.
pub async fn insert(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    entry: NewEntry,
) -> Result<EntryRow, sqlx::Error> {
    let row: EntryRow = sqlx::query_as(
        "insert into entries \
             (meeting_id, kind, payer_id, recipient_id, amount_rubles, description, occurred_at) \
         values ($1, $2, $3, $4, $5, $6, coalesce($7, now())) \
         returning id, meeting_id, kind, payer_id, recipient_id, \
                   amount_rubles, description, occurred_at, created_at",
    )
    .bind(meeting_id)
    .bind(entry.kind)
    .bind(entry.payer_id)
    .bind(entry.recipient_id)
    .bind(entry.amount_rubles)
    .bind(entry.description)
    .bind(entry.occurred_at)
    .fetch_one(&mut *conn)
    .await?;

    for (participant_id, weight_quarters) in entry.shares {
        sqlx::query(
            "insert into entry_shares (entry_id, participant_id, weight_quarters) \
             values ($1, $2, $3)",
        )
        .bind(row.id)
        .bind(participant_id)
        .bind(weight_quarters)
        .execute(&mut *conn)
        .await?;
    }

    Ok(row)
}

/// Новые сверху — так история показана в интерфейсе.
///
/// Второй ключ сортировки не украшение: две записи с одинаковым `occurred_at`
/// без него шли бы в произвольном порядке, и один и тот же запрос мог бы
/// вернуть историю в разном виде.
pub async fn list_for_meeting(
    conn: &mut PgConnection,
    meeting_id: Uuid,
) -> Result<Vec<EntryRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, meeting_id, kind, payer_id, recipient_id, \
                amount_rubles, description, occurred_at, created_at \
         from entries where meeting_id = $1 \
         order by occurred_at desc, id",
    )
    .bind(meeting_id)
    .fetch_all(conn)
    .await
}

/// Доли всех записей встречи одним запросом: домену они нужны сразу все,
/// и запрос на каждую запись отдельно был бы N+1.
pub async fn shares_for_meeting(
    conn: &mut PgConnection,
    meeting_id: Uuid,
) -> Result<Vec<ShareRow>, sqlx::Error> {
    sqlx::query_as(
        "select s.entry_id, s.participant_id, s.weight_quarters \
         from entry_shares s join entries e on e.id = s.entry_id \
         where e.meeting_id = $1",
    )
    .bind(meeting_id)
    .fetch_all(conn)
    .await
}

pub async fn delete(conn: &mut PgConnection, id: Uuid) -> Result<bool, sqlx::Error> {
    let result = sqlx::query("delete from entries where id = $1")
        .bind(id)
        .execute(conn)
        .await?;

    Ok(result.rows_affected() > 0)
}
```

Дописать в `backend/src/db/mod.rs`:

```rust
pub mod entries;
```

- [ ] **Step 4: Прогнать тесты**

Run: `cd backend && cargo test --test persistence`

Ожидается: `test result: ok. 15 passed; 0 failed`.

- [ ] **Step 5: Закоммитить**

```bash
git add backend/src/db backend/tests/persistence.rs
git commit -m "feat(db): store entries with their partial shares"
```

---

### Task 9: Лог «Что менялось»

**Files:**
- Create: `backend/src/db/log.rs`
- Modify: `backend/src/db/mod.rs`
- Modify: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать падающий тест**

Дописать в `backend/tests/persistence.rs`:

```rust
use backend::db::log;

#[tokio::test]
async fn keeps_the_newest_log_records_within_the_limit() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    for index in 0..5 {
        log::append(&mut tx, meeting_id, &format!("событие {index}"))
            .await
            .expect("запись в лог");
    }

    let recent = log::recent(&mut tx, meeting_id, 3).await.expect("лог");

    // Новые сверху, лишние отброшены. Все пять событий получили одинаковый
    // created_at — now() даёт время начала транзакции, — поэтому порядок здесь
    // держится целиком на тай-брейке по id.
    assert_eq!(recent.len(), 3);
    assert_eq!(recent[0].text, "событие 4");
    assert_eq!(recent[1].text, "событие 3");
    assert_eq!(recent[2].text, "событие 2");
}
```

- [ ] **Step 2: Запустить и убедиться, что не компилируется**

Run: `cd backend && cargo test --test persistence`

Ожидается: ошибка компиляции — модуля `log` нет.

- [ ] **Step 3: Реализовать**

`backend/src/db/log.rs`:

```rust
use sqlx::PgConnection;
use uuid::Uuid;

use super::records::LogRow;

/// Пишется в той же транзакции, что и сама правка, — так лог не может
/// разойтись с данными.
pub async fn append(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    text: &str,
) -> Result<(), sqlx::Error> {
    sqlx::query("insert into meeting_log (meeting_id, text) values ($1, $2)")
        .bind(meeting_id)
        .bind(text)
        .execute(conn)
        .await?;

    Ok(())
}

/// Последние события, новые сверху. Второй ключ сортировки — `id`: события
/// одной транзакции получают одинаковый `now()`, и без него порядок был бы
/// произвольным.
pub async fn recent(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    limit: i64,
) -> Result<Vec<LogRow>, sqlx::Error> {
    sqlx::query_as(
        "select id, text, created_at from meeting_log where meeting_id = $1 \
         order by created_at desc, id desc limit $2",
    )
    .bind(meeting_id)
    .bind(limit)
    .fetch_all(conn)
    .await
}
```

Дописать в `backend/src/db/mod.rs`:

```rust
pub mod log;
```

- [ ] **Step 4: Прогнать тесты**

Run: `cd backend && cargo test --test persistence`

Ожидается: `test result: ok. 16 passed; 0 failed`.

- [ ] **Step 5: Закоммитить**

```bash
git add backend/src/db backend/tests/persistence.rs
git commit -m "feat(db): append and read the change log"
```

---

### Task 10: Сборка фактов для домена

Единственное место, где слой БД знает про доменные типы. Замыкает план: после него `reckon`
считает по реальным данным.

**Files:**
- Create: `backend/src/db/facts.rs`
- Modify: `backend/src/db/mod.rs`
- Modify: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать падающий тест**

Дописать в `backend/tests/persistence.rs`:

```rust
use backend::db::facts;
use backend::domain::{MeetingStatus, ParticipantId, reckon};

#[tokio::test]
async fn reckons_the_dacha_meeting_from_stored_rows() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let nastya = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("Настя");
    let vlad = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("Влад");
    let egor = participants::insert(&mut tx, meeting_id, "Егор", "🐸")
        .await
        .expect("Егор");
    let marina = participants::insert(&mut tx, meeting_id, "Марина", "🦩")
        .await
        .expect("Марина");

    for (payer, amount) in [(vlad.id, 8400), (nastya.id, 3200), (marina.id, 2100)] {
        entries::insert(
            &mut tx,
            meeting_id,
            NewEntry {
                kind: EntryKindRow::Expense,
                payer_id: payer,
                recipient_id: None,
                amount_rubles: amount,
                description: String::new(),
                occurred_at: None,
                shares: Vec::new(),
            },
        )
        .await
        .expect("расход");
    }

    let stored = facts::load(&mut tx, meeting_id)
        .await
        .expect("чтение фактов");
    let reckoning = reckon(stored.as_facts());

    // Те же числа, что в юнит-тестах домена и на скриншотах дизайна —
    // но приехавшие через Postgres.
    assert_eq!(reckoning.spent, 13700);
    assert_eq!(reckoning.per_person, 3425);
    assert_eq!(reckoning.net[&ParticipantId(nastya.id)], -225);
    assert_eq!(reckoning.net[&ParticipantId(vlad.id)], 4975);
    assert_eq!(reckoning.net[&ParticipantId(egor.id)], -3425);
    assert_eq!(reckoning.net[&ParticipantId(marina.id)], -1325);
    assert_eq!(reckoning.status, MeetingStatus::Alarm(3));
}

#[tokio::test]
async fn maps_partial_shares_into_the_domain() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Раз", "🐻")
        .await
        .expect("первый");
    let half = participants::insert(&mut tx, meeting_id, "Два", "🦊")
        .await
        .expect("второй");
    let excluded = participants::insert(&mut tx, meeting_id, "Три", "🐸")
        .await
        .expect("третий");

    // Веса 4, 2, 0 из 6: целые части долей 66 и 33, распределено 99, остаток
    // рубля уходит плательщику — у него дробная часть больше (4/6 против 2/6).
    // Итоговые доли 67 / 33 / 0.
    entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: vec![(half.id, 2), (excluded.id, 0)],
        },
    )
    .await
    .expect("расход с долями");

    let stored = facts::load(&mut tx, meeting_id)
        .await
        .expect("чтение фактов");
    let reckoning = reckon(stored.as_facts());

    // Заплатил 100, своя доля 67.
    assert_eq!(reckoning.net[&ParticipantId(payer.id)], 33);
    assert_eq!(reckoning.net[&ParticipantId(half.id)], -33);
    // Исключённый не платит ничего, в том числе не получает рубль остатка.
    assert_eq!(reckoning.net[&ParticipantId(excluded.id)], 0);
}
```

- [ ] **Step 2: Запустить и убедиться, что не компилируется**

Run: `cd backend && cargo test --test persistence`

Ожидается: ошибка компиляции — модуля `facts` нет.

- [ ] **Step 3: Реализовать**

`backend/src/db/facts.rs`:

```rust
//! Единственное место, где слой БД знает про доменные типы. Если отображение
//! поменяется, менять придётся только этот файл.

use std::collections::BTreeMap;

use sqlx::PgConnection;
use uuid::Uuid;

use super::records::{EntryKindRow, EntryRow, ParticipantRow, ShareRow};
use super::{entries, participants};
use crate::domain::{Entry, EntryKind, MeetingFacts, Participant, ParticipantId, Weight};

/// Владеющий аналог `MeetingFacts`: домен принимает срезы по ссылке, поэтому
/// кому-то надо владеть векторами.
#[derive(Debug, Clone)]
pub struct StoredFacts {
    participants: Vec<Participant>,
    entries: Vec<Entry>,
}

impl StoredFacts {
    pub fn as_facts(&self) -> MeetingFacts<'_> {
        MeetingFacts {
            participants: &self.participants,
            entries: &self.entries,
        }
    }
}

/// Читает участников, записи и доли встречи и складывает их в доменные типы.
/// Три запроса, а не N+1: доли берутся все сразу и раскладываются по записям
/// в памяти.
pub async fn load(
    conn: &mut PgConnection,
    meeting_id: Uuid,
) -> Result<StoredFacts, sqlx::Error> {
    let participant_rows = participants::list_for_meeting(&mut *conn, meeting_id).await?;
    let entry_rows = entries::list_for_meeting(&mut *conn, meeting_id).await?;
    let share_rows = entries::shares_for_meeting(&mut *conn, meeting_id).await?;

    Ok(build(&participant_rows, &entry_rows, &share_rows))
}

fn build(
    participant_rows: &[ParticipantRow],
    entry_rows: &[EntryRow],
    share_rows: &[ShareRow],
) -> StoredFacts {
    let participants = participant_rows
        .iter()
        .map(|row| Participant {
            id: ParticipantId(row.id),
            position: row.position,
        })
        .collect();

    let mut shares_by_entry: BTreeMap<Uuid, Vec<Weight>> = BTreeMap::new();
    for share in share_rows {
        shares_by_entry
            .entry(share.entry_id)
            .or_default()
            .push(Weight {
                participant_id: ParticipantId(share.participant_id),
                // Диапазон 0..=3 задан CHECK в схеме, поэтому преобразование
                // не может не сойтись. Берём `try_from`, а не `as`: если схема
                // и код однажды разойдутся, лучше упасть здесь, чем молча
                // завернуть значение и испортить расчёт долей.
                quarters: u8::try_from(share.weight_quarters)
                    .expect("weight_quarters вне диапазона 0..=3 — схема и код разошлись"),
            });
    }

    let entries = entry_rows
        .iter()
        .map(|row| Entry {
            kind: match row.kind {
                EntryKindRow::Expense => EntryKind::Expense,
                EntryKindRow::Transfer => EntryKind::Transfer,
            },
            payer_id: ParticipantId(row.payer_id),
            recipient_id: row.recipient_id.map(ParticipantId),
            amount: row.amount_rubles,
            weights: shares_by_entry.remove(&row.id).unwrap_or_default(),
        })
        .collect();

    StoredFacts {
        participants,
        entries,
    }
}
```

Дописать в `backend/src/db/mod.rs`:

```rust
pub mod facts;
```

- [ ] **Step 4: Прогнать тесты**

Run: `cd backend && cargo test --test persistence`

Ожидается: `test result: ok. 18 passed; 0 failed`.

Если тест про доли упал — не правьте ожидания, пока не пересчитаете руками. Веса 4, 2, 0 дают сумму
6; целые части `100×4/6 = 66` (остаток 4) и `100×2/6 = 33` (остаток 2); распределено 99; рубль
остатка уходит наибольшей дробной части, то есть плательщику. Доли 67 / 33 / 0, балансы
`+100 − 67 = 33`, `−33`, `0`. Если получились другие числа, ошибка в отображении строк в доменные
типы, а не в ожиданиях: скорее всего `weight_quarters` потерялся по дороге и расход поделился
поровну.

- [ ] **Step 5: Проверить линтером и закоммитить**

Run: `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

```bash
git add backend/src/db backend/tests/persistence.rs
git commit -m "feat(db): assemble domain facts from stored rows"
```

---

### Task 11: Поднять базу при старте сервера

**Files:**
- Modify: `backend/src/main.rs`

- [ ] **Step 1: Подключить базу к приложению**

Заменить начало `main` в `backend/src/main.rs`:

```rust
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
    // ... остальное без изменений
```

Обработку ошибок нельзя делать через `expect`, хотя так короче. `expect` печатает ошибку через
`Debug`, и человек увидел бы `MissingEnv("DATABASE_URL")` вместо написанного для него текста —
то есть все сообщения `StartupError` оказались бы бесполезны ровно там, где их читают. Проверено:
с `expect` вывод был `конфигурация базы: MissingEnv("DATABASE_URL")`. Поэтому рядом с `main`:

```rust
/// Печатает причину и выходит с ненулевым кодом.
fn or_exit<T>(result: Result<T, backend::db::StartupError>) -> T {
    match result {
        Ok(value) => value,
        Err(error) => {
            eprintln!("не удалось запустить сервер: {error}");
            std::process::exit(1);
        }
    }
}
```

Заглушка `/api/ws` удаляется вместе с обработчиками `ws_handler` и `handle_socket`: по спеке
realtime не делаем, а мёртвый эндпоинт вводит в заблуждение. Импорты `extract::ws::*` тоже уходят,
и вместе с ними — фича `ws` у axum в `Cargo.toml`: оставленная, она тянула бы в сборку
`tokio-tungstenite` ради того, чего в приложении нет.

Хендлер `health` теперь получает состояние, но пула не использует — сигнатуру не меняем.

- [ ] **Step 2: Проверить, что сервер поднимается и отвечает**

Run: `cd backend && cargo run`

В другом терминале:

Run: `curl http://127.0.0.1:3000/api/health`

Ожидается: `{"status":"ok"}`. В логе `cargo run` — строка `listening on 0.0.0.0:3000` и никаких
ошибок миграций (они уже применены к бранчу `dev`, повторный запуск ничего не делает).

Остановите сервер.

- [ ] **Step 3: Проверить, что без `DATABASE_URL` падение внятное**

Run (PowerShell):

```
cd backend; Rename-Item .env .env.off; cargo run 2>&1 | Select-String "DATABASE_URL"; Rename-Item .env.off .env
```

Ожидается: сообщение про незаданную переменную. Сервер, поднявшийся без базы, был бы хуже упавшего:
он отвечал бы 500 на каждый запрос.

- [ ] **Step 4: Проверить всё и закоммитить**

Run: `cd backend && cargo test --lib && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

```bash
git add backend/src/main.rs
git commit -m "feat(backend): connect to Postgres and drop the websocket stub"
```

---

### Task 12: Правка записи

Редактирование расхода и перевода — операция, которой нет в дизайне: мы добавили её к спеке, потому
что иначе опечатку в сумме приходится лечить удалением и повторным вводом, теряя разбивку по долям.

**Files:**
- Modify: `backend/src/db/entries.rs`
- Modify: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать падающий тест**

Дописать в `backend/tests/persistence.rs`:

```rust
use backend::db::entries::EntryPatch;

#[tokio::test]
async fn patches_amount_and_leaves_shares_alone() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");
    let other = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй");

    let entry = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 1000,
            description: "опечатка".to_owned(),
            occurred_at: None,
            shares: vec![(other.id, 2)],
        },
    )
    .await
    .expect("вставка");

    let patched = entries::update(
        &mut tx,
        entry.id,
        EntryPatch {
            amount_rubles: Some(1200),
            description: Some("Продукты".to_owned()),
            ..EntryPatch::default()
        },
    )
    .await
    .expect("правка")
    .expect("запись существует");

    assert_eq!(patched.amount_rubles, 1200);
    assert_eq!(patched.description, "Продукты");
    assert_eq!(patched.payer_id, payer.id);

    // `shares: None` означает «не трогать» — разбивка по долям цела.
    let shares = entries::shares_for_meeting(&mut tx, meeting_id)
        .await
        .expect("доли");
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].weight_quarters, 2);
}

#[tokio::test]
async fn replaces_shares_wholesale_when_given() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");
    let second = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй");
    let third = participants::insert(&mut tx, meeting_id, "Егор", "🐸")
        .await
        .expect("третий");

    let entry = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 900,
            description: String::new(),
            occurred_at: None,
            shares: vec![(second.id, 2)],
        },
    )
    .await
    .expect("вставка");

    entries::update(
        &mut tx,
        entry.id,
        EntryPatch {
            shares: Some(vec![(third.id, 0)]),
            ..EntryPatch::default()
        },
    )
    .await
    .expect("правка")
    .expect("запись существует");

    // Прежняя доля второго удалена, а не дополнена новой.
    let shares = entries::shares_for_meeting(&mut tx, meeting_id)
        .await
        .expect("доли");
    assert_eq!(shares.len(), 1);
    assert_eq!(shares[0].participant_id, third.id);
    assert_eq!(shares[0].weight_quarters, 0);
}

#[tokio::test]
async fn empty_share_list_means_split_equally_again() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");
    let other = participants::insert(&mut tx, meeting_id, "Влад", "🦉")
        .await
        .expect("второй");

    let entry = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 500,
            description: String::new(),
            occurred_at: None,
            shares: vec![(other.id, 1)],
        },
    )
    .await
    .expect("вставка");

    entries::update(
        &mut tx,
        entry.id,
        EntryPatch {
            shares: Some(Vec::new()),
            ..EntryPatch::default()
        },
    )
    .await
    .expect("правка")
    .expect("запись существует");

    // Пустой список — не то же самое, что `None`: он снимает все неполные доли,
    // то есть возвращает расход к делению поровну.
    let shares = entries::shares_for_meeting(&mut tx, meeting_id)
        .await
        .expect("доли");
    assert!(shares.is_empty());
}
```

- [ ] **Step 2: Запустить и убедиться, что не компилируется**

Run: `cd backend && cargo test --test persistence`

Ожидается: ошибка компиляции — нет `EntryPatch` и `entries::update`.

- [ ] **Step 3: Реализовать**

Дописать в `backend/src/db/entries.rs`:

```rust
/// Частичная правка записи. `None` в поле означает «не менять».
///
/// `kind` менять нельзя: превращение расхода в перевод — это другая запись,
/// и в интерфейсе это делается удалением и повторным вводом. Согласованность
/// `kind` и `recipient_id` при правке получателя обеспечивает слой API, а
/// последней линией — CHECK в схеме.
#[derive(Debug, Clone, Default)]
pub struct EntryPatch {
    pub payer_id: Option<Uuid>,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: Option<i64>,
    pub description: Option<String>,
    pub occurred_at: Option<DateTime<Utc>>,
    /// `None` — доли не трогать. `Some(vec![])` — снять все неполные доли,
    /// то есть вернуть расход к делению поровну. Переданный список заменяет
    /// прежний целиком, а не дополняет его.
    pub shares: Option<Vec<(Uuid, i16)>>,
}

pub async fn update(
    conn: &mut PgConnection,
    id: Uuid,
    patch: EntryPatch,
) -> Result<Option<EntryRow>, sqlx::Error> {
    let row: Option<EntryRow> = sqlx::query_as(&format!(
        "update entries set \
             payer_id = coalesce($2, payer_id), \
             recipient_id = coalesce($3, recipient_id), \
             amount_rubles = coalesce($4, amount_rubles), \
             description = coalesce($5, description), \
             occurred_at = coalesce($6, occurred_at) \
         where id = $1 returning {COLUMNS}"
    ))
    .bind(id)
    .bind(patch.payer_id)
    .bind(patch.recipient_id)
    .bind(patch.amount_rubles)
    .bind(patch.description)
    .bind(patch.occurred_at)
    .fetch_optional(&mut *conn)
    .await?;

    let Some(row) = row else {
        return Ok(None);
    };

    if let Some(shares) = patch.shares {
        sqlx::query("delete from entry_shares where entry_id = $1")
            .bind(row.id)
            .execute(&mut *conn)
            .await?;

        for (participant_id, weight_quarters) in shares {
            sqlx::query(
                "insert into entry_shares (entry_id, participant_id, weight_quarters) \
                 values ($1, $2, $3)",
            )
            .bind(row.id)
            .bind(participant_id)
            .bind(weight_quarters)
            .execute(&mut *conn)
            .await?;
        }
    }

    Ok(Some(row))
}
```

Правка долей — это удаление и вставка заново, поэтому вызывающий обязан передать транзакцию: между
двумя запросами запись на мгновение остаётся вообще без долей, и чтение в этот момент посчитало бы
расход поделённым поровну.

- [ ] **Step 4: Прогнать тесты**

Run: `cd backend && cargo test --test persistence`

Ожидается: `test result: ok. 19 passed; 0 failed`.

- [ ] **Step 5: Закоммитить**

```bash
git add backend/src/db/entries.rs backend/tests/persistence.rs
git commit -m "feat(db): patch entries and replace their shares"
```

---

### Task 13: Список встреч с фильтрами

Поиск и фильтр по участнику — на стороне базы. Сортировка остаётся на Rust: два режима из четырёх
(`total-desc` и `open-first`) зависят от посчитанных значений, которых в SQL нет.

**Files:**
- Modify: `backend/src/db/meetings.rs`
- Modify: `backend/tests/persistence.rs`

- [ ] **Step 1: Написать падающий тест**

Дописать в `backend/tests/persistence.rs`:

```rust
#[tokio::test]
async fn filters_meetings_by_query_and_participant() {
    let pool = test_pool().await;
    let mut tx = pool.begin().await.expect("транзакция");

    let bbq = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "Солевые шашлыки".to_owned(),
            description: "Выехали на озеро с мангалом".to_owned(),
            emoji: "🔥".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 8, 3).expect("дата"),
        },
    )
    .await
    .expect("шашлыки");

    let dacha = meetings::insert(
        &mut tx,
        NewMeeting {
            title: "Дача у Влада".to_owned(),
            description: "Баня и продукты".to_owned(),
            emoji: "🏡".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 7, 23).expect("дата"),
        },
    )
    .await
    .expect("дача");

    participants::insert(&mut tx, bbq.id, "Настя", "🦊")
        .await
        .expect("участник шашлыков");
    participants::insert(&mut tx, dacha.id, "Влад", "🦉")
        .await
        .expect("участник дачи");

    // Без фильтров — обе, новые сверху по дате встречи.
    let all = meetings::list_filtered(&mut tx, None, None)
        .await
        .expect("список");
    let titles: Vec<&str> = all.iter().map(|row| row.title.as_str()).collect();
    assert_eq!(titles, ["Солевые шашлыки", "Дача у Влада"]);

    // Поиск идёт и по названию, и по описанию, регистр не важен.
    let by_title = meetings::list_filtered(&mut tx, Some("ШАШЛЫК"), None)
        .await
        .expect("поиск по названию");
    assert_eq!(by_title.len(), 1);
    assert_eq!(by_title[0].id, bbq.id);

    let by_description = meetings::list_filtered(&mut tx, Some("баня"), None)
        .await
        .expect("поиск по описанию");
    assert_eq!(by_description.len(), 1);
    assert_eq!(by_description[0].id, dacha.id);

    // Фильтр по участнику: встреча попадает в выдачу, если такой участник в ней есть.
    let by_participant = meetings::list_filtered(&mut tx, None, Some("Влад"))
        .await
        .expect("фильтр по участнику");
    assert_eq!(by_participant.len(), 1);
    assert_eq!(by_participant[0].id, dacha.id);

    // Фильтры складываются, а не заменяют друг друга.
    let both = meetings::list_filtered(&mut tx, Some("шашлык"), Some("Влад"))
        .await
        .expect("оба фильтра");
    assert!(both.is_empty());
}
```

- [ ] **Step 2: Запустить и убедиться, что не компилируется**

Run: `cd backend && cargo test --test persistence`

Ожидается: ошибка компиляции — нет `list_filtered`.

- [ ] **Step 3: Реализовать**

Дописать в `backend/src/db/meetings.rs`:

```rust
/// Список встреч с фильтрами. Порядок по умолчанию — по дате встречи, новые
/// сверху; остальные три режима сортировки применяет слой API, потому что
/// `total-desc` и `open-first` зависят от посчитанных значений.
///
/// Приведение `$1::text` обязательно: без него Postgres не может определить
/// тип параметра, когда тот равен NULL.
pub async fn list_filtered(
    conn: &mut PgConnection,
    query: Option<&str>,
    participant: Option<&str>,
) -> Result<Vec<MeetingRow>, sqlx::Error> {
    sqlx::query_as(&format!(
        "select {COLUMNS} from meetings m \
         where ($1::text is null \
                or m.title ilike '%' || $1 || '%' \
                or m.description ilike '%' || $1 || '%') \
           and ($2::text is null or exists ( \
                 select 1 from participants p \
                 where p.meeting_id = m.id and p.name = $2)) \
         order by m.held_on desc, m.created_at desc, m.id"
    ))
    .bind(query)
    .bind(participant)
    .fetch_all(conn)
    .await
}
```

Третий ключ сортировки `m.id` — та же причина, что и в остальных запросах: две встречи с одинаковой
датой и одинаковым `created_at` без него шли бы в произвольном порядке.

- [ ] **Step 4: Прогнать тесты**

Run: `cd backend && cargo test --test persistence`

Ожидается: `test result: ok. 20 passed; 0 failed`.

- [ ] **Step 5: Проверить линтером и закоммитить**

Run: `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo fmt --check`

```bash
git add backend/src/db/meetings.rs backend/tests/persistence.rs
git commit -m "feat(db): filter the meetings list by query and participant"
```

---

## Проверка по завершении плана

- [ ] `cd backend && cargo test --lib` — юнит-тесты домена зелёные (38 штук), база не нужна
- [ ] `cd backend && cargo test` — плюс 20 интеграционных тестов против бранча `test`
- [ ] `cd backend && cargo clippy --all-targets -- -D warnings` — без предупреждений
- [ ] `cd backend && cargo fmt --check` — без расхождений
- [ ] `cd backend && cargo run` и `curl http://127.0.0.1:3000/api/health` → `{"status":"ok"}`
- [ ] `git ls-files backend/.env` не выводит ничего — строка подключения не в git
- [ ] В бранче `test` в консоли Neon видны пять таблиц, и они пустые: транзакции откатились

## Что дальше

План 3 — ручки API поверх этого слоя и `reckon`: сериализация в JSON с `camelCase`, валидация
с 422, маппинг ошибок, фильтры и сортировка списка встреч, тексты лога. План 4 — обложки:
загрузка, ограничение размера, отдача с ETag.

Домен и слой БД к тому моменту меняться не должны. Если при написании ручки выясняется, что данных
не хватает, это повод вернуться к спеке, а не дописать запрос в хендлер.
