# Exact Split and «Paid By» Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Расход делится точными суммами вместо четвертей (пустое поле — «поровну», 0 — исключён, разница с итогом раскладывается пропорционально до 25%), участник встречи может платить за других, а поле суммы складывает позиции чека.

**Architecture:** Вся денежная логика — в чистом домене на Rust: новое правило разбивки `split_amount` и свёртка балансов по кошелькам `fold_into_wallets`; домен тотален и не падает на любых данных. Правила ввода (порог 25%, «вписано больше, чем потрачено», запрет цепочек) проверяет слой API и отвечает 422. Фронт повторяет разбивку в превью — осознанное дублирование, сверенное общими тестовыми векторами. Схема меняется одной миграцией `0002`.

**Tech Stack:** Rust 1.97, Axum 0.7, sqlx 0.9 / Postgres (Neon), proptest; React 19, TypeScript 6, TanStack Query 5, CSS Modules, Vitest.

**Спека:** [`docs/superpowers/specs/2026-10-03-exact-split-design.md`](../specs/2026-10-03-exact-split-design.md). Читать перед началом.

---

## Предварительные условия

1. `backend/.env` содержит `DATABASE_URL` (бранч `dev`) и `TEST_DATABASE_URL` (бранч `test`) — см. `docs/setup-neon.md`.
2. Проверено 2026-10-03: в бранчах `dev` и `test` таблица `entry_shares` пуста, применена только миграция 1. Защита в миграции `0002` на них не сработает.
3. Прод (`production`) не проверен — это делает пользователь перед деплоем (Task 9).
4. Команды ниже запускаются из корня репозитория через Git Bash.

## Решения, принятые до начала

**Домен меняется аддитивно, потом переключается.** Task 1 добавляет `split_amount` рядом со старым расчётом по четвертям, Task 2 добавляет кошельки, и только Task 3 переводит `Entry` на явные суммы и удаляет четверти. Так каждый коммит собирается и проходит тесты.

**Одна миграция на обе части.** `0002_exact_shares_and_paid_by.sql` пишется в Task 3 целиком, включая `participants.paid_by`, хотя колонку читают только с Task 4. Миграцию, уже применённую к бранчу `test`, менять нельзя: sqlx сверяет контрольные суммы и откажется стартовать.

**`db::participants::insert` не меняет сигнатуру.** У неё 35 вызовов в тестах. Плательщик назначается отдельной функцией `set_paid_by`.

**Порог 25% и «вписано больше» — правила ввода, а не расчёта.** Они живут в `domain::check_split`, чтобы тестироваться без базы, но вызывает их только API. `split_amount` считает любые данные — в том числе ставшие «неправильными» после удаления или добавления участника.

**`PATCH /entries/:id` проверяет разбивку по итоговому состоянию.** Если поменяли только сумму, разбивка берётся из базы: иначе можно было бы уменьшить сумму ниже вписанного.

**Кнопка «+» — CSS `:focus-within`, а не состояние React.** Кнопка всегда в DOM и показывается, пока фокус внутри обёртки. `mousedown` на ней гасится, чтобы поле не теряло фокус и клавиатура телефона не пряталась.

## Структура файлов

```
backend/
  migrations/0002_exact_shares_and_paid_by.sql   новая: rubles вместо weight_quarters, participants.paid_by
  src/domain/
    types.rs        FixedShare, Participant.paid_by, Entry.fixed (Weight и FULL_QUARTERS удаляются)
    shares.rs       split_amount, check_split, SplitProblem, distribute
    wallets.rs      новая: wallet_of, fold_into_wallets
    reckoning.rs    план строится по свёрнутым балансам
    properties.rs   смеси «поровну / сумма / исключён» и случайные paid_by
    testing.rs      pin, paid_by, expense_with_fixed
    mod.rs          экспорты
  src/db/
    records.rs      ShareRow.rubles, ParticipantRow.paid_by
    entries.rs      rubles, shares_for_entry
    participants.rs paid_by в выборках, set_paid_by
    facts.rs        FixedShare, participant() публичная
  src/api/
    entries.rs      ShareInput.rubles, проверка разбивки при записи и правке
    participants.rs paidById в теле, логи о плательщике
    validate.rs     shares (рубли), split, paid_by
    texts.rs        payer_assigned, payer_removed
    view.rs         ShareView.rubles, ParticipantView.paid_by_id
  tests/api.rs, tests/persistence.rs
frontend/src/
  domain/
    amountExpression.ts (+ test)   новая: «390 + 1200 + 624»
    sharePreview.ts (+ test)       переписывается: previewSplit, splitStatus
    wallets.ts (+ test)            новая: payerOf, coveredBy, walletName
    format.ts (+ test)             parseAmount становится обёрткой
  api/types.ts, api/meetings.ts    Share.rubles, Participant.paidById, paidById в теле
  components/ui/AmountInput.tsx (+ .module.css)   новая: поле суммы с кнопкой «+»
  components/modals/
    ShareRow.tsx (+ .module.css)   переписывается: тап по человеку + поле суммы
    ExpenseModal.tsx (+ .module.css) один плательщик, точные суммы, строка состояния
    ParticipantModal.tsx           поле «Кто платит»
    MeetingFormModal.module.css    .readonly, .hint
  components/meeting/
    ParticipantCard.tsx (+ .module.css), ParticipantsSection.tsx
    DebtsSection.tsx, DebtsTableView.tsx, DebtsBalanceView.tsx
    HistoryRow.tsx
  routes/MeetingPage.tsx           people в ParticipantModal
README.md
.claude/launch.json                конфигурация backend для ручной проверки
```

---

### Task 1: Домен — правило разбивки точными суммами

Аддитивно: `split_amount` и `check_split` появляются рядом со старым расчётом по четвертям. Старый `expense_shares` переписывается на общий `distribute` — его тесты обязаны остаться зелёными.

**Files:**
- Modify: `backend/src/domain/types.rs`
- Modify: `backend/src/domain/testing.rs`
- Modify: `backend/src/domain/shares.rs` (весь файл)
- Modify: `backend/src/domain/mod.rs`

- [ ] **Step 1: Добавить тип `FixedShare`**

В `backend/src/domain/types.rs` после `struct Weight` (перед `struct Entry`) вставить:

```rust
/// Явная доля участника в расходе, в рублях. `0` — участник из расхода
/// исключён. Участник без такой записи делит остаток поровну.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedShare {
    pub participant_id: ParticipantId,
    pub rubles: i64,
}
```

- [ ] **Step 2: Добавить хелпер `pin`**

В `backend/src/domain/testing.rs` поменять импорт:

```rust
use super::types::{Entry, EntryKind, FixedShare, Participant, ParticipantId, Weight};
```

и дописать в конец файла:

```rust
/// Явная доля участника: вписанная сумма или `0`, если он исключён.
pub fn pin(participant: Participant, rubles: i64) -> FixedShare {
    FixedShare {
        participant_id: participant.id,
        rubles,
    }
}
```

- [ ] **Step 3: Написать падающие тесты**

В `backend/src/domain/shares.rs` в модуле `tests` поменять импорт на

```rust
    use super::*;
    use crate::domain::testing::{expense, expense_with_weights, participants, pin};
    use uuid::Uuid;
```

и дописать в конец модуля `tests` (перед закрывающей `}`):

```rust
    #[test]
    fn split_without_pins_is_an_even_split() {
        let people = participants(4);

        let shares = split_amount(8400, &people, &[]);

        for person in &people {
            assert_eq!(shares[&person.id], 2100, "участник {}", person.position);
        }
    }

    #[test]
    fn pinned_pay_exactly_and_the_rest_is_split_evenly() {
        let people = participants(4);

        let shares = split_amount(1001, &people, &[pin(people[0], 400)]);

        // Остаток 601 на троих: 201 / 200 / 200, лишний рубль — по position.
        assert_eq!(shares[&people[0].id], 400);
        assert_eq!(shares[&people[1].id], 201);
        assert_eq!(shares[&people[2].id], 200);
        assert_eq!(shares[&people[3].id], 200);
    }

    #[test]
    fn unpinned_pay_nothing_when_pins_cover_the_amount() {
        let people = participants(3);

        let shares = split_amount(1000, &people, &[pin(people[0], 600), pin(people[1], 400)]);

        assert_eq!(shares[&people[0].id], 600);
        assert_eq!(shares[&people[1].id], 400);
        assert_eq!(shares[&people[2].id], 0);
    }

    #[test]
    fn zero_excludes_from_the_even_split() {
        let people = participants(3);

        let shares = split_amount(101, &people, &[pin(people[2], 0)]);

        // Исключённый не получает даже рубля остатка.
        assert_eq!(shares[&people[0].id], 51);
        assert_eq!(shares[&people[1].id], 50);
        assert_eq!(shares[&people[2].id], 0);
    }

    #[test]
    fn everyone_pinned_to_the_amount_pays_exactly_that() {
        let people = participants(3);
        let fixed = [pin(people[0], 50), pin(people[1], 30), pin(people[2], 20)];

        let shares = split_amount(100, &people, &fixed);

        assert_eq!(shares[&people[0].id], 50);
        assert_eq!(shares[&people[1].id], 30);
        assert_eq!(shares[&people[2].id], 20);
    }

    #[test]
    fn surplus_is_taken_back_in_proportion() {
        let people = participants(3);
        // Вписано 1 200 при расходе 1 000 — скидка.
        let fixed = [pin(people[0], 700), pin(people[1], 400), pin(people[2], 100)];

        let shares = split_amount(1000, &people, &fixed);

        // Целые части 583 / 333 / 83, остатки равны, рубль — первому по position.
        assert_eq!(shares[&people[0].id], 584);
        assert_eq!(shares[&people[1].id], 333);
        assert_eq!(shares[&people[2].id], 83);
    }

    #[test]
    fn pins_over_the_amount_fall_back_to_proportion_with_unpinned_at_zero() {
        let people = participants(3);
        // Такое API не примет, но домен обязан посчитать: данные бывают и такими,
        // если в расход со скидкой потом добавили участника.
        let fixed = [pin(people[0], 800), pin(people[1], 400)];

        let shares = split_amount(1000, &people, &fixed);

        assert_eq!(shares[&people[0].id], 667);
        assert_eq!(shares[&people[1].id], 333);
        assert_eq!(shares[&people[2].id], 0);
    }

    #[test]
    fn everyone_excluded_splits_evenly_among_all() {
        let people = participants(3);
        let fixed = [pin(people[0], 0), pin(people[1], 0), pin(people[2], 0)];

        let shares = split_amount(100, &people, &fixed);

        assert_eq!(shares[&people[0].id], 34);
        assert_eq!(shares[&people[1].id], 33);
        assert_eq!(shares[&people[2].id], 33);
    }

    #[test]
    fn excluded_never_gets_a_remainder_rouble_under_proportion() {
        let people = participants(3);
        let fixed = [pin(people[0], 1), pin(people[1], 1), pin(people[2], 0)];

        let shares = split_amount(101, &people, &fixed);

        assert_eq!(shares[&people[0].id], 51);
        assert_eq!(shares[&people[1].id], 50);
        assert_eq!(shares[&people[2].id], 0);
    }

    #[test]
    fn pins_of_strangers_are_ignored() {
        let people = participants(2);
        let stranger = crate::domain::testing::participant(99);

        let shares = split_amount(100, &people, &[pin(stranger, 70)]);

        assert_eq!(shares.len(), 2);
        assert_eq!(shares[&people[0].id], 50);
        assert_eq!(shares[&people[1].id], 50);
    }

    #[test]
    fn split_survives_pins_that_overflow_i64_when_added() {
        let people = participants(2);
        let fixed = [pin(people[0], i64::MAX), pin(people[1], i64::MAX)];

        let shares = split_amount(i64::MAX, &people, &fixed);

        assert_eq!(shares.values().sum::<i64>(), i64::MAX);
    }

    /// Ресторан из спеки: шестеро в порядке добавления — Катя, Алексей, Настя,
    /// Аня, Женя, Вероника. Счёт 11 996, по позициям вышло 11 990.
    #[test]
    fn restaurant_spreads_the_missing_six_roubles_in_proportion() {
        let people = participants(6);
        let (katya, alexey, nastya, anya, zhenya, veronika) =
            (people[0], people[1], people[2], people[3], people[4], people[5]);
        let fixed = [
            pin(katya, 2214),
            pin(alexey, 3440),
            pin(nastya, 0),
            pin(anya, 0),
            pin(zhenya, 3430),
            pin(veronika, 2906),
        ];

        let shares = split_amount(11996, &people, &fixed);

        // Целые части дают 11 994, два рубля — наибольшим остаткам: Алексею и Жене.
        assert_eq!(shares[&katya.id], 2215);
        assert_eq!(shares[&alexey.id], 3442);
        assert_eq!(shares[&nastya.id], 0);
        assert_eq!(shares[&anya.id], 0);
        assert_eq!(shares[&zhenya.id], 3432);
        assert_eq!(shares[&veronika.id], 2907);
    }

    #[test]
    fn restaurant_couple_may_split_their_part_any_way() {
        let people = participants(6);
        let (katya, alexey, nastya, anya, zhenya, veronika) =
            (people[0], people[1], people[2], people[3], people[4], people[5]);
        let fixed = [
            pin(katya, 2214),
            pin(alexey, 1720),
            pin(nastya, 1720),
            pin(anya, 0),
            pin(zhenya, 3430),
            pin(veronika, 2906),
        ];

        let shares = split_amount(11996, &people, &fixed);

        // Три рубля остатка: Алексей и Настя (равные остатки, по position), затем Женя.
        assert_eq!(shares[&alexey.id] + shares[&nastya.id], 3442);
        assert_eq!(shares[&alexey.id], 1721);
        assert_eq!(shares[&nastya.id], 1721);
        assert_eq!(shares[&zhenya.id], 3432);
        assert_eq!(shares[&katya.id], 2215);
        assert_eq!(shares[&veronika.id], 2907);
    }

    #[test]
    fn restaurant_empty_field_takes_the_whole_rest() {
        let people = participants(6);
        let (katya, alexey, nastya, anya, zhenya, veronika) =
            (people[0], people[1], people[2], people[3], people[4], people[5]);
        let fixed = [
            pin(alexey, 3440),
            pin(nastya, 0),
            pin(anya, 0),
            pin(zhenya, 3430),
            pin(veronika, 2906),
        ];

        let shares = split_amount(11996, &people, &fixed);

        assert_eq!(shares[&katya.id], 2220);
        assert_eq!(shares[&alexey.id], 3440);
        assert_eq!(shares[&zhenya.id], 3430);
        assert_eq!(shares[&veronika.id], 2906);
    }

    #[test]
    fn check_accepts_pins_within_the_amount_while_someone_splits_the_rest() {
        let people = participants(3);

        assert_eq!(check_split(1000, &people, &[pin(people[0], 1000)]), Ok(()));
    }

    #[test]
    fn check_rejects_pins_over_the_amount_while_someone_splits_the_rest() {
        let people = participants(3);

        assert_eq!(
            check_split(1000, &people, &[pin(people[0], 1200)]),
            Err(SplitProblem::PinnedOverAmount { pinned: 1200 })
        );
    }

    #[test]
    fn check_accepts_up_to_a_quarter_off_when_everyone_is_pinned() {
        let people = participants(3);
        let with = |rubles| [pin(people[0], rubles), pin(people[1], 0), pin(people[2], 0)];

        assert_eq!(check_split(1000, &people, &with(750)), Ok(()));
        assert_eq!(check_split(1000, &people, &with(1250)), Ok(()));
    }

    #[test]
    fn check_rejects_more_than_a_quarter_off_when_everyone_is_pinned() {
        let people = participants(3);
        let with = |rubles| [pin(people[0], rubles), pin(people[1], 0), pin(people[2], 0)];

        assert_eq!(
            check_split(1000, &people, &with(749)),
            Err(SplitProblem::PinnedFarFromAmount { pinned: 749 })
        );
        assert_eq!(
            check_split(1000, &people, &with(1251)),
            Err(SplitProblem::PinnedFarFromAmount { pinned: 1251 })
        );
    }

    #[test]
    fn check_accepts_everyone_excluded() {
        let people = participants(2);

        assert_eq!(
            check_split(100, &people, &[pin(people[0], 0), pin(people[1], 0)]),
            Ok(())
        );
    }

    #[test]
    fn check_accepts_the_restaurant_bill() {
        let people = participants(6);
        let fixed = [
            pin(people[0], 2214),
            pin(people[1], 3440),
            pin(people[2], 0),
            pin(people[3], 0),
            pin(people[4], 3430),
            pin(people[5], 2906),
        ];

        assert_eq!(check_split(11996, &people, &fixed), Ok(()));
    }
```

- [ ] **Step 4: Убедиться, что тесты не компилируются**

Run: `cd backend && cargo test --lib domain::shares`
Expected: ошибка компиляции `cannot find function split_amount` / `check_split` / `SplitProblem`.

- [ ] **Step 5: Реализация**

Заменить всё, что в `backend/src/domain/shares.rs` стоит **до** `#[cfg(test)]`, на:

