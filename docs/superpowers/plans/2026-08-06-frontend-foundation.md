# Frontend Foundation and Meetings List Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Довести фронтенд до работающего списка встреч на живых данных: токены дизайна, шрифты, клиент API, роутинг, шапка и экран списка с фильтрами, сортировкой и созданием встречи.

**Architecture:** Тонкий клиент. Он не считает деньги — балансы, план переводов и статус приходят посчитанными из `GET /api/meetings`. Задача фронта: отрисовать, отфильтровать через сервер и отправить мутацию. Единственная арифметика на клиенте — живое превью долей в модалке расхода, и это план 5.

**Tech Stack:** React 19, TypeScript 6, Vite 8, react-router-dom 7, TanStack Query 5, CSS Modules, @fontsource, Vitest.

---

## Предварительные условия

1. Бэкенд в `master`: 11 ручек, 125 тестов зелёные.
2. `backend/.env` с `DATABASE_URL` (бранч `dev`). Бэкенд поднимается `cd backend && cargo run`.
3. В бранче `dev` уже лежат две демо-встречи («Дача у Влада», «Кино в пятницу») — список будет не пустым с первого запуска.
4. Хендофф: [`docs/design/handoff.md`](../../design/handoff.md), прототип
   [`meetup-splitter.dc.html`](../../design/meetup-splitter.dc.html), скриншот
   [`screens/01-meetings-list.png`](../../design/screens/01-meetings-list.png).

## Решения, принятые до начала

**Тестируем то, где есть логика.** Спека прямо говорит: «Тяжёлых компонентных тестов не будет —
ценность в денежной математике, а она в Rust». Поэтому Vitest покрывает `format.ts`
(и в плане 5 — `sharePreview.ts`), а компоненты проверяются сборкой (`tsc`), линтером и глазами
по скриншоту. Это осознанный отказ, а не экономия: тест, проверяющий, что кнопка отрендерила
слово «Создать», ломается от любой правки вёрстки и не ловит ничего.

**Верификация — в браузере, а не «должно работать».** У каждой видимой задачи последний шаг —
поднять дев-сервер и посмотреть. Бэкенд при этом должен быть запущен: список ходит в реальный API.

**Градиент обложки — от идентификатора встречи, а не от позиции в списке.** Хендофф говорит
«по индексу карточки», но тогда при смене сортировки у встречи меняется картинка, и список
«моргает» на ровном месте. Берём стабильный хеш от `id`. Отступление осознанное, вносится
в раздел отступлений спеки.

**Цвет статуса — в одном месте.** Статус приходит с сервера строкой (`settled` / `attention` /
`alarm` / `no-participants`) и влияет на рамку карточки, бейдж и число в статкарточке. Соответствие
статуса цвету живёт в `statusTone.ts` и нигде не дублируется.

**Ошибку показываем, а не проглатываем.** Наш API отвечает единой формой
(`{ error, field?, message? }`). Клиент разбирает её в типизированный `ApiError`; необработанный
отказ запроса виден в интерфейсе, а не только в консоли.

**`base: '/reckoner/'` включается сразу.** GitHub Pages отдаёт сайт из подпапки, и если поставить
базовый путь в конце, придётся переписывать все ссылки и пути к ассетам. Дев-сервер после этого
живёт на `http://127.0.0.1:5173/reckoner/` — это нормально и совпадает с продом.

## Структура файлов

```
frontend/src/
  api/          client.ts, types.ts, meetings.ts
  domain/       format.ts, format.test.ts, statusTone.ts, cover.ts
  routes/       MeetingsListPage.tsx
  components/
    layout/     AppHeader.tsx, Container.tsx, WakeScreen.tsx
    meetings/   MeetingCard.tsx, MeetingFilters.tsx, MeetingsEmpty.tsx, StatusBadge.tsx
    modals/     Modal.tsx, MeetingFormModal.tsx
    ui/         Button.tsx, Input.tsx, Select.tsx, Avatar.tsx, AvatarStack.tsx, Money.tsx
  styles/       tokens.css, base.css
  App.tsx, main.tsx
```

Каждый компонент — с co-located `.module.css`.

---

### Task 1: Зависимости, шрифты, токены и базовые стили

**Files:**
- Modify: `frontend/package.json`, `frontend/vite.config.ts`, `frontend/src/main.tsx`
- Create: `frontend/src/styles/tokens.css`, `frontend/src/styles/base.css`
- Delete: `frontend/src/App.css`, `frontend/src/index.css`

- [ ] **Step 1: Поставить зависимости**

```bash
cd frontend && npm install react-router-dom @tanstack/react-query @fontsource/onest @fontsource/unbounded @fontsource/jetbrains-mono
```

```bash
cd frontend && npm install -D vitest jsdom
```

`@fontsource` вместо Google Fonts: это PWA, она должна отрисовывать текст офлайн и не ходить
за шрифтом на сторонний домен.

- [ ] **Step 2: Написать токены**

Создать `frontend/src/styles/tokens.css` — прямой перенос таблицы токенов из хендоффа. Значения
не «примерно такие», а ровно те: вся сверка со скриншотами держится на этом файле.

