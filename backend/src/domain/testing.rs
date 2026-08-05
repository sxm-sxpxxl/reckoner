//! Хелперы для тестов домена. Компилируются только под `cargo test`.

use uuid::Uuid;

use super::types::{Entry, EntryKind, Participant, ParticipantId, Weight};

/// Участник с предсказуемым идентификатором: `participant(1)` всегда даёт
/// один и тот же id, поэтому ожидания в тестах можно писать явно.
pub fn participant(position: i32) -> Participant {
    Participant {
        id: ParticipantId(Uuid::from_u128(position as u128 + 1)),
        position,
    }
}

/// `count` участников с позициями `0..count`.
pub fn participants(count: i32) -> Vec<Participant> {
    (0..count).map(participant).collect()
}

/// Расход, который делится на всех поровну.
pub fn expense(payer: Participant, amount: i64) -> Entry {
    Entry {
        kind: EntryKind::Expense,
        payer_id: payer.id,
        recipient_id: None,
        amount,
        weights: Vec::new(),
    }
}

/// Расход с явными неполными долями: пары `(участник, четверти)`.
pub fn expense_with_weights(
    payer: Participant,
    amount: i64,
    weights: &[(Participant, u8)],
) -> Entry {
    Entry {
        weights: weights
            .iter()
            .map(|(participant, quarters)| Weight {
                participant_id: participant.id,
                quarters: *quarters,
            })
            .collect(),
        ..expense(payer, amount)
    }
}

pub fn transfer(from: Participant, to: Participant, amount: i64) -> Entry {
    Entry {
        kind: EntryKind::Transfer,
        payer_id: from.id,
        recipient_id: Some(to.id),
        amount,
        weights: Vec::new(),
    }
}