```rust
use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use super::types::{Entry, FULL_QUARTERS, FixedShare, Participant, ParticipantId};

/// Доли одного расхода в целых рублях.
///
/// Гарантия: при неотрицательной сумме и уникальных участниках сумма всех
/// долей ровно равна `entry.amount`. На ней держится инвариант «сумма
/// балансов равна нулю», а на нём — способность плана переводов закрыть
/// встречу в ноль. Исключение — пустой список участников: в этом случае
/// возвращается пустая карта независимо от суммы.
pub fn expense_shares(entry: &Entry, participants: &[Participant]) -> BTreeMap<ParticipantId, i64> {
    debug_assert!(
        entry.amount >= 0,
        "сумма расхода не может быть отрицательной: это гарантирует слой API"
    );
    debug_assert_unique(participants);

    // Ранний выход для читаемости: дальше нечего делить, и карта пуста.
    if participants.is_empty() {
        return BTreeMap::new();
    }

    let mut weights: Vec<(Participant, i64)> = participants
        .iter()
        .map(|participant| (*participant, entry.quarters_for(participant.id)))
        .collect();

    // Расход, из которого исключили всех, спека требует делить на всех поровну.
    // Иначе сумма весов была бы нулевой и делить было бы не по чему.
    if weights.iter().all(|(_, quarters)| *quarters == 0) {
        for (_, quarters) in &mut weights {
            *quarters = FULL_QUARTERS;
        }
    }

    distribute(entry.amount, &weights)
}

/// Разбивка суммы расхода, когда у части участников вписаны точные суммы.
///
/// `fixed` — явные доли: вписанная сумма или `0`, если участник исключён.
/// Кого в `fixed` нет, тот делит остаток поровну. Правила — из спеки
/// `2026-10-03-exact-split-design.md`, `F` — сумма вписанного:
///
/// 1. Кто-то делит остаток и `F ≤ amount` — вписанные платят ровно своё,
///    остаток делится поровну между остальными.
/// 2. Остаток не делит никто либо `F > amount`, и при этом `F > 0` — сумма
///    делится пропорционально вписанному; у кого суммы нет, тот платит 0.
/// 3. Вписаны одни нули — исключены все, и сумма делится на всех поровну.
///
/// Гарантия та же, что у `expense_shares`: сумма долей ровно равна `amount`,
/// пустой список участников даёт пустую карту. Явные доли тех, кого нет
/// в `participants`, не учитываются.
pub fn split_amount(
    amount: i64,
    participants: &[Participant],
    fixed: &[FixedShare],
) -> BTreeMap<ParticipantId, i64> {
    debug_assert!(
        amount >= 0,
        "сумма расхода не может быть отрицательной: это гарантирует слой API"
    );
    debug_assert!(
        fixed.iter().all(|share| share.rubles >= 0),
        "вписанная сумма не может быть отрицательной: это гарантирует схема"
    );
    debug_assert_unique(participants);

    if participants.is_empty() {
        return BTreeMap::new();
    }

    let pins: Vec<(Participant, Option<i64>)> = participants
        .iter()
        .map(|participant| (*participant, pinned_for(fixed, participant.id)))
        .collect();
    // В i128: каждая вписанная сумма помещается в i64, а их сумма — не обязательно.
    let pinned: i128 = pins
        .iter()
        .filter_map(|(_, rubles)| rubles.map(i128::from))
        .sum();
    let someone_even = pins.iter().any(|(_, rubles)| rubles.is_none());

    // Правило 1.
    if someone_even && pinned <= i128::from(amount) {
        // `pinned ≤ amount`, поэтому остаток помещается в i64 и не отрицателен.
        let rest = amount - pinned as i64;
        // Вес 1 у тех, кто делит остаток, 0 у вписанных: их доля добавится ниже.
        let weights: Vec<(Participant, i64)> = pins
            .iter()
            .map(|(participant, rubles)| (*participant, i64::from(rubles.is_none())))
            .collect();

        let mut shares = distribute(rest, &weights);
        for (participant, rubles) in &pins {
            if let Some(rubles) = rubles {
                *shares
                    .get_mut(&participant.id)
                    .expect("каждый участник уже в карте долей") += rubles;
            }
        }

        return shares;
    }

    let weights: Vec<(Participant, i64)> = if pinned > 0 {
        // Правило 2.
        pins.iter()
            .map(|(participant, rubles)| (*participant, rubles.unwrap_or(0)))
            .collect()
    } else {
        // Правило 3.
        participants
            .iter()
            .map(|participant| (*participant, 1))
            .collect()
    };

    distribute(amount, &weights)
}

/// Что не так с разбивкой, которую прислал клиент. `split_amount` посчитает
/// любые данные — это правила ввода: они ловят опечатки до того, как те
/// превратятся в чьи-то долги.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SplitProblem {
    /// Кто-то делит остаток поровну, а вписано больше, чем потрачено.
    PinnedOverAmount { pinned: i64 },
    /// Вписано у всех, и сумма вписанного расходится с расходом больше чем на
    /// четверть. Похоже на опечатку вроде 22 140 вместо 2 214.
    PinnedFarFromAmount { pinned: i64 },
}

/// Проверка разбивки при записи. Четверть отсечки перекрывает чаевые,
/// сервисный сбор и обычные скидки. Правило 3 («исключены все») проходит:
/// так было и с четвертями.
pub fn check_split(
    amount: i64,
    participants: &[Participant],
    fixed: &[FixedShare],
) -> Result<(), SplitProblem> {
    let pins: Vec<Option<i64>> = participants
        .iter()
        .map(|participant| pinned_for(fixed, participant.id))
        .collect();
    let total: i128 = pins.iter().flatten().map(|rubles| i128::from(*rubles)).sum();
    let someone_even = pins.iter().any(Option::is_none);
    // Только для текста ошибки: сумма больше i64 — заведомо опечатка, и
    // точное число в ней неважно.
    let pinned = i64::try_from(total).unwrap_or(i64::MAX);
    let amount_wide = i128::from(amount);

    if someone_even {
        return if total > amount_wide {
            Err(SplitProblem::PinnedOverAmount { pinned })
        } else {
            Ok(())
        };
    }

    if total > 0 && 4 * (amount_wide - total).abs() > amount_wide {
        return Err(SplitProblem::PinnedFarFromAmount { pinned });
    }

    Ok(())
}

fn pinned_for(fixed: &[FixedShare], participant: ParticipantId) -> Option<i64> {
    fixed
        .iter()
        .find(|share| share.participant_id == participant)
        .map(|share| share.rubles)
}

/// Делит `amount` пропорционально весам в целых рублях: целая часть каждому,
/// остаток по рублю тем, у кого больше дробная часть. При равенстве — по
/// порядку добавления, затем по id, чтобы порядок был полным и результат не
/// зависел от порядка строк, пришедших из базы.
///
/// Требует положительной суммы весов. Участник с нулевым весом не получает ни
/// рубля: его остаток от деления нулевой, а рублей в остатке всегда строго
/// меньше, чем участников с ненулевым остатком.
fn distribute(amount: i64, weights: &[(Participant, i64)]) -> BTreeMap<ParticipantId, i64> {
    let total: i128 = weights.iter().map(|(_, weight)| i128::from(*weight)).sum();
    debug_assert!(total > 0, "делить не по чему: сумма весов нулевая");

    let mut shares: BTreeMap<ParticipantId, i64> = BTreeMap::new();
    let mut remainders: Vec<(Participant, i128)> = Vec::with_capacity(weights.len());
    let mut distributed: i64 = 0;
    for (participant, weight) in weights {
        let numerator = i128::from(amount) * i128::from(*weight);
        // Вес не больше суммы весов, поэтому целая часть не больше `amount` —
        // сужение до i64 безопасно.
        let base = (numerator / total) as i64;
        shares.insert(participant.id, base);
        distributed += base;
        remainders.push((*participant, numerator % total));
    }

    remainders.sort_by_key(|(participant, remainder)| {
        (Reverse(*remainder), participant.position, participant.id)
    });

    let mut leftover = amount - distributed;
    for (participant, _) in &remainders {
        if leftover <= 0 {
            break;
        }
        *shares
            .get_mut(&participant.id)
            .expect("участник уже добавлен в карту долей") += 1;
        leftover -= 1;
    }

    shares
}

fn debug_assert_unique(participants: &[Participant]) {
    debug_assert!(
        participants
            .iter()
            .map(|participant| participant.id)
            .collect::<BTreeSet<_>>()
            .len()
            == participants.len(),
        "участники должны быть уникальны: доли ключуются по id"
    );
}
```

- [ ] **Step 6: Экспорты**

В `backend/src/domain/mod.rs` заменить

```rust
pub use shares::expense_shares;
pub use status::MeetingStatus;
pub use types::{
    Entry, EntryKind, FULL_QUARTERS, MeetingFacts, Participant, ParticipantId, Weight,
};
```

на

```rust
pub use shares::{SplitProblem, check_split, expense_shares, split_amount};
pub use status::MeetingStatus;
pub use types::{
    Entry, EntryKind, FULL_QUARTERS, FixedShare, MeetingFacts, Participant, ParticipantId,
    Weight,
};
```

- [ ] **Step 7: Тесты зелёные, включая старые**

Run: `cd backend && cargo test --lib domain`
Expected: PASS, все тесты `domain::shares` (старые на четверти и новые) и property-тесты.

- [ ] **Step 8: fmt и clippy**

Run: `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings`
Expected: без предупреждений.

- [ ] **Step 9: Commit**

```bash
git add backend/src/domain
git commit -m "feat(domain): split an expense by exact amounts

split_amount pays pinned people exactly what was typed and splits the rest
evenly; when everyone is pinned it spreads the difference in proportion.
check_split carries the input rules the API will enforce. The old quarter
split now goes through the same distribute.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 2: Домен — кошельки «платит за»

**Files:**
- Modify: `backend/src/domain/types.rs`
- Modify: `backend/src/domain/testing.rs`
- Modify: `backend/src/domain/shares.rs` (тест `breaks_ties_by_position_not_by_id`)
- Modify: `backend/src/db/facts.rs`
- Create: `backend/src/domain/wallets.rs`
- Modify: `backend/src/domain/reckoning.rs`
- Modify: `backend/src/domain/properties.rs` (весь файл)
- Modify: `backend/src/domain/mod.rs`

- [ ] **Step 1: Поле `paid_by` у участника**

В `backend/src/domain/types.rs` заменить `struct Participant` с комментарием на:

```rust
/// Участник встречи. Домену нужны идентификатор, порядок добавления —
/// `position` делает раздачу остатка рублей детерминированной — и связь
/// «платит за».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Participant {
    pub id: ParticipantId,
    pub position: i32,
    /// Кто платит за участника: другой участник той же встречи. `None` —
    /// платит сам за себя. Как это влияет на расчёт — `wallets.rs`.
    pub paid_by: Option<ParticipantId>,
}
```

- [ ] **Step 2: Починить литералы `Participant`**

`backend/src/domain/testing.rs`, в `participant()`:

```rust
    Participant {
        id: ParticipantId(Uuid::from_u128(position as u128 + 1)),
        position,
        paid_by: None,
    }
```

и дописать в конец файла:

```rust
/// Тот же участник, но за него платит `payer`.
pub fn paid_by(person: Participant, payer: Participant) -> Participant {
    Participant {
        paid_by: Some(payer.id),
        ..person
    }
}
```

`backend/src/domain/shares.rs`, тест `breaks_ties_by_position_not_by_id`: в оба литерала `Participant { id: …, position: … }` добавить `paid_by: None,`.

`backend/src/db/facts.rs`, в `build`:

```rust
        .map(|row| Participant {
            id: ParticipantId(row.id),
            position: row.position,
            // Колонки `paid_by` в строке пока нет.
            paid_by: None,
        })
```

Run: `cd backend && cargo test --lib domain`
Expected: PASS (поведение не менялось).

- [ ] **Step 3: Падающие тесты кошельков**

Создать `backend/src/domain/wallets.rs`:

```rust
//! «Платит за»: кошельки внутри встречи.
//!
//! Кошелёк — участник, за которого никто не платит, вместе с теми, за кого
//! платит он. Балансы считаются по каждому человеку, а план переводов — по
//! кошелькам: баланс оплачиваемого целиком переходит к его плательщику.

use std::collections::BTreeMap;

use super::types::{Participant, ParticipantId};

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{paid_by, participant, participants};

    fn balances(entries: &[(Participant, i64)]) -> BTreeMap<ParticipantId, i64> {
        entries
            .iter()
            .map(|(person, balance)| (person.id, *balance))
            .collect()
    }

    #[test]
    fn participant_without_payer_is_their_own_wallet() {
        let people = participants(2);

        assert_eq!(wallet_of(&people[0], &people), people[0].id);
    }

    #[test]
    fn covered_balance_moves_to_the_payer() {
        let people = participants(3);
        let (katya, zhenya) = (people[0], people[2]);
        let anya = paid_by(people[1], zhenya);
        let people = vec![katya, anya, zhenya];

        let folded = fold_into_wallets(&people, &balances(&[(katya, -70), (anya, -30), (zhenya, 100)]));

        assert_eq!(folded[&katya.id], -70);
        assert_eq!(folded[&anya.id], 0);
        assert_eq!(folded[&zhenya.id], 70);
    }

    #[test]
    fn one_payer_covers_several() {
        let people = participants(3);
        let alexey = people[0];
        let nastya = paid_by(people[1], alexey);
        let other = paid_by(people[2], alexey);
        let people = vec![alexey, nastya, other];

        let folded = fold_into_wallets(&people, &balances(&[(alexey, 50), (nastya, -20), (other, -30)]));

        assert_eq!(folded[&alexey.id], 0);
        assert_eq!(folded[&nastya.id], 0);
        assert_eq!(folded[&other.id], 0);
    }

    #[test]
    fn link_to_a_covered_payer_is_ignored() {
        // Цепочка A → B → C. API её не допустит, но две одновременные правки
        // могут её создать. B уходит в кошелёк C, а связь A с B не действует:
        // иначе A оказался бы в кошельке того, кого самого нет.
        let people = participants(3);
        let c = people[2];
        let b = paid_by(people[1], c);
        let a = paid_by(people[0], b);
        let people = vec![a, b, c];

        assert_eq!(wallet_of(&a, &people), a.id);
        assert_eq!(wallet_of(&b, &people), c.id);

        let folded = fold_into_wallets(&people, &balances(&[(a, -10), (b, -20), (c, 30)]));
        assert_eq!(folded[&a.id], -10);
        assert_eq!(folded[&b.id], 0);
        assert_eq!(folded[&c.id], 10);
    }

    #[test]
    fn link_to_a_stranger_is_ignored() {
        let people = participants(1);
        let stranger = participant(99);
        let person = paid_by(people[0], stranger);

        assert_eq!(wallet_of(&person, &[person]), person.id);
    }

    #[test]
    fn folding_keeps_the_sum() {
        let people = participants(4);
        let people = vec![
            people[0],
            paid_by(people[1], people[0]),
            people[2],
            paid_by(people[3], people[2]),
        ];
        let net = balances(&[(people[0], 300), (people[1], -100), (people[2], -150), (people[3], -50)]);

        let folded = fold_into_wallets(&people, &net);

        assert_eq!(folded.values().sum::<i64>(), net.values().sum::<i64>());
    }
}
```

В `backend/src/domain/mod.rs` после `pub mod types;` добавить `pub mod wallets;`, а к экспортам — `pub use wallets::{fold_into_wallets, wallet_of};`.

Run: `cd backend && cargo test --lib domain::wallets`
Expected: ошибка компиляции `cannot find function wallet_of` / `fold_into_wallets`.

- [ ] **Step 4: Реализация**

В `backend/src/domain/wallets.rs` между `use super::types…;` и `#[cfg(test)]` вставить:

```rust
/// Кому зачисляется баланс участника.
///
/// Связь «платит за» действует, только если плательщик есть среди участников
/// встречи, это не сам участник и за самого плательщика никто не платит.
/// Иначе участник платит сам за себя. API таких связей не допускает, но они
/// могут возникнуть из двух одновременных правок, и тогда расчёт не должен ни
/// зациклиться, ни потерять рубли.
pub fn wallet_of(participant: &Participant, participants: &[Participant]) -> ParticipantId {
    let Some(payer_id) = participant.paid_by else {
        return participant.id;
    };

    match participants.iter().find(|candidate| candidate.id == payer_id) {
        Some(payer) if payer.id != participant.id && payer.paid_by.is_none() => payer.id,
        _ => participant.id,
    }
}

/// Сворачивает балансы по кошелькам: баланс оплачиваемого прибавляется к
/// балансу плательщика, а сам становится нулевым. Сумма балансов при этом не
/// меняется — рубли только переезжают.
///
/// В результате есть каждый участник встречи; кого нет в `net`, считается
/// в расчёте.
pub fn fold_into_wallets(
    participants: &[Participant],
    net: &BTreeMap<ParticipantId, i64>,
) -> BTreeMap<ParticipantId, i64> {
    let mut folded: BTreeMap<ParticipantId, i64> = participants
        .iter()
        .map(|participant| (participant.id, 0))
        .collect();

    for participant in participants {
        let balance = net.get(&participant.id).copied().unwrap_or(0);
        *folded
            .get_mut(&wallet_of(participant, participants))
            .expect("кошелёк — всегда участник встречи") += balance;
    }

    folded
}
```

Run: `cd backend && cargo test --lib domain::wallets`
Expected: PASS.

- [ ] **Step 5: Падающий тест расчёта встречи**

В `backend/src/domain/reckoning.rs` в модуле `tests` поменять импорт на

```rust
    use crate::domain::testing::{expense, paid_by, participants, transfer};
```

и дописать тест:

```rust
    #[test]
    fn covered_participant_never_appears_in_the_plan() {
        // Женя платит за Аню. Расход Жени 400 на четверых — по 100.
        let people = participants(4);
        let (katya, zhenya, veronika) = (people[0], people[2], people[3]);
        let anya = paid_by(people[1], zhenya);
        let people = vec![katya, anya, zhenya, veronika];
        let entries = vec![expense(zhenya, 400)];

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(reckoning.contributed[&zhenya.id], 400);
        assert_eq!(reckoning.net[&anya.id], 0);
        assert_eq!(reckoning.net[&zhenya.id], 200);
        assert_eq!(reckoning.net[&katya.id], -100);
        assert_eq!(reckoning.net[&veronika.id], -100);
        assert_eq!(
            reckoning.settlement,
            vec![
                Transfer {
                    from: katya.id,
                    to: zhenya.id,
                    amount: 100
                },
                Transfer {
                    from: veronika.id,
                    to: zhenya.id,
                    amount: 100
                },
            ]
        );
        assert_eq!(reckoning.status, MeetingStatus::Attention(2));
    }
```

Run: `cd backend && cargo test --lib domain::reckoning`
Expected: FAIL — `net[&anya.id]` равен `-100`, в плане есть перевод от Ани.

- [ ] **Step 6: План по кошелькам**

В `backend/src/domain/reckoning.rs`:

импорт — добавить строку `use super::wallets::fold_into_wallets;`;

поле `net` в `Reckoning`:

```rust
    /// Баланс кошелька: плюс — ему должны, минус — он должен. У того, за кого
    /// платят, всегда 0: его баланс прибавлен к балансу плательщика.
    pub net: BTreeMap<ParticipantId, i64>,
```

первая строка `reckon`:

```rust
    // Балансы считаются по людям, а сводятся по кошелькам: иначе в плане
    // появились бы переводы от тех, за кого платят другие.
    let net = fold_into_wallets(facts.participants, &net_balances(facts));
```

Run: `cd backend && cargo test --lib domain`
Expected: PASS.

- [ ] **Step 7: Property-тесты с кошельками**

Заменить `backend/src/domain/properties.rs` целиком:

```rust
//! Property-тесты инвариантов домена. Файл содержит только тесты.

use proptest::prelude::*;

use super::testing::{expense_with_weights, participant, transfer};
use super::types::{Entry, MeetingFacts, Participant};
use super::{fold_into_wallets, net_balances, settlement_plan, wallet_of};

/// Черновик записи в терминах индексов участников. Proptest генерирует любые
/// индексы, а `build` приводит их к существующим по модулю — так любой
/// сгенерированный вход остаётся корректным и ничего не приходится отбрасывать.
#[derive(Debug, Clone)]
enum Draft {
    Expense {
        payer: usize,
        amount: i64,
        quarters: Vec<u8>,
    },
    Transfer {
        from: usize,
        to: usize,
        amount: i64,
    },
}

fn draft_strategy() -> impl Strategy<Value = Draft> {
    prop_oneof![
        (
            0usize..64,
            1i64..1_000_000,
            prop::collection::vec(0u8..=4, 1..8),
        )
            .prop_map(|(payer, amount, quarters)| Draft::Expense {
                payer,
                amount,
                quarters,
            }),
        (0usize..64, 0usize..64, 1i64..1_000_000).prop_map(|(from, to, amount)| Draft::Transfer {
            from,
            to,
            amount
        }),
    ]
}

/// Ссылки «платит за» по индексам. Редкие: иначе почти все оказывались бы
/// в чужих кошельках и проверять было бы нечего.
fn links_strategy() -> impl Strategy<Value = Vec<Option<usize>>> {
    prop::collection::vec(prop::option::weighted(0.3, 0usize..64), 1..8)
}

fn roster(count: usize, links: &[Option<usize>]) -> Vec<Participant> {
    let plain: Vec<Participant> = (0..count as i32).map(participant).collect();

    plain
        .iter()
        .enumerate()
        .map(|(index, person)| {
            // Ссылку на себя запрещает CHECK в схеме — домен её не увидит.
            // Цепочки и круги не запрещает ничего, кроме API, поэтому домен
            // обязан их пережить.
            let payer = links[index % links.len()]
                .map(|target| target % count)
                .filter(|target| *target != index);
            Participant {
                paid_by: payer.map(|target| plain[target].id),
                ..*person
            }
        })
        .collect()
}

fn build(people: &[Participant], drafts: &[Draft]) -> Vec<Entry> {
    let count = people.len();
    drafts
        .iter()
        .filter_map(|draft| match draft {
            Draft::Expense {
                payer,
                amount,
                quarters,
            } => {
                // Полная доля (4/4) в списке весов не хранится.
                let weights: Vec<(Participant, u8)> = people
                    .iter()
                    .enumerate()
                    .filter_map(|(index, person)| {
                        let quarter = quarters[index % quarters.len()];
                        (quarter < 4).then_some((*person, quarter))
                    })
                    .collect();
                Some(expense_with_weights(
                    people[*payer % count],
                    *amount,
                    &weights,
                ))
            }
            Draft::Transfer { from, to, amount } => {
                let sender = *from % count;
                let recipient = *to % count;
                // Перевод самому себе запрещён на уровне API, домен его не увидит.
                (sender != recipient).then(|| transfer(people[sender], people[recipient], *amount))
            }
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Сумма балансов всегда ноль — и до свёртки по кошелькам, и после.
    /// Инвариант обеспечивает целочисленная раздача остатка долей, свёртка
    /// только перекладывает рубли, и именно на нём держится способность плана
    /// закрыть встречу.
    #[test]
    fn balances_sum_to_zero_before_and_after_folding(
        count in 1usize..8,
        links in links_strategy(),
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count, &links);
        let entries = build(&people, &drafts);

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });
        prop_assert_eq!(net.values().sum::<i64>(), 0);

        let folded = fold_into_wallets(&people, &net);
        prop_assert_eq!(folded.values().sum::<i64>(), 0);
    }

    /// У того, за кого действительно платят, после свёртки ноль: в плане
    /// переводов его не будет.
    #[test]
    fn covered_participants_end_at_zero(
        count in 1usize..8,
        links in links_strategy(),
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count, &links);
        let entries = build(&people, &drafts);
        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        let folded = fold_into_wallets(&people, &net);

        for person in &people {
            if wallet_of(person, &people) != person.id {
                prop_assert_eq!(folded[&person.id], 0, "у оплачиваемого остался баланс");
            }
        }
    }

    /// План сводит каждый баланс кошелька в ноль, и переводов в нём строго
    /// меньше, чем участников: это и означает «минимальный», а не «все всем».
    ///
    /// Граница следует из устройства алгоритма: на каждом шаге закрывается хотя
    /// бы одна сторона, а последний шаг закрывает обе, поэтому переводов не
    /// больше `должники + кредиторы − 1`, а это не больше `count − 1`.
    #[test]
    fn settlement_plan_zeroes_every_balance(
        count in 1usize..8,
        links in links_strategy(),
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count, &links);
        let entries = build(&people, &drafts);
        let net = fold_into_wallets(
            &people,
            &net_balances(MeetingFacts {
                participants: &people,
                entries: &entries,
            }),
        );

        let plan = settlement_plan(&people, &net);

        let mut settled = net.clone();
        for item in &plan {
            *settled.get_mut(&item.from).expect("должник есть") += item.amount;
            *settled.get_mut(&item.to).expect("кредитор есть") -= item.amount;
        }
        prop_assert!(
            settled.values().all(|balance| *balance == 0),
            "после плана остались долги: {:?}",
            settled
        );
        prop_assert!(
            plan.len() < count,
            "переводов {} при {} участниках — план не минимальный",
            plan.len(),
            count
        );
    }
}
```

Run: `cd backend && cargo test --lib domain`
Expected: PASS.

- [ ] **Step 8: fmt, clippy, commit**

Run: `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib`
Expected: PASS, без предупреждений.

```bash
git add backend/src/domain backend/src/db/facts.rs
git commit -m "feat(domain): fold a covered participant's balance into the payer

A participant may be paid for by another. Balances are still computed per
person, then folded by wallet before the plan is built, so the covered one
never shows up in transfers. A chain or a link to a stranger is ignored
rather than looped over.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 3: Бэкенд переходит на точные суммы

Переключение: `Entry` несёт явные доли в рублях, четверти удаляются из домена, схемы, слоя БД и API. Здесь же — миграция `0002` целиком.

**Files:**
- Create: `backend/migrations/0002_exact_shares_and_paid_by.sql`
- Modify: `backend/src/domain/types.rs`, `testing.rs`, `shares.rs`, `properties.rs`, `reckoning.rs`, `mod.rs`
- Modify: `backend/src/db/records.rs`, `entries.rs`, `facts.rs`
- Modify: `backend/src/api/entries.rs`, `validate.rs`, `view.rs`
- Modify: `backend/tests/api.rs`, `backend/tests/persistence.rs`

- [ ] **Step 1: Миграция**

Создать `backend/migrations/0002_exact_shares_and_paid_by.sql`:

```sql
-- Точные суммы вместо четвертей и «платит за» внутри встречи.
-- Дизайн: docs/superpowers/specs/2026-10-03-exact-split-design.md.

-- Неполные четверти (¼, ½, ¾) в новую модель напрямую не ложатся, и перед
-- деплоем проверено, что их нет ни в одной базе. Если всё-таки появились,
-- миграция падает целиком: сервер не поднимется, и Render оставит работать
-- прежнюю версию. Это лучше, чем молча поменять чьи-то деньги.
do $$
begin
    if exists (select 1 from entry_shares where weight_quarters between 1 and 3) then
        raise exception
            'в entry_shares есть неполные доли (¼, ½, ¾): их нужно перевести в рубли вручную, см. docs/superpowers/specs/2026-10-03-exact-split-design.md';
    end if;
end
$$;

-- Строки нет — участник делит остаток поровну. 0 — исключён из расхода.
-- Больше нуля — точная сумма. После проверки выше остались только нулевые
-- доли, то есть исключённые, и значение по умолчанию переводит их ровно в 0.
alter table entry_shares add column rubles bigint not null default 0 check (rubles >= 0);
alter table entry_shares alter column rubles drop default;
alter table entry_shares drop column weight_quarters;

-- Кто платит за участника. Принадлежность той же встрече и отсутствие цепочек
-- проверяет API — так же, как для payer_id и recipient_id у записей.
alter table participants
    add column paid_by uuid references participants (id) on delete set null,
    add constraint participants_paid_by_not_self check (paid_by <> id);
```

- [ ] **Step 2: Тест схемы (падающий)**

В `backend/tests/persistence.rs` после теста `migrations_apply_and_schema_is_queryable` добавить:

```rust
#[tokio::test]
async fn exact_shares_and_payers_are_in_the_schema() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;

    let columns: Vec<(String, String)> = sqlx::query_as(
        "select table_name::text, column_name::text from information_schema.columns \
         where table_schema = 'public' and table_name in ('entry_shares', 'participants')",
    )
    .fetch_all(&mut *tx)
    .await
    .expect("список колонок");

    let has = |table: &str, column: &str| {
        columns
            .iter()
            .any(|(name, field)| name == table && field == column)
    };
    assert!(has("entry_shares", "rubles"), "нет entry_shares.rubles: {columns:?}");
    assert!(
        !has("entry_shares", "weight_quarters"),
        "четверти остались в схеме"
    );
    assert!(has("participants", "paid_by"), "нет participants.paid_by");
}
```

Run: `cd backend && cargo test --test persistence exact_shares_and_payers_are_in_the_schema`
Expected: PASS — миграция применяется при первом подключении тестов. Если FAIL с текстом `в entry_shares есть неполные доли` — бранч `test` не пуст, остановиться и сообщить.

- [ ] **Step 3: Домен — `Entry.fixed`**

`backend/src/domain/types.rs`: удалить `FULL_QUARTERS`, `struct Weight` и `impl Entry` целиком; в `struct Entry` заменить поле `weights` и обновить комментарий к типу:

```rust
/// Расход или перевод. Суммы — целые рубли, всегда больше нуля;
/// это гарантирует слой API.
///
/// Тип один в один повторяет строку таблицы `entries` вместе с её долями,
/// поэтому `recipient_id` заполнен только у перевода, а `fixed` у перевода
/// всегда пусто. Эти инварианты проверяются CHECK-constraint'ами в схеме
/// базы, а не типами: домен получает уже провалидированные строки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: EntryKind,
    pub payer_id: ParticipantId,
    /// Заполнен только у перевода.
    pub recipient_id: Option<ParticipantId>,
    pub amount: i64,
    /// Явные доли расхода: вписанные суммы и исключённые участники (`0`).
    /// Кого здесь нет, тот делит остаток поровну. У перевода всегда пусто.
    pub fixed: Vec<FixedShare>,
}
```

`backend/src/domain/testing.rs`: импорт `use super::types::{Entry, EntryKind, FixedShare, Participant, ParticipantId};`; в `expense()` и `transfer()` заменить `weights: Vec::new()` на `fixed: Vec::new()`; функцию `expense_with_weights` заменить на:

```rust
/// Расход с явными долями: пары `(участник, рубли)`, `0` — исключён.
pub fn expense_with_fixed(payer: Participant, amount: i64, fixed: &[(Participant, i64)]) -> Entry {
    Entry {
        fixed: fixed
            .iter()
            .map(|(participant, rubles)| pin(*participant, *rubles))
            .collect(),
        ..expense(payer, amount)
    }
}
```

`backend/src/domain/shares.rs`: импорт `use super::types::{Entry, FixedShare, Participant, ParticipantId};`; функцию `expense_shares` целиком заменить на:

```rust
/// Доли одного расхода в целых рублях — `split_amount` по сумме записи и её
/// явным долям.
///
/// Гарантия: при неотрицательной сумме и уникальных участниках сумма всех
/// долей ровно равна `entry.amount`. На ней держится инвариант «сумма
/// балансов равна нулю», а на нём — способность плана переводов закрыть
/// встречу в ноль. Пустой список участников даёт пустую карту.
pub fn expense_shares(entry: &Entry, participants: &[Participant]) -> BTreeMap<ParticipantId, i64> {
    split_amount(entry.amount, participants, &entry.fixed)
}
```

В тестах `shares.rs`: импорт `use crate::domain::testing::{expense, expense_with_fixed, participants, pin};`; удалить тесты `respects_half_shares`, `respects_three_quarter_share_with_remainder`, `excluded_participant_never_pays_even_a_remainder_rouble`, `falls_back_to_equal_split_when_everyone_is_excluded` — их случаи покрывают `zero_excludes_from_the_even_split` и `everyone_excluded_splits_evenly_among_all`. В `single_participant_pays_the_whole_expense` заменить вторую половину:

```rust
        // Даже если исключить единственного участника, платить всё равно ему:
        // вписаны одни нули, значит срабатывает правило «делим на всех».
        let excluded = expense_with_fixed(people[0], 101, &[(people[0], 0)]);
        assert_eq!(expense_shares(&excluded, &people)[&people[0].id], 101);
```

`backend/src/domain/mod.rs`, экспорт типов:

```rust
pub use types::{Entry, EntryKind, FixedShare, MeetingFacts, Participant, ParticipantId};
```

- [ ] **Step 4: Property-тесты на точные суммы**

Заменить `backend/src/domain/properties.rs` целиком:

```rust
//! Property-тесты инвариантов домена. Файл содержит только тесты.

use proptest::prelude::*;

use super::testing::{expense_with_fixed, participant, transfer};
use super::types::{Entry, EntryKind, MeetingFacts, Participant};
use super::{expense_shares, fold_into_wallets, net_balances, settlement_plan, wallet_of};

/// Черновик записи в терминах индексов участников. Proptest генерирует любые
/// индексы, а `build` приводит их к существующим по модулю — так любой
/// сгенерированный вход остаётся корректным и ничего не приходится отбрасывать.
#[derive(Debug, Clone)]
enum Draft {
    Expense {
        payer: usize,
        amount: i64,
        /// По участнику: `None` — делит остаток поровну, `Some(0)` —
        /// исключён, `Some(n)` — вписано `n`.
        pins: Vec<Option<i64>>,
    },
    Transfer {
        from: usize,
        to: usize,
        amount: i64,
    },
}

fn draft_strategy() -> impl Strategy<Value = Draft> {
    prop_oneof![
        (
            0usize..64,
            1i64..1_000_000,
            // Чаще вписано, чем пусто: иначе правило пропорции срабатывало бы
            // редко — оно требует, чтобы вписано было у всех.
            prop::collection::vec(prop::option::weighted(0.6, 0i64..1_000_000), 1..8),
        )
            .prop_map(|(payer, amount, pins)| Draft::Expense {
                payer,
                amount,
                pins,
            }),
        (0usize..64, 0usize..64, 1i64..1_000_000).prop_map(|(from, to, amount)| Draft::Transfer {
            from,
            to,
            amount
        }),
    ]
}

/// Ссылки «платит за» по индексам. Редкие: иначе почти все оказывались бы
/// в чужих кошельках и проверять было бы нечего.
fn links_strategy() -> impl Strategy<Value = Vec<Option<usize>>> {
    prop::collection::vec(prop::option::weighted(0.3, 0usize..64), 1..8)
}

fn roster(count: usize, links: &[Option<usize>]) -> Vec<Participant> {
    let plain: Vec<Participant> = (0..count as i32).map(participant).collect();

    plain
        .iter()
        .enumerate()
        .map(|(index, person)| {
            // Ссылку на себя запрещает CHECK в схеме — домен её не увидит.
            // Цепочки и круги не запрещает ничего, кроме API, поэтому домен
            // обязан их пережить.
            let payer = links[index % links.len()]
                .map(|target| target % count)
                .filter(|target| *target != index);
            Participant {
                paid_by: payer.map(|target| plain[target].id),
                ..*person
            }
        })
        .collect()
}

fn build(people: &[Participant], drafts: &[Draft]) -> Vec<Entry> {
    let count = people.len();
    drafts
        .iter()
        .filter_map(|draft| match draft {
            Draft::Expense {
                payer,
                amount,
                pins,
            } => {
                let fixed: Vec<(Participant, i64)> = people
                    .iter()
                    .enumerate()
                    .filter_map(|(index, person)| {
                        pins[index % pins.len()].map(|rubles| (*person, rubles))
                    })
                    .collect();
                Some(expense_with_fixed(people[*payer % count], *amount, &fixed))
            }
            Draft::Transfer { from, to, amount } => {
                let sender = *from % count;
                let recipient = *to % count;
                // Перевод самому себе запрещён на уровне API, домен его не увидит.
                (sender != recipient).then(|| transfer(people[sender], people[recipient], *amount))
            }
        })
        .collect()
}

fn expenses(entries: &[Entry]) -> impl Iterator<Item = &Entry> {
    entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Expense)
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Сумма долей каждого расхода равна самому расходу при любой смеси
    /// «поровну», вписанных сумм и исключённых, и доли не отрицательны.
    #[test]
    fn shares_add_up_to_every_expense(
        count in 1usize..8,
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count, &[None]);
        let entries = build(&people, &drafts);

        for entry in expenses(&entries) {
            let shares = expense_shares(entry, &people);
            prop_assert_eq!(shares.values().sum::<i64>(), entry.amount);
            prop_assert!(shares.values().all(|share| *share >= 0));
        }
    }

    /// Пока кто-то делит остаток и вписано не больше расхода, вписанные платят
    /// ровно вписанное: округление их не касается.
    #[test]
    fn pinned_amounts_are_exact_while_someone_splits_the_rest(
        count in 1usize..8,
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count, &[None]);
        let entries = build(&people, &drafts);

        for entry in expenses(&entries) {
            let pinned: i64 = entry.fixed.iter().map(|share| share.rubles).sum();
            let someone_even = people.iter().any(|person| {
                entry
                    .fixed
                    .iter()
                    .all(|share| share.participant_id != person.id)
            });
            if !someone_even || pinned > entry.amount {
                continue;
            }

            let shares = expense_shares(entry, &people);
            for share in &entry.fixed {
                prop_assert_eq!(shares[&share.participant_id], share.rubles);
            }
        }
    }

    /// Сумма балансов всегда ноль — и до свёртки по кошелькам, и после.
    /// Инвариант обеспечивает целочисленная раздача остатка долей, свёртка
    /// только перекладывает рубли, и именно на нём держится способность плана
    /// закрыть встречу.
    #[test]
    fn balances_sum_to_zero_before_and_after_folding(
        count in 1usize..8,
        links in links_strategy(),
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count, &links);
        let entries = build(&people, &drafts);

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });
        prop_assert_eq!(net.values().sum::<i64>(), 0);

        let folded = fold_into_wallets(&people, &net);
        prop_assert_eq!(folded.values().sum::<i64>(), 0);
    }

    /// У того, за кого действительно платят, после свёртки ноль: в плане
    /// переводов его не будет.
    #[test]
    fn covered_participants_end_at_zero(
        count in 1usize..8,
        links in links_strategy(),
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count, &links);
        let entries = build(&people, &drafts);
        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        let folded = fold_into_wallets(&people, &net);

        for person in &people {
            if wallet_of(person, &people) != person.id {
                prop_assert_eq!(folded[&person.id], 0, "у оплачиваемого остался баланс");
            }
        }
    }

    /// План сводит каждый баланс кошелька в ноль, и переводов в нём строго
    /// меньше, чем участников: это и означает «минимальный», а не «все всем».
    ///
    /// Граница следует из устройства алгоритма: на каждом шаге закрывается хотя
    /// бы одна сторона, а последний шаг закрывает обе, поэтому переводов не
    /// больше `должники + кредиторы − 1`, а это не больше `count − 1`.
    #[test]
    fn settlement_plan_zeroes_every_balance(
        count in 1usize..8,
        links in links_strategy(),
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count, &links);
        let entries = build(&people, &drafts);
        let net = fold_into_wallets(
            &people,
            &net_balances(MeetingFacts {
                participants: &people,
                entries: &entries,
            }),
        );

        let plan = settlement_plan(&people, &net);

        let mut settled = net.clone();
        for item in &plan {
            *settled.get_mut(&item.from).expect("должник есть") += item.amount;
            *settled.get_mut(&item.to).expect("кредитор есть") -= item.amount;
        }
        prop_assert!(
            settled.values().all(|balance| *balance == 0),
            "после плана остались долги: {:?}",
            settled
        );
        prop_assert!(
            plan.len() < count,
            "переводов {} при {} участниках — план не минимальный",
            plan.len(),
            count
        );
    }
}
```

- [ ] **Step 5: Ресторан на уровне расчёта встречи**

В `backend/src/domain/reckoning.rs` в тестах поменять импорт на

```rust
    use crate::domain::testing::{expense, expense_with_fixed, paid_by, participants, transfer};