```css
/* Токены дизайна. Перенесены один-в-один из docs/design/handoff.md, раздел
   «Design Tokens». Менять значения здесь — значит расходиться с хендоффом,
   поэтому любое отличие должно быть сначала внесено туда. */

:root {
  /* Поверхности */
  --bg: #f2eee5;
  --surface: #ffffff;
  --surface-2: #fbf9f4;

  /* Текст */
  --ink: #16150f;
  --text-2: #6c6759;
  --text-3: #8b8577;
  --text-muted: #a9a395;
  --text-description: #4b4739;

  /* Линии */
  --divider: rgba(22, 21, 15, 0.07);
  --border: rgba(22, 21, 15, 0.1);
  --border-strong: rgba(22, 21, 15, 0.16);

  /* Акцент */
  --accent: #e04a22;
  --accent-hover: #c93f1b;
  --accent-shadow: #a8320f;
  --link: #d6431b;

  /* Статусы */
  --ok-text: #25693e;
  --ok-bg: #dff0e3;
  --ok-panel: #eaf6ec;
  --ok-border: #9bc9a6;
  --ok-accent: #2e7d4f;
  --warn-text: #8a6403;
  --warn-bg: #fbefc9;
  --warn-border: #e0c067;
  --alarm-text: #a6301d;
  --alarm-bg: #fbded6;
  --alarm-border: #e09e8e;

  /* Долги */
  --debt: #c3402e;
  --debt-cell-bg: #fcf1ee;
  --cell-empty: #d6d0c2;

  /* Чипы */
  --chip-partial-bg: #fff3e4;
  --chip-partial-border: #e8a25e;
  --chip-picked-bg: #ffe3d6;
  --chip-picked-border: #e04a22;

  /* Радиусы */
  --r-pill: 999px;
  --r-modal: 22px;
  --r-card: 20px;
  --r-panel: 18px;
  --r-tile: 16px;
  --r-stat: 14px;
  --r-input: 12px;
  --r-logo: 11px;

  /* Тени */
  --shadow-card: 0 1px 2px rgba(22, 21, 15, 0.05);
  --shadow-card-hover: 0 10px 26px rgba(22, 21, 15, 0.1);
  --shadow-modal: 0 24px 60px rgba(22, 21, 15, 0.3);
  --shadow-primary: 0 2px 0 var(--accent-shadow);

  /* Шрифты */
  --font-ui: 'Onest', system-ui, sans-serif;
  --font-display: 'Unbounded', var(--font-ui);
  --font-mono: 'JetBrains Mono', ui-monospace, monospace;

  /* Раскладка */
  --container: 1180px;
  --container-pad: 28px;
}

/* Палитра аватаров: цвет кружка по `colorIndex`, который назначает база.
   Восемь значений — ровно столько, сколько PALETTE_SIZE в db/participants.rs;
   разойдутся — часть участников останется без цвета. */
:root {
  --avatar-0: #ffd9c2;
  --avatar-1: #d8e9dc;
  --avatar-2: #e3dcf3;
  --avatar-3: #fbe7a8;
  --avatar-4: #cfe4f2;
  --avatar-5: #f5d2de;
  --avatar-6: #dce7c8;
  --avatar-7: #eadccb;
}
```

- [ ] **Step 3: Написать базовые стили**

Создать `frontend/src/styles/base.css`:

```css
*,
*::before,
*::after {
  box-sizing: border-box;
}

html,
body {
  margin: 0;
  padding: 0;
}

body {
  background: var(--bg);
  color: var(--ink);
  font-family: var(--font-ui);
  font-size: 14px;
  line-height: 1.5;
  -webkit-font-smoothing: antialiased;
}

button,
input,
select,
textarea {
  font: inherit;
  color: inherit;
}

/* Хендофф ставит на инпутах `outline: none` и сам просит вернуть видимый фокус
   в проде. Возвращаем — но только для клавиатуры: `:focus-visible` не рисует
   кольцо на клик мышью, поэтому дизайн не страдает. */
:focus-visible {
  outline: 2px solid var(--accent);
  outline-offset: 2px;
  border-radius: 4px;
}

/* Хит-таргеты ≥ 40px — требование хендоффа для тача. Кнопки меньшего размера
   добирают площадь псевдоэлементом, а не увеличением самой пилюли. */
.tap-target {
  position: relative;
}

.tap-target::after {
  content: '';
  position: absolute;
  inset: 50% 50%;
  width: 40px;
  height: 40px;
  transform: translate(-50%, -50%);
}

@keyframes fadeIn {
  from {
    opacity: 0;
  }
  to {
    opacity: 1;
  }
}

@keyframes riseIn {
  from {
    opacity: 0;
    transform: translateY(10px);
  }
  to {
    opacity: 1;
    transform: none;
  }
}

@media (prefers-reduced-motion: reduce) {
  *,
  *::before,
  *::after {
    animation-duration: 0.01ms !important;
    transition-duration: 0.01ms !important;
  }
}
```

- [ ] **Step 4: Подключить шрифты и стили в точке входа**

Заменить `frontend/src/main.tsx` целиком:

