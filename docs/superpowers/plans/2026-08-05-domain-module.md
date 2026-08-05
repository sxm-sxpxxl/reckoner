# Domain Module Implementation Plan

> **For agentic workers:** REQUIRED SUB-SKILL: Use superpowers:subagent-driven-development (recommended) or superpowers:executing-plans to implement this plan task-by-task. Steps use checkbox (`- [ ]`) syntax for tracking.

**Goal:** Собрать оттестированный доменный модуль, который по фактам встречи считает доли расходов, балансы участников, минимальный план переводов и статус встречи — в целых рублях, без обращений к базе и HTTP.

**Architecture:** Чистые функции в `backend/src/domain/`, разложенные по одному файлу на ответственность: типы, доли, балансы, план, статус, агрегатор. Единственная точка входа для будущего слоя API — `reckon(MeetingFacts) -> Reckoning`. Крейт превращается в библиотеку + бинарник, чтобы план 2 смог писать интеграционные тесты против библиотеки. Вся разработка через TDD: тест → падение → минимальная реализация → зелёный → коммит.

**Tech Stack:** Rust 1.97 (edition 2024), `uuid` для идентификаторов, `proptest` для проверки инвариантов. Целочисленная арифметика, промежуточные умножения в `i128`.

**Spec:** [`docs/superpowers/specs/2026-08-04-reckoner-design.md`](../specs/2026-08-04-reckoner-design.md), разделы «Деньги и алгоритм», «Модуль `domain`», «Тесты».

---

## Структура файлов

| Файл | Ответственность |
| --- | --- |
| `backend/src/lib.rs` | Корень библиотеки: объявляет `pub mod domain` |
| `backend/src/domain/mod.rs` | Только проводка: `pub mod` и `pub use` |
| `backend/src/domain/types.rs` | `ParticipantId`, `Participant`, `Entry`, `EntryKind`, `Weight`, `MeetingFacts` |
| `backend/src/domain/testing.rs` | Хелперы для тестов, только под `cfg(test)` |
| `backend/src/domain/shares.rs` | `expense_shares` — доли одного расхода |
| `backend/src/domain/balance.rs` | `contributions`, `net_balances` |
| `backend/src/domain/settle.rs` | `Transfer`, `settlement_plan` |
| `backend/src/domain/status.rs` | `MeetingStatus` |
| `backend/src/domain/reckoning.rs` | `Reckoning`, `reckon` — агрегатор |

Тесты — юнит-тесты в тех же файлах (`#[cfg(test)] mod tests`), общие хелперы в `testing.rs`. Отдельного каталога `tests/` в этом плане нет: он появится в плане 2 для интеграционных тестов ручек.

`backend/src/main.rs` не меняется — домен ему пока не нужен.

---

### Task 1: Библиотечный крейт и каркас домена

**Files:**
- Create: `backend/src/lib.rs`
- Create: `backend/src/domain/mod.rs`
- Create: `backend/src/domain/types.rs`
- Create: `backend/src/domain/testing.rs`
- Modify: `backend/Cargo.toml`

- [ ] **Step 1: Добавить зависимость `uuid`**

Run: `cd backend && cargo add uuid --features v4,serde`

Ожидается: в `Cargo.toml` появляется строка вида `uuid = { version = "1.x", features = ["v4", "serde"] }`.

- [ ] **Step 2: Создать корень библиотеки**

`backend/src/lib.rs`:

```rust
//! Библиотечная часть бэкенда. Бинарник (`main.rs`) поднимает HTTP-сервер,
//! а вся логика живёт здесь, чтобы её можно было тестировать отдельно.

pub mod domain;
```

- [ ] **Step 3: Создать типы домена**

`backend/src/domain/types.rs`:

```rust
use uuid::Uuid;

/// Идентификатор участника. Newtype, чтобы его нельзя было спутать
/// с идентификатором встречи или записи.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParticipantId(pub Uuid);

impl From<Uuid> for ParticipantId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

/// Участник встречи. Домену нужны только идентификатор и порядок добавления:
/// `position` делает раздачу остатка рублей детерминированной.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Participant {
    pub id: ParticipantId,
    pub position: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Expense,
    Transfer,
}

/// Полная доля участника, для которого явный вес не задан.
pub const FULL_QUARTERS: i64 = 4;

/// Доля участника в расходе в четвертях: 0, 1, 2 или 3.
/// Полная доля (4/4) в списке не хранится — её отсутствие и есть полная доля.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Weight {
    pub participant_id: ParticipantId,
    pub quarters: u8,
}

/// Расход или перевод. Суммы — целые рубли, всегда больше нуля;
/// это гарантирует слой API.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: EntryKind,
    pub payer_id: ParticipantId,
    /// Заполнен только у перевода.
    pub recipient_id: Option<ParticipantId>,
    pub amount: i64,
    /// Только неполные доли. У перевода всегда пусто.
    pub weights: Vec<Weight>,
}

impl Entry {
    /// Вес участника в четвертях: явный, если задан, иначе полная доля.
    pub fn quarters_for(&self, participant: ParticipantId) -> i64 {
        self.weights
            .iter()
            .find(|weight| weight.participant_id == participant)
            .map(|weight| i64::from(weight.quarters))
            .unwrap_or(FULL_QUARTERS)
    }
}

/// Факты встречи — всё, что домену нужно для расчёта.
#[derive(Debug, Clone, Copy)]
pub struct MeetingFacts<'a> {
    pub participants: &'a [Participant],
    pub entries: &'a [Entry],
}
```

- [ ] **Step 4: Создать хелперы для тестов**

`backend/src/domain/testing.rs`:

```rust
//! Хелперы для тестов домена. Компилируются только под `cargo test`.

use uuid::Uuid;

use super::types::{Entry, EntryKind, Participant, ParticipantId, Weight};

/// Участник с предсказуемым идентификатором: `participant(1)` всегда даёт
/// один и тот же id, поэтому ожидания в тестах можно писать явно.
pub fn participant(position: i32) -> Participant {
    Participant {
        id: ParticipantId(Uuid::from_u128(position as u128 + 1)),
        position,
    }
}

/// `count` участников с позициями `0..count`.
pub fn participants(count: i32) -> Vec<Participant> {
    (0..count).map(participant).collect()
}

/// Расход, который делится на всех поровну.
pub fn expense(payer: Participant, amount: i64) -> Entry {
    Entry {
        kind: EntryKind::Expense,
        payer_id: payer.id,
        recipient_id: None,
        amount,
        weights: Vec::new(),
    }
}

/// Расход с явными неполными долями: пары `(участник, четверти)`.
pub fn expense_with_weights(
    payer: Participant,
    amount: i64,
    weights: &[(Participant, u8)],
) -> Entry {
    Entry {
        weights: weights
            .iter()
            .map(|(participant, quarters)| Weight {
                participant_id: participant.id,
                quarters: *quarters,
            })
            .collect(),
        ..expense(payer, amount)
    }
}

pub fn transfer(from: Participant, to: Participant, amount: i64) -> Entry {
    Entry {
        kind: EntryKind::Transfer,
        payer_id: from.id,
        recipient_id: Some(to.id),
        amount,
        weights: Vec::new(),
    }
}
```