```

и дописать:

```rust
    /// Шестеро в порядке добавления: Катя, Алексей, Настя, Аня, Женя,
    /// Вероника. Настя — в кошельке Алексея, Аня — в кошельке Жени.
    fn restaurant() -> Vec<Participant> {
        let people = participants(6);
        let (alexey, zhenya) = (people[1], people[4]);
        vec![
            people[0],
            alexey,
            paid_by(people[2], alexey),
            paid_by(people[3], zhenya),
            zhenya,
            people[5],
        ]
    }

    #[test]
    fn restaurant_bill_needs_three_transfers_to_the_payer() {
        let people = restaurant();
        let (katya, alexey, nastya, anya, zhenya, veronika) =
            (people[0], people[1], people[2], people[3], people[4], people[5]);
        let entries = vec![expense_with_fixed(
            zhenya,
            11996,
            &[
                (katya, 2214),
                (alexey, 3440),
                (nastya, 0),
                (anya, 0),
                (zhenya, 3430),
                (veronika, 2906),
            ],
        )];

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(reckoning.net[&zhenya.id], 8564);
        assert_eq!(reckoning.net[&anya.id], 0);
        assert_eq!(reckoning.net[&nastya.id], 0);
        assert_eq!(reckoning.net[&alexey.id], -3442);
        assert_eq!(reckoning.net[&veronika.id], -2907);
        assert_eq!(reckoning.net[&katya.id], -2215);
        assert_eq!(
            reckoning.settlement,
            vec![
                Transfer {
                    from: alexey.id,
                    to: zhenya.id,
                    amount: 3442
                },
                Transfer {
                    from: veronika.id,
                    to: zhenya.id,
                    amount: 2907
                },
                Transfer {
                    from: katya.id,
                    to: zhenya.id,
                    amount: 2215
                },
            ]
        );
        assert_eq!(reckoning.status, MeetingStatus::Alarm(3));
    }

    #[test]
    fn restaurant_plan_does_not_depend_on_how_a_couple_splits_their_part() {
        let people = restaurant();
        let (katya, alexey, nastya, anya, zhenya, veronika) =
            (people[0], people[1], people[2], people[3], people[4], people[5]);
        let entries = vec![expense_with_fixed(
            zhenya,
            11996,
            &[
                (katya, 2214),
                (alexey, 1720),
                (nastya, 1720),
                (anya, 0),
                (zhenya, 3430),
                (veronika, 2906),
            ],
        )];

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(reckoning.net[&alexey.id], -3442);
        assert_eq!(reckoning.net[&nastya.id], 0);
        assert_eq!(reckoning.settlement.len(), 3);
    }
```

Добавить `Participant` в импорт модуля: `use super::types::{EntryKind, MeetingFacts, Participant, ParticipantId};` — если clippy скажет, что в основном коде он не используется, импортировать его только в тестах: `use crate::domain::Participant;` внутри `mod tests`.

Run: `cd backend && cargo test --lib domain`
Expected: компиляция падает в `db/facts.rs` и `api/validate.rs` — это ожидаемо, их чинят следующие шаги.

- [ ] **Step 6: Слой БД**

`backend/src/db/records.rs`, `ShareRow`:

```rust
#[derive(Debug, Clone, Copy, FromRow)]
pub struct ShareRow {
    pub entry_id: Uuid,
    pub participant_id: Uuid,
    /// `0` — участник исключён из расхода, больше нуля — вписанная сумма.
    pub rubles: i64,
}
```

`backend/src/db/entries.rs`:

- комментарий и тип `NewEntry.shares`:

```rust
/// Новая запись вместе с явными долями. Кто делит остаток поровну, в `shares`
/// не передаётся: отсутствие строки и есть «поровну».
#[derive(Debug, Clone)]
pub struct NewEntry {
    pub kind: EntryKindRow,
    pub payer_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: i64,
    pub description: String,
    /// `None` — «сейчас». Явное значение нужно тестам и импорту.
    pub occurred_at: Option<DateTime<Utc>>,
    /// Пары `(участник, рубли)`: вписанные суммы и исключённые (`0`).
    pub shares: Vec<(Uuid, i64)>,
}
```

- в `insert` и в `update` цикл вставки долей:

```rust
    for (participant_id, rubles) in entry.shares {
        sqlx::query(
            "insert into entry_shares (entry_id, participant_id, rubles) \
             values ($1, $2, $3)",
        )
        .bind(row.id)
        .bind(participant_id)
        .bind(rubles)
        .execute(&mut *conn)
        .await?;
    }
```

(в `update` переменная цикла — `shares`, а не `entry.shares`);

- в `shares_for_meeting` и `shares_for_meetings`: `s.weight_quarters` → `s.rubles`;
- `EntryPatch.shares`:

```rust
    /// `None` — доли не трогать. `Some(vec![])` — снять разбивку, то есть
    /// вернуть расход к делению поровну на всех. Переданный список заменяет
    /// прежний целиком, а не дополняет его.
    pub shares: Option<Vec<(Uuid, i64)>>,
```

- новая функция после `shares_for_meeting`:

```rust
/// Доли одной записи — чтобы проверить разбивку, когда правка меняет только
/// сумму расхода.
pub async fn shares_for_entry(
    conn: &mut PgConnection,
    entry_id: Uuid,
) -> Result<Vec<ShareRow>, sqlx::Error> {
    sqlx::query_as(
        "select entry_id, participant_id, rubles from entry_shares where entry_id = $1",
    )
    .bind(entry_id)
    .fetch_all(conn)
    .await
}
```

`backend/src/db/facts.rs`:

```rust
use super::records::{EntryKindRow, EntryRow, ParticipantRow, ShareRow};
use super::{entries, participants};
use crate::domain::{Entry, EntryKind, FixedShare, MeetingFacts, Participant, ParticipantId};
```

в `build` заменить построение участников и долей:

```rust
    let participants = participant_rows.iter().map(participant).collect();

    let mut shares_by_entry: BTreeMap<Uuid, Vec<FixedShare>> = BTreeMap::new();
    for share in share_rows {
        shares_by_entry
            .entry(share.entry_id)
            .or_default()
            .push(FixedShare {
                participant_id: ParticipantId(share.participant_id),
                rubles: share.rubles,
            });
    }
```

в построении `Entry`: `weights: …` → `fixed: shares_by_entry.remove(&row.id).unwrap_or_default(),`; и в конец файла:

```rust
/// Участник в терминах домена. Публичная: тем же отображением пользуется
/// проверка разбивки в слое API.
pub fn participant(row: &ParticipantRow) -> Participant {
    Participant {
        id: ParticipantId(row.id),
        position: row.position,
        // Колонка `paid_by` в строке пока не читается.
        paid_by: None,
    }
}
```

- [ ] **Step 7: API — тело и проверки**

`backend/src/api/validate.rs`: импорты

```rust
use std::collections::BTreeSet;

use uuid::Uuid;

use crate::db::facts;
use crate::db::records::ParticipantRow;
use crate::domain::{FixedShare, ParticipantId, SplitProblem, check_split};

use super::entries::ShareInput;
use super::error::ApiError;
use super::money::format_rubles;
```

функцию `shares` заменить, `split` добавить:

```rust
/// Явные доли, как их прислал клиент, в том виде, в каком они хранятся:
/// пары `(участник, рубли)`. `0` — участник исключён из расхода. Кого в списке
/// нет, тот делит остаток поровну.
pub fn shares(
    raw: &[ShareInput],
    participants: &[ParticipantRow],
) -> Result<Vec<(Uuid, i64)>, ApiError> {
    let mut seen: BTreeSet<Uuid> = BTreeSet::new();
    let mut stored = Vec::with_capacity(raw.len());

    for share in raw {
        if share.rubles < 0 {
            return Err(ApiError::validation(
                "shares",
                "сумма доли не может быть отрицательной",
            ));
        }

        belongs_to_meeting(participants, share.participant_id, "shares")?;

        if !seen.insert(share.participant_id) {
            return Err(ApiError::validation(
                "shares",
                "участник указан в долях дважды",
            ));
        }

        stored.push((share.participant_id, share.rubles));
    }

    Ok(stored)
}

/// Правила ввода разбивки: вписанное не больше расхода, пока кто-то делит
/// остаток, и не дальше четверти от него, если вписано у всех. Сам расчёт
/// примет что угодно — это защита от опечаток.
pub fn split(
    amount: i64,
    participants: &[ParticipantRow],
    shares: &[(Uuid, i64)],
) -> Result<(), ApiError> {
    let people: Vec<_> = participants.iter().map(facts::participant).collect();
    let fixed: Vec<FixedShare> = shares
        .iter()
        .map(|(participant_id, rubles)| FixedShare {
            participant_id: ParticipantId(*participant_id),
            rubles: *rubles,
        })
        .collect();

    check_split(amount, &people, &fixed).map_err(|problem| match problem {
        SplitProblem::PinnedOverAmount { pinned } => ApiError::validation(
            "shares",
            format!("вписано {} — больше, чем потрачено", format_rubles(pinned)),
        ),
        SplitProblem::PinnedFarFromAmount { pinned } => ApiError::validation(
            "shares",
            format!(
                "вписано {} из {} — проверьте суммы",
                format_rubles(pinned),
                format_rubles(amount)
            ),
        ),
    })
}
```

Если `money::format_rubles` не `pub` для соседних модулей, сделать `pub(crate)`.

`backend/src/api/entries.rs`:

```rust
/// Явная доля участника, как её присылает клиент: точная сумма в рублях или
/// `0`, если участник исключён. Кого в списке нет, тот делит остаток поровну.
#[derive(Debug, Clone, Copy, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareInput {
    pub participant_id: Uuid,
    pub rubles: i64,
}
```

в `add_entry` после строки `let shares = check_shares(...)?;`:

```rust
    if body.kind == EntryKindInput::Expense {
        validate::split(amount, &participants, &shares)?;
    }
```

`check_shares` возвращает `Result<Vec<(Uuid, i64)>, ApiError>`;

в `UpdateEntry.shares` комментарий: «`None` — доли не трогать. `Some(vec![])` — снять разбивку, то есть вернуть расход к делению поровну на всех.»;

в `update_entry` после вычисления `shares` (перед `let kind = …`):

```rust
    // Разбивку проверяем по тому, что получится после правки: сумму могли
    // поменять без долей, и прежние доли перестали бы ей соответствовать.
    if !is_transfer && (amount.is_some() || shares.is_some()) {
        let effective = match &shares {
            Some(list) => list.clone(),
            None => db::entries::shares_for_entry(&mut *conn, entry_id)
                .await?
                .iter()
                .map(|row| (row.participant_id, row.rubles))
                .collect(),
        };

        validate::split(
            amount.unwrap_or(current.amount_rubles),
            &participants,
            &effective,
        )?;
    }
```

`backend/src/api/view.rs`:

```rust
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareView {
    pub participant_id: Uuid,
    /// `0` — исключён из расхода, больше нуля — вписанная сумма.
    pub rubles: i64,
}
```

в `group_shares`: `weight_quarters: share.weight_quarters` → `rubles: share.rubles`; комментарий к `EntryView.shared_by_all`:

```rust
    /// `false`, если разбивка задана хоть у кого-то: вписана сумма или участник
    /// исключён. Строки хранятся только у таких участников, поэтому флаг —
    /// это ровно «список долей пуст».
```

Тест `partial_share_clears_the_shared_by_all_flag` заменить:

```rust
    #[test]
    fn fixed_share_clears_the_shared_by_all_flag() {
        // Строка в `entry_shares` есть только у тех, у кого разбивка задана,
        // поэтому любая строка означает «делят не все поровну».
        let meeting_id = Uuid::from_u128(2);
        let (first, second) = (Uuid::from_u128(31), Uuid::from_u128(32));
        let entry_id = Uuid::from_u128(41);
        let meeting = meeting_row(meeting_id);
        let participants = vec![
            participant_row(meeting_id, first, "Настя", 0),
            participant_row(meeting_id, second, "Влад", 1),
        ];
        let entries = vec![expense_row(meeting_id, entry_id, first, 100)];
        let shares = vec![ShareRow {
            entry_id,
            participant_id: second,
            rubles: 30,
        }];

        let view = meeting_view(&meeting, &participants, &entries, &shares, &[]);
        let json = serde_json::to_value(&view).expect("сериализация");

        assert_eq!(json["entries"][0]["sharedByAll"], false);
        assert_eq!(
            json["entries"][0]["shares"],
            serde_json::json!([{ "participantId": second.to_string(), "rubles": 30 }])
        );
        // Второму вписано 30, первый забирает остаток 70; заплатил первый,
        // значит второй должен ему свои 30.
        assert_eq!(json["participants"][1]["netRubles"], -30);
        assert_eq!(json["totals"]["pendingTransfers"], 1);
        assert_eq!(json["status"], "attention");
    }
```

- [ ] **Step 8: Unit-тесты зелёные**

Run: `cd backend && cargo test --lib`
Expected: PASS.

- [ ] **Step 9: Интеграционные тесты — существующие под новую модель**

`backend/tests/api.rs` — добавить хелпер после `seed_dacha`:

```rust
/// Тело расхода с явными долями: пары `(участник, рубли)`, `0` — исключён.
fn expense_body(payer: Uuid, amount: i64, shares: &[(Uuid, i64)]) -> api_entries::CreateEntry {
    api_entries::CreateEntry {
        kind: api_entries::EntryKindInput::Expense,
        payer_id: payer,
        recipient_id: None,
        amount_rubles: amount,
        description: String::new(),
        occurred_at: None,
        shares: shares
            .iter()
            .map(|(participant_id, rubles)| api_entries::ShareInput {
                participant_id: *participant_id,
                rubles: *rubles,
            })
            .collect(),
    }
}
```

Тест `partial_shares_are_stored_without_the_full_ones` заменить:

```rust
#[tokio::test]
async fn fixed_shares_are_stored_as_sent() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let view = api_entries::add_entry(
        &mut tx,
        meeting_id,
        expense_body(people[0], 100, &[(people[1], 20), (people[2], 0)]),
    )
    .await
    .expect("расход с разбивкой");

    assert!(!view.entries[0].shared_by_all);
    assert_eq!(view.entries[0].shares.len(), 2);
    let pinned = view.entries[0]
        .shares
        .iter()
        .find(|share| share.participant_id == people[1])
        .expect("доля второго");
    assert_eq!(pinned.rubles, 20);
    // Второму 20, третий исключён, остаток 80 — первому и четвёртому по 40.
    assert_eq!(view.participants[0].net_rubles, 60);
    assert_eq!(view.participants[1].net_rubles, -20);
    assert_eq!(view.participants[2].net_rubles, 0);
    assert_eq!(view.participants[3].net_rubles, -40);
}
```

В `transfer_with_shares_is_rejected` и `duplicate_share_is_rejected` заменить `weight_quarters: 2` → `rubles: 20`, `weight_quarters: 1` → `rubles: 10`. Тест `share_out_of_range_is_rejected` переименовать в `negative_share_is_rejected`, `.expect_err("доля больше полной")` → `.expect_err("отрицательная доля")`, `weight_quarters: 5` → `rubles: -5`. В `patch_replaces_shares_wholesale`: `weight_quarters: 0` → `rubles: 0`, комментарий к проверке: `// 1000 на трёх, четвёртый исключён → 334 / 333 / 333.`

`backend/tests/persistence.rs`:

- `stores_an_expense_with_partial_shares` → переименовать в `stores_an_expense_with_fixed_shares`; `shares: vec![(other.id, 2)]` → `vec![(other.id, 2100)]`; `assert_eq!(shares[0].weight_quarters, 2)` → `assert_eq!(shares[0].rubles, 2100)`;
- `maps_partial_shares_into_the_domain` заменить:

```rust
#[tokio::test]
async fn maps_fixed_shares_into_the_domain() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Раз", "🐻")
        .await
        .expect("первый");
    let pinned = participants::insert(&mut tx, meeting_id, "Два", "🦊")
        .await
        .expect("второй");
    let excluded = participants::insert(&mut tx, meeting_id, "Три", "🐸")
        .await
        .expect("третий");

    // Второму вписано 30, третий исключён — плательщик забирает остаток 70.
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
            shares: vec![(pinned.id, 30), (excluded.id, 0)],
        },
    )
    .await
    .expect("расход с разбивкой");

    let stored = facts::load(&mut tx, meeting_id)
        .await
        .expect("чтение фактов");
    let reckoning = reckon(stored.as_facts());

    assert_eq!(reckoning.net[&ParticipantId(payer.id)], 30);
    assert_eq!(reckoning.net[&ParticipantId(pinned.id)], -30);
    assert_eq!(reckoning.net[&ParticipantId(excluded.id)], 0);
}
```

- `patches_amount_and_leaves_shares_alone`: `vec![(other.id, 2)]` → `vec![(other.id, 300)]`, `shares[0].weight_quarters, 2` → `shares[0].rubles, 300`;
- `replaces_shares_wholesale_when_given`: `vec![(second.id, 2)]` → `vec![(second.id, 300)]`, `shares[0].weight_quarters, 0` → `shares[0].rubles, 0`;
- `empty_share_list_means_split_equally_again`: `vec![(other.id, 1)]` → `vec![(other.id, 100)]`, комментарий в конце: `// Пустой список — не то же самое, что `None`: он снимает разбивку, то есть возвращает расход к делению поровну.`
- новый тест после `stores_an_expense_with_fixed_shares`:

```rust
#[tokio::test]
async fn schema_rejects_a_negative_share() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let meeting_id = seed_meeting(&mut tx).await;

    let payer = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("плательщик");

    let result = entries::insert(
        &mut tx,
        meeting_id,
        NewEntry {
            kind: EntryKindRow::Expense,
            payer_id: payer.id,
            recipient_id: None,
            amount_rubles: 100,
            description: String::new(),
            occurred_at: None,
            shares: vec![(payer.id, -1)],
        },
    )
    .await;

    // Последняя линия: API отрицательные суммы не пропустит, но и CHECK тоже.
    assert!(result.is_err(), "отрицательная доля записалась");
}
```

- [ ] **Step 10: Новые интеграционные тесты API (падают)**

В `backend/tests/api.rs` дописать:

```rust
#[tokio::test]
async fn pins_over_the_amount_are_rejected_while_someone_splits_the_rest() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        expense_body(people[0], 1000, &[(people[1], 1200)]),
    )
    .await
    .expect_err("вписано больше расхода");

    assert_eq!(validation_field(&error), "shares");
}

#[tokio::test]
async fn pins_far_from_the_amount_are_rejected() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    // Вписано у всех 1 600 при расходе 1 000 — расхождение больше четверти.
    let error = api_entries::add_entry(
        &mut tx,
        meeting_id,
        expense_body(
            people[0],
            1000,
            &[(people[0], 400), (people[1], 400), (people[2], 400), (people[3], 400)],
        ),
    )
    .await
    .expect_err("расхождение больше четверти");

    assert_eq!(validation_field(&error), "shares");
}

#[tokio::test]
async fn shortfall_within_a_quarter_is_spread_in_proportion() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let view = api_entries::add_entry(
        &mut tx,
        meeting_id,
        expense_body(
            people[0],
            1000,
            &[(people[0], 250), (people[1], 250), (people[2], 250), (people[3], 240)],
        ),
    )
    .await
    .expect("расхождение в 10 ₽ допустимо");

    // Доли 253 / 253 / 252 / 242: два рубля остатка — первым двум по position.
    assert_eq!(view.participants[0].net_rubles, 747);
    assert_eq!(view.participants[1].net_rubles, -253);
    assert_eq!(view.participants[2].net_rubles, -252);
    assert_eq!(view.participants[3].net_rubles, -242);
}

#[tokio::test]
async fn patching_only_the_amount_checks_the_stored_shares() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    let entry_id = db::entries::insert(
        &mut tx,
        meeting_id,
        db::entries::NewEntry {
            kind: db::records::EntryKindRow::Expense,
            payer_id: people[0],
            recipient_id: None,
            amount_rubles: 400,
            description: "Продукты".to_owned(),
            occurred_at: None,
            shares: vec![(people[1], 300)],
        },
    )
    .await
    .expect("вставка расхода")
    .id;

    // Второму вписано 300; сумма 200 сделала бы вписанное больше расхода.
    let error = api_entries::update_entry(
        &mut tx,
        meeting_id,
        entry_id,
        api_entries::UpdateEntry {
            payer_id: None,
            recipient_id: None,
            amount_rubles: Some(200),
            description: None,
            occurred_at: None,
            shares: None,
        },
    )
    .await
    .expect_err("сумма меньше вписанного");

    assert_eq!(validation_field(&error), "shares");
}
```

