//! Единственное место, где слой БД знает про доменные типы. Если отображение
//! поменяется, менять придётся только этот файл.

use std::collections::BTreeMap;

use sqlx::PgConnection;
use uuid::Uuid;

use super::records::{EntryKindRow, EntryRow, ParticipantRow, ShareRow};
use super::{entries, participants};
use crate::domain::{Entry, EntryKind, FixedShare, MeetingFacts, Participant, ParticipantId};

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

/// Собирает факты из уже прочитанных строк. Публичная, потому что список встреч
/// читает строки пачкой на все встречи сразу (`… where meeting_id = any($1)`)
/// и раскладывает их по встречам сам — тогда `load` на каждую встречу означал бы
/// три запроса на карточку.
pub fn build(
    participant_rows: &[ParticipantRow],
    entry_rows: &[EntryRow],
    share_rows: &[ShareRow],
) -> StoredFacts {
    let participants = participant_rows.iter().map(participant).collect();

    let mut shares_by_entry: BTreeMap<Uuid, Vec<FixedShare>> = BTreeMap::new();
    for share in share_rows {
        shares_by_entry
            .entry(share.entry_id)
            .or_default()
            .push(FixedShare {
                participant_id: ParticipantId(share.participant_id),
                rubles: share.rubles,
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
            fixed: shares_by_entry.remove(&row.id).unwrap_or_default(),
        })
        .collect();

    StoredFacts {
        participants,
        entries,
    }
}

/// Участник в терминах домена. Публичная: тем же отображением пользуется
/// проверка разбивки в слое API.
pub fn participant(row: &ParticipantRow) -> Participant {
    Participant {
        id: ParticipantId(row.id),
        position: row.position,
        paid_by: row.paid_by.map(ParticipantId),
    }
}
