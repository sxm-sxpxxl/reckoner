# reckoner

Монорепозиторий: фронтенд на React + TypeScript (PWA, устанавливается на
Android/iOS/Desktop без нативных обёрток) и бэкенд на Rust (Axum).

## Стек

- **frontend/** — Vite + React + TypeScript, PWA через `vite-plugin-pwa`
  (манифест, service worker, установка на устройство).
- **backend/** — Rust + Axum: `GET /api/health`, `GET /api/ws`
  (WebSocket-эхо как заготовка под realtime), CORS открыт для разработки.

## Запуск в разработке

Два терминала:

```
# терминал 1 — backend
cd backend
cargo run
```

```
# терминал 2 — frontend
cd frontend
npm install
npm run dev
```

Фронтенд поднимется на своём dev-порту (см. вывод Vite, обычно 5173) и
проксирует запросы `/api/*` на `http://localhost:3000`, где слушает backend.
Откройте страницу в браузере — она должна показать `ok` и ответ
`GET /api/health`.

## Проверка PWA-установки локально

```
cd frontend
npm run build
npm run preview
```

Откройте адрес, который выведет `npm run preview` (обычно
`http://localhost:4173`). В адресной строке браузера (Chrome/Edge) должна
появиться иконка установки приложения — значит манифест и service worker
подключены корректно. Иконки `pwa-192x192.png` / `pwa-512x512.png` сейчас —
временные плейсхолдеры (залитые одним цветом), замените на реальные перед
релизом.

## Известные нюансы

- Версии Rust-зависимостей (`axum = "0.7"`, `tower-http = "0.5"`) закреплены
  осознанно ниже самых новых — так они совместимы с более старыми
  тулчейнами Rust (проверено от Rust 1.75). Если на машине уже стоит свежий
  Rust и хочется актуальных версий — можно поднять до `axum = "0.8"` и
  `tower-http = "0.7"` (`cargo add axum@0.8 tower-http@0.7 --features
  ws,macros / cors`, затем `cargo check`).
- Если `cargo check` падает с ошибкой вида `requires rustc 1.78 or newer` —
  обновите Rust (`rustup update`) или дополнительно понизьте версии в
  `Cargo.toml`.
- CORS сейчас открыт (`CorsLayer::new().allow_origin(Any)`) — это ок для
  разработки, перед деплоем нужно сузить до конкретных origin.

## Дальше

Открыть папку проекта в Rider/RustRover (или другой JetBrains IDE) и
запустить `claude` в терминале — дальше реальные иконки, авторизация,
деплой backend и логика приложения.