Run: `cd backend && cargo test --test api && cargo test --test persistence`
Expected: PASS. Если какой-то тест падает с `PoolTimedOut` / `ConnectionReset` — это Neon, перезапустить (README, раздел про тесты).

- [ ] **Step 11: fmt, clippy, commit**

Run: `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib`
Expected: PASS, без предупреждений.

```bash
git add backend
git commit -m "feat: store expense shares as exact roubles instead of quarters

Migration 0002 turns entry_shares.weight_quarters into rubles (no row means
an even share of the rest, 0 means excluded) and adds participants.paid_by
for the next step. It refuses to run if any partial quarter exists rather
than silently changing someone's money.

The API rejects pins over the amount while someone splits the rest, and
pins more than a quarter off the amount when everyone is pinned. A patch
that changes only the amount is checked against the stored shares.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 4: Бэкенд — «платит за» в базе и API

**Files:**
- Modify: `backend/src/db/records.rs`, `participants.rs`, `facts.rs`
- Modify: `backend/src/api/validate.rs`, `texts.rs`, `participants.rs`, `view.rs`
- Modify: `backend/tests/api.rs`, `backend/tests/persistence.rs`

- [ ] **Step 1: Строка участника и запросы**

`backend/src/db/records.rs`, в `ParticipantRow` после `position`:

```rust
    /// Кто платит за участника; `None` — платит сам.
    pub paid_by: Option<Uuid>,
```

`backend/src/db/participants.rs`: во всех четырёх запросах список колонок `id, meeting_id, name, emoji, color_index, position, created_at` заменить на `id, meeting_id, name, emoji, color_index, position, paid_by, created_at` (в `insert` — в `returning`, в `list_for_meeting` и `list_for_meetings` — в `select`, в `update` — в `returning`). Комментарий к `delete` дополнить:

```rust
/// Каскады уносят все записи, где участник плательщик или получатель, и его
/// доли — это требование дизайна, а не побочный эффект. Тех, за кого он
/// платил, база отвязывает сама (`on delete set null`): они снова платят сами
/// за себя.
```

После `update` добавить:

```rust
/// Назначает или снимает плательщика. Отдельно от `update`, потому что имя и
/// эмодзи перезаписываются всегда, а плательщик — только когда его поменяли.
pub async fn set_paid_by(
    conn: &mut PgConnection,
    id: Uuid,
    paid_by: Option<Uuid>,
) -> Result<Option<ParticipantRow>, sqlx::Error> {
    sqlx::query_as(
        "update participants set paid_by = $2 where id = $1 \
         returning id, meeting_id, name, emoji, color_index, position, paid_by, created_at",
    )
    .bind(id)
    .bind(paid_by)
    .fetch_optional(conn)
    .await
}
```

`backend/src/db/facts.rs`, в `participant()`:

```rust
pub fn participant(row: &ParticipantRow) -> Participant {
    Participant {
        id: ParticipantId(row.id),
        position: row.position,
        paid_by: row.paid_by.map(ParticipantId),
    }
}
```

`backend/src/api/view.rs`, в тестовом `participant_row` добавить `paid_by: None,`.

Run: `cd backend && cargo test --lib`
Expected: PASS.

- [ ] **Step 2: Тесты слоя БД (падающие)**

В `backend/tests/persistence.rs` после `updates_and_deletes_a_participant`:

```rust
#[tokio::test]
async fn stores_a_payer_and_forgets_it_when_the_payer_leaves() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let meeting_id = seed_meeting(&mut tx).await;

    let anya = participants::insert(&mut tx, meeting_id, "Аня", "🧞")
        .await
        .expect("Аня");
    let zhenya = participants::insert(&mut tx, meeting_id, "Женя", "🐨")
        .await
        .expect("Женя");

    let covered = participants::set_paid_by(&mut tx, anya.id, Some(zhenya.id))
        .await
        .expect("назначение")
        .expect("участник существует");
    assert_eq!(covered.paid_by, Some(zhenya.id));

    participants::delete(&mut tx, zhenya.id)
        .await
        .expect("удаление");

    let listed = participants::list_for_meeting(&mut tx, meeting_id)
        .await
        .expect("список");
    assert_eq!(listed.len(), 1);
    assert_eq!(listed[0].paid_by, None, "Аня должна снова платить сама");
}

#[tokio::test]
async fn schema_rejects_paying_for_oneself() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let meeting_id = seed_meeting(&mut tx).await;

    let person = participants::insert(&mut tx, meeting_id, "Настя", "🦊")
        .await
        .expect("участник");

    let result = participants::set_paid_by(&mut tx, person.id, Some(person.id)).await;

    assert!(result.is_err(), "ссылка на себя записалась");
}
```

Run: `cd backend && cargo test --test persistence payer paying_for_oneself`
Expected: PASS — слой БД готов с шага 1. Если FAIL — сверить список колонок в запросах.

- [ ] **Step 3: Тексты лога (падающий тест)**

В `backend/src/api/texts.rs` в модуль `tests` добавить:

```rust
    #[test]
    fn payer_lines_do_not_mark_gender() {
        assert_eq!(payer_assigned("Аня", "Женя"), "Аня: теперь платит Женя");
        assert_eq!(payer_removed("Аня"), "Аня: снова платит за себя");
    }
```

Run: `cd backend && cargo test --lib api::texts`
Expected: ошибка компиляции `cannot find function payer_assigned`.

После `participant_deleted` добавить:

```rust
pub fn payer_assigned(name: &str, payer: &str) -> String {
    format!("{name}: теперь платит {payer}")
}

pub fn payer_removed(name: &str) -> String {
    format!("{name}: снова платит за себя")
}
```

Run: `cd backend && cargo test --lib api::texts`
Expected: PASS.

- [ ] **Step 4: Проверка плательщика**

В `backend/src/api/validate.rs` после `belongs_to_meeting`:

```rust
/// Плательщик за участника. `participant_id` — `None`, пока участника ещё нет
/// (его добавляют сразу с плательщиком). Цепочек не бывает: у плательщика
/// нет своего плательщика, а у того, за кого платят, нет своих оплачиваемых.
pub fn paid_by(
    participants: &[ParticipantRow],
    participant_id: Option<Uuid>,
    payer_id: Uuid,
) -> Result<Uuid, ApiError> {
    let payer = belongs_to_meeting(participants, payer_id, "paidById")?;

    if Some(payer.id) == participant_id {
        return Err(ApiError::validation(
            "paidById",
            "нельзя выбрать плательщиком самого участника",
        ));
    }

    if payer.paid_by.is_some() {
        return Err(ApiError::validation(
            "paidById",
            "за выбранного плательщика уже платит другой участник",
        ));
    }

    if let Some(id) = participant_id
        && participants.iter().any(|row| row.paid_by == Some(id))
    {
        return Err(ApiError::validation(
            "paidById",
            "участник уже платит за других — сначала снимите эту связь",
        ));
    }

    Ok(payer.id)
}
```

- [ ] **Step 5: Тело запросов участника**

`backend/src/api/participants.rs`: импорт `use serde::{Deserialize, Deserializer};`.

```rust
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateParticipant {
    pub name: String,
    pub emoji: Option<String>,
    /// Кто платит за нового участника; нет поля или `null` — платит сам.
    #[serde(default)]
    pub paid_by_id: Option<Uuid>,
}
```

```rust
/// `None` в поле означает «не менять». Позиция и цвет не меняются никогда:
/// они закреплены за участником с момента добавления.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateParticipant {
    pub name: Option<String>,
    pub emoji: Option<String>,
    /// `None` — не менять, `Some(None)` — снять плательщика,
    /// `Some(Some(id))` — назначить.
    #[serde(default, deserialize_with = "present")]
    pub paid_by_id: Option<Option<Uuid>>,
}

/// Отличает отсутствующее поле от `null`. Без этого serde схлопывает оба
/// случая в `None`, и снять плательщика было бы нельзя.
fn present<'de, D>(deserializer: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Uuid>::deserialize(deserializer).map(Some)
}
```

В конец файла — unit-тест разбора:

```rust
#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_null_and_id_payer_are_three_different_things() {
        let id = Uuid::from_u128(7);

        let missing: UpdateParticipant = serde_json::from_str("{}").expect("пустое тело");
        let null: UpdateParticipant =
            serde_json::from_str(r#"{"paidById":null}"#).expect("null");
        let set: UpdateParticipant =
            serde_json::from_str(&format!(r#"{{"paidById":"{id}"}}"#)).expect("id");

        assert_eq!(missing.paid_by_id, None);
        assert_eq!(null.paid_by_id, Some(None));
        assert_eq!(set.paid_by_id, Some(Some(id)));
    }
}
```

Run: `cd backend && cargo test --lib api::participants`
Expected: PASS.

- [ ] **Step 6: Ручки участника**

В `backend/src/api/participants.rs`:

`require_participant` возвращает и строку, и весь список — правке нужны остальные участники для проверки плательщика:

```rust
/// Участник, принадлежащий этой встрече, и весь её состав. Чужой — `404`,
/// а не `422`: по этому адресу его не существует, и различать «нет» и «есть,
/// но не здесь» клиенту незачем.
async fn require_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    participant_id: Uuid,
) -> Result<(db::records::ParticipantRow, Vec<db::records::ParticipantRow>), ApiError> {
    meetings::require_meeting(&mut *conn, meeting_id).await?;

    let participants = db::participants::list_for_meeting(&mut *conn, meeting_id).await?;
    let current = participants
        .iter()
        .find(|row| row.id == participant_id)
        .cloned()
        .ok_or(ApiError::NotFound)?;

    Ok((current, participants))
}
```

`remove_participant`: `let (current, _) = require_participant(...).await?;`.

`add_participant` целиком:

```rust
pub async fn add_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    body: CreateParticipant,
) -> Result<MeetingView, ApiError> {
    meetings::require_meeting(&mut *conn, meeting_id).await?;
    let participants = db::participants::list_for_meeting(&mut *conn, meeting_id).await?;

    let name = validate::participant_name(&body.name)?;
    let emoji = validate::emoji_or_default(body.emoji.as_deref(), texts::DEFAULT_PARTICIPANT_EMOJI);
    let paid_by = match body.paid_by_id {
        Some(payer_id) => Some(validate::paid_by(&participants, None, payer_id)?),
        None => None,
    };

    let participant = db::participants::insert(&mut *conn, meeting_id, &name, &emoji).await?;
    db::log::append(
        &mut *conn,
        meeting_id,
        &texts::participant_joined(&participant.name),
    )
    .await?;

    if let Some(payer_id) = paid_by {
        db::participants::set_paid_by(&mut *conn, participant.id, Some(payer_id)).await?;
        db::log::append(
            &mut *conn,
            meeting_id,
            &texts::payer_assigned(&participant.name, name_of(&participants, payer_id)),
        )
        .await?;
    }

    meetings::load_view(conn, meeting_id).await
}

/// Имя участника из уже прочитанного состава встречи. Идентификатор к этому
/// моменту проверен, поэтому промах — ошибка в коде, а не во вводе.
fn name_of(participants: &[db::records::ParticipantRow], id: Uuid) -> &str {
    participants
        .iter()
        .find(|row| row.id == id)
        .map(|row| row.name.as_str())
        .expect("плательщик проверен валидацией")
}
```

`update_participant` целиком:

```rust
pub async fn update_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    participant_id: Uuid,
    body: UpdateParticipant,
) -> Result<MeetingView, ApiError> {
    let (current, participants) =
        require_participant(&mut *conn, meeting_id, participant_id).await?;

    // Запрос в слое данных требует оба значения: позиция и цвет остаются, а имя
    // с эмодзи перезаписываются целиком. Незаданное поле берём из текущей
    // строки — так частичная правка получается без второго запроса.
    let name = match body.name.as_deref() {
        Some(raw) => validate::participant_name(raw)?,
        None => current.name.clone(),
    };
    let emoji = validate::emoji_or_default(body.emoji.as_deref(), &current.emoji);
    let paid_by = match body.paid_by_id {
        None => current.paid_by,
        Some(None) => None,
        Some(Some(payer_id)) => Some(validate::paid_by(
            &participants,
            Some(current.id),
            payer_id,
        )?),
    };

    let updated = db::participants::update(&mut *conn, participant_id, &name, &emoji)
        .await?
        .ok_or(ApiError::NotFound)?;

    let profile_changed = updated.name != current.name || updated.emoji != current.emoji;
    let payer_changed = paid_by != current.paid_by;

    if payer_changed {
        db::participants::set_paid_by(&mut *conn, participant_id, paid_by).await?;
    }

    // Правка без изменений тоже пишет строку профиля — так было и до
    // плательщиков, и клиент на это не рассчитывает иначе.
    if profile_changed || !payer_changed {
        db::log::append(
            &mut *conn,
            meeting_id,
            &texts::participant_updated(&updated.name),
        )
        .await?;
    }

    if payer_changed {
        let line = match paid_by {
            Some(payer_id) => texts::payer_assigned(&updated.name, name_of(&participants, payer_id)),
            None => texts::payer_removed(&updated.name),
        };
        db::log::append(&mut *conn, meeting_id, &line).await?;
    }

    meetings::load_view(conn, meeting_id).await
}
```

- [ ] **Step 7: Ответ — `paidById`**

`backend/src/api/view.rs`, в `ParticipantView` после `position`:

```rust
    /// Кто платит за участника; `null` — платит сам.
    pub paid_by_id: Option<Uuid>,
```

комментарий к `net_rubles`:

```rust
    /// Баланс кошелька: плюс — должны ему, минус — должен он. У того, за кого
    /// платят, всегда 0: его баланс прибавлен к балансу плательщика.
```

в `meeting_view` при построении `ParticipantView`: `paid_by_id: row.paid_by,`.

Тест в модуль `tests` view.rs:

```rust
    #[test]
    fn covered_balance_is_folded_into_the_payer() {
        // Женя платит за Аню. Расход Жени 300 на троих — по 100.
        let meeting_id = Uuid::from_u128(5);
        let (zhenya, anya, katya) = (Uuid::from_u128(71), Uuid::from_u128(72), Uuid::from_u128(73));
        let meeting = meeting_row(meeting_id);
        let participants = vec![
            participant_row(meeting_id, zhenya, "Женя", 0),
            ParticipantRow {
                paid_by: Some(zhenya),
                ..participant_row(meeting_id, anya, "Аня", 1)
            },
            participant_row(meeting_id, katya, "Катя", 2),
        ];
        let entries = vec![expense_row(meeting_id, Uuid::from_u128(81), zhenya, 300)];

        let view = meeting_view(&meeting, &participants, &entries, &[], &[]);
        let json = serde_json::to_value(&view).expect("сериализация");

        assert_eq!(json["participants"][1]["paidById"], anya_payer(zhenya));
        assert_eq!(json["participants"][0]["paidById"], serde_json::Value::Null);
        assert_eq!(json["participants"][0]["netRubles"], 100);
        assert_eq!(json["participants"][1]["netRubles"], 0);
        assert_eq!(
            json["settlement"],
            serde_json::json!([
                { "fromId": katya.to_string(), "toId": zhenya.to_string(), "amountRubles": 100 },
            ])
        );
    }

    fn anya_payer(payer: Uuid) -> serde_json::Value {
        serde_json::Value::String(payer.to_string())
    }
```

Run: `cd backend && cargo test --lib`
Expected: PASS.

- [ ] **Step 8: Интеграционные тесты (существующие литералы)**

В `backend/tests/api.rs` во все литералы `participants::CreateParticipant { … }` (4 места) добавить `paid_by_id: None,`, во все `participants::UpdateParticipant { … }` (2 места) — `paid_by_id: None,`.

Run: `cd backend && cargo test --test api`
Expected: PASS.

- [ ] **Step 9: Новые интеграционные тесты**

В `backend/tests/api.rs` дописать:

```rust
/// Назначает или снимает плательщика через ту же ручку, что и клиент.
async fn set_payer(
    conn: &mut sqlx::PgConnection,
    meeting_id: Uuid,
    participant: Uuid,
    payer: Option<Uuid>,
) -> Result<backend::api::view::MeetingView, ApiError> {
    participants::update_participant(
        conn,
        meeting_id,
        participant,
        participants::UpdateParticipant {
            name: None,
            emoji: None,
            paid_by_id: Some(payer),
        },
    )
    .await
}

/// Ресторан из спеки `2026-10-03-exact-split-design.md`: шестеро в порядке
/// добавления — Катя, Алексей, Настя, Аня, Женя, Вероника.
async fn seed_restaurant(conn: &mut sqlx::PgConnection) -> (Uuid, Vec<Uuid>) {
    let meeting = db::meetings::insert(
        &mut *conn,
        NewMeeting {
            title: "Ресторан".to_owned(),
            description: String::new(),
            emoji: "🍽️".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 10, 2).expect("дата"),
        },
    )
    .await
    .expect("вставка встречи");

    let mut people = Vec::new();
    for name in ["Катя", "Алексей", "Настя", "Аня", "Женя", "Вероника"] {
        let row = db::participants::insert(&mut *conn, meeting.id, name, "🦊")
            .await
            .expect("вставка участника");
        people.push(row.id);
    }

    (meeting.id, people)
}

#[tokio::test]
async fn restaurant_bill_settles_on_the_payer() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_restaurant(&mut tx).await;
    let (katya, alexey, nastya, anya, zhenya, veronika) =
        (people[0], people[1], people[2], people[3], people[4], people[5]);

    set_payer(&mut tx, meeting_id, nastya, Some(alexey))
        .await
        .expect("Настя → Алексей");
    set_payer(&mut tx, meeting_id, anya, Some(zhenya))
        .await
        .expect("Аня → Женя");

    let view = api_entries::add_entry(
        &mut tx,
        meeting_id,
        expense_body(
            zhenya,
            11996,
            &[
                (katya, 2214),
                (alexey, 3440),
                (nastya, 0),
                (anya, 0),
                (zhenya, 3430),
                (veronika, 2906),
            ],
        ),
    )
    .await
    .expect("счёт за ресторан");

    let net = |id: Uuid| {
        view.participants
            .iter()
            .find(|person| person.id == id)
            .expect("участник")
            .net_rubles
    };
    assert_eq!(net(zhenya), 8564);
    assert_eq!(net(nastya), 0);
    assert_eq!(net(anya), 0);

    let plan: Vec<(Uuid, Uuid, i64)> = view
        .settlement
        .iter()
        .map(|transfer| (transfer.from_id, transfer.to_id, transfer.amount_rubles))
        .collect();
    assert_eq!(
        plan,
        vec![(alexey, zhenya, 3442), (veronika, zhenya, 2907), (katya, zhenya, 2215)]
    );
    assert_eq!(view.participants[2].paid_by_id, Some(alexey));
}

