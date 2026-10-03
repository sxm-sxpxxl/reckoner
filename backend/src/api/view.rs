//! Формы ответов. Всё, что уходит клиенту, описано здесь и только здесь.
//!
//! Ключи — `camelCase`: так их ждёт фронтенд. Значения — уже посчитанные:
//! балансы, план переводов и статус приходят из `domain::reckon`, а не
//! из базы.

use std::collections::BTreeMap;

use chrono::{DateTime, NaiveDate, Utc};
use serde::Serialize;
use uuid::Uuid;

use crate::db::facts;
use crate::db::records::{EntryKindRow, EntryRow, LogRow, MeetingRow, ParticipantRow, ShareRow};
use crate::domain::{MeetingStatus, ParticipantId, reckon};

/// Встреча целиком — ответ `GET /api/meetings/:id` и любой мутирующей ручки.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingView {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub emoji: String,
    pub held_on: NaiveDate,
    pub has_cover: bool,
    pub cover_version: i32,
    pub created_at: DateTime<Utc>,
    pub participants: Vec<ParticipantView>,
    pub entries: Vec<EntryView>,
    pub settlement: Vec<TransferView>,
    pub totals: TotalsView,
    pub status: MeetingStatusView,
    pub log: Vec<LogView>,
}

/// Карточка списка: то, что видно, не заходя во встречу.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MeetingCard {
    pub id: Uuid,
    pub title: String,
    pub description: String,
    pub emoji: String,
    pub held_on: NaiveDate,
    pub has_cover: bool,
    pub cover_version: i32,
    pub total_rubles: i64,
    pub participants: Vec<ParticipantChip>,
    pub pending_transfers: usize,
    pub status: MeetingStatusView,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantView {
    pub id: Uuid,
    pub name: String,
    pub emoji: String,
    pub color_index: i16,
    pub position: i32,
    /// «внёс N ₽»: только оплаченные расходы, отправленные переводы сюда
    /// не входят.
    pub contributed_rubles: i64,
    /// Баланс: плюс — должны ему, минус — должен он.
    pub net_rubles: i64,
}