```tsx
import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

// Сабсеты: кириллица и латиница. Интерфейс русский, но имена и описания
// пользователь пишет какие хочет.
import '@fontsource/onest/cyrillic-400.css'
import '@fontsource/onest/cyrillic-500.css'
import '@fontsource/onest/cyrillic-600.css'
import '@fontsource/onest/cyrillic-700.css'
import '@fontsource/onest/latin-400.css'
import '@fontsource/onest/latin-500.css'
import '@fontsource/onest/latin-600.css'
import '@fontsource/onest/latin-700.css'
import '@fontsource/unbounded/cyrillic-600.css'
import '@fontsource/unbounded/cyrillic-700.css'
import '@fontsource/unbounded/latin-600.css'
import '@fontsource/unbounded/latin-700.css'
import '@fontsource/jetbrains-mono/cyrillic-500.css'
import '@fontsource/jetbrains-mono/cyrillic-700.css'
import '@fontsource/jetbrains-mono/latin-500.css'
import '@fontsource/jetbrains-mono/latin-700.css'

import './styles/tokens.css'
import './styles/base.css'
import App from './App'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
```

Если какого-то сабсета в пакете нет, сборка упадёт с внятным «Failed to resolve import» —
посмотреть доступные файлы в `node_modules/@fontsource/<имя>/` и поправить импорты.

- [ ] **Step 5: Убрать шаблонные стили**

```bash
cd frontend && rm src/App.css src/index.css
```

- [ ] **Step 5a: Завести `vite-env.d.ts`**

В заготовке его нет, а без него не соберётся ничего дальше: типы `vite/client` объявляют и
`import.meta.env`, и модули `*.module.css`. Без этой строки каждый импорт стилей — ошибка
компилятора.

Создать `frontend/src/vite-env.d.ts`:

```ts
/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Адрес бэкенда. В дев-режиме пуст: `/api` проксирует Vite. */
  readonly VITE_API_BASE_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
```

- [ ] **Step 6: Базовый путь и PWA-манифест**

В `frontend/vite.config.ts`: добавить `base: '/reckoner/'` первым полем в объекте конфига
и починить манифест по таблице из спеки:

```ts
export default defineConfig({
  // GitHub Pages отдаёт сайт из подпапки репозитория. Ставится сразу, а не
  // перед деплоем: иначе пришлось бы переписывать все пути к ассетам.
  base: '/reckoner/',
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      devOptions: {
        enabled: true,
      },
      manifest: {
        name: 'Фонд встреч',
        short_name: 'Фонд встреч',
        display: 'standalone',
        start_url: '/reckoner/',
        scope: '/reckoner/',
        background_color: '#F2EEE5',
        theme_color: '#F2EEE5',
        icons: [
          // Иконки-плейсхолдеры из шаблона пока остаются: настоящие со знаком
          // «Ф» — задача этапа PWA, до деплоя.
          { src: 'pwa-192x192.png', sizes: '192x192', type: 'image/png' },
          { src: 'pwa-512x512.png', sizes: '512x512', type: 'image/png' },
        ],
      },
    }),
  ],
  server: { /* без изменений */ },
})
```

- [ ] **Step 7: Временный `App.tsx`, чтобы собралось**

Заменить `frontend/src/App.tsx` заглушкой — настоящий каркас в Task 4:

```tsx
export default function App() {
  return <div style={{ padding: 40 }}>Фонд встреч</div>
}
```

- [ ] **Step 8: Проверить сборку**

Run: `cd frontend && npm run build`
Expected: сборка проходит, в `dist/` есть `index.html` и шрифты в `assets/`.

- [ ] **Step 9: Посмотреть глазами**

Поднять дев-сервер и открыть `http://127.0.0.1:5173/reckoner/`.
Expected: бежевый фон `#F2EEE5`, текст «Фонд встреч» шрифтом Onest, а не системным.
Если фон белый — не подключился `tokens.css`; если шрифт системный — не подхватились `@fontsource`.

- [ ] **Step 10: Коммит**

```bash
git add frontend && git commit -m "feat(ui): carry the design tokens and self-hosted fonts into the app"
```

---

### Task 2: Типы API и клиент

**Files:**
- Create: `frontend/src/api/types.ts`, `frontend/src/api/client.ts`

- [ ] **Step 1: Описать типы ответов**

Создать `frontend/src/api/types.ts`. Это контракт с бэкендом — поля должны совпадать
с `backend/src/api/view.rs` буква в букву:

```ts
/** Ответы API. Форма зафиксирована в backend/src/api/view.rs и проверена
 *  интеграционными тестами; менять здесь в одностороннем порядке нельзя. */

export type MeetingStatus = 'settled' | 'attention' | 'alarm' | 'no-participants'

export type EntryKind = 'expense' | 'transfer'

/** Участник в карточке списка: аватар и имя, без денег. */
export interface ParticipantChip {
  id: string
  name: string
  emoji: string
  colorIndex: number
}

export interface Participant extends ParticipantChip {
  position: number
  /** «внёс N ₽»: только оплаченные расходы, отправленные переводы не в счёт. */
  contributedRubles: number
  /** Плюс — должны ему, минус — должен он. */
  netRubles: number
}

export interface Share {
  participantId: string
  /** 0..3. Полной доли здесь не бывает: её отсутствие и есть полная доля. */
  weightQuarters: number
}

export interface Entry {
  id: string
  kind: EntryKind
  payerId: string
  recipientId: string | null
  amountRubles: number
  description: string
  occurredAt: string
  shares: Share[]
  sharedByAll: boolean
}

export interface Transfer {
  fromId: string
  toId: string
  amountRubles: number
}

export interface Totals {
  spentRubles: number
  perPersonRubles: number
  pendingTransfers: number
}

export interface LogRecord {
  id: number
  text: string
  createdAt: string
}

/** Карточка списка. */
export interface MeetingCard {
  id: string
  title: string
  description: string
  emoji: string
  heldOn: string
  hasCover: boolean
  coverVersion: number
  totalRubles: number
  participants: ParticipantChip[]
  pendingTransfers: number
  status: MeetingStatus
}

/** Встреча целиком — ответ чтения и любой мутации. */
export interface Meeting {
  id: string
  title: string
  description: string
  emoji: string
  heldOn: string
  hasCover: boolean
  coverVersion: number
  createdAt: string
  participants: Participant[]
  entries: Entry[]
  settlement: Transfer[]
  totals: Totals
  status: MeetingStatus
  log: LogRecord[]
}

export type SortMode = 'date-desc' | 'date-asc' | 'total-desc' | 'open-first'

export interface ListFilters {
  q?: string
  participant?: string
  sort?: SortMode
}

export interface CreateMeetingBody {
  title?: string
  description?: string
  emoji?: string
  heldOn?: string
}

/** Единая форма отказа: см. backend/src/api/error.rs. */
export interface ApiErrorBody {
  error: string
  field?: string
  message?: string
}
```

- [ ] **Step 2: Написать клиент**

Создать `frontend/src/api/client.ts`:

```ts
import type { ApiErrorBody } from './types'

/** В дев-режиме пусто: Vite проксирует `/api` на бэкенд (см. vite.config.ts).
 *  В проде — полный адрес сервиса на Render, задаётся при сборке. */
const BASE = import.meta.env.VITE_API_BASE_URL ?? ''

/**
 * Отказ API в разобранном виде.
 *
 * Сервер отвечает единой формой на все ошибки, поэтому и клиенту хватает
 * одного класса. `field` есть только у `422` и указывает, какой инпут
 * подсветить.
 */
export class ApiError extends Error {
  readonly status: number
  readonly kind: string
  readonly field?: string

  constructor(status: number, kind: string, message: string, field?: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.kind = kind
    this.field = field
  }

  /** Текст для человека. Разработческие подробности сюда не попадают. */
  get humanMessage(): string {
    if (this.kind === 'validation') return this.message
    if (this.kind === 'not-found') return 'Встреча не найдена — возможно, её удалили.'
    return 'Что-то пошло не так. Попробуйте ещё раз.'
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response

  try {
    response = await fetch(BASE + path, {
      ...init,
      headers: init?.body ? { 'content-type': 'application/json' } : undefined,
    })
  } catch {
    // Сеть не дошла: сервер спит, интернета нет, CORS. Отличить нельзя —
    // `fetch` во всех этих случаях бросает одинаково.
    throw new ApiError(0, 'network', 'Сервер не отвечает')
  }

  if (response.status === 204) return undefined as T

  const text = await response.text()
  const body: unknown = text ? JSON.parse(text) : null

  if (!response.ok) {
    const error = (body ?? {}) as Partial<ApiErrorBody>
    throw new ApiError(
      response.status,
      error.error ?? 'unknown',
      error.message ?? 'Запрос не прошёл',
      error.field,
    )
  }

  return body as T
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: 'POST', body: JSON.stringify(body ?? {}) }),
  patch: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'PATCH', body: JSON.stringify(body) }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
}
```

- [ ] **Step 3: Проверить типы**

Run: `cd frontend && npx tsc -b`
Expected: без ошибок.

- [ ] **Step 4: Коммит**

```bash
git add frontend/src/api && git commit -m "feat(ui): type the API contract and parse its single error shape"
```

---

### Task 3: Форматирование денег и дат

**Files:**
- Create: `frontend/src/domain/format.ts`, `frontend/src/domain/format.test.ts`
- Modify: `frontend/package.json`

- [ ] **Step 1: Подключить Vitest**

В `frontend/package.json` в `scripts` добавить `"test": "vitest run"`.

- [ ] **Step 2: Написать падающие тесты**

