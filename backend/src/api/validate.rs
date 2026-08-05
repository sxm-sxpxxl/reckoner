//! Проверки, общие для нескольких ресурсов. Правило, продублированное
//! в двух ручках, однажды разойдётся — поэтому оно живёт в одном месте.

use std::collections::BTreeSet;

use uuid::Uuid;

use crate::db::records::ParticipantRow;
use crate::domain::FULL_QUARTERS;

use super::entries::ShareInput;
use super::error::ApiError;

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

/// Доли, приведённые к тому, что хранится в базе. `raw` — как прислал клиент.
///
/// Полная доля (4/4) отбрасывается: её отсутствие в таблице и есть полная доля,
/// а хранить её значило бы держать одно и то же в двух видах.
pub fn shares(
    raw: &[ShareInput],
    participants: &[ParticipantRow],
) -> Result<Vec<(Uuid, i16)>, ApiError> {
    let mut seen: BTreeSet<Uuid> = BTreeSet::new();
    let mut stored = Vec::with_capacity(raw.len());

    for share in raw {
        if !(0..=FULL_QUARTERS).contains(&i64::from(share.weight_quarters)) {
            return Err(ApiError::validation(
                "shares",
                "доля задаётся четвертями от 0 до 4",
            ));
        }

        belongs_to_meeting(participants, share.participant_id, "shares")?;

        if !seen.insert(share.participant_id) {
            return Err(ApiError::validation(
                "shares",
                "участник указан в долях дважды",
            ));
        }

        if i64::from(share.weight_quarters) == FULL_QUARTERS {
            continue;
        }

        stored.push((share.participant_id, share.weight_quarters));
    }

    Ok(stored)
}
