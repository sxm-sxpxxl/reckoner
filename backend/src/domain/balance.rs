use std::collections::BTreeMap;

use super::types::{EntryKind, MeetingFacts, ParticipantId};

/// Сколько каждый участник оплатил расходами — подпись «внёс N ₽».
/// Отправленные переводы сюда не входят.
/// В карте есть каждый участник встречи; тот, кто ничего не платил, получает 0.
pub fn contributions(facts: MeetingFacts) -> BTreeMap<ParticipantId, i64> {
    let mut paid: BTreeMap<ParticipantId, i64> = facts
        .participants
        .iter()
        .map(|participant| (participant.id, 0))
        .collect();

    for entry in facts.entries {
        if entry.kind == EntryKind::Expense
            && let Some(sum) = paid.get_mut(&entry.payer_id)
        {
            *sum += entry.amount;
        }
    }

    paid
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

    #[test]
    fn counts_expenses_but_not_transfers_from_the_same_payer() {
        let people = participants(2);
        let entries = vec![
            expense(people[0], 1000),
            transfer(people[0], people[1], 200),
        ];

        let paid = contributions(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        // Заплатил за встречу 1000 и отдельно перевёл 200 в счёт долга —
        // «внёс» относится только к первому.
        assert_eq!(paid[&people[0].id], 1000);
        assert_eq!(paid[&people[1].id], 0);
    }
}