Создать `frontend/src/domain/format.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { formatCardDate, formatLogTime, formatRubles, formatSigned } from './format'

// Неразрывный пробел записан константой намеренно: от обычного его глазами
// не отличить, и тест с обычным пробелом молча проверял бы не то.
// Тот же разделитель, что в текстах лога на сервере (backend/src/api/money.rs) —
// иначе одна сумма выглядела бы по-разному в истории и в логе.
const NBSP = '\u00A0'

describe('formatRubles', () => {
  it('разделяет разряды неразрывным пробелом', () => {
    expect(formatRubles(0)).toBe(`0${NBSP}₽`)
    expect(formatRubles(999)).toBe(`999${NBSP}₽`)
    expect(formatRubles(13700)).toBe(`13${NBSP}700${NBSP}₽`)
    expect(formatRubles(1234567)).toBe(`1${NBSP}234${NBSP}567${NBSP}₽`)
  })
})

describe('formatSigned', () => {
  it('ставит плюс кредитору и минус должнику', () => {
    // Знак ставим сами, а форматируем модуль, поэтому какой минус подставил бы
    // `Intl` — неважно. Это и причина так делать: у ru-RU он U+2212, и зависеть
    // от версии ICU не хочется.
    expect(formatSigned(4975)).toBe(`+4${NBSP}975${NBSP}₽`)
    expect(formatSigned(-1325)).toBe(`−1${NBSP}325${NBSP}₽`)
  })

  it('нулевой баланс называет словом, а не нулём', () => {
    expect(formatSigned(0)).toBe('ровно')
  })
})

describe('formatCardDate', () => {
  it('печатает день и месяц без года', () => {
    // heldOn приходит как «2026-07-23» — календарная дата без времени,
    // и разбирать её как момент времени нельзя: в зоне восточнее UTC
    // `new Date('2026-07-23')` даёт 23 июля 03:00, а западнее — 22 июля.
    expect(formatCardDate('2026-07-23')).toBe('23 июля')
    expect(formatCardDate('2026-08-03')).toBe('3 августа')
    expect(formatCardDate('2026-01-01')).toBe('1 января')
  })
})

describe('formatLogTime', () => {
  it('печатает дату и время через пробел', () => {
    // occurredAt — момент времени в UTC, его показываем в зоне пользователя.
    const moment = new Date(2026, 6, 23, 21, 5).toISOString()
    expect(formatLogTime(moment)).toBe('23.07 21:05')
  })
})
```

- [ ] **Step 3: Прогнать и убедиться в падении**

Run: `cd frontend && npm test`
Expected: FAIL — модуль `./format` не найден.

- [ ] **Step 4: Реализовать**

Создать `frontend/src/domain/format.ts`:

```ts
/** Форматирование для показа. Все суммы — целые рубли: копеек в приложении нет. */

const RUBLES = new Intl.NumberFormat('ru-RU', { maximumFractionDigits: 0 })

/** `13 700 ₽`, разряды через неразрывный пробел. */
export function formatRubles(amount: number): string {
  // `Intl` для ru-RU уже разделяет разряды U+00A0; между числом и знаком рубля
  // ставим такой же, чтобы сумма не разрывалась переносом строки.
  return `${RUBLES.format(amount)} ₽`
}

/** `+4 975 ₽` / `−1 325 ₽` / `ровно` — подпись баланса. */
export function formatSigned(net: number): string {
  if (net === 0) return 'ровно'
  // U+2212 MINUS SIGN, а не дефис: рядом с плюсом дефис выглядит короче
  // и ниже. Знак ставим сами, поэтому форматируем модуль.
  const sign = net > 0 ? '+' : '−'

  return `${sign}${formatRubles(Math.abs(net))}`
}

const CARD_DATE = new Intl.DateTimeFormat('ru-RU', { day: 'numeric', month: 'long' })

/**
 * `23 июля` из `2026-07-23`.
 *
 * Дата разбирается по частям, а не через `new Date(iso)`: строка вида
 * `2026-07-23` трактуется как полночь UTC, и в зоне пользователя это может
 * оказаться предыдущий день. Встреча — календарная дата, а не момент времени.
 */
export function formatCardDate(heldOn: string): string {
  const [year, month, day] = heldOn.split('-').map(Number)

  return CARD_DATE.format(new Date(year, month - 1, day))
}

/** `23.07 21:05` — история и лог. Момент времени, показываем в зоне клиента. */
export function formatLogTime(iso: string): string {
  const moment = new Date(iso)
  const pad = (value: number) => String(value).padStart(2, '0')

  return `${pad(moment.getDate())}.${pad(moment.getMonth() + 1)} ${pad(moment.getHours())}:${pad(moment.getMinutes())}`
}
```

- [ ] **Step 5: Прогнать тесты**

Run: `cd frontend && npm test`
Expected: PASS, 6 тестов.

Если `formatRubles(-225)` вернул `−225 ₽` (с U+2212), а тест ждёт `-225 ₽`: `Intl` для ru-RU
использует U+2212. Тогда правильный ответ — поправить **тест**, а не форматтер: знак от `Intl`
и есть типографски верный. Но проверить это надо фактическим выводом, а не догадкой.

- [ ] **Step 6: Коммит**

```bash
git add frontend && git commit -m "feat(ui): format money and dates the way the handoff specifies"
```

---

### Task 4: Каркас приложения — роутинг, шапка, экран пробуждения

**Files:**
- Create: `frontend/src/components/layout/{AppHeader.tsx,AppHeader.module.css,Container.tsx,Container.module.css,WakeScreen.tsx,WakeScreen.module.css}`
- Modify: `frontend/src/App.tsx`
- Create: `frontend/src/routes/MeetingsListPage.tsx`

- [ ] **Step 1: Контейнер**

`Container.tsx` — `max-width: 1180px`, `padding: 0 28px` из токенов. Тонкая обёртка,
чтобы ширина страницы задавалась в одном месте.

- [ ] **Step 2: Шапка**

`AppHeader.tsx` по разделу 1 хендоффа: sticky, `z-index: 30`, нижняя граница
`1px solid rgba(22,21,15,.09)`, фон `rgba(242,238,229,.88)` + `backdrop-filter: blur(12px)`.
Слева логотип — квадрат `34×34`, radius `11px`, фон `#16150F`, белая «Ф» шрифтом Unbounded 700
`15px`, рядом «Фонд встреч» `17px/700`. Клик по логотипу ведёт на `/`.