#[tokio::test]
async fn participant_can_join_with_a_payer() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let view = participants::add_participant(
        &mut tx,
        meeting_id,
        participants::CreateParticipant {
            name: "Аня".to_owned(),
            emoji: None,
            paid_by_id: Some(people[1]),
        },
    )
    .await
    .expect("добавление с плательщиком");

    assert_eq!(view.participants[4].paid_by_id, Some(people[1]));
    // Лог новыми сверху: сначала строка о плательщике, под ней — о добавлении.
    assert_eq!(view.log[0].text, "Аня: теперь платит Влад");
    assert_eq!(view.log[1].text, "Аня присоединяется к встрече");
}

#[tokio::test]
async fn null_payer_means_paying_for_oneself_again() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    set_payer(&mut tx, meeting_id, people[0], Some(people[1]))
        .await
        .expect("назначение");
    let view = set_payer(&mut tx, meeting_id, people[0], None)
        .await
        .expect("снятие");

    assert_eq!(view.participants[0].paid_by_id, None);
    assert_eq!(view.log[0].text, "Настя: снова платит за себя");
}

#[tokio::test]
async fn renaming_keeps_the_payer() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    set_payer(&mut tx, meeting_id, people[0], Some(people[1]))
        .await
        .expect("назначение");
    let view = participants::update_participant(
        &mut tx,
        meeting_id,
        people[0],
        participants::UpdateParticipant {
            name: Some("Анастасия".to_owned()),
            emoji: None,
            paid_by_id: None,
        },
    )
    .await
    .expect("переименование");

    assert_eq!(view.participants[0].paid_by_id, Some(people[1]));
    assert_eq!(view.log[0].text, "Профиль участника обновлён: Анастасия");
}

#[tokio::test]
async fn participant_cannot_pay_for_oneself() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    let error = set_payer(&mut tx, meeting_id, people[0], Some(people[0]))
        .await
        .expect_err("сам себе плательщик");

    assert_eq!(validation_field(&error), "paidById");
}

#[tokio::test]
async fn payer_from_another_meeting_is_rejected() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;
    let (_, strangers) = seed_dacha(&mut tx).await;

    let error = set_payer(&mut tx, meeting_id, people[0], Some(strangers[0]))
        .await
        .expect_err("плательщик из другой встречи");

    assert_eq!(validation_field(&error), "paidById");
}

#[tokio::test]
async fn covered_participant_cannot_pay_for_others() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    set_payer(&mut tx, meeting_id, people[0], Some(people[1]))
        .await
        .expect("Настя → Влад");
    let error = set_payer(&mut tx, meeting_id, people[2], Some(people[0]))
        .await
        .expect_err("Егор → Настя дал бы цепочку");

    assert_eq!(validation_field(&error), "paidById");
}

#[tokio::test]
async fn payer_of_others_cannot_get_a_payer() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    set_payer(&mut tx, meeting_id, people[0], Some(people[1]))
        .await
        .expect("Настя → Влад");
    let error = set_payer(&mut tx, meeting_id, people[1], Some(people[2]))
        .await
        .expect_err("Влад → Егор дал бы цепочку");

    assert_eq!(validation_field(&error), "paidById");
}

#[tokio::test]
async fn removing_the_payer_frees_the_covered() {
    let pool = test_pool().await;
    let mut tx = support::begin(&pool).await;
    let (meeting_id, people) = seed_dacha(&mut tx).await;

    set_payer(&mut tx, meeting_id, people[0], Some(people[1]))
        .await
        .expect("Настя → Влад");
    let view = participants::remove_participant(&mut tx, meeting_id, people[1])
        .await
        .expect("удаление Влада");

    assert_eq!(view.participants[0].paid_by_id, None);
}
```

Run: `cd backend && cargo test --test api && cargo test --test persistence && cargo test --test http`
Expected: PASS.

- [ ] **Step 10: fmt, clippy, commit**

Run: `cd backend && cargo fmt && cargo clippy --all-targets -- -D warnings && cargo test --lib`
Expected: PASS, без предупреждений.

```bash
git add backend
git commit -m "feat(api): let a participant be paid for by another

paidById on create and update (null on update clears it, a missing field
leaves it alone) is checked to be someone in the same meeting, not the
participant, and not part of a chain. The response carries paidById, and
netRubles is now the wallet balance. Removing the payer frees the covered
participant through the schema.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 5: Фронт — разбор выражений и кошельки

Аддитивно: модалки пока не трогаем.

**Files:**
- Create: `frontend/src/domain/amountExpression.ts`, `amountExpression.test.ts`
- Create: `frontend/src/domain/wallets.ts`, `wallets.test.ts`
- Modify: `frontend/src/domain/format.ts`, `format.test.ts`
- Modify: `frontend/src/api/types.ts`

- [ ] **Step 1: Падающие тесты выражений**

Создать `frontend/src/domain/amountExpression.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import { parseAmountExpression } from './amountExpression'

describe('parseAmountExpression', () => {
  it('складывает позиции чека', () => {
    expect(parseAmountExpression('390 + 1200 + 624')).toEqual({
      rubles: 2214,
      rounded: false,
      compound: true,
      invalid: false,
    })
  })

  it('понимает разряды через пробел и запятую в дробной части', () => {
    expect(parseAmountExpression('1 250,50 + 99,50')).toEqual({
      rubles: 1350,
      rounded: false,
      compound: true,
      invalid: false,
    })
  })

  it('не теряет копейки на плавающей точке', () => {
    // В float 0.1 + 0.2 + 0.7 — это 0.9999999999999999, и округление вниз дало бы 0.
    expect(parseAmountExpression('0,1 + 0,2 + 0,7').rubles).toBe(1)
  })

  it('округляет вниз один раз — итог', () => {
    expect(parseAmountExpression('0,6 + 0,6')).toEqual({
      rubles: 1,
      rounded: true,
      compound: true,
      invalid: false,
    })
  })

  it('одно число — обычная сумма', () => {
    expect(parseAmountExpression('11996')).toEqual({
      rubles: 11996,
      rounded: false,
      compound: false,
      invalid: false,
    })
  })

  it('висящий плюс во время набора — ещё не ошибка', () => {
    expect(parseAmountExpression('390 + ')).toEqual({
      rubles: 390,
      rounded: false,
      compound: false,
      invalid: false,
    })
  })

  it('пустое поле — пусто, а не ноль', () => {
    for (const raw of ['', '   ', '+']) {
      const parsed = parseAmountExpression(raw)

      expect(Number.isNaN(parsed.rubles)).toBe(true)
      expect(parsed.invalid).toBe(false)
    }
  })

  it('мусор делает поле невалидным', () => {
    for (const raw of ['390 + абв', '-5', '12,5,3', '1e3']) {
      const parsed = parseAmountExpression(raw)

      expect(Number.isNaN(parsed.rubles)).toBe(true)
      expect(parsed.invalid).toBe(true)
    }
  })
})
```

Run: `cd frontend && npx vitest run src/domain/amountExpression.test.ts`
Expected: FAIL — `Failed to resolve import "./amountExpression"`.

- [ ] **Step 2: Реализация**

Создать `frontend/src/domain/amountExpression.ts`:

```ts
/**
 * Разбор поля суммы, в котором можно складывать: «390 + 1200 + 624».
 *
 * Счёт в ресторане приходит по позициям, и складывать их в уме перед вводом —
 * ровно та работа, которую приложение должно забрать себе.
 *
 * Пробелы допускаются где угодно, десятичный разделитель — запятая или точка.
 * Считаем в копейках и округляем вниз один раз — итог, а не каждую позицию:
 * «99,50 + 0,50» — это ровно 100, а не 99.
 */

export interface ParsedAmount {
  /** Итог в целых рублях. `NaN`, если поле пустое или в нём не число. */
  rubles: number
  /** У итога была дробная часть, и она отброшена. */
  rounded: boolean
  /** Слагаемых больше одного — итог стоит показать под полем. */
  compound: boolean
  /** В поле есть что-то кроме чисел и плюсов. Пустое поле невалидным не считается. */
  invalid: boolean
}

/** Число без знака: «1250», «1250,5», «0.99». */
const TERM = /^\d+(?:[.,]\d+)?$/

const EMPTY: ParsedAmount = { rubles: NaN, rounded: false, compound: false, invalid: false }

export function parseAmountExpression(raw: string): ParsedAmount {
  // Пустые слагаемые — это висящий плюс во время набора («390 +»), а не ошибка.
  const terms = raw
    .split('+')
    .map((term) => term.replace(/\s/g, ''))
    .filter((term) => term !== '')

  if (terms.length === 0) return EMPTY

  const compound = terms.length > 1

  if (!terms.every((term) => TERM.test(term))) {
    return { rubles: NaN, rounded: false, compound, invalid: true }
  }

  const kopecks = terms.reduce((sum, term) => sum + toKopecks(term), 0)

  return {
    rubles: Math.floor(kopecks / 100),
    rounded: kopecks % 100 !== 0,
    compound,
    invalid: false,
  }
}

/** «1250,5» → 125050. Цифры после второй дробной — меньше копейки, их отбрасываем. */
function toKopecks(term: string): number {
  const [whole, fraction = ''] = term.replace(',', '.').split('.')

  return Number(whole) * 100 + Number(fraction.slice(0, 2).padEnd(2, '0'))
}
```

Run: `cd frontend && npx vitest run src/domain/amountExpression.test.ts`
Expected: PASS.

- [ ] **Step 3: `parseAmount` — обёртка**

В `frontend/src/domain/format.ts` заменить комментарий и тело `parseAmount`:

```ts
import { parseAmountExpression } from './amountExpression'

const RUBLES = new Intl.NumberFormat('ru-RU', { maximumFractionDigits: 0 })

/**
 * Разбор введённой суммы — `parseAmountExpression` без подсказок про позиции.
 * Плюс работает и здесь: складывать при вводе перевода так же удобно, как
 * при вводе расхода.
 *
 * Копеек в приложении нет, поэтому дробная часть отбрасывается, а `rounded`
 * позволяет сказать об этом вслух вместо того, чтобы молча потерять половину
 * суммы. `rubles` — `NaN`, если ввод не число: вызывающий проверяет через
 * `Number.isFinite`.
 */
export function parseAmount(raw: string): { rubles: number; rounded: boolean } {
  const { rubles, rounded } = parseAmountExpression(raw)

  return { rubles, rounded }
}
```

(строка с `RUBLES` уже есть — оставить её одну, импорт поставить над ней).

В `frontend/src/domain/format.test.ts` добавить `parseAmount` в импорт и тест:

```ts
describe('parseAmount', () => {
  it('отбрасывает копейки и говорит об этом', () => {
    expect(parseAmount('1 250,50')).toEqual({ rubles: 1250, rounded: true })
    expect(parseAmount('3425')).toEqual({ rubles: 3425, rounded: false })
  })

  it('пустое поле — не число', () => {
    expect(Number.isNaN(parseAmount('').rubles)).toBe(true)
  })
})
```

Run: `cd frontend && npx vitest run src/domain/format.test.ts`
Expected: PASS.

- [ ] **Step 4: Тип `paidById`**

В `frontend/src/api/types.ts` заменить `interface Participant`:

```ts
export interface Participant extends ParticipantChip {
  position: number
  /** «внёс N ₽»: только оплаченные расходы, отправленные переводы не в счёт. */
  contributedRubles: number
  /** Баланс кошелька: плюс — должны ему, минус — должен он. У того, за кого
   *  платят, всегда 0: его баланс прибавлен к балансу плательщика. */
  netRubles: number
  /** Кто платит за участника; `null` — платит сам. */
  paidById: string | null
}
```

- [ ] **Step 5: Падающие тесты кошельков**

Создать `frontend/src/domain/wallets.test.ts`:

```ts
import { describe, expect, it } from 'vitest'
import type { Participant } from '../api/types'
import { coveredBy, payerOf, walletName } from './wallets'

function person(id: string, paidById: string | null = null): Participant {
  return {
    id,
    name: id,
    emoji: '🦊',
    colorIndex: 0,
    position: 0,
    contributedRubles: 0,
    netRubles: 0,
    paidById,
  }
}

describe('payerOf', () => {
  it('у того, кто платит сам, плательщика нет', () => {
    const people = [person('Женя')]

    expect(payerOf(people[0], people)).toBeUndefined()
  })

  it('находит плательщика', () => {
    const people = [person('Аня', 'Женя'), person('Женя')]

    expect(payerOf(people[0], people)?.id).toBe('Женя')
  })

  it('связь с тем, за кого самого платят, не действует — как на сервере', () => {
    const people = [person('А', 'Б'), person('Б', 'В'), person('В')]

    expect(payerOf(people[0], people)).toBeUndefined()
    expect(payerOf(people[1], people)?.id).toBe('В')
  })

  it('связь с посторонним не действует', () => {
    const people = [person('Аня', 'кто-то')]

    expect(payerOf(people[0], people)).toBeUndefined()
  })
})

describe('coveredBy и walletName', () => {
  const people = [person('Женя'), person('Аня', 'Женя'), person('Катя'), person('Настя', 'Женя')]

  it('перечисляет оплачиваемых в порядке встречи', () => {
    expect(coveredBy(people[0], people).map((other) => other.id)).toEqual(['Аня', 'Настя'])
  })

  it('подписывает кошелёк', () => {
    expect(walletName(people[0], people)).toBe('Женя + Аня, Настя')
    expect(walletName(people[2], people)).toBe('Катя')
  })
})
```

Run: `cd frontend && npx vitest run src/domain/wallets.test.ts`
Expected: FAIL — `Failed to resolve import "./wallets"`.

- [ ] **Step 6: Реализация**

Создать `frontend/src/domain/wallets.ts`:

```ts
import type { Participant } from '../api/types'

/**
 * Кто фактически платит за участника.
 *
 * Повторяет `wallet_of` из `backend/src/domain/wallets.rs`: связь действует,
 * только если плательщик есть во встрече, это не сам участник и за самого
 * плательщика никто не платит. Иначе участник платит сам, и сервер считает
 * его баланс отдельно — показывать его надо так же.
 */
export function payerOf(person: Participant, participants: Participant[]): Participant | undefined {
  if (person.paidById === null) return undefined

  const payer = participants.find((other) => other.id === person.paidById)

  if (!payer || payer.id === person.id || payer.paidById !== null) return undefined

  return payer
}

/** Те, за кого платит участник, в порядке встречи. */
export function coveredBy(person: Participant, participants: Participant[]): Participant[] {
  return participants.filter((other) => payerOf(other, participants)?.id === person.id)
}

/** Подпись кошелька: «Женя + Аня, Настя» — или просто имя. */
export function walletName(person: Participant, participants: Participant[]): string {
  const covered = coveredBy(person, participants)

  return covered.length === 0
    ? person.name
    : `${person.name} + ${covered.map((other) => other.name).join(', ')}`
}
```

Run: `cd frontend && npx vitest run src/domain/wallets.test.ts`
Expected: PASS.

- [ ] **Step 7: Типы, линт, все тесты, commit**

Run: `cd frontend && npx tsc -b && npm run lint && npm test`
Expected: без ошибок, все тесты PASS.

```bash
git add frontend/src/domain frontend/src/api/types.ts
git commit -m "feat(ui): add dishes up in an amount field and know who pays for whom

parseAmountExpression sums '390 + 1200 + 624' in kopecks and floors once;
parseAmount now goes through it. wallets.ts mirrors the server's wallet_of
so the UI hides exactly the balances the server folds.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 6: Фронт — модалка расхода на точных суммах

**Files:**
- Modify: `frontend/src/domain/sharePreview.ts`, `sharePreview.test.ts` (оба целиком)
- Modify: `frontend/src/api/types.ts`, `frontend/src/api/meetings.ts`
- Create: `frontend/src/components/ui/AmountInput.tsx`, `AmountInput.module.css`
- Modify: `frontend/src/components/modals/ShareRow.tsx`, `ShareRow.module.css` (оба целиком)
- Modify: `frontend/src/components/modals/ExpenseModal.tsx`, `ExpenseModal.module.css` (оба целиком)

- [ ] **Step 1: Падающие тесты превью**

Заменить `frontend/src/domain/sharePreview.test.ts` целиком:

```ts
import { describe, expect, it } from 'vitest'
import { previewSplit, splitStatus } from './sharePreview'

/** Участники в порядке `position`, как их отдаёт сервер. */
const three = ['a', 'b', 'c']
const four = ['a', 'b', 'c', 'd']

/** Ресторан из спеки — те же числа, что в `backend/src/domain/shares.rs`. */
const restaurant = ['katya', 'alexey', 'nastya', 'anya', 'zhenya', 'veronika']

describe('previewSplit', () => {
  it('без вписанных сумм делит поровну', () => {
    expect(previewSplit(400, four, {})).toEqual({ a: 100, b: 100, c: 100, d: 100 })
  })

  it('раздаёт остаток по порядку', () => {
    expect(previewSplit(100, three, {})).toEqual({ a: 34, b: 33, c: 33 })
  })

  it('вписанные платят ровно своё, остальные — остаток поровну', () => {
    expect(previewSplit(1001, four, { a: 400 })).toEqual({ a: 400, b: 201, c: 200, d: 200 })
  })

  it('ноль исключает из расхода', () => {
    expect(previewSplit(101, three, { c: 0 })).toEqual({ a: 51, b: 50, c: 0 })
  })

  it('пустым ничего, если вписанное покрывает расход', () => {
    expect(previewSplit(1000, three, { a: 600, b: 400 })).toEqual({ a: 600, b: 400, c: 0 })
  })

  it('вписано у всех и сходится — ровно вписанное', () => {
    expect(previewSplit(100, three, { a: 50, b: 30, c: 20 })).toEqual({ a: 50, b: 30, c: 20 })
  })

  it('скидку вычитает пропорционально', () => {
    expect(previewSplit(1000, three, { a: 700, b: 400, c: 100 })).toEqual({ a: 584, b: 333, c: 83 })
  })

  it('вписано больше расхода при пустых — пропорция, пустым ноль', () => {
    expect(previewSplit(1000, three, { a: 800, b: 400 })).toEqual({ a: 667, b: 333, c: 0 })
  })

  it('исключены все — делим на всех', () => {
    expect(previewSplit(100, three, { a: 0, b: 0, c: 0 })).toEqual({ a: 34, b: 33, c: 33 })
  })

  it('ресторан: шесть рублей раскладываются пропорционально', () => {
    expect(
      previewSplit(11996, restaurant, {
        katya: 2214,
        alexey: 3440,
        nastya: 0,
        anya: 0,
        zhenya: 3430,
        veronika: 2906,
      }),
    ).toEqual({ katya: 2215, alexey: 3442, nastya: 0, anya: 0, zhenya: 3432, veronika: 2907 })
  })

  it('ресторан: пара может вписать свою часть как угодно', () => {
    expect(
      previewSplit(11996, restaurant, {
        katya: 2214,
        alexey: 1720,
        nastya: 1720,
        anya: 0,
        zhenya: 3430,
        veronika: 2906,
      }),
    ).toEqual({ katya: 2215, alexey: 1721, nastya: 1721, anya: 0, zhenya: 3432, veronika: 2907 })
  })

  it('ресторан: пустое поле забирает остаток', () => {
    expect(
      previewSplit(11996, restaurant, { alexey: 3440, nastya: 0, anya: 0, zhenya: 3430, veronika: 2906 }),
    ).toEqual({ katya: 2220, alexey: 3440, nastya: 0, anya: 0, zhenya: 3430, veronika: 2906 })
  })

  it('сумма долей всегда равна расходу', () => {
    const cases = [{}, { a: 400 }, { a: 0, b: 7 }, { a: 300, b: 300, c: 300, d: 100 }]

    for (const pinned of cases) {
      for (const amount of [1, 7, 999, 1000, 13700]) {
        const shares = previewSplit(amount, four, pinned)
        const total = Object.values(shares).reduce((sum, value) => sum + value, 0)

        expect(total).toBe(amount)
      }
    }
  })
})

