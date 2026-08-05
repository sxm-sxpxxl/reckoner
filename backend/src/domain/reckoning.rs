use std::collections::BTreeMap;

use super::balance::{contributions, net_balances};
use super::settle::{Transfer, settlement_plan};
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
    // «На человека» — справочная цифра для статкарточки, а не основа расчёта:
    // при неделимой сумме она не совпадёт ни с чьей долей. Реальные доли
    // считает `expense_shares` по каждому расходу отдельно.
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