/// Участник в карточке списка: аватар и имя, без денег.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ParticipantChip {
    pub id: Uuid,
    pub name: String,
    pub emoji: String,
    pub color_index: i16,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EntryView {
    pub id: Uuid,
    pub kind: EntryKindView,
    pub payer_id: Uuid,
    pub recipient_id: Option<Uuid>,
    pub amount_rubles: i64,
    pub description: String,
    pub occurred_at: DateTime<Utc>,
    pub shares: Vec<ShareView>,
    /// `false`, если разбивка задана хоть у кого-то: вписана сумма или участник
    /// исключён. Строки хранятся только у таких участников, поэтому флаг —
    /// это ровно «список долей пуст».
    pub shared_by_all: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum EntryKindView {
    Expense,
    Transfer,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShareView {
    pub participant_id: Uuid,
    /// `0` — исключён из расхода, больше нуля — вписанная сумма.
    pub rubles: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TransferView {
    pub from_id: Uuid,
    pub to_id: Uuid,
    pub amount_rubles: i64,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TotalsView {
    pub spent_rubles: i64,
    pub per_person_rubles: i64,
    pub pending_transfers: usize,
}

/// Статус для клиента. Количество переводов домен носит внутри статуса,
/// но в теле оно уже есть в `totals.pendingTransfers`, и дублировать его
/// в двух местах — способ однажды разойтись.
#[derive(Debug, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum MeetingStatusView {
    Settled,
    Attention,
    Alarm,
    NoParticipants,
}

impl From<MeetingStatus> for MeetingStatusView {
    fn from(status: MeetingStatus) -> Self {
        match status {
            MeetingStatus::NoParticipants => Self::NoParticipants,
            MeetingStatus::Settled => Self::Settled,
            MeetingStatus::Attention(_) => Self::Attention,
            MeetingStatus::Alarm(_) => Self::Alarm,
        }
    }
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LogView {
    pub id: i64,
    pub text: String,
    pub created_at: DateTime<Utc>,
}

pub fn meeting_view(
    meeting: &MeetingRow,
    participants: &[ParticipantRow],
    entries: &[EntryRow],
    shares: &[ShareRow],
    log: &[LogRow],
) -> MeetingView {
    let stored = facts::build(participants, entries, shares);
    let reckoning = reckon(stored.as_facts());
    let mut shares_by_entry = group_shares(shares);

    MeetingView {
        id: meeting.id,
        title: meeting.title.clone(),
        description: meeting.description.clone(),
        emoji: meeting.emoji.clone(),
        held_on: meeting.held_on,
        has_cover: meeting.cover_mime.is_some(),
        cover_version: meeting.cover_version,
        created_at: meeting.created_at,
        participants: participants
            .iter()
            .map(|row| ParticipantView {
                id: row.id,
                name: row.name.clone(),
                emoji: row.emoji.clone(),
                color_index: row.color_index,
                position: row.position,
                contributed_rubles: lookup(&reckoning.contributed, row.id),
                net_rubles: lookup(&reckoning.net, row.id),
            })
            .collect(),
        entries: entries
            .iter()
            .map(|row| {
                let shares = shares_by_entry.remove(&row.id).unwrap_or_default();
                EntryView {
                    id: row.id,
                    kind: match row.kind {
                        EntryKindRow::Expense => EntryKindView::Expense,
                        EntryKindRow::Transfer => EntryKindView::Transfer,
                    },
                    payer_id: row.payer_id,
                    recipient_id: row.recipient_id,
                    amount_rubles: row.amount_rubles,
                    description: row.description.clone(),
                    occurred_at: row.occurred_at,
                    shared_by_all: shares.is_empty(),
                    shares,
                }
            })
            .collect(),
        settlement: reckoning
            .settlement
            .iter()
            .map(|transfer| TransferView {
                from_id: transfer.from.0,
                to_id: transfer.to.0,
                amount_rubles: transfer.amount,
            })
            .collect(),
        totals: TotalsView {
            spent_rubles: reckoning.spent,
            per_person_rubles: reckoning.per_person,
            pending_transfers: reckoning.settlement.len(),
        },
        status: reckoning.status.into(),
        log: log
            .iter()
            .map(|row| LogView {
                id: row.id,
                text: row.text.clone(),
                created_at: row.created_at,
            })
            .collect(),
    }
}

pub fn meeting_card(
    meeting: &MeetingRow,
    participants: &[ParticipantRow],
    entries: &[EntryRow],
    shares: &[ShareRow],
) -> MeetingCard {
    let stored = facts::build(participants, entries, shares);
    let reckoning = reckon(stored.as_facts());

    MeetingCard {
        id: meeting.id,
        title: meeting.title.clone(),
        description: meeting.description.clone(),
        emoji: meeting.emoji.clone(),
        held_on: meeting.held_on,
        has_cover: meeting.cover_mime.is_some(),
        cover_version: meeting.cover_version,
        total_rubles: reckoning.spent,
        participants: participants
            .iter()
            .map(|row| ParticipantChip {
                id: row.id,
                name: row.name.clone(),
                emoji: row.emoji.clone(),
                color_index: row.color_index,
            })
            .collect(),
        pending_transfers: reckoning.settlement.len(),
        status: reckoning.status.into(),
    }
}

/// Участника, которого нет в карте расчёта, считаем в расчёте: так же
/// поступает и сам домен.
fn lookup(values: &BTreeMap<ParticipantId, i64>, id: Uuid) -> i64 {
    values.get(&ParticipantId(id)).copied().unwrap_or(0)
}

/// Доли, разложенные по записям. Внутри записи сортируются по идентификатору
/// участника: фронт ищет долю по `participantId`, а не по порядку, но ответ
/// на одни и те же данные обязан быть одинаковым — иначе его нельзя ни
/// закешировать, ни проверить тестом.
fn group_shares(shares: &[ShareRow]) -> BTreeMap<Uuid, Vec<ShareView>> {
    let mut grouped: BTreeMap<Uuid, Vec<ShareView>> = BTreeMap::new();

    for share in shares {
        grouped.entry(share.entry_id).or_default().push(ShareView {
            participant_id: share.participant_id,
            rubles: share.rubles,
        });
    }

    for list in grouped.values_mut() {
        list.sort_by_key(|share| share.participant_id);
    }

    grouped
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::TimeZone;

    fn timestamp(day: u32, hour: u32) -> DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 7, day, hour, 0, 0)
            .single()
            .expect("время")
    }

    fn meeting_row(id: Uuid) -> MeetingRow {
        MeetingRow {
            id,
            title: "Дача у Влада".to_owned(),
            description: "Три дня на природе".to_owned(),
            emoji: "🏡".to_owned(),
            held_on: NaiveDate::from_ymd_opt(2026, 7, 23).expect("дата"),
            cover_mime: None,
            cover_version: 0,
            created_at: timestamp(23, 19),
            updated_at: timestamp(23, 19),
        }
    }

    fn participant_row(meeting_id: Uuid, id: Uuid, name: &str, position: i32) -> ParticipantRow {
        ParticipantRow {
            id,
            meeting_id,
            name: name.to_owned(),
            emoji: "🦊".to_owned(),
            color_index: i16::try_from(position).expect("индекс цвета"),
            position,
            created_at: timestamp(23, 19),
        }
    }

    fn expense_row(meeting_id: Uuid, id: Uuid, payer_id: Uuid, amount: i64) -> EntryRow {
        EntryRow {
            id,
            meeting_id,
            kind: EntryKindRow::Expense,
            payer_id,
            recipient_id: None,
            amount_rubles: amount,
            description: "Продукты на все дни".to_owned(),
            occurred_at: timestamp(23, 20),
            created_at: timestamp(23, 20),
        }
    }

    #[test]
    fn meeting_view_matches_the_shape_from_the_spec() {
        // Числа те же, что в доменном тесте `fixture_dacha_needs_three_transfers_to_vlad`:
        // 13 700 на четверых, три перевода Владу.
        let meeting_id = Uuid::from_u128(1);
        let (nastya, vlad, egor, marina) = (
            Uuid::from_u128(11),
            Uuid::from_u128(12),
            Uuid::from_u128(13),
            Uuid::from_u128(14),
        );
        let meeting = meeting_row(meeting_id);
        let participants = vec![
            participant_row(meeting_id, nastya, "Настя", 0),
            participant_row(meeting_id, vlad, "Влад", 1),
            participant_row(meeting_id, egor, "Егор", 2),
            participant_row(meeting_id, marina, "Марина", 3),
        ];
        let entries = vec![
            expense_row(meeting_id, Uuid::from_u128(21), vlad, 8400),
            expense_row(meeting_id, Uuid::from_u128(22), nastya, 3200),
            expense_row(meeting_id, Uuid::from_u128(23), marina, 2100),
        ];
        let log = vec![LogRow {
            id: 12,
            text: "Встреча создана".to_owned(),
            created_at: timestamp(23, 19),
        }];

        let view = meeting_view(&meeting, &participants, &entries, &[], &log);
        let json = serde_json::to_value(&view).expect("сериализация");

        assert_eq!(json["id"], meeting_id.to_string());
        assert_eq!(json["heldOn"], "2026-07-23");
        assert_eq!(json["hasCover"], false);
        assert_eq!(json["coverVersion"], 0);
        assert_eq!(
            json["totals"],
            serde_json::json!({
                "spentRubles": 13700,
                "perPersonRubles": 3425,
                "pendingTransfers": 3
            })
        );
        assert_eq!(json["status"], "alarm");
        assert_eq!(json["participants"][0]["name"], "Настя");
        assert_eq!(json["participants"][0]["contributedRubles"], 3200);
        assert_eq!(json["participants"][0]["netRubles"], -225);
        assert_eq!(json["participants"][2]["contributedRubles"], 0);
        assert_eq!(json["participants"][2]["netRubles"], -3425);
        assert_eq!(
            json["settlement"],
            serde_json::json!([
                { "fromId": egor.to_string(), "toId": vlad.to_string(), "amountRubles": 3425 },
                { "fromId": marina.to_string(), "toId": vlad.to_string(), "amountRubles": 1325 },
                { "fromId": nastya.to_string(), "toId": vlad.to_string(), "amountRubles": 225 },
            ])
        );
        assert_eq!(json["entries"][0]["kind"], "expense");
        assert_eq!(json["entries"][0]["recipientId"], serde_json::Value::Null);
        assert_eq!(json["entries"][0]["amountRubles"], 8400);
        assert_eq!(json["entries"][0]["sharedByAll"], true);
        assert_eq!(json["entries"][0]["shares"], serde_json::json!([]));
        assert_eq!(json["log"][0]["text"], "Встреча создана");
    }

    #[test]
    fn fixed_share_clears_the_shared_by_all_flag() {
        // Строка в `entry_shares` есть только у тех, у кого разбивка задана,
        // поэтому любая строка означает «делят не все поровну».
        let meeting_id = Uuid::from_u128(2);
        let (first, second) = (Uuid::from_u128(31), Uuid::from_u128(32));
        let entry_id = Uuid::from_u128(41);
        let meeting = meeting_row(meeting_id);
        let participants = vec![
            participant_row(meeting_id, first, "Настя", 0),
            participant_row(meeting_id, second, "Влад", 1),
        ];
        let entries = vec![expense_row(meeting_id, entry_id, first, 100)];
        let shares = vec![ShareRow {
            entry_id,
            participant_id: second,
            rubles: 30,
        }];

        let view = meeting_view(&meeting, &participants, &entries, &shares, &[]);
        let json = serde_json::to_value(&view).expect("сериализация");

        assert_eq!(json["entries"][0]["sharedByAll"], false);
        assert_eq!(
            json["entries"][0]["shares"],
            serde_json::json!([{ "participantId": second.to_string(), "rubles": 30 }])
        );
        // Второму вписано 30, первый забирает остаток 70; заплатил первый,
        // значит второй должен ему свои 30.
        assert_eq!(json["participants"][1]["netRubles"], -30);
        assert_eq!(json["totals"]["pendingTransfers"], 1);
        assert_eq!(json["status"], "attention");
    }

    #[test]
    fn meeting_without_participants_has_its_own_status() {
        let meeting_id = Uuid::from_u128(3);
        let view = meeting_view(&meeting_row(meeting_id), &[], &[], &[], &[]);
        let json = serde_json::to_value(&view).expect("сериализация");

        assert_eq!(json["status"], "no-participants");
        assert_eq!(json["totals"]["perPersonRubles"], 0);
    }

    #[test]
    fn card_carries_only_what_the_list_shows() {
        let meeting_id = Uuid::from_u128(4);
        let payer = Uuid::from_u128(51);
        let meeting = meeting_row(meeting_id);
        let participants = vec![participant_row(meeting_id, payer, "Настя", 0)];
        let entries = vec![expense_row(meeting_id, Uuid::from_u128(61), payer, 700)];

        let card = meeting_card(&meeting, &participants, &entries, &[]);
        let json = serde_json::to_value(&card).expect("сериализация");

        assert_eq!(json["totalRubles"], 700);
        assert_eq!(json["pendingTransfers"], 0);
        assert_eq!(json["status"], "settled");
        assert_eq!(
            json["participants"],
            serde_json::json!([{
                "id": payer.to_string(),
                "name": "Настя",
                "emoji": "🦊",
                "colorIndex": 0
            }])
        );
        // Ни истории, ни лога, ни плана переводов: список их не показывает,
        // и возить их в каждой карточке значило бы тратить трафик впустую.
        assert!(json.get("entries").is_none());
        assert!(json.get("log").is_none());
        assert!(json.get("settlement").is_none());
    }
}
