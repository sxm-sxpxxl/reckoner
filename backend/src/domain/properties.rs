//! Property-тесты инвариантов домена. Файл содержит только тесты.

use proptest::prelude::*;

use super::testing::{expense_with_weights, participant, transfer};
use super::types::{Entry, MeetingFacts, Participant};
use super::{net_balances, settlement_plan};

/// Черновик записи в терминах индексов участников. Proptest генерирует любые
/// индексы, а `build` приводит их к существующим по модулю — так любой
/// сгенерированный вход остаётся корректным и ничего не приходится отбрасывать.
#[derive(Debug, Clone)]
enum Draft {
    Expense {
        payer: usize,
        amount: i64,
        quarters: Vec<u8>,
    },
    Transfer {
        from: usize,
        to: usize,
        amount: i64,
    },
}

fn draft_strategy() -> impl Strategy<Value = Draft> {
    prop_oneof![
        (
            0usize..64,
            1i64..1_000_000,
            prop::collection::vec(0u8..=4, 1..8),
        )
            .prop_map(|(payer, amount, quarters)| Draft::Expense {
                payer,
                amount,
                quarters,
            }),
        (0usize..64, 0usize..64, 1i64..1_000_000).prop_map(|(from, to, amount)| Draft::Transfer {
            from,
            to,
            amount
        }),
    ]
}

fn roster(count: usize) -> Vec<Participant> {
    (0..count as i32).map(participant).collect()
}

fn build(people: &[Participant], drafts: &[Draft]) -> Vec<Entry> {
    let count = people.len();
    drafts
        .iter()
        .filter_map(|draft| match draft {
            Draft::Expense {
                payer,
                amount,
                quarters,
            } => {
                // Полная доля (4/4) в списке весов не хранится.
                let weights: Vec<(Participant, u8)> = people
                    .iter()
                    .enumerate()
                    .filter_map(|(index, person)| {
                        let quarter = quarters[index % quarters.len()];
                        (quarter < 4).then_some((*person, quarter))
                    })
                    .collect();
                Some(expense_with_weights(
                    people[*payer % count],
                    *amount,
                    &weights,
                ))
            }
            Draft::Transfer { from, to, amount } => {
                let sender = *from % count;
                let recipient = *to % count;
                // Перевод самому себе запрещён на уровне API, домен его не увидит.
                (sender != recipient).then(|| transfer(people[sender], people[recipient], *amount))
            }
        })
        .collect()
}

proptest! {
    #![proptest_config(ProptestConfig::with_cases(256))]

    /// Сумма балансов всегда ноль. Инвариант обеспечивает целочисленная
    /// раздача остатка долей, и именно на нём держится способность плана
    /// закрыть встречу.
    #[test]
    fn balances_sum_to_zero(
        count in 1usize..8,
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count);
        let entries = build(&people, &drafts);

        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        prop_assert_eq!(net.values().sum::<i64>(), 0);
    }

    /// План сводит каждый баланс в ноль, и переводов в нём строго меньше, чем
    /// участников: это и означает «минимальный», а не «все всем».
    ///
    /// Граница следует из устройства алгоритма: на каждом шаге закрывается хотя
    /// бы одна сторона, а последний шаг закрывает обе, поэтому переводов не
    /// больше `должники + кредиторы − 1`, а это не больше `count − 1`.
    #[test]
    fn settlement_plan_zeroes_every_balance(
        count in 1usize..8,
        drafts in prop::collection::vec(draft_strategy(), 0..12),
    ) {
        let people = roster(count);
        let entries = build(&people, &drafts);
        let net = net_balances(MeetingFacts {
            participants: &people,
            entries: &entries,
        });

        let plan = settlement_plan(&people, &net);

        let mut settled = net.clone();
        for item in &plan {
            *settled.get_mut(&item.from).expect("должник есть") += item.amount;
            *settled.get_mut(&item.to).expect("кредитор есть") -= item.amount;
        }
        prop_assert!(
            settled.values().all(|balance| *balance == 0),
            "после плана остались долги: {:?}",
            settled
        );
        prop_assert!(
            plan.len() < count,
            "переводов {} при {} участниках — план не минимальный",
            plan.len(),
            count
        );
    }
}
