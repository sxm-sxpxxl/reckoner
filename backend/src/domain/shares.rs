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
    let total: i128 = pins
        .iter()
        .flatten()
        .map(|rubles| i128::from(*rubles))
        .sum();
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{expense, expense_with_weights, participants, pin};
    use uuid::Uuid;

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

    #[test]
    fn spreads_several_leftover_roubles_one_each() {
        let people = participants(7);
        let entry = expense(people[0], 100);

        let shares = expense_shares(&entry, &people);

        // 100 на семерых — по 14, остаток 2 ₽ уходит двум первым по одному
        // рублю, а не одному человеку целиком.
        assert_eq!(shares[&people[0].id], 15);
        assert_eq!(shares[&people[1].id], 15);
        for person in &people[2..] {
            assert_eq!(shares[&person.id], 14, "участник {}", person.position);
        }
        assert_eq!(shares.values().sum::<i64>(), 100);
    }

    #[test]
    fn gives_the_only_rouble_to_the_first_participant() {
        let people = participants(3);
        let entry = expense(people[0], 1);

        let shares = expense_shares(&entry, &people);

        // Граница: целая часть у всех нулевая, весь расход — это остаток.
        assert_eq!(shares[&people[0].id], 1);
        assert_eq!(shares[&people[1].id], 0);
        assert_eq!(shares[&people[2].id], 0);
    }

    #[test]
    fn respects_half_shares() {
        let people = participants(3);
        // Двое делят половину, один — полную долю: веса 4, 2, 2 из 8.
        let entry = expense_with_weights(people[0], 100, &[(people[1], 2), (people[2], 2)]);

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
    fn falls_back_to_equal_split_when_everyone_is_excluded() {
        let people = participants(3);
        let entry = expense_with_weights(
            people[0],
            100,
            &[(people[0], 0), (people[1], 0), (people[2], 0)],
        );

        let shares = expense_shares(&entry, &people);

        // Спека: если сумма весов нулевая, расход делится на всех поровну.
        // Сумма взята неделимая, чтобы откат прошёл через раздачу остатка,
        // а не мимо неё.
        assert_eq!(shares[&people[0].id], 34);
        assert_eq!(shares[&people[1].id], 33);
        assert_eq!(shares[&people[2].id], 33);
        assert_eq!(shares.values().sum::<i64>(), 100);
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

    #[test]
    fn breaks_ties_by_position_not_by_id() {
        // Хелпер `participant` выводит id из позиции, поэтому во всех остальных
        // тестах два порядка совпадают и тай-брейк по position не проверяется.
        // Здесь они расходятся намеренно.
        let first = Participant {
            id: ParticipantId(Uuid::from_u128(9)),
            position: 0,
        };
        let second = Participant {
            id: ParticipantId(Uuid::from_u128(2)),
            position: 1,
        };
        let people = vec![first, second];
        let entry = expense(first, 3);

        let shares = expense_shares(&entry, &people);

        // Лишний рубль — добавленному раньше, а не тому, у кого меньше id.
        assert_eq!(shares[&first.id], 2);
        assert_eq!(shares[&second.id], 1);
    }

    #[test]
    fn single_participant_pays_the_whole_expense() {
        let people = participants(1);

        let full = expense(people[0], 101);
        assert_eq!(expense_shares(&full, &people)[&people[0].id], 101);

        // Даже если исключить единственного участника, платить всё равно ему:
        // сумма весов нулевая, значит срабатывает откат.
        let excluded = expense_with_weights(people[0], 101, &[(people[0], 0)]);
        assert_eq!(expense_shares(&excluded, &people)[&people[0].id], 101);
    }

    #[test]
    fn survives_amounts_that_would_overflow_i64_when_scaled() {
        let people = participants(3);
        let entry = expense(people[0], i64::MAX);

        let shares = expense_shares(&entry, &people);

        // Смысл расширения до i128: `amount * 4` в i64 здесь бы переполнилось.
        assert_eq!(shares.values().sum::<i64>(), i64::MAX);
    }

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
        let fixed = [
            pin(people[0], 700),
            pin(people[1], 400),
            pin(people[2], 100),
        ];

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
        let (katya, alexey, nastya, anya, zhenya, veronika) = (
            people[0], people[1], people[2], people[3], people[4], people[5],
        );
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
        let (katya, alexey, nastya, anya, zhenya, veronika) = (
            people[0], people[1], people[2], people[3], people[4], people[5],
        );
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
        let (katya, alexey, nastya, anya, zhenya, veronika) = (
            people[0], people[1], people[2], people[3], people[4], people[5],
        );
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
}