describe('splitStatus', () => {
  it('ничего не вписано — сказать нечего', () => {
    expect(splitStatus(1000, four, {})).toEqual({ kind: 'plain' })
  })

  it('только исключённые — это обычное деление поровну', () => {
    expect(splitStatus(1000, four, { c: 0 })).toEqual({ kind: 'plain' })
  })

  it('вписано не у всех — остаток на пустых', () => {
    expect(splitStatus(1000, four, { a: 400 })).toEqual({ kind: 'rest', rest: 600, among: 3 })
  })

  it('вписано больше расхода при пустых — ошибка', () => {
    expect(splitStatus(1000, three, { a: 1200 })).toEqual({ kind: 'over', pinned: 1200 })
  })

  it('ресторан: шесть рублей разложены', () => {
    expect(
      splitStatus(11996, restaurant, {
        katya: 2214,
        alexey: 3440,
        nastya: 0,
        anya: 0,
        zhenya: 3430,
        veronika: 2906,
      }),
    ).toEqual({ kind: 'adjusted', difference: 6 })
  })

  it('скидка — разница отрицательная', () => {
    expect(splitStatus(1000, three, { a: 700, b: 400, c: 100 })).toEqual({
      kind: 'adjusted',
      difference: -200,
    })
  })

  it('четверть — граница, дальше — ошибка', () => {
    expect(splitStatus(1000, three, { a: 750, b: 0, c: 0 })).toEqual({
      kind: 'adjusted',
      difference: 250,
    })
    expect(splitStatus(1000, three, { a: 749, b: 0, c: 0 })).toEqual({ kind: 'far', pinned: 749 })
  })

  it('сходится ровно — сказать нечего', () => {
    expect(splitStatus(100, three, { a: 50, b: 30, c: 20 })).toEqual({ kind: 'plain' })
  })

  it('исключены все — сказать нечего', () => {
    expect(splitStatus(100, three, { a: 0, b: 0, c: 0 })).toEqual({ kind: 'plain' })
  })
})
```

Run: `cd frontend && npx vitest run src/domain/sharePreview.test.ts`
Expected: FAIL — `previewSplit` / `splitStatus` не экспортируются.

- [ ] **Step 2: Реализация превью**

Заменить `frontend/src/domain/sharePreview.ts` целиком:

```ts
/**
 * Живое превью разбивки расхода.
 *
 * Повторяет `split_amount` и `check_split` из `backend/src/domain/shares.rs`,
 * и это дублирование осознанное: в модалке расхода доля против каждого имени
 * обновляется на каждый ввод, а записи ещё нет — сходить за ней на сервер нельзя.
 *
 * Источник истины — сервер: превью показывает, а сохраняет и пересчитывает он.
 * Расхождение поймают тесты: они берут те же числа, что доменные тесты в Rust.
 */

/** Вписанные суммы по участнику: `0` — исключён. Нет ключа — делит остаток поровну. */
export type Pinned = Record<string, number>

export function previewSplit(
  amount: number,
  /** Участники в порядке `position` — он и есть тай-брейк при раздаче остатка. */
  participantIds: string[],
  pinned: Pinned,
): Record<string, number> {
  if (participantIds.length === 0) return {}

  const fixed = participantIds.map((id) => pinned[id])
  const total = fixed.reduce<number>((sum, value) => sum + (value ?? 0), 0)
  const someoneEven = fixed.some((value) => value === undefined)

  // Правило 1: вписанные платят своё, остаток — поровну между остальными.
  if (someoneEven && total <= amount) {
    const rest = distribute(
      amount - total,
      participantIds,
      fixed.map((value) => (value === undefined ? 1 : 0)),
    )

    return Object.fromEntries(
      participantIds.map((id, index) => [id, rest[id] + (fixed[index] ?? 0)]),
    )
  }

  // Правило 2: пропорционально вписанному; у кого суммы нет — ноль.
  if (total > 0) return distribute(amount, participantIds, fixed.map((value) => value ?? 0))

  // Правило 3: исключены все — делим на всех поровну.
  return distribute(amount, participantIds, participantIds.map(() => 1))
}

/**
 * Что сказать под списком «Делим на». `over` и `far` запрещают сохранение —
 * сервер ответил бы на них 422.
 */
export type SplitStatus =
  | { kind: 'plain' }
  | { kind: 'rest'; rest: number; among: number }
  | { kind: 'adjusted'; difference: number }
  | { kind: 'over'; pinned: number }
  | { kind: 'far'; pinned: number }

export function splitStatus(amount: number, participantIds: string[], pinned: Pinned): SplitStatus {
  const fixed = participantIds.map((id) => pinned[id])
  const total = fixed.reduce<number>((sum, value) => sum + (value ?? 0), 0)
  const even = fixed.filter((value) => value === undefined).length
  const anyPositive = fixed.some((value) => value !== undefined && value > 0)

  if (even > 0) {
    if (total > amount) return { kind: 'over', pinned: total }

    // Если ничего не вписано, это обычное деление поровну, и говорить нечего.
    return anyPositive ? { kind: 'rest', rest: amount - total, among: even } : { kind: 'plain' }
  }

  if (total === 0) return { kind: 'plain' }
  if (4 * Math.abs(amount - total) > amount) return { kind: 'far', pinned: total }

  return total === amount ? { kind: 'plain' } : { kind: 'adjusted', difference: amount - total }
}

/**
 * Целая часть каждому, остаток по рублю тем, у кого больше дробная часть,
 * при равенстве — по порядку. `BigInt`, потому что `amount × вес` с вписанными
 * суммами в вес может выйти за 2^53, и тогда остатки сравнивались бы неточно.
 */
function distribute(amount: number, ids: string[], weights: number[]): Record<string, number> {
  const total = weights.reduce((sum, weight) => sum + BigInt(weight), 0n)
  const shares: Record<string, number> = {}
  const remainders: { id: string; remainder: bigint; index: number }[] = []
  let distributed = 0n

  ids.forEach((id, index) => {
    const numerator = BigInt(amount) * BigInt(weights[index])
    const whole = numerator / total

    shares[id] = Number(whole)
    distributed += whole
    remainders.push({ id, remainder: numerator % total, index })
  })

  remainders.sort((left, right) =>
    left.remainder === right.remainder
      ? left.index - right.index
      : right.remainder > left.remainder
        ? 1
        : -1,
  )

  let leftover = BigInt(amount) - distributed

  for (const entry of remainders) {
    if (leftover <= 0n) break

    shares[entry.id] += 1
    leftover -= 1n
  }

  return shares
}
```

Run: `cd frontend && npx vitest run src/domain/sharePreview.test.ts`
Expected: PASS.

- [ ] **Step 3: Типы API**

`frontend/src/api/types.ts`:

```ts
export interface Share {
  participantId: string
  /** `0` — исключён из расхода, больше нуля — вписанная сумма. Кого в списке
   *  нет, тот делит остаток поровну. */
  rubles: number
}
```

`frontend/src/api/meetings.ts`:

```ts
export interface ShareBody {
  participantId: string
  rubles: number
}
```

- [ ] **Step 4: Поле суммы с кнопкой «+»**

Создать `frontend/src/components/ui/AmountInput.tsx`:

```tsx
import { useRef } from 'react'

import styles from './AmountInput.module.css'

/**
 * Поле суммы, в котором можно складывать позиции: «390 + 1200 + 624».
 *
 * На цифровой клавиатуре телефона плюса нет, поэтому, пока фокус в поле,
 * рядом стоит кнопка «+». Видимость — через `:focus-within` в CSS, а не через
 * состояние: кнопка всегда в DOM, и клик по ней не теряется из-за того, что
 * поле успело потерять фокус и кнопка исчезла раньше клика.
 */
export default function AmountInput({
  value,
  onChange,
  placeholder,
  label,
  className,
  autoFocus,
}: {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  /** Для программ чтения с экрана: у поля нет видимой подписи. */
  label: string
  /** Классы самого поля: внешний вид задаёт форма, в которой оно стоит. */
  className?: string
  autoFocus?: boolean
}) {
  const input = useRef<HTMLInputElement>(null)

  return (
    <span className={styles.wrap}>
      <input
        ref={input}
        className={className}
        inputMode="decimal"
        value={value}
        placeholder={placeholder}
        aria-label={label}
        autoFocus={autoFocus}
        onChange={(event) => onChange(event.target.value)}
      />

      <button
        type="button"
        className={styles.plus}
        // С клавиатуры плюс просто печатают — кнопка в порядке табуляции не нужна.
        tabIndex={-1}
        aria-label="Добавить позицию"
        // Иначе нажатие забрало бы фокус у поля, и клавиатура телефона спряталась бы.
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => {
          onChange(`${value.trimEnd()} + `)
          input.current?.focus()
        }}
      >
        +
      </button>
    </span>
  )
}
```

Создать `frontend/src/components/ui/AmountInput.module.css`:

```css
.wrap {
  display: flex;
  align-items: center;
  gap: 6px;
  width: 100%;
  min-width: 0;
}

.wrap > input {
  flex: 1 1 auto;
  min-width: 0;
}

/* Плюс нужен только во время ввода: на цифровой клавиатуре телефона его нет. */
.plus {
  display: none;
  flex: none;
  align-items: center;
  justify-content: center;
  min-width: 40px;
  min-height: 40px;
  border-radius: var(--r-pill);
  border: 1px solid var(--border);
  background: var(--wash-1);
  color: var(--ink);
  font-family: var(--font-mono);
  font-size: 18px;
  font-weight: 700;
  cursor: pointer;
  transition: transform 0.1s ease;
}

.wrap:focus-within .plus {
  display: inline-flex;
}

.plus:active {
  transform: scale(0.9);
}
```

- [ ] **Step 5: Строка «Делим на»**

Заменить `frontend/src/components/modals/ShareRow.tsx` целиком:

```tsx
import type { Participant } from '../../api/types'
import type { ParsedAmount } from '../../domain/amountExpression'
import { formatRubles } from '../../domain/format'
import AmountInput from '../ui/AmountInput'
import Avatar from '../ui/Avatar'
import form from './MeetingFormModal.module.css'
import styles from './ShareRow.module.css'

/**
 * Строка «Делим на»: участвует ли человек и сколько с него.
 *
 * Пустое поле — человек делит остаток поровну, его доля видна серым
 * плейсхолдером. Вписанное число — ровно его доля, если правило пропорции её
 * не сдвинуло; тогда под полем стоит итог.
 */
export default function ShareRow({
  participant,
  payer,
  included,
  text,
  parsed,
  share,
  onToggle,
  onText,
}: {
  participant: Participant
  /** Кто платит за участника — подпись «платит Женя». */
  payer?: Participant
  included: boolean
  text: string
  parsed: ParsedAmount
  /** Итоговая доля из превью. */
  share: number
  onToggle: () => void
  onText: (text: string) => void
}) {
  const pinned = included && Number.isFinite(parsed.rubles)
  // Под полем — что вышло из ввода: сумма позиций и сдвиг пропорцией.
  const sum = pinned && parsed.compound ? `= ${formatRubles(parsed.rubles)}` : ''
  const moved = pinned && share !== parsed.rubles ? `→ ${formatRubles(share)}` : ''
  const result = [sum, moved].filter(Boolean).join(' ')

  return (
    <div className={`${styles.row} ${included ? '' : styles.excluded}`}>
      {/* Кнопка, а не div: участие должно переключаться и с клавиатуры. */}
      <button
        type="button"
        className={styles.who}
        onClick={onToggle}
        aria-pressed={included}
        aria-label={`Участие: ${participant.name}`}
      >
        <Avatar
          emoji={participant.emoji}
          colorIndex={participant.colorIndex}
          name={participant.name}
          size={30}
        />
        <span className={styles.text}>
          <span className={styles.name}>{participant.name}</span>
          {payer && <span className={styles.caption}>платит {payer.name}</span>}
        </span>
      </button>

      {included ? (
        <span className={styles.money}>
          <AmountInput
            className={`${form.input} ${styles.input} ${parsed.invalid ? form.invalid : ''}`}
            value={text}
            onChange={onText}
            placeholder={formatRubles(share)}
            label={`Сумма: ${participant.name}`}
          />
          {result && <span className={styles.result}>{result}</span>}
        </span>
      ) : (
        <span className={styles.dash}>—</span>
      )}
    </div>
  )
}
```

Заменить `frontend/src/components/modals/ShareRow.module.css` целиком:

```css
.row {
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 8px 10px;
}

.row:not(:last-child) {
  border-bottom: 1px solid var(--divider);
}

/* Аватар и имя — одна кнопка: тап по человеку включает и выключает его. */
.who {
  flex: 1 1 auto;
  min-width: 0;
  min-height: 40px;
  display: flex;
  align-items: center;
  gap: 10px;
  padding: 0;
  border: none;
  background: none;
  color: inherit;
  font: inherit;
  text-align: left;
  cursor: pointer;
}

.text {
  min-width: 0;
  display: grid;
}

.name {
  font-size: 14px;
  font-weight: 600;
  overflow: hidden;
  text-overflow: ellipsis;
  white-space: nowrap;
}

.caption {
  font-size: 12px;
  color: var(--text-3);
}

/* Выключенный приглушён, но не спрятан: его должно быть видно, чтобы вернуть. */
.excluded .who {
  opacity: 0.45;
}

.money {
  flex: 0 0 152px;
  display: grid;
  justify-items: end;
  gap: 3px;
}

.input {
  min-height: 40px;
  padding: 8px 10px;
  font-family: var(--font-mono);
  font-weight: 700;
  text-align: right;
}

.result {
  font-family: var(--font-mono);
  font-size: 12px;
  color: var(--text-3);
}

.dash {
  flex: 0 0 152px;
  padding-right: 12px;
  text-align: right;
  font-family: var(--font-mono);
  color: var(--text-muted);
}
```

- [ ] **Step 6: Модалка расхода**

Заменить `frontend/src/components/modals/ExpenseModal.tsx` целиком:

```tsx
import { useMemo, useState, type FormEvent } from 'react'

import { ApiError } from '../../api/client'
import { useAddEntry, useUpdateEntry, type ShareBody } from '../../api/meetings'
import type { Entry, Meeting } from '../../api/types'
import { parseAmountExpression } from '../../domain/amountExpression'
import { formatRubles } from '../../domain/format'
import { previewSplit, splitStatus, type Pinned, type SplitStatus } from '../../domain/sharePreview'
import { payerOf } from '../../domain/wallets'
import AmountInput from '../ui/AmountInput'
import Button from '../ui/Button'
import Select from '../ui/Select'
import Modal from './Modal'
import ShareRow from './ShareRow'
import form from './MeetingFormModal.module.css'
import styles from './ExpenseModal.module.css'

/** Строка «Делим на». Сумма хранится текстом: пока человек печатает,
 *  «390 +» ещё не число, и превращать её в `NaN` на каждый символ нельзя. */
interface SplitRow {
  included: boolean
  text: string
}

const EVEN: SplitRow = { included: true, text: '' }

/** Строки формы из сохранённой разбивки: нет строки на сервере — пустое
 *  поле, `0` — выключен, остальное — вписанная сумма. */
function rowsFromEntry(entry: Entry | undefined, meeting: Meeting): Record<string, SplitRow> {
  const rows: Record<string, SplitRow> = {}

  for (const person of meeting.participants) rows[person.id] = EVEN

  for (const share of entry?.shares ?? []) {
    rows[share.participantId] =
      share.rubles === 0 ? { included: false, text: '' } : { included: true, text: String(share.rubles) }
  }

  return rows
}

/** Подпись под списком. `error` запрещает сохранение. */
function statusLine(status: SplitStatus, amount: number): { text: string; error: boolean } | null {
  switch (status.kind) {
    case 'plain':
      return null
    case 'rest':
      return { text: `Остаток ${formatRubles(status.rest)} — поровну на ${status.among}`, error: false }
    case 'adjusted':
      return status.difference > 0
        ? { text: `+${formatRubles(status.difference)} разложены пропорционально`, error: false }
        : { text: `−${formatRubles(-status.difference)} вычтены пропорционально`, error: false }
    case 'over':
      return { text: `Вписано ${formatRubles(status.pinned)} — больше, чем потрачено`, error: true }
    case 'far':
      return {
        text: `Вписано ${formatRubles(status.pinned)} из ${formatRubles(amount)} — проверьте суммы`,
        error: true,
      }
  }
}

