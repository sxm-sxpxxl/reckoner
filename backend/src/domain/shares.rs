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
    for participant in participants {
        let numerator = i128::from(entry.amount) * i128::from(FULL_QUARTERS);
        let base = (numerator / i128::from(total_quarters)) as i64;
        shares.insert(participant.id, base);
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
}