Справа — слот: на списке кнопка «+ Новая встреча», на встрече «← Все встречи». Слот, а не
условие внутри шапки: страница знает, что ей нужно, а шапка про страницы знать не должна.

```tsx
interface AppHeaderProps {
  /** Правая часть шапки. На списке — «+ Новая встреча», на встрече — «← Все встречи». */
  action?: ReactNode
}
```

- [ ] **Step 3: Экран пробуждения**

`WakeScreen.tsx` — на бесплатном Render сервис засыпает через 15 минут, и первый запрос
после сна висит 30–60 секунд. Без этого экрана друг увидит белую страницу и решит, что
сайт сломался.

Логика: при монтировании приложения дёргаем `GET /api/health`; если за 2 секунды ответа нет —
показываем экран «Будим сервер» со спиннером и подписью «Это займёт около минуты — бесплатный
хостинг засыпает без посетителей». Запрос **не отменяем** и таймаутов на него не ставим:
он всё равно дойдёт, а отмена только заставила бы начать заново.

- [ ] **Step 4: Собрать приложение**

`App.tsx`: `QueryClientProvider` + `BrowserRouter basename="/reckoner"` + `Routes`.

```tsx
const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Мутация возвращает встречу целиком и кладётся прямо в кэш, поэтому
      // перезапрашивать сразу после неё нечего. Возврат фокуса — другое дело:
      // за это время встречу мог поменять друг.
      refetchOnWindowFocus: true,
      staleTime: 30_000,
      retry: 1,
    },
  },
})
```

`basename` — потому что Pages отдаёт сайт из подпапки; без него роутер не узнает свои же ссылки.

- [ ] **Step 5: Проверить в браузере**

Поднять бэкенд (`cd backend && cargo run`) и дев-сервер, открыть
`http://127.0.0.1:5173/reckoner/`.
Expected: шапка с логотипом и заголовком, бежевый фон, пустая страница списка.
Экран пробуждения при живом бэкенде мелькнуть не должен: `/api/health` отвечает мгновенно.

- [ ] **Step 6: Коммит**

```bash
git add frontend && git commit -m "feat(ui): add the app shell, routing and the cold-start screen"
```

---

### Task 5: UI-примитивы

**Files:**
- Create: `frontend/src/components/ui/{Button,Input,Select,Avatar,AvatarStack,Money}.tsx` + `.module.css`
- Create: `frontend/src/domain/statusTone.ts`, `frontend/src/domain/cover.ts`

- [ ] **Step 1: Button**

Три вида по хендоффу: `primary` (фон `#E04A22`, белый текст, «литая» тень `0 2px 0 #A8320F`,
`padding 11px 20px`, radius 999), `secondary` (граница `rgba(22,21,15,.16)`, прозрачный фон,
hover `rgba(22,21,15,.06)`), `danger` (контурная красная, hover — заливка). Плюс `dark` для
кнопок в пустых состояниях. Состояние `loading` обязательно: оптимистичных обновлений мы
не делаем, и кнопка на время запроса должна показывать, что что-то происходит.

- [ ] **Step 2: Avatar и AvatarStack**

`Avatar` — круг с эмодзи на цвете из палитры по `colorIndex`; размер параметром
(28 / 30 / 36 / 38 / 42 — все размеры из хендоффа).

`AvatarStack` — до 5 кружков внахлёст: `margin-right: -7px`, белая обводка `2px`.
Если участников больше пяти, шестым кружком показываем «+N», а не молча обрезаем.

- [ ] **Step 3: Money**

Обёртка `<span>` с `font-family: var(--font-mono)`. Все денежные значения в дизайне
моноширинные — отдельный компонент не даёт об этом забыть.

- [ ] **Step 4: statusTone**

Создать `frontend/src/domain/statusTone.ts` — единственное место, где статус превращается
в цвет и подпись:

```ts
import type { MeetingStatus } from '../api/types'

export interface StatusTone {
  /** Подпись бейджа. `pendingTransfers` нужен для «Осталось N». */
  label: (pendingTransfers: number) => string
  text: string
  bg: string
  border: string
}

export const STATUS_TONES: Record<MeetingStatus, StatusTone> = {
  settled: {
    label: () => 'Все в расчёте',
    text: 'var(--ok-text)',
    bg: 'var(--ok-bg)',
    border: 'var(--ok-border)',
  },
  attention: {
    label: (n) => `Осталось ${n}`,
    text: 'var(--warn-text)',
    bg: 'var(--warn-bg)',
    border: 'var(--warn-border)',
  },
  alarm: {
    label: (n) => `Осталось ${n}`,
    text: 'var(--alarm-text)',
    bg: 'var(--alarm-bg)',
    border: 'var(--alarm-border)',
  },
  'no-participants': {
    label: () => 'Нет участников',
    text: 'var(--text-3)',
    bg: 'var(--surface-2)',
    border: 'var(--border)',
  },
}
```

- [ ] **Step 5: cover.ts**