- [ ] **Step 5: Создать проводку модуля**

`backend/src/domain/mod.rs`:

```rust
//! Денежная логика встречи: доли расходов, балансы, план переводов, статус.
//!
//! Модуль намеренно ничего не знает ни про базу, ни про HTTP: на вход — обычные
//! структуры, на выход — посчитанные значения. Все суммы в целых рублях.

pub mod types;

#[cfg(test)]
pub mod testing;

pub use types::{
    Entry, EntryKind, MeetingFacts, Participant, ParticipantId, Weight, FULL_QUARTERS,
};
```

- [ ] **Step 6: Проверить, что всё собирается**

Run: `cd backend && cargo test --lib`

Ожидается: `Compiling backend`, затем `test result: ok. 0 passed; 0 failed`. Тестов пока нет — важно, что типы компилируются.

- [ ] **Step 7: Проверить линтером**

Run: `cd backend && cargo clippy --all-targets -- -D warnings`

Ожидается: `Finished`, без предупреждений. Если clippy ругается на `unwrap_or(FULL_QUARTERS)` — оставьте как есть, это не `unwrap_or_default`, константа осмысленная.

- [ ] **Step 8: Коммит**

```bash
git add backend/Cargo.toml backend/Cargo.lock backend/src/lib.rs backend/src/domain
git commit -m "feat(domain): add domain module skeleton and types"
```

---

### Task 2: Доли расхода — равное деление

**Files:**
- Create: `backend/src/domain/shares.rs`
- Modify: `backend/src/domain/mod.rs`

- [ ] **Step 1: Написать падающий тест**

`backend/src/domain/shares.rs`:

```rust
use std::collections::BTreeMap;

use super::types::{Entry, Participant, ParticipantId};

pub fn expense_shares(
    entry: &Entry,
    participants: &[Participant],
) -> BTreeMap<ParticipantId, i64> {
    todo!("реализуется в следующем шаге")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{expense, participants};

    #[test]
    fn splits_evenly_when_amount_divides() {
        let people = participants(4);
        let entry = expense(people[1], 8400);

        let shares = expense_shares(&entry, &people);

        for person in &people {
            assert_eq!(shares[&person.id], 2100, "участник {}", person.position);
        }
    }

    #[test]
    fn returns_empty_map_without_participants() {
        let people = participants(1);
        let entry = expense(people[0], 500);

        let shares = expense_shares(&entry, &[]);

        assert!(shares.is_empty());
    }
}
```

Добавить в `backend/src/domain/mod.rs` после строки `pub mod types;`:

```rust
pub mod shares;
```

и в блок `pub use`:

```rust
pub use shares::expense_shares;
```

- [ ] **Step 2: Запустить тест и убедиться, что он падает**

Run: `cd backend && cargo test --lib shares`

Ожидается: оба теста падают с `panicked at 'not yet implemented'` из `todo!`.

- [ ] **Step 3: Реализовать равное деление**

Заменить тело `expense_shares` в `backend/src/domain/shares.rs`:

```rust
/// Доли одного расхода в целых рублях.
///
/// Гарантия: сумма всех долей ровно равна `entry.amount`. На ней держится
/// инвариант «сумма балансов равна нулю», а на нём — способность плана
/// переводов закрыть встречу в ноль.
pub fn expense_shares(
    entry: &Entry,
    participants: &[Participant],
) -> BTreeMap<ParticipantId, i64> {
    let mut shares: BTreeMap<ParticipantId, i64> = BTreeMap::new();
    if participants.is_empty() {
        return shares;
    }

    let total_quarters = FULL_QUARTERS * participants.len() as i64;
    for participant in participants {
        let numerator = i128::from(entry.amount) * i128::from(FULL_QUARTERS);
        let base = (numerator / i128::from(total_quarters)) as i64;
        shares.insert(participant.id, base);
    }

    shares
}
```

Дописать `FULL_QUARTERS` в импорт наверху файла:

```rust
use super::types::{Entry, Participant, ParticipantId, FULL_QUARTERS};
```

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib shares`

Ожидается: `test result: ok. 2 passed; 0 failed`.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/shares.rs backend/src/domain/mod.rs
git commit -m "feat(domain): split expense evenly between participants"
```

---

### Task 3: Доли расхода — раздача остатка по наибольшей дробной части

Равное деление из Task 2 теряет рубли, когда сумма не делится: 100 на трёх даст 33 + 33 + 33 = 99. Этот таск добивает остаток.

**Files:**
- Modify: `backend/src/domain/shares.rs`

- [ ] **Step 1: Написать падающий тест**

Добавить в `mod tests` в `backend/src/domain/shares.rs`:

```rust
    #[test]
    fn hands_out_remainder_to_earliest_participants() {
        let people = participants(3);
        let entry = expense(people[0], 100);

        let shares = expense_shares(&entry, &people);

        // 100 на трёх — это 34 / 33 / 33. Лишний рубль достаётся тому, кто
        // добавлен раньше: дробные части равны, значит решает position.
        assert_eq!(shares[&people[0].id], 34);
        assert_eq!(shares[&people[1].id], 33);
        assert_eq!(shares[&people[2].id], 33);
    }

    #[test]
    fn shares_always_add_up_to_the_amount() {
        let people = participants(7);
        let entry = expense(people[0], 1000);

        let shares = expense_shares(&entry, &people);

        assert_eq!(shares.values().sum::<i64>(), 1000);
    }
```

- [ ] **Step 2: Запустить тесты и убедиться, что новые падают**

Run: `cd backend && cargo test --lib shares`

Ожидается: `hands_out_remainder_to_earliest_participants` падает на `assert_eq!(shares[&people[0].id], 34)` — получено 33. `shares_always_add_up_to_the_amount` падает: сумма 994 вместо 1000.

- [ ] **Step 3: Реализовать раздачу остатка**

Заменить тело `expense_shares` целиком:

```rust
pub fn expense_shares(
    entry: &Entry,
    participants: &[Participant],
) -> BTreeMap<ParticipantId, i64> {
    let mut shares: BTreeMap<ParticipantId, i64> = BTreeMap::new();
    if participants.is_empty() {
        return shares;
    }

    let total_quarters = FULL_QUARTERS * participants.len() as i64;

    // Целая часть каждому, дробные части копим, чтобы раздать остаток.
    let mut remainders: Vec<(Participant, i128)> = Vec::new();
    let mut distributed: i64 = 0;
    for participant in participants {
        let numerator = i128::from(entry.amount) * i128::from(FULL_QUARTERS);
        let base = (numerator / i128::from(total_quarters)) as i64;
        shares.insert(participant.id, base);
        distributed += base;
        remainders.push((*participant, numerator % i128::from(total_quarters)));
    }

    // Остаток рублей — тем, у кого дробная часть больше; при равенстве
    // по порядку добавления, чтобы результат был детерминированным.
    remainders.sort_by(|left, right| {
        right
            .1
            .cmp(&left.1)
            .then(left.0.position.cmp(&right.0.position))
    });

    let mut leftover = entry.amount - distributed;
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
```

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib shares`

Ожидается: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/shares.rs
git commit -m "feat(domain): hand out expense remainder by largest fraction"
```

