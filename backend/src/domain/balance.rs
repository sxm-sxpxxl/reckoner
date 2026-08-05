use std::collections::BTreeMap;

use super::shares::expense_shares;
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
        // Плательщик выложил деньги в обоих случаях: и когда это расход,
        // и когда это перевод.
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
                // Вложенные `if let` тут не пройдут clippy::collapsible_if —
                // в edition 2024 нужен let-chain.
                if let Some(recipient) = entry.recipient_id
                    && let Some(balance) = net.get_mut(&recipient)
                {
                    *balance -= entry.amount;
                }
            }
        }
    }

    net
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{expense, participant, participants, transfer};

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
        let entries = vec![expense(people[0], 100), transfer(people[1], people[0], 50)];

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
        let entries = vec![expense(people[0], 100), transfer(people[1], people[0], 150)];

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
}