```ts
/** Градиенты-заглушки обложек из хендоффа. */
const GRADIENTS = [
  'linear-gradient(135deg,#F7C6A8,#E7A183)',
  'linear-gradient(135deg,#BFD9C6,#8FBBA1)',
  'linear-gradient(135deg,#C9CFEA,#9AA6D6)',
  'linear-gradient(135deg,#F0D69B,#DCB86A)',
]

/**
 * Градиент по идентификатору встречи.
 *
 * Хендофф предлагает брать индекс карточки в списке, но тогда при смене
 * сортировки у встречи меняется картинка — список моргает, и обложка перестаёт
 * быть приметой встречи. Хеш от `id` даёт стабильный цвет на всю жизнь встречи.
 */
export function coverGradient(id: string): string {
  let hash = 0
  for (let index = 0; index < id.length; index += 1) {
    hash = (hash * 31 + id.charCodeAt(index)) >>> 0
  }

  return GRADIENTS[hash % GRADIENTS.length]
}
```

- [ ] **Step 6: Проверить сборку**

Run: `cd frontend && npx tsc -b && npm run lint`
Expected: чисто.

- [ ] **Step 7: Коммит**

```bash
git add frontend && git commit -m "feat(ui): add the shared primitives, status tones and cover gradients"
```

---

### Task 6: Список встреч

**Files:**
- Create: `frontend/src/api/meetings.ts`
- Create: `frontend/src/components/meetings/{MeetingCard,MeetingFilters,MeetingsEmpty,StatusBadge}.tsx` + `.module.css`
- Modify: `frontend/src/routes/MeetingsListPage.tsx`

- [ ] **Step 1: Запросы**

Создать `frontend/src/api/meetings.ts` с `useMeetings(filters)` на TanStack Query.
Ключ запроса включает фильтры целиком — тогда смена сортировки не переиспользует чужой ответ.

Фильтрация и сортировка идут **на сервер**, а не по загруженному массиву: так решено в спеке,
и это единственный способ, чтобы `open-first` и `total-desc` считались по настоящим данным,
а не по тому куску, что оказался на клиенте.

Поиск с задержкой 300 мс — иначе каждая буква уходит запросом к Neon.

- [ ] **Step 2: StatusBadge**

`11.5px/600`, `padding 5px 10px`, radius 999, цвета из `STATUS_TONES`.

- [ ] **Step 3: MeetingCard**

По разделу 2 хендоффа: белая, `padding 5px`, radius `20px`, **граница 1.5px цвета статуса**.
Верх — блок `height 112px`, radius `16px`, фон = обложка (`hasCover` → `/api/meetings/:id/cover?v=N`)
или градиент по `coverGradient(id)`. Внизу слева эмодзи `34px`, внизу справа бейдж.
Низ (`padding 15px 13px 13px`): название `18px/600` + дата справа `12.5px`; ниже `AvatarStack`
и сумма моноширинным `700 16px`.

Вся карточка — ссылка на `/meetings/:id`. Именно ссылка, а не `div` с `onClick`: иначе встречу
нельзя открыть в новой вкладке и скопировать адрес.

Hover: `translateY(-2px)` + `0 10px 26px rgba(22,21,15,.10)`.

- [ ] **Step 4: MeetingFilters**

Flex, `gap 10px`, wrap. Поиск `flex: 1 1 240px`, плейсхолдер «Поиск по встречам».
Селект участника: «Все участники» + уникальные имена по всем встречам, отсортированные.

Список имён берём из уже загруженных карточек: отдельной ручки для него нет, а заводить её
ради выпадающего списка — лишняя сущность. Ограничение честное и его надо знать: если фильтр
по участнику сузил выдачу, в селекте останутся только имена из неё. Поэтому имена собираются
из **нефильтрованного** запроса, который висит в кэше рядом.

Селект сортировки: «Сначала новые» (default), «Сначала старые», «По сумме ↓», «Сначала незакрытые».

- [ ] **Step 5: MeetingsEmpty**

Пунктирная рамка `1.5px dashed rgba(22,21,15,.18)`, radius `20px`, `padding 56px 24px`,
эмодзи 🍃 `40px`, «Ничего не нашлось», подпись «Поменяйте фильтры или заведите первую встречу»,
тёмная кнопка «Создать встречу».

- [ ] **Step 6: Страница**

H1 «Кто, кому и сколько» шрифтом Unbounded 700 `clamp(30px, 4vw, 44px)`; лид-параграф
`16px`, `#6C6759`, `max-width: 46ch` — текст берётся из раздела Copy хендоффа дословно.
Сетка `repeat(auto-fill, minmax(320px, 1fr))`, `gap 18px`.

Состояния загрузки и ошибки: скелет карточек на первую загрузку, текст ошибки с кнопкой
«Повторить» — если бэкенд не поднят, страница должна это сказать, а не остаться пустой.

- [ ] **Step 7: Проверить в браузере**

Дев-сервер + живой бэкенд.
Expected: две демо-встречи из бранча `dev` («Дача у Влада» — жёлтая рамка и «Осталось 2»,
«Кино в пятницу» — зелёная «Все в расчёте»). Проверить руками: поиск, фильтр по участнику,
все четыре сортировки, пустое состояние (искать заведомо отсутствующее слово).