> По итогам ревью этого таска добавились ещё два теста —
> `spreads_several_leftover_roubles_one_each` (сто рублей на семерых: остаток из двух рублей уходит
> двум людям по одному, а не одному целиком) и `gives_the_only_rouble_to_the_first_participant`
> (граница, где целая часть у всех нулевая). Причина: без них реализация, сваливающая весь остаток
> одному человеку, проходила все четыре теста. После них в `shares` шесть тестов — отсюда счёт
> в следующих тасках.

---

### Task 4: Доли расхода — неполные доли в четвертях

**Files:**
- Modify: `backend/src/domain/shares.rs`

- [ ] **Step 1: Написать падающий тест**

Добавить в `mod tests`:

```rust
    #[test]
    fn respects_half_shares() {
        let people = participants(3);
        // Двое делят половину, один — полную долю: веса 4, 2, 2 из 8.
        let entry = expense_with_weights(
            people[0],
            100,
            &[(people[1], 2), (people[2], 2)],
        );

        let shares = expense_shares(&entry, &people);

        assert_eq!(shares[&people[0].id], 50);
        assert_eq!(shares[&people[1].id], 25);
        assert_eq!(shares[&people[2].id], 25);
    }

    #[test]
    fn respects_three_quarter_share_with_remainder() {
        let people = participants(2);
        // Веса 4 и 3 из 7: точные доли 57.14 и 42.86.
        let entry = expense_with_weights(people[0], 100, &[(people[1], 3)]);

        let shares = expense_shares(&entry, &people);

        // Дробная часть больше у второго (6/7 против 1/7), рубль его.
        assert_eq!(shares[&people[0].id], 57);
        assert_eq!(shares[&people[1].id], 43);
        assert_eq!(shares.values().sum::<i64>(), 100);
    }

    #[test]
    fn excluded_participant_never_pays_even_a_remainder_rouble() {
        let people = participants(3);
        // 101 на двоих, третий исключён: 51 / 50 / 0.
        let entry = expense_with_weights(people[0], 101, &[(people[2], 0)]);

        let shares = expense_shares(&entry, &people);

        assert_eq!(shares[&people[2].id], 0);
        assert_eq!(shares[&people[0].id], 51);
        assert_eq!(shares[&people[1].id], 50);
        assert_eq!(shares.values().sum::<i64>(), 101);
    }

    #[test]
    fn shares_do_not_depend_on_participant_order() {
        let people = participants(3);
        let entry = expense(people[0], 100);
        let reversed: Vec<Participant> = people.iter().rev().copied().collect();

        // Порядок строк, пришедших из базы, при равных ключах сортировки
        // не гарантирован — результат не должен от него зависеть.
        assert_eq!(
            expense_shares(&entry, &people),
            expense_shares(&entry, &reversed)
        );
    }
```

Первый из этих двух тестов проверяет, что участник с нулевой долей не получает рубля остатка. Это
свойство обеспечивает убывающая сортировка по остатку, реализованная ещё в Task 3, но проверить его
стало возможно только сейчас, когда веса умеют различаться. Второй закрепляет тай-брейк
`(position, id)`: без него результат зависел бы от порядка элементов во входном срезе.

Дописать `expense_with_weights` в импорт в `mod tests`:

```rust
    use crate::domain::testing::{expense, expense_with_weights, participants};
```

- [ ] **Step 2: Запустить тесты и убедиться, что новые падают**

Run: `cd backend && cargo test --lib shares`

Ожидается: `respects_half_shares` падает — получено 34/33/33 вместо 50/25/25, потому что веса пока
игнорируются. `respects_three_quarter_share_with_remainder` падает: 50 вместо 57.

Этот второй тест — первый в файле, который закрепляет **убывающее** направление сортировки по
остатку. До него все веса были равны, все остатки совпадали, и разворот сравнения не сломал бы ни
одного теста.

- [ ] **Step 3: Учесть веса**

В `expense_shares` меняется только получение веса: вместо константы `FULL_QUARTERS` для всех
берём `entry.quarters_for` для каждого. Остальное — предусловия, раздача остатка, сортировка —
остаётся ровно как после Task 3. Итоговое тело функции:

```rust
pub fn expense_shares(
    entry: &Entry,
    participants: &[Participant],
) -> BTreeMap<ParticipantId, i64> {
    debug_assert!(
        entry.amount >= 0,
        "сумма расхода не может быть отрицательной: это гарантирует слой API"
    );
    debug_assert!(
        participants
            .iter()
            .map(|participant| participant.id)
            .collect::<BTreeSet<_>>()
            .len()
            == participants.len(),
        "участники должны быть уникальны: доли ключуются по id"
    );

    let mut shares: BTreeMap<ParticipantId, i64> = BTreeMap::new();
    // Не только оптимизация: этот выход не даёт `total_quarters` стать нулём.
    if participants.is_empty() {
        return shares;
    }

    let weighted: Vec<(Participant, i64)> = participants
        .iter()
        .map(|participant| (*participant, entry.quarters_for(participant.id)))
        .collect();
    let total_quarters: i64 = weighted.iter().map(|(_, quarters)| *quarters).sum();

    // Целая часть каждому, дробные части копим, чтобы раздать остаток.
    let mut remainders: Vec<(Participant, i128)> = Vec::new();
    let mut distributed: i64 = 0;
    for (participant, quarters) in &weighted {
        let numerator = i128::from(entry.amount) * i128::from(*quarters);
        // Вес участника не больше суммы весов, поэтому `base` по модулю
        // не превосходит `amount` — сужение до i64 безопасно.
        let base = (numerator / i128::from(total_quarters)) as i64;
        shares.insert(participant.id, base);
        distributed += base;
        remainders.push((*participant, numerator % i128::from(total_quarters)));
    }

    // Остаток рублей — тем, у кого дробная часть больше. Убывание по остатку —
    // не косметика: именно оно не даст рублю остатка достаться участнику
    // с нулевой долей (его остаток всегда 0). При равенстве — по порядку
    // добавления, затем по id, чтобы порядок был полным и результат не зависел
    // от порядка строк, пришедших из базы.
    remainders.sort_by_key(|(participant, remainder)| {
        (Reverse(*remainder), participant.position, participant.id)
    });

    let mut leftover = entry.amount - distributed;
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
```

`FULL_QUARTERS` в этом файле больше не нужен — убрать из импорта, оставив:

```rust
use super::types::{Entry, Participant, ParticipantId};
```

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib shares`

Ожидается: `test result: ok. 10 passed; 0 failed`.

Внимание: `total_quarters` может быть нулём, если у всех доля 0 — тогда делим на ноль и получаем панику. Это закрывает Task 5.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/shares.rs
git commit -m "feat(domain): respect partial expense shares in quarters"
```

---

### Task 5: Доли расхода — нулевые веса

Два краевых случая: если доля нулевая у всех, спека требует считать всех по полной доле; если нулевая у одного, он не должен заплатить ни рубля — в том числе при раздаче остатка.

**Files:**
- Modify: `backend/src/domain/shares.rs`

