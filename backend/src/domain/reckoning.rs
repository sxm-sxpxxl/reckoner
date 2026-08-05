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
    use crate::domain::testing::{expense, participants, transfer};

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
    fn fixture_bbq_has_participants_but_no_expenses() {
        // «Солевые шашлыки»: 5 участников, расходов нет — зелёный статус
        // и нулевая сумма на карточке (скриншот 01-meetings-list.png).
        let people = participants(5);

        let reckoning = reckon(MeetingFacts {
            participants: &people,
            entries: &[],
        });

        assert_eq!(reckoning.spent, 0);
        assert_eq!(reckoning.per_person, 0);
        assert!(reckoning.net.values().all(|balance| *balance == 0));
        assert!(reckoning.settlement.is_empty());
        assert_eq!(reckoning.status, MeetingStatus::Settled);
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
        assert_eq!(reckoning.contributed[&vlad.id], 8400);
        assert_eq!(reckoning.contributed[&egor.id], 0);
        assert_eq!(reckoning.net[&nastya.id], -225);
        assert_eq!(reckoning.net[&vlad.id], 4975);
        assert_eq!(reckoning.net[&egor.id], -3425);
        assert_eq!(reckoning.net[&marina.id], -1325);
        assert_eq!(
            reckoning.settlement,
            vec![
                Transfer {
                    from: egor.id,
                    to: vlad.id,
                    amount: 3425
                },
                Transfer {
                    from: marina.id,
                    to: vlad.id,
                    amount: 1325
                },
                Transfer {
                    from: nastya.id,
                    to: vlad.id,
                    amount: 225
                },
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
            vec![Transfer {
                from: lesha_z.id,
                to: lesha_p.id,
                amount: 220
            }]
        );
        assert_eq!(reckoning.status, MeetingStatus::Attention(1));
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
