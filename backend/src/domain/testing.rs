//! Хелперы для тестов домена. Компилируются только под `cargo test`.

use uuid::Uuid;

use super::types::{Entry, EntryKind, FixedShare, Participant, ParticipantId};

/// Участник с предсказуемым идентификатором: `participant(1)` всегда даёт
/// один и тот же id, поэтому ожидания в тестах можно писать явно.
pub fn participant(position: i32) -> Participant {
    debug_assert!(
        position >= 0,
        "position — это порядок добавления участника, начинается с нуля"
    );
    Participant {
        id: ParticipantId(Uuid::from_u128(position as u128 + 1)),
        position,
        paid_by: None,
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
        fixed: Vec::new(),
    }
}

/// Расход с явными долями: пары `(участник, рубли)`, `0` — исключён.
pub fn expense_with_fixed(payer: Participant, amount: i64, fixed: &[(Participant, i64)]) -> Entry {
    Entry {
        fixed: fixed
            .iter()
            .map(|(participant, rubles)| pin(*participant, *rubles))
            .collect(),
        ..expense(payer, amount)
    }
}

/// Перевод долга от одного участника другому.
pub fn transfer(from: Participant, to: Participant, amount: i64) -> Entry {
    Entry {
        kind: EntryKind::Transfer,
        payer_id: from.id,
        recipient_id: Some(to.id),
        amount,
        fixed: Vec::new(),
    }
}

/// Явная доля участника: вписанная сумма или `0`, если он исключён.
pub fn pin(participant: Participant, rubles: i64) -> FixedShare {
    FixedShare {
        participant_id: participant.id,
        rubles,
    }
}

/// Тот же участник, но за него платит `payer`.
pub fn paid_by(person: Participant, payer: Participant) -> Participant {
    Participant {
        paid_by: Some(payer.id),
        ..person
    }
}
