# Deployment Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Приложение живёт в интернете: фронтенд на GitHub Pages, бэкенд на Render, база — бранч `main` в Neon. Деплой автоматический с `master`.

**Architecture:** Три части на трёх бесплатных хостингах, связанные тремя переменными: `VITE_API_BASE_URL` (фронт знает адрес API), `ALLOWED_ORIGIN` (бэкенд знает, кого пускать), `DATABASE_URL` (бэкенд знает базу). Ошибка в любой из них ломает связь целиком, поэтому каждая проверяется отдельно.

**Tech Stack:** Docker (Rust 1.97), Render, GitHub Actions, GitHub Pages.

---

## Решения, принятые до начала

**Бэкенд собирается в Docker, а не нативным окружением Render.** Проект на edition 2024,
которой нужен Rust ≥ 1.85. Версию в нативном образе Render мы не контролируем, и если она
окажется старее — сборка упадёт с невнятной ошибкой про edition. В Dockerfile версия написана
явно.

**Многослойная сборка.** Сначала собираются зависимости по одному `Cargo.toml`, потом код.
Правка исходников не пересобирает все зависимости — на бесплатном Render, где сборка и так
идёт минутами, это разница между тремя минутами и десятью.

**Образ рантайма — `debian:bookworm-slim`, не `scratch`.** Нужны сертификаты для TLS до Neon;
со `scratch` пришлось бы копировать их руками, а выигрыш в полсотни мегабайт здесь никого
не волнует.

**`ALLOWED_ORIGIN` задана — обязана разбираться.** Если переменная есть, но кривая, сервер
не поднимается. Молча вернуться к открытому CORS — худший исход: прод остался бы нараспашку,
а в логе была бы строчка, которую никто не читает.

**Миграции применяются на старте приложения.** Отдельного шага в деплое нет: инстанс один,
`sqlx::migrate!` уже вызывается в `main`. Второй инстанс потребовал бы блокировки — его нет.

**Что делает пользователь руками** (Task 6): создаёт сервис на Render, кладёт туда две
переменные, включает Pages и заводит одну repo variable. Ключи и пароли через меня не проходят.

---

### Task 1: Порт и CORS из окружения

**Files:**
- Modify: `backend/src/main.rs`
- Modify: `backend/.env.example`

- [ ] **Step 1: Слушать порт, который дал Render**

Render передаёт порт в `PORT` и считает сервис живым, только если тот на нём отвечает.
Захардкоженный 3000 означал бы бесконечный «deploy in progress».

```rust
/// Render передаёт порт в `PORT`; локально его нет, и тогда 3000 — тот же,
/// что ждёт дев-прокси Vite.
fn port() -> u16 {
    match std::env::var("PORT") {
        Ok(value) => value.parse().unwrap_or_else(|_| {
            eprintln!("не удалось запустить сервер: PORT не число: {value}");
            std::process::exit(1);
        }),
        Err(_) => 3000,
    }
}
```

- [ ] **Step 2: Сузить CORS**

Функция `cors_layer()` из отложенной задачи плана 3: при заданной `ALLOWED_ORIGIN` пускаем
только её, при кривой — выходим с ошибкой, без переменной — открыто и с предупреждением в лог.

- [ ] **Step 3: Проверить локально**

Run: `cd backend && cargo run`
Expected: предупреждение про незаданную `ALLOWED_ORIGIN`, затем `listening on 0.0.0.0:3000`.

Затем с переменной: `ALLOWED_ORIGIN=https://example.com cargo run` — предупреждения нет.
И с кривой: `ALLOWED_ORIGIN=не-адрес cargo run` — сервер не поднимается.

- [ ] **Step 4: Коммит**

---

### Task 2: Dockerfile

**Files:**
- Create: `backend/Dockerfile`, `backend/.dockerignore`

- [ ] **Step 1: Многослойная сборка**

```dockerfile
FROM rust:1.97-slim-bookworm AS builder
WORKDIR /app

# Сначала только манифесты и пустышка вместо кода: так слой с зависимостями
# переиспользуется, пока не изменился Cargo.toml.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src && echo 'fn main() {}' > src/main.rs && echo '' > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
COPY migrations ./migrations
# Пустышка оставила свои артефакты — трогаем файлы, иначе cargo решит,
# что пересобирать нечего, и в образ уедет заглушка.
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

FROM debian:bookworm-slim
# Сертификаты нужны для TLS до Neon: без них соединение падает
# с невнятной ошибкой проверки цепочки.
RUN apt-get update && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*
COPY --from=builder /app/target/release/backend /usr/local/bin/backend
CMD ["backend"]
```

`.dockerignore`: `target`, `.env`, `tests`.

- [ ] **Step 2: Проверить сборку**

Docker локально не установлен, поэтому проверка — на Render при первом деплое. Ошибку сборки
там видно в логе целиком.

- [ ] **Step 3: Коммит**

---

### Task 3: `render.yaml`

**Files:**
- Create: `render.yaml`

```yaml
services:
  - type: web
    name: reckoner-api
    runtime: docker
    plan: free
    # Регион обязан совпадать с регионом Neon: один запрос к API делает
    # несколько обращений к базе, и задержка между ними умножается.
    region: frankfurt
    dockerfilePath: ./backend/Dockerfile
    dockerContext: ./backend
    healthCheckPath: /api/health
    autoDeploy: true
    envVars:
      # sync: false — значение вводится в панели и в репозиторий не попадает.
      - key: DATABASE_URL
        sync: false
      - key: ALLOWED_ORIGIN
        sync: false
```

---

### Task 4: CI

**Files:**
- Create: `.github/workflows/ci.yml`

Проверки те же, что гоняются локально. Интеграционные тесты в CI **не** запускаются: им нужен
бранч `test` в Neon, а класть его строку в секреты репозитория ради этого — лишний риск
и лишняя связность. Поэтому `cargo test --lib` (домен и слой API) плюс сборка и линтеры фронта.

---

### Task 5: Публикация фронтенда

**Files:**
- Create: `.github/workflows/pages.yml`

Сборка с `VITE_API_BASE_URL` из repo variable, `dist/index.html` → `dist/404.html`,
публикация через `actions/deploy-pages`.

Про `404.html`: Pages отдаёт статику и про роуты SPA не знает. Прямой заход на
`/reckoner/meetings/<id>` без этого файла вернёт 404 — то есть любая ссылка на встречу,
кинутая в чат, не откроется.

---

### Task 6: Что делает пользователь

Инструкция в `docs/setup-deploy.md`: бранч `main` в Neon, сервис на Render, включение Pages,
repo variable, порядок первого запуска (сначала Render — узнать адрес API, потом Pages
с этим адресом, потом `ALLOWED_ORIGIN` с адресом Pages).

---

### Task 7: Проверка на проде

Открыть сайт, создать встречу, добавить участника и расход, проверить обложку, перезагрузить
страницу встречи по прямой ссылке, посмотреть заголовки CORS.

## Что дальше

Проект закончен. Останутся мелочи по вкусу: свой домен, иконка в Pages, аналитика.