- [ ] **Step 1: Написать падающий тест**

Добавить в `mod tests`:

```rust
    #[test]
    fn falls_back_to_equal_split_when_everyone_is_excluded() {
        let people = participants(4);
        let entry = expense_with_weights(
            people[0],
            8400,
            &[
                (people[0], 0),
                (people[1], 0),
                (people[2], 0),
                (people[3], 0),
            ],
        );

        let shares = expense_shares(&entry, &people);

        // Спека: если сумма весов нулевая, расход делится на всех поровну.
        for person in &people {
            assert_eq!(shares[&person.id], 2100, "участник {}", person.position);
        }
    }

```

Второй краевой случай — «исключённый не платит ни рубля остатка» — в этом таске **не появляется**:
он уже выполняется после Task 4 и проверяется там тестом
`excluded_participant_never_pays_even_a_remainder_rouble`. Причина в том, что участник с нулевым
весом всегда имеет нулевой остаток, а сортировка по убыванию остатка ставит его позади всех, у кого
остаток положительный; рублей остатка при этом всегда строго меньше, чем участников с положительным
остатком. Дополнительный фильтр для этого не нужен.

- [ ] **Step 2: Запустить тест и убедиться, что он падает**

Run: `cd backend && cargo test --lib shares`

Ожидается: `falls_back_to_equal_split_when_everyone_is_excluded` падает с паникой деления на ноль
(`attempt to divide by zero`) — единственный красный тест в этом таске. Остальные должны остаться
зелёными.

- [ ] **Step 3: Обработать нулевые веса**

В `expense_shares` после вычисления `weighted` и `total_quarters` вставить откат на равное деление, а в накопление `remainders` добавить фильтр по нулевому весу. Итоговое тело функции:

```rust
pub fn expense_shares(
    entry: &Entry,
    participants: &[Participant],
) -> BTreeMap<ParticipantId, i64> {
    debug_assert!(
        entry.amount >= 0,
        "сумма расхода не может быть отрицательной: это гарантирует слой API"
    );
    debug_assert!(
        participants
            .iter()
            .map(|participant| participant.id)
            .collect::<BTreeSet<_>>()
            .len()
            == participants.len(),
        "участники должны быть уникальны: доли ключуются по id"
    );

    let mut shares: BTreeMap<ParticipantId, i64> = BTreeMap::new();
    // Первая из двух причин, по которым `total_quarters` не может быть нулём.
    if participants.is_empty() {
        return shares;
    }

    let mut weighted: Vec<(Participant, i64)> = participants
        .iter()
        .map(|participant| (*participant, entry.quarters_for(participant.id)))
        .collect();
    let mut total_quarters: i64 = weighted.iter().map(|(_, quarters)| *quarters).sum();

    // Вторая: расход, из которого исключили всех, спека требует делить на всех
    // поровну — иначе здесь было бы деление на ноль.
    if total_quarters == 0 {
        for (_, quarters) in weighted.iter_mut() {
            *quarters = FULL_QUARTERS;
        }
        total_quarters = FULL_QUARTERS * participants.len() as i64;
    }

    // Целая часть каждому, дробные части копим, чтобы раздать остаток.
    let mut remainders: Vec<(Participant, i128)> = Vec::new();
    let mut distributed: i64 = 0;
    for (participant, quarters) in &weighted {
        let numerator = i128::from(entry.amount) * i128::from(*quarters);
        // Вес участника не больше суммы весов, поэтому `base` по модулю
        // не превосходит `amount` — сужение до i64 безопасно.
        let base = (numerator / i128::from(total_quarters)) as i64;
        shares.insert(participant.id, base);
        distributed += base;
        remainders.push((*participant, numerator % i128::from(total_quarters)));
    }

    // Остаток рублей — тем, у кого дробная часть больше. Убывание по остатку —
    // не косметика: у участника с нулевой долей остаток всегда нулевой, а рублей
    // остатка всегда строго меньше, чем участников с положительным остатком, —
    // вместе это и не даёт исключённому из расхода заплатить ни рубля. При
    // равенстве — по порядку добавления, затем по id, чтобы порядок был полным
    // и результат не зависел от порядка строк, пришедших из базы.
    remainders.sort_by_key(|(participant, remainder)| {
        (Reverse(*remainder), participant.position, participant.id)
    });

    let mut leftover = entry.amount - distributed;
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
```

Вернуть `FULL_QUARTERS` в импорт:

```rust
use super::types::{Entry, Participant, ParticipantId, FULL_QUARTERS};
```

Обратите внимание: единственное изменение в этом таске — откат на равное деление при нулевой сумме
весов. Фильтра по нулевому весу в раздаче остатка нет и не нужно: это свойство уже обеспечено
сортировкой (см. комментарий над ней) и проверено в Task 4.

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib shares`

Ожидается: `test result: ok. 11 passed; 0 failed`.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/shares.rs
git commit -m "feat(domain): handle zero weights in expense shares"
```

---

### Task 6: Взносы участников

`contributions` — это подпись «внёс N ₽» в карточке участника. Считаются только оплаченные расходы; отправленные переводы в неё не входят (сверено с прототипом, `docs/design/meetup-splitter.dc.html:630`).

**Files:**
- Create: `backend/src/domain/balance.rs`
- Modify: `backend/src/domain/mod.rs`

- [ ] **Step 1: Написать падающий тест**

`backend/src/domain/balance.rs`:

```rust
use std::collections::BTreeMap;

use super::types::{MeetingFacts, ParticipantId};

pub fn contributions(facts: MeetingFacts) -> BTreeMap<ParticipantId, i64> {
    todo!("реализуется в следующем шаге")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{expense, participants, transfer};

    #[test]
    fn sums_expenses_paid_by_each_participant() {
        let people = participants(3);
        let entries = vec![
            expense(people[0], 1950),
            expense(people[1], 1290),
            expense(people[0], 60),
        ];

        let paid = contributions(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(paid[&people[0].id], 2010);
        assert_eq!(paid[&people[1].id], 1290);
        assert_eq!(paid[&people[2].id], 0);
    }

    #[test]
    fn ignores_transfers() {
        let people = participants(2);
        let entries = vec![transfer(people[0], people[1], 500)];

        let paid = contributions(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(paid[&people[0].id], 0);
        assert_eq!(paid[&people[1].id], 0);
    }
}
```

Добавить в `backend/src/domain/mod.rs`:

```rust
pub mod balance;
```

и в блок `pub use`:

```rust
pub use balance::contributions;
```

- [ ] **Step 2: Запустить тест и убедиться, что он падает**

Run: `cd backend && cargo test --lib balance`

Ожидается: оба теста падают с `not yet implemented`.

- [ ] **Step 3: Реализовать**

Заменить тело `contributions` в `backend/src/domain/balance.rs`:

```rust
/// Сколько каждый участник оплатил расходами — подпись «внёс N ₽».
/// Отправленные переводы сюда не входят.
pub fn contributions(facts: MeetingFacts) -> BTreeMap<ParticipantId, i64> {
    let mut paid: BTreeMap<ParticipantId, i64> = facts
        .participants
        .iter()
        .map(|participant| (participant.id, 0))
        .collect();

    for entry in facts.entries {
        if entry.kind == EntryKind::Expense {
            if let Some(sum) = paid.get_mut(&entry.payer_id) {
                *sum += entry.amount;
            }
        }
    }

    paid
}
```

