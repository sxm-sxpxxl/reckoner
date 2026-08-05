//! Единственное место, где слой БД знает про доменные типы. Если отображение
//! поменяется, менять придётся только этот файл.

use std::collections::BTreeMap;

use sqlx::PgConnection;
use uuid::Uuid;

use super::records::{EntryKindRow, EntryRow, ParticipantRow, ShareRow};
use super::{entries, participants};
use crate::domain::{Entry, EntryKind, MeetingFacts, Participant, ParticipantId, Weight};

/// Владеющий аналог `MeetingFacts`: домен принимает срезы по ссылке, поэтому
/// кому-то надо владеть векторами.
#[derive(Debug, Clone)]
pub struct StoredFacts {
    participants: Vec<Participant>,
    entries: Vec<Entry>,
}

impl StoredFacts {
    pub fn as_facts(&self) -> MeetingFacts<'_> {
        MeetingFacts {
            participants: &self.participants,
            entries: &self.entries,
        }
    }
}

/// Читает участников, записи и доли встречи и складывает их в доменные типы.
/// Три запроса, а не N+1: доли берутся все сразу и раскладываются по записям
/// в памяти.
pub async fn load(conn: &mut PgConnection, meeting_id: Uuid) -> Result<StoredFacts, sqlx::Error> {
    let participant_rows = participants::list_for_meeting(&mut *conn, meeting_id).await?;
    let entry_rows = entries::list_for_meeting(&mut *conn, meeting_id).await?;
    let share_rows = entries::shares_for_meeting(&mut *conn, meeting_id).await?;

    Ok(build(&participant_rows, &entry_rows, &share_rows))
}

fn build(
    participant_rows: &[ParticipantRow],
    entry_rows: &[EntryRow],
    share_rows: &[ShareRow],
) -> StoredFacts {
    let participants = participant_rows
        .iter()
        .map(|row| Participant {
            id: ParticipantId(row.id),
            position: row.position,
        })
        .collect();

    let mut shares_by_entry: BTreeMap<Uuid, Vec<Weight>> = BTreeMap::new();
    for share in share_rows {
        shares_by_entry
            .entry(share.entry_id)
            .or_default()
            .push(Weight {
                participant_id: ParticipantId(share.participant_id),
                // Диапазон 0..=3 задан CHECK в схеме, поэтому преобразование
                // не может не сойтись. Берём `try_from`, а не `as`: если схема
                // и код однажды разойдутся, лучше упасть здесь, чем молча
                // завернуть значение и испортить расчёт долей.
                quarters: u8::try_from(share.weight_quarters)
                    .expect("weight_quarters вне диапазона 0..=3 — схема и код разошлись"),
            });
    }

    let entries = entry_rows
        .iter()
        .map(|row| Entry {
            kind: match row.kind {
                EntryKindRow::Expense => EntryKind::Expense,
                EntryKindRow::Transfer => EntryKind::Transfer,
            },
            payer_id: ParticipantId(row.payer_id),
            recipient_id: row.recipient_id.map(ParticipantId),
            amount: row.amount_rubles,
            weights: shares_by_entry.remove(&row.id).unwrap_or_default(),
        })
        .collect();

    StoredFacts {
        participants,
        entries,
    }
}