- [ ] **Step 8: Коммит**

```bash
git add frontend && git commit -m "feat(ui): show the meetings list with server-side filters and sorting"
```

---

### Task 7: Модалка новой встречи

**Files:**
- Create: `frontend/src/components/modals/{Modal,MeetingFormModal}.tsx` + `.module.css`
- Modify: `frontend/src/routes/MeetingsListPage.tsx`, `frontend/src/api/meetings.ts`

- [ ] **Step 1: Каркас модалки**

`Modal.tsx` по разделу 4 хендоффа: оверлей `rgba(22,21,15,.42)` + `blur(3px)`, клик по фону
закрывает, клик внутри — `stopPropagation`. Панель `#FBF9F4`, radius `22px`, `max-width 460px`,
`max-height 88vh`, `overflow: auto`, `padding 24px`. Заголовок + круглая `✕` (`30×30`).

Доступность, чего в прототипе нет: `role="dialog"`, `aria-modal`, закрытие по Escape, возврат
фокуса на элемент, который открыл модалку, и блокировка прокрутки страницы под оверлеем.
Без этого модалка на телефоне прокручивает фон вместо себя.

- [ ] **Step 2: Форма встречи**

Поля: Название, Описание (textarea 3 строки), Дата (по умолчанию сегодня) и эмодзи.

Дата и эмодзи — наше добавление к хендоффу, зафиксированное в спеке: в прототипе дата
встречи равна моменту создания записи, из-за чего шашлыки в субботу, заведённые в среду,
показывают среду.

Компонент работает и на создание, и на правку — у правки появляется «Удалить» с подтверждением.
Правка вызывается со страницы встречи, это план 5, но форма пишется сразу целиком: раздваивать
её потом дороже.

- [ ] **Step 3: Мутация**

`useCreateMeeting` инвалидирует список и уводит на страницу созданной встречи — как в хендоффе
(«Создать» → новая встреча сразу открывается).

Ошибку валидации показываем у поля, которое назвал сервер в `field`. Ради этого поле в ошибке
и заводилось.

- [ ] **Step 4: Проверить в браузере**

Создать встречу через модалку.
Expected: встреча создаётся, приложение уходит на её страницу (пока пустую — это план 5),
возврат в список показывает новую карточку со статусом «Нет участников».
Проверить отдельно: пустое название допустимо и превращается в «Новая встреча» — так решено
на сервере; Escape закрывает; клик по фону закрывает; фокус возвращается на кнопку.

- [ ] **Step 5: Коммит**

```bash
git add frontend && git commit -m "feat(ui): create a meeting from a modal and open it"
```

---

### Task 8: Сверка со скриншотом и чистка

**Files:**
- Modify: по результатам сверки
- Modify: `docs/superpowers/specs/2026-08-04-reckoner-design.md`

- [ ] **Step 1: Сверить со скриншотом**

Открыть `docs/design/screens/01-meetings-list.png` рядом с браузером и пройти по списку:

- шапка: логотип `34×34` с «Ф», подпись «Фонд встреч», справа оранжевая «+ Новая встреча»;
- H1 «Кто, кому и сколько» — Unbounded, плотный трекинг;
- лид в две строки, ширина не больше 46 символов;
- три фильтра в строку, поиск шире селектов;
- карточки: цветная рамка `1.5px`, обложка-градиент `112px`, эмодзи снизу слева, бейдж снизу
  справа, дата справа от названия, аватары внахлёст, сумма моноширинным справа;
- фон страницы `#F2EEE5`, а не белый.

- [ ] **Step 2: Проверить узкий экран**

Сузить окно до 360 px.
Expected: сетка в одну колонку, фильтры переносятся, ничего не уезжает за край,
горизонтальной прокрутки страницы нет.

- [ ] **Step 3: Проверить клавиатуру**

Пройти Tab'ом по странице.
Expected: видимое кольцо фокуса на всех интерактивных элементах, порядок обхода осмысленный,
карточка встречи открывается по Enter.

- [ ] **Step 4: Полная проверка**

Run: `cd frontend && npm run build && npm run lint && npm test`
Expected: всё зелёное.

- [ ] **Step 5: Внести отступление в спеку**

В `docs/superpowers/specs/2026-08-04-reckoner-design.md`, таблица «Отклонения от хендоффа»,
добавить строку:

```markdown
| Градиент обложки | по индексу карточки в списке | по хешу `id` встречи | при смене сортировки у встречи менялась картинка |
```

- [ ] **Step 6: Коммит**

```bash
git add frontend docs && git commit -m "chore(ui): verify the list against the handoff screenshot"
```

---

## Проверка по завершении плана

```bash
cd frontend && npm run build && npm run lint && npm test
```

Вручную, при запущенном бэкенде: список показывает встречи из `dev`, все три фильтра работают,
все четыре сортировки работают, пустое состояние появляется, новая встреча создаётся и
открывается, узкий экран не ломается.

## Что дальше

План 5 — страница встречи: обложка-дропзона, сводка, участники, история, блок «Кто кому должен»
в трёх представлениях, «Что менялось», модалки участника, расхода с долями и перевода.
Затем обложки, PWA и деплой (включая отложенные задачи 12–13 плана 3: тесты уровня HTTP
и сужение CORS).