Дописать `EntryKind` в импорт:

```rust
use super::types::{EntryKind, MeetingFacts, ParticipantId};
```

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib balance`

Ожидается: `test result: ok. 2 passed; 0 failed`.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/balance.rs backend/src/domain/mod.rs
git commit -m "feat(domain): sum expenses paid by each participant"
```

---

### Task 7: Балансы участников

**Files:**
- Modify: `backend/src/domain/balance.rs`
- Modify: `backend/src/domain/mod.rs`

- [ ] **Step 1: Написать падающий тест**

Добавить в `backend/src/domain/balance.rs` заглушку функции сразу после `contributions`:

```rust
pub fn net_balances(facts: MeetingFacts) -> BTreeMap<ParticipantId, i64> {
    todo!("реализуется в следующем шаге")
}
```

и тесты в `mod tests`:

```rust
    #[test]
    fn nets_expenses_against_shares() {
        // Встреча «Дача у Влада» из демо-данных прототипа.
        let people = participants(4);
        let (nastya, vlad, egor, marina) = (people[0], people[1], people[2], people[3]);
        let entries = vec![
            expense(vlad, 8400),
            expense(nastya, 3200),
            expense(marina, 2100),
        ];

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        // Всего 13 700 на четверых — по 3 425 с каждого.
        assert_eq!(net[&nastya.id], -225);
        assert_eq!(net[&vlad.id], 4975);
        assert_eq!(net[&egor.id], -3425);
        assert_eq!(net[&marina.id], -1325);
    }

    #[test]
    fn transfer_moves_balance_from_sender_to_recipient() {
        let people = participants(2);
        let entries = vec![
            expense(people[0], 100),
            transfer(people[1], people[0], 50),
        ];

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        // Расход 100 делится по 50; второй уже перевёл свои 50 — все в расчёте.
        assert_eq!(net[&people[0].id], 0);
        assert_eq!(net[&people[1].id], 0);
    }

    #[test]
    fn overpaying_transfer_reverses_the_debt() {
        let people = participants(2);
        let entries = vec![
            expense(people[0], 100),
            transfer(people[1], people[0], 150),
        ];

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        // Второй перевёл на 100 больше, чем был должен — теперь должны ему.
        assert_eq!(net[&people[0].id], -100);
        assert_eq!(net[&people[1].id], 100);
    }

    #[test]
    fn balances_always_sum_to_zero() {
        let people = participants(3);
        let entries = vec![
            expense(people[0], 100),
            expense(people[1], 7),
            transfer(people[2], people[0], 13),
        ];

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(net.values().sum::<i64>(), 0);
    }

    #[test]
    fn skips_entries_referencing_participants_outside_the_meeting() {
        let people = participants(2);
        let stranger = participant(99);
        let entries = vec![transfer(stranger, people[0], 500)];

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        // В базе такого не бывает: удаление участника каскадом убирает и его
        // записи. Но защитный код должен быть проверен — домен обязан не
        // паниковать и не заводить балансы для посторонних. Нулевая сумма
        // балансов на таких данных не гарантируется и не проверяется.
        assert_eq!(net.len(), 2);
        assert!(!net.contains_key(&stranger.id));
        assert_eq!(net[&people[0].id], -500);
        assert_eq!(net[&people[1].id], 0);
    }
```

Заменить импорт хелперов в `mod tests` — добавился `participant`:

```rust
    use crate::domain::testing::{expense, participant, participants, transfer};
```

Добавить в блок `pub use` в `backend/src/domain/mod.rs`:

```rust
pub use balance::{contributions, net_balances};
```

(заменив прежнюю строку `pub use balance::contributions;`)

- [ ] **Step 2: Запустить тесты и убедиться, что новые падают**

Run: `cd backend && cargo test --lib balance`

Ожидается: пять новых тестов падают с `not yet implemented`.

- [ ] **Step 3: Реализовать**

Заменить тело `net_balances`:

```rust
/// Баланс каждого участника.
///
/// `net = оплаченные расходы + отправленные переводы − полученные переводы
///        − доли по всем расходам`
///
/// Плюс означает «ему должны», минус — «он должен». Для корректных данных
/// (все идентификаторы принадлежат встрече) сумма балансов равна нулю.
pub fn net_balances(facts: MeetingFacts) -> BTreeMap<ParticipantId, i64> {
    let mut net: BTreeMap<ParticipantId, i64> = facts
        .participants
        .iter()
        .map(|participant| (participant.id, 0))
        .collect();

    for entry in facts.entries {
        if let Some(balance) = net.get_mut(&entry.payer_id) {
            *balance += entry.amount;
        }

        match entry.kind {
            EntryKind::Expense => {
                for (id, share) in expense_shares(entry, facts.participants) {
                    if let Some(balance) = net.get_mut(&id) {
                        *balance -= share;
                    }
                }
            }
            EntryKind::Transfer => {
                if let Some(recipient) = entry.recipient_id {
                    if let Some(balance) = net.get_mut(&recipient) {
                        *balance -= entry.amount;
                    }
                }
            }
        }
    }

    net
}
```

Дописать импорт долей наверху файла:

```rust
use super::shares::expense_shares;
```

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib balance`

Ожидается: `test result: ok. 7 passed; 0 failed`.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/balance.rs backend/src/domain/mod.rs
git commit -m "feat(domain): compute participant net balances"
```

---

### Task 8: План переводов

**Files:**
- Create: `backend/src/domain/settle.rs`
- Modify: `backend/src/domain/mod.rs`

- [ ] **Step 1: Написать падающий тест**

`backend/src/domain/settle.rs`:

```rust
use std::collections::BTreeMap;

use super::types::{Participant, ParticipantId};

/// Один перевод в плане закрытия встречи.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub from: ParticipantId,
    pub to: ParticipantId,
    pub amount: i64,
}

