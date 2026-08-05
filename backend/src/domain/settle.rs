use std::cmp::Reverse;
use std::collections::BTreeMap;

use super::types::{Participant, ParticipantId};

/// Один перевод в плане закрытия встречи.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Transfer {
    pub from: ParticipantId,
    pub to: ParticipantId,
    pub amount: i64,
}

/// Минимальный по количеству список переводов, закрывающий встречу в ноль.
///
/// Жадно сводим наибольшего должника с наибольшим кредитором: на каждом шаге
/// закрывается хотя бы одна из двух сторон, поэтому переводов выходит не больше
/// `должники + кредиторы − 1` — меньше уже нельзя.
///
/// План закрывает все балансы в ноль тогда и только тогда, когда сумма балансов
/// нулевая. `net_balances` это гарантирует для данных, где все идентификаторы
/// принадлежат встрече.
///
/// Результат детерминированный: участники упорядочиваются по `position`, а
/// сортировка по суммам стабильная, поэтому при равных суммах первым переводит
/// тот, кого добавили раньше, независимо от порядка строк из базы.
pub fn settlement_plan(
    participants: &[Participant],
    net: &BTreeMap<ParticipantId, i64>,
) -> Vec<Transfer> {
    let mut ordered: Vec<Participant> = participants.to_vec();
    ordered.sort_by_key(|participant| participant.position);

    // Участник, которого нет в карте балансов, считается в расчёте.
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

    // `sort_by_key` стабильная, поэтому при равных суммах сохраняется порядок
    // по position, заданный выше.
    debtors.sort_by_key(|(_, amount)| Reverse(*amount));
    creditors.sort_by_key(|(_, amount)| Reverse(*amount));

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

        // Долги и кредиты строго больше нуля по построению, значит `amount > 0`
        // и хотя бы один индекс сдвинется — цикл конечен.
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
        let net = balances(&[(nastya, -225), (vlad, 4975), (egor, -3425), (marina, -1325)]);

        let plan = settlement_plan(&people, &net);

        assert_eq!(
            plan,
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
    }

    #[test]
    fn splits_a_debt_across_two_creditors() {
        let people = participants(3);
        let net = balances(&[(people[0], -300), (people[1], 200), (people[2], 100)]);

        let plan = settlement_plan(&people, &net);

        assert_eq!(
            plan,
            vec![
                Transfer {
                    from: people[0].id,
                    to: people[1].id,
                    amount: 200
                },
                Transfer {
                    from: people[0].id,
                    to: people[2].id,
                    amount: 100
                },
            ]
        );
    }

    #[test]
    fn breaks_equal_debts_by_position_not_by_input_order() {
        // Двое должны одинаково — кто переводит первым, решает порядок
        // добавления, а не порядок строк, пришедших из базы. Срез намеренно
        // перемешан: без сортировки по position план вышел бы обратным.
        let people = participants(3);
        let (early, late, creditor) = (people[0], people[1], people[2]);
        let net = balances(&[(early, -100), (late, -100), (creditor, 200)]);

        let plan = settlement_plan(&[late, creditor, early], &net);

        assert_eq!(
            plan,
            vec![
                Transfer {
                    from: early.id,
                    to: creditor.id,
                    amount: 100
                },
                Transfer {
                    from: late.id,
                    to: creditor.id,
                    amount: 100
                },
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
