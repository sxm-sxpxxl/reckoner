# Reckoner

Who owes whom, and how much. A shared-expense tracker for get-togethers with friends: you add
what everyone paid, and the app works out the smallest set of transfers that settles everyone up.

A React + TypeScript PWA (installable on Android, iOS and desktop, no native wrappers) with a
Rust/Axum backend and Postgres.

The interface is in Russian and the app is called «Финальная расплата»; this README is in English.
`reckoner` stayed as the repository and crate name.

## Status

Feature-complete. Everything described below is built and covered by tests.

The backend runs on Render and answers at
[`/api/health`](https://reckoner-api.onrender.com/api/health); the frontend publishes to GitHub
Pages from `master`. See [Deployment](#deployment) for how the pieces are wired together.

## What it does

Each participant can spend money on the meetup, once or several times. That raises the total and
is split across everyone present. From the totals the app derives, for every participant who has
underpaid, the list of transfers they need to make to come out even — recomputed on every change.

- No sign-in. Anyone who opens the site sees every meetup, and can search, filter by participant
  and sort the list. A colour tells you at a glance whether a meetup is settled: green for
  nothing outstanding, yellow for one or two transfers left, red for three or more.
- Inside a meetup: cover image, title, description, participants, and the history of operations
  sorted by time. Each entry records who paid, how much, what for, and — for a transfer — who
  received it.
- Expenses can be split exactly: leave a participant's field empty to share the rest evenly,
  type their amount — several dishes add up right in the field, `390 + 1200 + 624` — or switch
  them off. If the typed amounts miss the bill by up to a quarter (tips, a discount, a slip), the
  difference is spread in proportion.
- Within a meetup one participant can pay for others, a couple for instance: their balances fold
  into the payer's, and only the payer shows up in the transfers.
- Below the history, the outstanding transfers needed to close the meetup at par, in three
  views: a per-debtor list, a debtor/creditor matrix, and a balance bar chart.
- All amounts are whole roubles. Splitting is integer arithmetic with the remainder handed out by
  largest fractional part, so the shares of an expense always add up to it exactly, balances
  always sum to zero, and the transfer plan always closes a meetup at par.
- Cover photos are resized in the browser before upload and stored in Postgres; the server checks
  the type by magic bytes rather than the `Content-Type` header.
- Light and dark themes, chosen per visitor and kept in the browser.

## Stack

- **`frontend/`** — Vite, React, TypeScript, TanStack Query, CSS Modules. PWA via
  `vite-plugin-pwa`. Deployed to GitHub Pages.
- **`backend/`** — Rust, Axum, Postgres via `sqlx`. Runs as a Docker image on Render, database on
  Neon.

The backend keeps only facts in the database. Balances, the transfer plan and meetup status are
never stored: they are computed on read by a pure `domain` module that knows nothing about SQL or
HTTP, which keeps the money logic in one place and testable without any infrastructure.

## Repository layout

```
backend/                  Rust + Axum API, Dockerfile, migrations
frontend/                 React + TypeScript PWA
render.yaml               Render service definition
docs/design/              design handoff: tokens, screens, prototype
docs/superpowers/specs/   implementation spec
docs/superpowers/plans/   implementation plans, one per stage
docs/setup-neon.md        setting up the database
docs/setup-deploy.md      going live
```

## Getting started

Prerequisites: [Rust](https://rustup.rs) 1.85 or newer (the crate uses edition 2024) and
[Node.js](https://nodejs.org) LTS.

The backend needs a Postgres database. There is no local one — development and tests both use
branches of a free Neon project; [`docs/setup-neon.md`](docs/setup-neon.md) walks through it and
ends with a `backend/.env` holding `DATABASE_URL` and `TEST_DATABASE_URL`.
[`backend/.env.example`](backend/.env.example) documents the format.

Two terminals:

```bash
cd backend && cargo run
```

```bash
cd frontend && npm install && npm run dev
```

Vite prints the dev URL, usually `http://localhost:5173`. It proxies `/api/*` to
`http://localhost:3000`, where the backend listens, so there are no CORS problems in development.

### Tests

```bash
cd backend && cargo test --lib     # domain and API layer, no database
cd backend && cargo test           # everything, needs TEST_DATABASE_URL
cd frontend && npm test
```

The integration tests each run inside a transaction that is rolled back, so they leave nothing
behind. They talk to a real Neon branch, and on its free compute they occasionally fail with
`PoolTimedOut` or a dropped TLS connection — that is the database under load, not a regression.
Re-running usually passes. CI deliberately runs only `cargo test --lib` for this reason.

### Checking the PWA locally

```bash
cd frontend && npm run build && npm run preview
```

In Chrome or Edge an install button should appear in the address bar, which means the manifest
and service worker are wired up correctly.

## Deployment

Three free tiers, wired together by three variables:

| Piece | Where | Variable it needs |
| --- | --- | --- |
| Frontend | GitHub Pages | `VITE_API_BASE_URL` — repository variable |
| Backend | Render | `DATABASE_URL`, `ALLOWED_ORIGIN` — service environment |
| Database | Neon, branch `production` | — |

Pushing to `master` rebuilds both: Pages every time, Render only when `backend/` or `render.yaml`
changed. [`docs/setup-deploy.md`](docs/setup-deploy.md) covers the manual half — creating the
service, enabling Pages, and the order the two addresses have to be filled in, since each side
needs the other's.

Render's free tier sleeps after 15 minutes idle and takes 30–60 seconds to wake. The app expects
that: it polls health on start and shows a "waking the server" screen rather than an error.

## Documentation

- [`docs/superpowers/specs/2026-08-04-reckoner-design.md`](docs/superpowers/specs/2026-08-04-reckoner-design.md)
  — the implementation spec: data model, REST API, settlement algorithm, deviations from the
  handoff. Start here.
- [`docs/design/handoff.md`](docs/design/handoff.md) — design handoff: colours, typography,
  spacing, per-screen behaviour and exact copy.
- [`docs/design/meetup-splitter.dc.html`](docs/design/meetup-splitter.dc.html) — HTML prototype
  with the calculations actually working. A behavioural reference; open it in a browser, do not
  port it into the codebase.
- [`docs/design/screens/`](docs/design/screens/) — screenshots of the original design.

The handoff and those screenshots show the original warm, light palette. The app shipped with a
dark one and the phrase-strewn background that came with the rename; the geometry, typography and
copy are still the handoff's. The spec's "Отклонения от хендоффа" table lists every departure and
why it was made.