pub fn settlement_plan(
    participants: &[Participant],
    net: &BTreeMap<ParticipantId, i64>,
) -> Vec<Transfer> {
    todo!("реализуется в следующем шаге")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::participants;

    fn balances(entries: &[(Participant, i64)]) -> BTreeMap<ParticipantId, i64> {
        entries
            .iter()
            .map(|(participant, balance)| (participant.id, *balance))
            .collect()
    }

    #[test]
    fn returns_empty_plan_when_everyone_is_settled() {
        let people = participants(3);
        let net = balances(&[(people[0], 0), (people[1], 0), (people[2], 0)]);

        assert!(settlement_plan(&people, &net).is_empty());
    }

    #[test]
    fn matches_biggest_debtor_with_biggest_creditor() {
        // Балансы встречи «Дача у Влада».
        let people = participants(4);
        let (nastya, vlad, egor, marina) = (people[0], people[1], people[2], people[3]);
        let net = balances(&[
            (nastya, -225),
            (vlad, 4975),
            (egor, -3425),
            (marina, -1325),
        ]);

        let plan = settlement_plan(&people, &net);

        assert_eq!(
            plan,
            vec![
                Transfer { from: egor.id, to: vlad.id, amount: 3425 },
                Transfer { from: marina.id, to: vlad.id, amount: 1325 },
                Transfer { from: nastya.id, to: vlad.id, amount: 225 },
            ]
        );
    }

    #[test]
    fn splits_a_debt_across_two_creditors() {
        let people = participants(3);
        let net = balances(&[(people[0], -300), (people[1], 200), (people[2], 100)]);

        let plan = settlement_plan(&people, &net);

        assert_eq!(
            plan,
            vec![
                Transfer { from: people[0].id, to: people[1].id, amount: 200 },
                Transfer { from: people[0].id, to: people[2].id, amount: 100 },
            ]
        );
    }

    #[test]
    fn plan_settles_every_balance_to_zero() {
        let people = participants(5);
        let net = balances(&[
            (people[0], -700),
            (people[1], 250),
            (people[2], -150),
            (people[3], 400),
            (people[4], 200),
        ]);

        let plan = settlement_plan(&people, &net);

        let mut settled = net.clone();
        for transfer in &plan {
            *settled.get_mut(&transfer.from).expect("должник есть") += transfer.amount;
            *settled.get_mut(&transfer.to).expect("кредитор есть") -= transfer.amount;
        }

        assert!(
            settled.values().all(|balance| *balance == 0),
            "после плана остались долги: {settled:?}"
        );
    }
}
```

Добавить в `backend/src/domain/mod.rs`:

```rust
pub mod settle;
```

и в блок `pub use`:

```rust
pub use settle::{settlement_plan, Transfer};
```

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `cd backend && cargo test --lib settle`

Ожидается: `returns_empty_plan_when_everyone_is_settled` и остальные падают с `not yet implemented`.

- [ ] **Step 3: Реализовать жадный алгоритм**

Заменить тело `settlement_plan`:

```rust
/// Минимальный по количеству список переводов, закрывающий встречу в ноль.
///
/// Жадно сводим наибольшего должника с наибольшим кредитором. Порядок
/// участников при равных суммах определяется `position`, поэтому результат
/// детерминированный.
pub fn settlement_plan(
    participants: &[Participant],
    net: &BTreeMap<ParticipantId, i64>,
) -> Vec<Transfer> {
    let mut ordered: Vec<Participant> = participants.to_vec();
    ordered.sort_by_key(|participant| participant.position);

    let mut debtors: Vec<(ParticipantId, i64)> = Vec::new();
    let mut creditors: Vec<(ParticipantId, i64)> = Vec::new();
    for participant in &ordered {
        let balance = net.get(&participant.id).copied().unwrap_or(0);
        if balance < 0 {
            debtors.push((participant.id, -balance));
        } else if balance > 0 {
            creditors.push((participant.id, balance));
        }
    }

    // Стабильная сортировка: при равных суммах сохраняется порядок position.
    debtors.sort_by(|left, right| right.1.cmp(&left.1));
    creditors.sort_by(|left, right| right.1.cmp(&left.1));

    let mut plan = Vec::new();
    let mut debtor = 0;
    let mut creditor = 0;
    while debtor < debtors.len() && creditor < creditors.len() {
        let amount = debtors[debtor].1.min(creditors[creditor].1);
        plan.push(Transfer {
            from: debtors[debtor].0,
            to: creditors[creditor].0,
            amount,
        });

        debtors[debtor].1 -= amount;
        creditors[creditor].1 -= amount;
        if debtors[debtor].1 == 0 {
            debtor += 1;
        }
        if creditors[creditor].1 == 0 {
            creditor += 1;
        }
    }

    plan
}
```

Долги и кредиты всегда строго больше нуля по построению, поэтому `amount > 0` и на каждой итерации хотя бы один индекс двигается — цикл конечен.

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib settle`

Ожидается: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/settle.rs backend/src/domain/mod.rs
git commit -m "feat(domain): build minimal settlement plan"
```

---

### Task 9: Статус встречи

**Files:**
- Create: `backend/src/domain/status.rs`
- Modify: `backend/src/domain/mod.rs`

- [ ] **Step 1: Написать падающий тест**

`backend/src/domain/status.rs`:

```rust
use super::settle::Transfer;

/// Статус встречи. Цвет карточки и текст бейджа в интерфейсе — производные
/// от него: зелёный, жёлтый, красный.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingStatus {
    /// Участников ещё нет.
    NoParticipants,
    /// Все в расчёте, переводов не осталось.
    Settled,
    /// Остался один или два перевода.
    Attention(usize),
    /// Осталось три и более переводов.
    Alarm(usize),
}

impl MeetingStatus {
    pub fn from_plan(participant_count: usize, plan: &[Transfer]) -> Self {
        todo!("реализуется в следующем шаге")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::participant;

    fn plan(length: usize) -> Vec<Transfer> {
        let from = participant(0);
        let to = participant(1);
        (0..length)
            .map(|_| Transfer { from: from.id, to: to.id, amount: 1 })
            .collect()
    }

    #[test]
    fn reports_no_participants_regardless_of_plan() {
        assert_eq!(
            MeetingStatus::from_plan(0, &plan(0)),
            MeetingStatus::NoParticipants
        );
    }

    #[test]
    fn empty_plan_means_settled() {
        assert_eq!(MeetingStatus::from_plan(5, &plan(0)), MeetingStatus::Settled);
    }

    #[test]
    fn one_or_two_transfers_need_attention() {
        assert_eq!(
            MeetingStatus::from_plan(3, &plan(1)),
            MeetingStatus::Attention(1)
        );
        assert_eq!(
            MeetingStatus::from_plan(3, &plan(2)),
            MeetingStatus::Attention(2)
        );
    }

    #[test]
    fn three_or_more_transfers_raise_alarm() {
        assert_eq!(MeetingStatus::from_plan(4, &plan(3)), MeetingStatus::Alarm(3));
        assert_eq!(MeetingStatus::from_plan(9, &plan(7)), MeetingStatus::Alarm(7));
    }
}
```

Добавить в `backend/src/domain/mod.rs`:

```rust
pub mod status;
```

и в блок `pub use`:

```rust
pub use status::MeetingStatus;
```

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `cd backend && cargo test --lib status`

Ожидается: четыре теста падают с `not yet implemented`.

- [ ] **Step 3: Реализовать**

Заменить тело `from_plan`:

```rust
    /// Спека: нет участников → `NoParticipants`; 0 переводов → `Settled`;
    /// 1–2 → `Attention`; 3 и больше → `Alarm`.
    pub fn from_plan(participant_count: usize, plan: &[Transfer]) -> Self {
        if participant_count == 0 {
            return Self::NoParticipants;
        }

        match plan.len() {
            0 => Self::Settled,
            length @ 1..=2 => Self::Attention(length),
            length => Self::Alarm(length),
        }
    }
```

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib status`

Ожидается: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/status.rs backend/src/domain/mod.rs
git commit -m "feat(domain): derive meeting status from settlement plan"
```

---

### Task 10: Агрегатор `reckon`

Единственная функция, которую будет вызывать слой API.

**Files:**
- Create: `backend/src/domain/reckoning.rs`
- Modify: `backend/src/domain/mod.rs`

- [ ] **Step 1: Написать падающий тест**

`backend/src/domain/reckoning.rs`:

```rust
use std::collections::BTreeMap;

use super::balance::{contributions, net_balances};
use super::settle::{settlement_plan, Transfer};
use super::status::MeetingStatus;
use super::types::{EntryKind, MeetingFacts, ParticipantId};

/// Всё, что считается по фактам встречи.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reckoning {
    /// «внёс N ₽» — сумма оплаченных расходов.
    pub contributed: BTreeMap<ParticipantId, i64>,
    /// Баланс: плюс — ему должны, минус — он должен.
    pub net: BTreeMap<ParticipantId, i64>,
    /// Переводы, которые закроют встречу в ноль.
    pub settlement: Vec<Transfer>,
    pub status: MeetingStatus,
    /// Сумма всех расходов.
    pub spent: i64,
    /// Расходы на одного участника; 0, если участников нет.
    pub per_person: i64,
}

