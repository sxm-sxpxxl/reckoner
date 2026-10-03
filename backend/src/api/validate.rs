//! Проверки, общие для нескольких ресурсов. Правило, продублированное
//! в двух ручках, однажды разойдётся — поэтому оно живёт в одном месте.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::db::facts;
use crate::db::records::ParticipantRow;
use crate::domain::{FixedShare, ParticipantId, SplitProblem, check_split};

use super::entries::ShareInput;
use super::error::ApiError;
use super::money::format_rubles;

/// Имя без пробелов по краям. Пустое имя — ошибка, а не «безымянный участник»:
/// такую карточку в интерфейсе не отличить от сбоя.
pub fn participant_name(raw: &str) -> Result<String, ApiError> {
    let trimmed = raw.trim();

    if trimmed.is_empty() {
        return Err(ApiError::validation("name", "имя не может быть пустым"));
    }

    Ok(trimmed.to_owned())
}

/// Эмодзи или значение по умолчанию: пустое поле — это «клиент не выбрал»,
/// а не ошибка ввода.
pub fn emoji_or_default(raw: Option<&str>, fallback: &str) -> String {
    raw.map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or(fallback)
        .to_owned()
}

/// Сумма — целое число рублей больше нуля. Ноль тоже ошибка: такая запись
/// ничего не меняет в расчёте и только мусорит историю.
pub fn amount(value: i64) -> Result<i64, ApiError> {
    if value <= 0 {
        return Err(ApiError::validation(
            "amountRubles",
            "сумма должна быть больше нуля",
        ));
    }

    Ok(value)
}

/// Участник должен принадлежать этой встрече. Поле в ошибке называет конкретный
/// параметр запроса, чтобы клиент подсветил нужный селект.
pub fn belongs_to_meeting<'a>(
    participants: &'a [ParticipantRow],
    id: Uuid,
    field: &'static str,
) -> Result<&'a ParticipantRow, ApiError> {
    participants
        .iter()
        .find(|row| row.id == id)
        .ok_or_else(|| ApiError::validation(field, "участник не найден в этой встрече"))
}

/// Явные доли, как их прислал клиент, в том виде, в каком они хранятся:
/// пары `(участник, рубли)`. `0` — участник исключён из расхода. Кого в списке
/// нет, тот делит остаток поровну.
pub fn shares(
    raw: &[ShareInput],
    participants: &[ParticipantRow],
) -> Result<Vec<(Uuid, i64)>, ApiError> {
    let mut seen: BTreeSet<Uuid> = BTreeSet::new();
    let mut stored = Vec::with_capacity(raw.len());

    for share in raw {
        if share.rubles < 0 {
            return Err(ApiError::validation(
                "shares",
                "сумма доли не может быть отрицательной",
            ));
        }

        belongs_to_meeting(participants, share.participant_id, "shares")?;

        if !seen.insert(share.participant_id) {
            return Err(ApiError::validation(
                "shares",
                "участник указан в долях дважды",
            ));
        }

        stored.push((share.participant_id, share.rubles));
    }

    Ok(stored)
}

/// Правила ввода разбивки: вписанное не больше расхода, пока кто-то делит
/// остаток, и не дальше четверти от него, если вписано у всех. Сам расчёт
/// примет что угодно — это защита от опечаток.
pub fn split(
    amount: i64,
    participants: &[ParticipantRow],
    shares: &[(Uuid, i64)],
) -> Result<(), ApiError> {
    let people: Vec<_> = participants.iter().map(facts::participant).collect();
    let fixed: Vec<FixedShare> = shares
        .iter()
        .map(|(participant_id, rubles)| FixedShare {
            participant_id: ParticipantId(*participant_id),
            rubles: *rubles,
        })
        .collect();

    check_split(amount, &people, &fixed).map_err(|problem| match problem {
        SplitProblem::PinnedOverAmount { pinned } => ApiError::validation(
            "shares",
            format!("вписано {} — больше, чем потрачено", format_rubles(pinned)),
        ),
        SplitProblem::PinnedFarFromAmount { pinned } => ApiError::validation(
            "shares",
            format!(
                "вписано {} из {} — проверьте суммы",
                format_rubles(pinned),
                format_rubles(amount)
            ),
        ),
    })
}
