//! «Платит за»: кошельки внутри встречи.
//!
//! Кошелёк — участник, за которого никто не платит, вместе с теми, за кого
//! платит он. Балансы считаются по каждому человеку, а план переводов — по
//! кошелькам: баланс оплачиваемого целиком переходит к его плательщику.

use std::collections::BTreeMap;

use super::types::{Participant, ParticipantId};

/// Кому зачисляется баланс участника.
///
/// Связь «платит за» действует, только если плательщик есть среди участников
/// встречи, это не сам участник и за самого плательщика никто не платит.
/// Иначе участник платит сам за себя. API таких связей не допускает, но они
/// могут возникнуть из двух одновременных правок, и тогда расчёт не должен ни
/// зациклиться, ни потерять рубли.
pub fn wallet_of(participant: &Participant, participants: &[Participant]) -> ParticipantId {
    let Some(payer_id) = participant.paid_by else {
        return participant.id;
    };

    match participants
        .iter()
        .find(|candidate| candidate.id == payer_id)
    {
        Some(payer) if payer.id != participant.id && payer.paid_by.is_none() => payer.id,
        _ => participant.id,
    }
}

/// Сворачивает балансы по кошелькам: баланс оплачиваемого прибавляется к
/// балансу плательщика, а сам становится нулевым. Сумма балансов при этом не
/// меняется — рубли только переезжают.
///
/// В результате есть каждый участник встречи; кого нет в `net`, считается
/// в расчёте.
pub fn fold_into_wallets(
    participants: &[Participant],
    net: &BTreeMap<ParticipantId, i64>,
) -> BTreeMap<ParticipantId, i64> {
    let mut folded: BTreeMap<ParticipantId, i64> = participants
        .iter()
        .map(|participant| (participant.id, 0))
        .collect();

    for participant in participants {
        let balance = net.get(&participant.id).copied().unwrap_or(0);
        *folded
            .get_mut(&wallet_of(participant, participants))
            .expect("кошелёк — всегда участник встречи") += balance;
    }

    folded
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::{paid_by, participant, participants};

    fn balances(entries: &[(Participant, i64)]) -> BTreeMap<ParticipantId, i64> {
        entries
            .iter()
            .map(|(person, balance)| (person.id, *balance))
            .collect()
    }

    #[test]
    fn participant_without_payer_is_their_own_wallet() {
        let people = participants(2);

        assert_eq!(wallet_of(&people[0], &people), people[0].id);
    }

    #[test]
    fn covered_balance_moves_to_the_payer() {
        let people = participants(3);
        let (katya, zhenya) = (people[0], people[2]);
        let anya = paid_by(people[1], zhenya);
        let people = vec![katya, anya, zhenya];

        let folded = fold_into_wallets(
            &people,
            &balances(&[(katya, -70), (anya, -30), (zhenya, 100)]),
        );

        assert_eq!(folded[&katya.id], -70);
        assert_eq!(folded[&anya.id], 0);
        assert_eq!(folded[&zhenya.id], 70);
    }

    #[test]
    fn one_payer_covers_several() {
        let people = participants(3);
        let alexey = people[0];
        let nastya = paid_by(people[1], alexey);
        let other = paid_by(people[2], alexey);
        let people = vec![alexey, nastya, other];

        let folded = fold_into_wallets(
            &people,
            &balances(&[(alexey, 50), (nastya, -20), (other, -30)]),
        );

        assert_eq!(folded[&alexey.id], 0);
        assert_eq!(folded[&nastya.id], 0);
        assert_eq!(folded[&other.id], 0);
    }

    #[test]
    fn link_to_a_covered_payer_is_ignored() {
        // Цепочка A → B → C. API её не допустит, но две одновременные правки
        // могут её создать. B уходит в кошелёк C, а связь A с B не действует:
        // иначе A оказался бы в кошельке того, кого самого нет.
        let people = participants(3);
        let c = people[2];
        let b = paid_by(people[1], c);
        let a = paid_by(people[0], b);
        let people = vec![a, b, c];

        assert_eq!(wallet_of(&a, &people), a.id);
        assert_eq!(wallet_of(&b, &people), c.id);

        let folded = fold_into_wallets(&people, &balances(&[(a, -10), (b, -20), (c, 30)]));
        assert_eq!(folded[&a.id], -10);
        assert_eq!(folded[&b.id], 0);
        assert_eq!(folded[&c.id], 10);
    }

    #[test]
    fn link_to_a_stranger_is_ignored() {
        let people = participants(1);
        let stranger = participant(99);
        let person = paid_by(people[0], stranger);

        assert_eq!(wallet_of(&person, &[person]), person.id);
    }

    #[test]
    fn folding_keeps_the_sum() {
        let people = participants(4);
        let people = vec![
            people[0],
            paid_by(people[1], people[0]),
            people[2],
            paid_by(people[3], people[2]),
        ];
        let net = balances(&[
            (people[0], 300),
            (people[1], -100),
            (people[2], -150),
            (people[3], -50),
        ]);

        let folded = fold_into_wallets(&people, &net);

        assert_eq!(folded.values().sum::<i64>(), net.values().sum::<i64>());
    }
}