pub fn reckon(facts: MeetingFacts) -> Reckoning {
    todo!("реализуется в следующем шаге")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{expense, participants};

    #[test]
    fn reports_totals_and_status_for_a_meeting_in_progress() {
        let people = participants(4);
        let (nastya, vlad, egor, marina) = (people[0], people[1], people[2], people[3]);
        let entries = vec![
            expense(vlad, 8400),
            expense(nastya, 3200),
            expense(marina, 2100),
        ];

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(reckoning.spent, 13700);
        assert_eq!(reckoning.per_person, 3425);
        assert_eq!(reckoning.contributed[&vlad.id], 8400);
        assert_eq!(reckoning.contributed[&egor.id], 0);
        assert_eq!(reckoning.net[&vlad.id], 4975);
        assert_eq!(reckoning.settlement.len(), 3);
        assert_eq!(reckoning.status, MeetingStatus::Alarm(3));
    }

    #[test]
    fn reports_zeroes_without_participants() {
        let reckoning = reckon(MeetingFacts {
            participants: &[],
            entries: &[],
        });

        assert_eq!(reckoning.spent, 0);
        assert_eq!(reckoning.per_person, 0);
        assert!(reckoning.settlement.is_empty());
        assert_eq!(reckoning.status, MeetingStatus::NoParticipants);
    }

    #[test]
    fn meeting_without_expenses_is_settled() {
        let people = participants(5);

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &[],
        });

        assert_eq!(reckoning.spent, 0);
        assert_eq!(reckoning.per_person, 0);
        assert_eq!(reckoning.status, MeetingStatus::Settled);
    }

    #[test]
    fn single_participant_owes_nobody() {
        let people = participants(1);
        let entries = vec![expense(people[0], 999)];

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        // Заплатил сам за себя: доля равна расходу, баланс нулевой,
        // переводить некому.
        assert_eq!(reckoning.spent, 999);
        assert_eq!(reckoning.per_person, 999);
        assert_eq!(reckoning.contributed[&people[0].id], 999);
        assert_eq!(reckoning.net[&people[0].id], 0);
        assert!(reckoning.settlement.is_empty());
        assert_eq!(reckoning.status, MeetingStatus::Settled);
    }
}
```

Добавить в `backend/src/domain/mod.rs`:

```rust
pub mod reckoning;
```

и в блок `pub use`:

```rust
pub use reckoning::{reckon, Reckoning};
```

- [ ] **Step 2: Запустить тесты и убедиться, что они падают**

Run: `cd backend && cargo test --lib reckoning`

Ожидается: четыре теста падают с `not yet implemented`.

- [ ] **Step 3: Реализовать**

Заменить тело `reckon`:

```rust
/// Единственная точка входа для слоя API: по фактам встречи считает всё,
/// что нужно отдать клиенту.
pub fn reckon(facts: MeetingFacts) -> Reckoning {
    let net = net_balances(facts);
    let settlement = settlement_plan(facts.participants, &net);
    let status = MeetingStatus::from_plan(facts.participants.len(), &settlement);

    let spent: i64 = facts
        .entries
        .iter()
        .filter(|entry| entry.kind == EntryKind::Expense)
        .map(|entry| entry.amount)
        .sum();
    let per_person = if facts.participants.is_empty() {
        0
    } else {
        spent / facts.participants.len() as i64
    };

    Reckoning {
        contributed: contributions(facts),
        net,
        settlement,
        status,
        spent,
        per_person,
    }
}
```

- [ ] **Step 4: Запустить тесты и убедиться, что они зелёные**

Run: `cd backend && cargo test --lib reckoning`

Ожидается: `test result: ok. 4 passed; 0 failed`.

- [ ] **Step 5: Коммит**

```bash
git add backend/src/domain/reckoning.rs backend/src/domain/mod.rs
git commit -m "feat(domain): add reckon aggregate entry point"
```

---

### Task 11: Фикстуры демо-встреч из прототипа

Три демо-встречи прототипа — самая честная проверка: их числа видны на скриншотах дизайна.

**Важно:** README хендоффа утверждает, что «Кино и шаурма» закрыта в ноль. Это ошибка в тексте хендоффа: по его же демо-данным у Лёши П остаётся +220, у Лёши З −220, то есть остаётся один перевод и статус жёлтый. Тест фиксирует посчитанную правду, а не прозу README.

**Files:**
- Modify: `backend/src/domain/reckoning.rs`

- [ ] **Step 1: Написать падающий тест**

Добавить в `mod tests` в `backend/src/domain/reckoning.rs`:

```rust
    #[test]
    fn fixture_bbq_has_participants_but_no_expenses() {
        // «Солевые шашлыки»: 5 участников, расходов нет — зелёный статус
        // и нулевая сумма на карточке (скриншот 01-meetings-list.png).
        let people = participants(5);

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &[],
        });

        assert_eq!(reckoning.spent, 0);
        assert_eq!(reckoning.status, MeetingStatus::Settled);
        assert!(reckoning.net.values().all(|balance| *balance == 0));
    }

    #[test]
    fn fixture_dacha_needs_three_transfers_to_vlad() {
        // «Дача у Влада» (скриншоты 02 и 03): 13 700 на четверых,
        // три перевода, все — Владу.
        let people = participants(4);
        let (nastya, vlad, egor, marina) = (people[0], people[1], people[2], people[3]);
        let entries = vec![
            expense(vlad, 8400),
            expense(nastya, 3200),
            expense(marina, 2100),
        ];

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(reckoning.spent, 13700);
        assert_eq!(reckoning.per_person, 3425);
        assert_eq!(reckoning.net[&nastya.id], -225);
        assert_eq!(reckoning.net[&vlad.id], 4975);
        assert_eq!(reckoning.net[&egor.id], -3425);
        assert_eq!(reckoning.net[&marina.id], -1325);
        assert_eq!(
            reckoning.settlement,
            vec![
                Transfer { from: egor.id, to: vlad.id, amount: 3425 },
                Transfer { from: marina.id, to: vlad.id, amount: 1325 },
                Transfer { from: nastya.id, to: vlad.id, amount: 225 },
            ]
        );
        assert_eq!(reckoning.status, MeetingStatus::Alarm(3));
    }

    #[test]
    fn fixture_kino_still_owes_one_transfer() {
        // «Кино и шаурма»: расходы 1950 и 1290 на троих (по 1080),
        // затем три перевода из демо-данных прототипа.
        //
        // README хендоффа называет эту встречу закрытой в ноль, но по его же
        // числам это не так: Егор рассчитался полностью, а между Лёшей З
        // и Лёшей П остаётся 220.
        let people = participants(3);
        let (lesha_p, lesha_z, egor) = (people[0], people[1], people[2]);
        let entries = vec![
            expense(lesha_p, 1950),
            expense(lesha_z, 1290),
            transfer(egor, lesha_p, 430),
            transfer(egor, lesha_z, 650),
            transfer(lesha_z, lesha_p, 220),
        ];

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        assert_eq!(reckoning.spent, 3240);
        assert_eq!(reckoning.per_person, 1080);
        assert_eq!(reckoning.net[&lesha_p.id], 220);
        assert_eq!(reckoning.net[&lesha_z.id], -220);
        assert_eq!(reckoning.net[&egor.id], 0);
        assert_eq!(
            reckoning.settlement,
            vec![Transfer { from: lesha_z.id, to: lesha_p.id, amount: 220 }]
        );
        assert_eq!(reckoning.status, MeetingStatus::Attention(1));
    }
