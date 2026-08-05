//! Денежная логика встречи: доли расходов, балансы, план переводов, статус.
//!
//! Модуль намеренно ничего не знает ни про базу, ни про HTTP: на вход — обычные
//! структуры, на выход — посчитанные значения. Все суммы в целых рублях.

pub mod types;

#[cfg(test)]
pub mod testing;

pub use types::{
    Entry, EntryKind, MeetingFacts, Participant, ParticipantId, Weight, FULL_QUARTERS,
};
