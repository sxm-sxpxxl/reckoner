# Сборка в Docker, а не в нативном окружении Render: проект на edition 2024,
# которой нужен Rust >= 1.85. Версию нативного образа мы не контролируем, и если
# она окажется старее — сборка упадёт с невнятной ошибкой про edition. Здесь
# версия написана явно.
FROM rust:1.97-slim-bookworm AS builder

WORKDIR /app

# Сначала только манифесты и заглушки вместо кода. Слой с зависимостями
# переиспользуется, пока не изменился Cargo.toml, — на бесплатном Render это
# разница между тремя минутами сборки и десятью.
COPY Cargo.toml Cargo.lock ./
RUN mkdir src \
    && echo 'fn main() {}' > src/main.rs \
    && echo '' > src/lib.rs \
    && cargo build --release --locked \
    && rm -rf src

COPY src ./src
COPY migrations ./migrations

# `touch` обязателен: заглушки оставили артефакты с более свежим временем, чем
# у распакованных файлов, и cargo решил бы, что пересобирать нечего, — в образ
# уехал бы пустой `fn main() {}`.
RUN touch src/main.rs src/lib.rs && cargo build --release --locked

FROM debian:bookworm-slim

# Сертификаты нужны для TLS до Neon: без них соединение падает на проверке
# цепочки, и сообщение об этом ничего не объясняет.
RUN apt-get update \
    && apt-get install -y --no-install-recommends ca-certificates \
    && rm -rf /var/lib/apt/lists/*

COPY --from=builder /app/target/release/backend /usr/local/bin/backend

# Порт слушается из переменной `PORT`, которую даёт Render; `EXPOSE` здесь
# только документирует локальный запуск.
EXPOSE 3000

CMD ["backend"]
