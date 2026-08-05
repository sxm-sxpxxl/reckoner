use std::cmp::Reverse;
use std::collections::{BTreeMap, BTreeSet};

use super::types::{Entry, Participant, ParticipantId};

/// Доли одного расхода в целых рублях.
///
/// Гарантия: при неотрицательной сумме и уникальных участниках сумма всех
/// долей ровно равна `entry.amount`. На ней держится инвариант «сумма
/// балансов равна нулю», а на нём — способность плана переводов закрыть
/// встречу в ноль.
pub fn expense_shares(entry: &Entry, participants: &[Participant]) -> BTreeMap<ParticipantId, i64> {
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
    // Не только оптимизация: это первая из двух причин, по которым
    // `total_quarters` не может быть нулём. Вторую — нулевые веса у всех —
    // закрывает следующая задача.
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
    // с нулевой долей (его остаток всегда 0), а рублей остатка всегда строго
    // меньше, чем участников с ненулевым остатком. При равенстве — по порядку
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{expense, expense_with_weights, participants};

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
}
