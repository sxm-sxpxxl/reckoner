//! Денежная логика встречи: доли расходов, балансы, план переводов, статус.
//!
//! Модуль намеренно ничего не знает ни про базу, ни про HTTP: на вход — обычные
//! структуры, на выход — посчитанные значения. Все суммы в целых рублях.

pub mod balance;
pub mod reckoning;
pub mod settle;

#[cfg(test)]
mod properties;

pub mod shares;
pub mod status;
pub mod types;

#[cfg(test)]
pub mod testing;

pub use balance::{contributions, net_balances};
pub use reckoning::{Reckoning, reckon};
pub use settle::{Transfer, settlement_plan};
pub use shares::expense_shares;
pub use status::MeetingStatus;
pub use types::{
    Entry, EntryKind, FULL_QUARTERS, MeetingFacts, Participant, ParticipantId, Weight,
};
