use std::collections::BTreeMap;

use super::balance::{contributions, net_balances};
use super::settle::{Transfer, settlement_plan};
use super::status::MeetingStatus;
use super::types::{EntryKind, MeetingFacts, ParticipantId};
use super::wallets::fold_into_wallets;

/// Всё, что считается по фактам встречи.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Reckoning {
    /// «внёс N ₽» — сумма оплаченных расходов.
    pub contributed: BTreeMap<ParticipantId, i64>,
    /// Баланс кошелька: плюс — ему должны, минус — он должен. У того, за кого
    /// платят, всегда 0: его баланс прибавлен к балансу плательщика.
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
    // Балансы считаются по людям, а сводятся по кошелькам: иначе в плане
    // появились бы переводы от тех, за кого платят другие.
    let net = fold_into_wallets(facts.participants, &net_balances(facts));
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
    use crate::domain::Participant;
    use crate::domain::testing::{expense, expense_with_fixed, paid_by, participants, transfer};

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
        let (katya, alexey, nastya, anya, zhenya, veronika) = (
            people[0], people[1], people[2], people[3], people[4], people[5],
        );
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
        let (katya, alexey, nastya, anya, zhenya, veronika) = (
            people[0], people[1], people[2], people[3], people[4], people[5],
        );
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
}