```

Дописать `transfer` в импорт в `mod tests`:

```rust
    use crate::domain::testing::{expense, participants, transfer};
```

- [ ] **Step 2: Запустить тесты**

Run: `cd backend && cargo test --lib reckoning`

Ожидается: `test result: ok. 7 passed; 0 failed`. Реализация уже готова — эти тесты проверяют её на реальных данных дизайна. Если какой-то падает, ошибка в реализации из Task 2–10, а не в тесте: сверьтесь с числами на скриншотах в `docs/design/screens/`.

- [ ] **Step 3: Коммит**

```bash
git add backend/src/domain/reckoning.rs
git commit -m "test(domain): cover demo meetings from the design prototype"
```

---

### Task 12: Property-тесты инвариантов

Юнит-тесты проверяют разобранные случаи. Property-тест проверяет то, на чём держится вся конструкция: сумма балансов равна нулю и план всегда сводит всех в ноль.

**Files:**
- Create: `backend/src/domain/properties.rs`
- Modify: `backend/src/domain/mod.rs`
- Modify: `backend/Cargo.toml`

- [ ] **Step 1: Добавить `proptest` в dev-зависимости**

Run: `cd backend && cargo add proptest --dev`

Ожидается: в `Cargo.toml` появляется секция `[dev-dependencies]` со строкой `proptest = "1.x"`.

- [ ] **Step 2: Написать property-тесты**

`backend/src/domain/properties.rs`:

```rust
//! Property-тесты инвариантов домена. Файл содержит только тесты.

use proptest::prelude::*;

use super::testing::{expense_with_weights, participant, transfer};
use super::types::{Entry, MeetingFacts, Participant};
use super::{net_balances, settlement_plan};

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
        (0usize..64, 0usize..64, 1i64..1_000_000)
            .prop_map(|(from, to, amount)| Draft::Transfer { from, to, amount }),
    ]
}

fn roster(count: usize) -> Vec<Participant> {
    (0..count as i32).map(participant).collect()
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
                (sender != recipient)
                    .then(|| transfer(people[sender], people[recipient], *amount))
            }
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Сумма балансов всегда ноль. Инвариант обеспечивает целочисленная
    /// раздача остатка долей, и именно на нём держится способность плана
    /// закрыть встречу.
    #[test]
    fn balances_sum_to_zero(
        count in 1usize..8,
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count);
        let entries = build(&people, &drafts);

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        prop_assert_eq!(net.values().sum::<i64>(), 0);
    }

    /// План сводит каждый баланс в ноль, и переводов в нём меньше, чем
    /// участников: это и означает «минимальный», а не «все всем».
    #[test]
    fn settlement_plan_zeroes_every_balance(
        count in 1usize..8,
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count);
        let entries = build(&people, &drafts);
        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

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
        prop_assert!(plan.len() < count.max(2));
    }
}
```

Добавить в `backend/src/domain/mod.rs` после `pub mod reckoning;`:

```rust
#[cfg(test)]
mod properties;
```

- [ ] **Step 3: Запустить тесты**

Run: `cd backend && cargo test --lib properties`

Ожидается: `test result: ok. 2 passed; 0 failed`. Если proptest найдёт контрпример, он выведет минимальный падающий вход и запишет его в `backend/proptest-regressions/` — это баг в реализации, разбирайтесь с ним, а не подгоняйте тест.

- [ ] **Step 4: Прогнать всё и проверить линтером**

Run: `cd backend && cargo test`

Ожидается: все тесты зелёные. Ориентир по количеству — 11 в `shares`, 7 в `balance`, 4 в `settle`, 4 в `status`, 7 в `reckoning`, 2 в `properties`.

Run: `cd backend && cargo clippy --all-targets -- -D warnings`

Ожидается: `Finished`, без предупреждений.

Run: `cd backend && cargo fmt --check`

Ожидается: пустой вывод. Если есть расхождения — выполните `cargo fmt` и включите правки в коммит.

- [ ] **Step 5: Добавить регрессии proptest в git**

`backend/proptest-regressions/` — это найденные контрпримеры, их нужно коммитить, чтобы они проверялись впредь. Каталога может не быть, если контрпримеров не нашлось.

```bash
git add backend/Cargo.toml backend/Cargo.lock backend/src/domain/properties.rs backend/src/domain/mod.rs
git add backend/proptest-regressions 2>/dev/null || true
git commit -m "test(domain): assert zero-sum and settlement invariants with proptest"
```

---

## Проверка по завершении плана

- [ ] `cd backend && cargo test` — все тесты зелёные
- [ ] `cd backend && cargo clippy --all-targets -- -D warnings` — без предупреждений
- [ ] `cd backend && cargo fmt --check` — без расхождений
- [ ] `cd backend && cargo run` — сервер по-прежнему поднимается и `GET /api/health` отвечает `{"status":"ok"}` (домен не должен был ничего сломать в бинарнике)
- [ ] Числа в тестах-фикстурах совпадают со скриншотами в `docs/design/screens/`

## Что дальше

План 2 — схема Postgres, миграции, слой доступа к данным и ручки API поверх `reckon`. Домен к тому моменту меняться не должен: если при написании ручек выясняется, что ему не хватает данных, это сигнал вернуться и уточнить спеку, а не дописывать логику в хендлер.

Один пункт из списка тестов спеки сюда не влезает по существу: «удаление участника посреди истории». Домен со своей стороны закрыт — `skips_entries_referencing_participants_outside_the_meeting` проверяет, что посторонние идентификаторы не ломают расчёт. Но само требование «удалили участника — исчезли его записи» реализуется каскадами в схеме, поэтому его тест переезжает в план 2, в интеграционные тесты ручек.