export default function ExpenseModal({
  meeting,
  entry,
  onClose,
}: {
  meeting: Meeting
  /** Задана — правим существующий расход. */
  entry?: Entry
  onClose: () => void
}) {
  const add = useAddEntry(meeting.id)
  const update = useUpdateEntry(meeting.id)

  const people = meeting.participants

  const [payerId, setPayerId] = useState(entry?.payerId ?? people[0]?.id ?? '')
  const [amountText, setAmountText] = useState(entry ? String(entry.amountRubles) : '')
  const [description, setDescription] = useState(entry?.description ?? '')
  const [rows, setRows] = useState(() => rowsFromEntry(entry, meeting))

  const pending = add.isPending || update.isPending
  const failure = add.error ?? update.error
  const error = failure instanceof ApiError ? failure : null

  const ids = useMemo(() => people.map((person) => person.id), [people])
  // Участник мог появиться, пока модалка открыта: для него строки ещё нет.
  const rowOf = (id: string) => rows[id] ?? EVEN

  const amount = parseAmountExpression(amountText)
  const amountOk = Number.isFinite(amount.rubles) && amount.rubles > 0
  const total = amountOk ? amount.rubles : 0

  const parsed = useMemo(
    () => Object.fromEntries(ids.map((id) => [id, parseAmountExpression(rows[id]?.text ?? '')])),
    [ids, rows],
  )

  // Выключенный — ноль, вписанное — своё, пустое поле — ключа нет, то есть «поровну».
  const pinned = useMemo(() => {
    const result: Pinned = {}

    for (const id of ids) {
      const row = rows[id] ?? EVEN

      if (!row.included) result[id] = 0
      else if (Number.isFinite(parsed[id].rubles)) result[id] = parsed[id].rubles
    }

    return result
  }, [ids, rows, parsed])

  const preview = useMemo(() => previewSplit(total, ids, pinned), [total, ids, pinned])
  const line = amountOk ? statusLine(splitStatus(total, ids, pinned), total) : null

  const someInvalid = amount.invalid || ids.some((id) => rowOf(id).included && parsed[id].invalid)
  const rounded = amount.rounded || ids.some((id) => rowOf(id).included && parsed[id].rounded)
  const valid = amountOk && payerId !== '' && !someInvalid && !line?.error

  const setRow = (id: string, patch: Partial<SplitRow>) => {
    setRows((current) => ({ ...current, [id]: { ...(current[id] ?? EVEN), ...patch } }))
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    if (!valid) return

    // Пустые поля в запрос не попадают: их отсутствие и есть «поровну».
    const shares: ShareBody[] = ids.flatMap((id) =>
      id in pinned ? [{ participantId: id, rubles: pinned[id] }] : [],
    )
    const body = { payerId, amountRubles: amount.rubles, description, shares }

    if (entry) {
      // `shares: []` — «снять разбивку», в отличие от «не трогать».
      update.mutate({ id: entry.id, ...body }, { onSuccess: onClose })

      return
    }

    add.mutate({ kind: 'expense', ...body }, { onSuccess: onClose })
  }

  return (
    <Modal
      title={entry ? 'Расход' : 'Новый расход'}
      onClose={onClose}
      footer={
        <>
          <span className={form.footSpacer} />
          <Button type="button" onClick={onClose}>
            Отмена
          </Button>
          <Button
            type="submit"
            form="expense-form"
            variant="primary"
            loading={pending}
            disabled={!valid}
          >
            {entry ? 'Сохранить' : 'Добавить'}
          </Button>
        </>
      }
    >
      <form id="expense-form" className={form.form} onSubmit={submit}>
        <div className={form.field}>
          <span className={form.label}>Кто заплатил</span>

          <div className={styles.payerRow}>
            <Select
              className={styles.payerSelect}
              value={payerId}
              onChange={setPayerId}
              label="Кто заплатил"
              options={people.map((person) => ({
                value: person.id,
                label: `${person.emoji} ${person.name}`,
              }))}
            />

            <span className={styles.payerAmount}>
              <AmountInput
                className={`${form.input} ${styles.amountField} ${
                  error?.field === 'amountRubles' || amount.invalid ? form.invalid : ''
                }`}
                value={amountText}
                onChange={setAmountText}
                placeholder="0 ₽"
                label="Сумма расхода"
                autoFocus
              />
            </span>
          </div>

          {amountOk && amount.compound && (
            <p className={styles.sum}>= {formatRubles(amount.rubles)}</p>
          )}

          {rounded && (
            <p className={styles.roundingNote}>Копейки не учитываем — суммы округлены вниз</p>
          )}
        </div>

        <div className={form.field}>
          <label className={form.label} htmlFor="expense-description">
            На что
          </label>
          <input
            id="expense-description"
            className={form.input}
            value={description}
            placeholder="Без описания"
            onChange={(event) => setDescription(event.target.value)}
          />
        </div>

        <div className={form.field}>
          <div className={styles.sharesHead}>
            <span className={form.label}>Делим на</span>
            <span className={styles.hint}>пустое поле — поровну</span>
          </div>

          <div className={styles.sharesPanel}>
            {people.map((person) => {
              const row = rowOf(person.id)

              return (
                <ShareRow
                  key={person.id}
                  participant={person}
                  payer={payerOf(person, people)}
                  included={row.included}
                  text={row.text}
                  parsed={parsed[person.id]}
                  share={preview[person.id] ?? 0}
                  onToggle={() => setRow(person.id, { included: !row.included })}
                  onText={(text) => setRow(person.id, { text })}
                />
              )
            })}
          </div>

          {line && <p className={line.error ? form.error : styles.status}>{line.text}</p>}
        </div>

        {error && <p className={form.error}>{error.humanMessage}</p>}
      </form>
    </Modal>
  )
}
```

Заменить `frontend/src/components/modals/ExpenseModal.module.css` целиком:

```css
/* Строка «кто заплатил»: селект тянется, поле суммы фиксированной ширины —
   с запасом под кнопку «+», которая появляется у поля в фокусе. */
.payerRow {
  display: flex;
  gap: 8px;
  align-items: center;
}

.payerSelect {
  flex: 1 1 auto;
  min-width: 0;
}

.payerAmount {
  flex: 0 0 152px;
  display: flex;
}

.amountField {
  font-family: var(--font-mono);
  font-weight: 700;
}

.sum {
  margin: 0;
  font-family: var(--font-mono);
  font-size: 12.5px;
  color: var(--text-3);
  text-align: right;
}

.sharesHead {
  display: flex;
  align-items: baseline;
  justify-content: space-between;
  gap: 10px;
}

.hint {
  font-size: 12px;
  color: var(--text-3);
}

.sharesPanel {
  border-radius: var(--r-stat);
  border: 1px solid var(--border);
  background: var(--surface);
  overflow: hidden;
}

.status {
  margin: 0;
  font-size: 12.5px;
  color: var(--text-2);
}

.roundingNote {
  margin: 0;
  font-size: 12.5px;
  color: var(--warn-text);
}
```

- [ ] **Step 7: Типы, линт, тесты**

Run: `cd frontend && npx tsc -b && npm run lint && npm test`
Expected: без ошибок, тесты PASS. `HistoryRow.tsx` пока использует `sharedByAll` — это нормально, поле в типе осталось.

- [ ] **Step 8: Commit**

```bash
git add frontend/src
git commit -m "feat(ui): split an expense by exact amounts

Each row in 'Делим на' is now a field: empty means an even share of the
rest, a number is that person's share, tapping the person switches them
off. The preview mirrors split_amount, and a line under the list says
what happened to the rest or why it cannot be saved. Amount fields add up
dishes, with a '+' button while focused since phone keypads have none.
'Someone else paid too' is gone: a bill lands on one person.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 7: Фронт — «Кто платит», карточки, долги, история

**Files:**
- Modify: `frontend/src/api/meetings.ts`
- Modify: `frontend/src/components/modals/ParticipantModal.tsx`
- Modify: `frontend/src/components/modals/MeetingFormModal.module.css`
- Modify: `frontend/src/routes/MeetingPage.tsx`
- Modify: `frontend/src/components/meeting/ParticipantCard.tsx`, `ParticipantCard.module.css`, `ParticipantsSection.tsx`
- Modify: `frontend/src/components/meeting/DebtsSection.tsx`, `DebtsTableView.tsx`, `DebtsBalanceView.tsx`
- Modify: `frontend/src/components/meeting/HistoryRow.tsx`

- [ ] **Step 1: Тело запроса участника**

`frontend/src/api/meetings.ts`:

```ts
export interface ParticipantBody {
  name?: string
  emoji?: string
  /** Кто платит за участника; `null` — платит сам. На правке отсутствие поля
   *  значит «не менять», поэтому форма шлёт его всегда. */
  paidById?: string | null
}
```

- [ ] **Step 2: Поле «Кто платит»**

`frontend/src/components/modals/MeetingFormModal.module.css` — дописать в конец (сначала убедиться `grep -n "^\.hint\|^\.readonly" frontend/src/components/modals/MeetingFormModal.module.css`, что таких классов нет):

```css
/* Поле, которое сейчас нельзя менять: выглядит как поле, но не откликается. */
.readonly {
  display: flex;
  align-items: center;
  color: var(--text-3);
}

.hint {
  margin: 0;
  font-size: 12px;
  color: var(--text-3);
}
```

`frontend/src/components/modals/ParticipantModal.tsx`:

импорт `import Select from '../ui/Select'`; сигнатура:

```tsx
export default function ParticipantModal({
  meetingId,
  participant,
  people,
  onClose,
}: {
  meetingId: string
  participant?: Participant
  /** Все участники встречи — из них выбирают того, кто платит. */
  people: Participant[]
  onClose: () => void
}) {
```

после `const [emoji, …]`:

```tsx
  const [paidById, setPaidById] = useState(participant?.paidById ?? '')

  // За того, кто сам платит за других, назначить плательщика нельзя: вышла бы
  // цепочка, а их сервер не принимает.
  const covers = participant ? people.filter((person) => person.paidById === participant.id) : []

  // Плательщиком может быть любой, кроме самого человека и тех, за кого уже
  // платят, — по той же причине.
  const payerOptions = [
    { value: '', label: 'Платит за себя' },
    ...people
      .filter((person) => person.id !== participant?.id && person.paidById === null)
      .map((person) => ({ value: person.id, label: `${person.emoji} ${person.name}` })),
  ]
```

`submit`:

```tsx
  const submit = (event: FormEvent) => {
    event.preventDefault()

    const body = { name, emoji, paidById: paidById === '' ? null : paidById }

    if (participant) {
      update.mutate({ id: participant.id, ...body }, { onSuccess: onClose })

      return
    }

    add.mutate(body, { onSuccess: onClose })
  }
```

текст подтверждения удаления:

```tsx
        text={`Вместе с ${participant.name} исчезнут все расходы и переводы, где он участвует. Суммы пересчитаются.${
          covers.length > 0
            ? ` За себя снова платят: ${covers.map((person) => person.name).join(', ')}.`
            : ''
        }`}
```

после поля «Аватар», перед `{error && …}`:

```tsx
        <div className={styles.field}>
          <span className={styles.label}>Кто платит</span>

          {covers.length > 0 ? (
            <>
              <div className={`${styles.input} ${styles.readonly}`}>Платит за себя</div>
              <p className={styles.hint}>
                Уже платит за других: {covers.map((person) => person.name).join(', ')}
              </p>
            </>
          ) : (
            <Select
              value={paidById}
              onChange={setPaidById}
              label="Кто платит"
              options={payerOptions}
            />
          )}
        </div>
```

`frontend/src/routes/MeetingPage.tsx`:

```tsx
        <ParticipantModal
          meetingId={id}
          participant={modal.participant}
          people={data.participants}
          onClose={close}
        />
```

- [ ] **Step 3: Карточка участника**

`frontend/src/components/meeting/ParticipantCard.tsx` — сигнатура и тело:

```tsx
export default function ParticipantCard({
  participant,
  payer,
  covers,
  onEdit,
}: {
  participant: Participant
  /** Кто платит за участника — тогда вместо баланса «платит Женя». */
  payer?: Participant
  /** За кого платит участник — подпись «+ Аня» рядом с именем. */
  covers: Participant[]
  onEdit: () => void
}) {
  const balance = payer
    ? { text: `платит ${payer.name}`, color: 'var(--text-3)' }
    : balanceLine(participant.netRubles)
```

разметка имени и баланса:

```tsx
        <div className={styles.name}>
          {participant.name}
          {covers.length > 0 && (
            <span className={styles.covers}> + {covers.map((person) => person.name).join(', ')}</span>
          )}
        </div>
        <div className={styles.contributed}>внёс {formatRubles(participant.contributedRubles)}</div>
        <div
          className={`${styles.balance} ${payer ? styles.payer : ''}`}
          style={{ color: balance.color }}
        >
          {balance.text}
        </div>
```

`frontend/src/components/meeting/ParticipantCard.module.css` — дописать:

```css
.covers {
  font-weight: 500;
  color: var(--text-3);
}

/* «платит Женя» — подпись, а не сумма: моноширинный шрифт ей ни к чему. */
.payer {
  font-family: inherit;
  font-weight: 600;
}
```

`frontend/src/components/meeting/ParticipantsSection.tsx`: импорт `import { coveredBy, payerOf } from '../../domain/wallets'`; в рендере карточки:

```tsx
            <ParticipantCard
              key={participant.id}
              participant={participant}
              payer={payerOf(participant, participants)}
              covers={coveredBy(participant, participants)}
              onEdit={() => onEdit(participant)}
            />
```

- [ ] **Step 4: Долги по кошелькам**

`frontend/src/components/meeting/DebtsSection.tsx`: импорты `import type { Meeting, Participant, Transfer } from '../../api/types'` и `import { payerOf, walletName } from '../../domain/wallets'`; после `people`:

```tsx
  // Матрица и полосы — по кошелькам: у тех, за кого платят, баланс всегда 0,
  // и пустые строки только мешали бы.
  const wallets = useMemo(
    () => meeting.participants.filter((person) => !payerOf(person, meeting.participants)),
    [meeting.participants],
  )
  const nameOf = (person: Participant) => walletName(person, meeting.participants)
```

вызовы видов:

```tsx
        <DebtsTableView settlement={meeting.settlement} participants={wallets} nameOf={nameOf} />
      ) : (
        <DebtsBalanceView participants={wallets} nameOf={nameOf} />
```

`frontend/src/components/meeting/DebtsTableView.tsx`: в пропсы добавить

```tsx
  /** Подпись кошелька: «Женя + Аня». */
  nameOf: (person: Participant) => string
```

в шапке столбца `title={person.name}` → `title={nameOf(person)}`, скрытый текст `{person.name}` → `{nameOf(person)}`; в заголовке строки `{debtor.emoji} {debtor.name}` → `{debtor.emoji} {nameOf(debtor)}`.

`frontend/src/components/meeting/DebtsBalanceView.tsx`: сигнатура

```tsx
export default function DebtsBalanceView({
  participants,
  nameOf,
}: {
  participants: Participant[]
  /** Подпись кошелька: «Женя + Аня». */
  nameOf: (person: Participant) => string
}) {
```

и `{person.emoji} {person.name}` → `{person.emoji} {nameOf(person)}`.

- [ ] **Step 5: Подпись в истории**

`frontend/src/components/meeting/HistoryRow.tsx`: импорт `import type { Entry, Participant, Share } from '../../api/types'`; перед компонентом:

```tsx
/** Приписка к описанию расхода, если он делится не поровну на всех. */
function splitNote(shares: Share[]): string {
  if (shares.some((share) => share.rubles > 0)) return ' · точные суммы'
  if (shares.length > 0) return ' · делят не все'

  return ''
}
```

и `note`:

```tsx
  const note = isTransfer ? 'перевод в счёт долга' : entry.description + splitNote(entry.shares)
```

- [ ] **Step 6: Типы, линт, тесты, commit**

Run: `cd frontend && npx tsc -b && npm run lint && npm test`
Expected: без ошибок, тесты PASS.

```bash
git add frontend/src
git commit -m "feat(ui): show who pays for whom across the meeting page

The participant form picks a payer (not oneself, not someone already
covered, and not at all for someone who pays for others). A covered card
says 'платит Женя' instead of a balance, the payer's name carries '+ Аня',
the debts matrix and bars list wallets only, and the history tells exact
amounts apart from plain exclusions.

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 8: README и проверка вживую

**Files:**
- Modify: `README.md`
- Modify: `.claude/launch.json`

- [ ] **Step 1: README**

В `README.md` заменить пункт

```
- Expenses can be split unevenly: a participant's share can be set to 1, ¾, ½, ¼ or 0. One form
  can also record several payers at once, becoming one expense per payer.
```

на

```
- Expenses can be split exactly: leave a participant's field empty to share the rest evenly,
  type their amount — several dishes add up right in the field, `390 + 1200 + 624` — or switch
  them off. If the typed amounts miss the bill by up to a quarter (tips, a discount, a slip), the
  difference is spread in proportion.
- Within a meetup one participant can pay for others, a couple for instance: their balances fold
  into the payer's, and only the payer shows up in the transfers.
```

- [ ] **Step 2: Конфигурация бэкенда для превью**

В `.claude/launch.json` добавить вторую конфигурацию (бэкенд читает `.env` из своего каталога, поэтому запускается из `backend/`):

```json
    {
      "name": "backend",
      "runtimeExecutable": "bash",
      "runtimeArgs": ["-c", "cd backend && cargo run"],
      "port": 3000
    }
```

- [ ] **Step 3: Ресторан в браузере**

1. `preview_start` с `name: "backend"`, затем с `name: "frontend"`. Проверить `preview_logs` бэкенда: миграция `0002` на бранче `dev` прошла, сервер слушает 3000.
2. Создать встречу «Ресторан», добавить шестерых: Катя, Алексей, Настя, Аня, Женя, Вероника.
3. Насте выбрать «Кто платит: Алексей», Ане — «Женя». Проверить: в карточках Насти и Ани «платит Алексей» / «платит Женя», у Алексея «+ Настя», у Жени «+ Аня»; в «Что менялось» строки `Настя: теперь платит Алексей`.
4. «Новый расход»: Женя, `11996`, «Ресторан». Выключить Настю и Аню. Вписать: Катя `390 + 1200 + 624` (под полем `= 2 214 → 2 215 ₽`), Алексей `3440`, Женя `3430`, Вероника `2906`. Под списком: `+6 ₽ разложены пропорционально`. Сохранить.
5. «Кто кому должен» списком: ровно три перевода Жене — Алексей 3 442, Вероника 2 907, Катя 2 215. Матрица и баланс: строк Ани и Насти нет, подписи «Женя + Аня», «Алексей + Настя».
6. Открыть расход на правку: Настя и Аня выключены, у остальных итоговые суммы, у Кати `2214`. Вписать Кате `22140` → красная строка `Вписано … из 11 996 ₽ — проверьте суммы`, «Сохранить» неактивна. Отменить.
7. `resize_window` `preset: "mobile"`, открыть «Новый расход»: строки не вылезают за экран, кнопка «+» появляется у поля в фокусе и дописывает плюс, не снимая фокуса. Вернуть `preset: "desktop"`.
8. Скриншот списка долгов — доказательство для пользователя.
9. Удалить тестовую встречу «Ресторан» из `dev`.

- [ ] **Step 4: Полный прогон**

Run: `cd backend && cargo fmt --check && cargo clippy --all-targets -- -D warnings && cargo test`
Run: `cd frontend && npx tsc -b && npm run lint && npm test && npm run build`
Expected: всё зелёное. Интеграционные тесты при сбое Neon перезапустить.

- [ ] **Step 5: Commit**

```bash
git add README.md .claude/launch.json
git commit -m "docs: describe exact splits and paying for others

Co-Authored-By: Claude Opus 5.5 <noreply@anthropic.com>"
```

---

### Task 9: Деплой

Действия наружу — каждое подтверждается пользователем отдельно.

- [ ] **Step 1: Прод без неполных долей**

Попросить пользователя выполнить в Neon SQL Editor на бранче `production`:

```sql
select count(*) from entry_shares where weight_quarters between 1 and 3;
```

Если больше нуля — **стоп**: сначала дописать в `0002` конвертацию по спеке (раздел «Старые четверти»), отдельным планом.

- [ ] **Step 2: Push с разрешения**

Спросить разрешение на `git push origin master`. После пуша: бэкенд на Render пересобирается сам (миграция применяется при старте), Pages — через workflow. Порядок важен: если Pages обновится раньше Render, новый фронт будет слать `rubles`, а старый бэкенд их не поймёт. Workflow Pages дольше сборки Docker не бывает, но если пользователь хочет гарантии — сначала запушить коммиты бэкенда (Tasks 1–4), дождаться `/api/health`, потом остальное.

- [ ] **Step 3: Проверка прода**

`https://reckoner-api.onrender.com/api/health` отвечает; на `https://sxm-sxpxxl.github.io/reckoner/` открыть любую встречу: участники на месте, у каждого `paidById: null`, расходы считаются как раньше.
