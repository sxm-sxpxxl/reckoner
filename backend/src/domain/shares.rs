use std::collections::BTreeMap;

use super::types::{Entry, FULL_QUARTERS, Participant, ParticipantId};

/// Доли одного расхода в целых рублях.
///
/// Гарантия: сумма всех долей ровно равна `entry.amount`. На ней держится
/// инвариант «сумма балансов равна нулю», а на нём — способность плана
/// переводов закрыть встречу в ноль.
pub fn expense_shares(entry: &Entry, participants: &[Participant]) -> BTreeMap<ParticipantId, i64> {
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
}
