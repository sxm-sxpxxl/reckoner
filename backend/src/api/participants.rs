//! Ручки участника. `colorIndex` и `position` назначает база — см.
//! `db::participants::insert`.

use axum::Json;
use axum::extract::{Path, State};
use serde::{Deserialize, Deserializer};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::db;

use super::JsonBody;
use super::error::ApiError;
use super::meetings;
use super::texts;
use super::validate;
use super::view::MeetingView;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CreateParticipant {
    pub name: String,
    pub emoji: Option<String>,
    /// Кто платит за нового участника; нет поля или `null` — платит сам.
    #[serde(default)]
    pub paid_by_id: Option<Uuid>,
}

pub async fn add_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    body: CreateParticipant,
) -> Result<MeetingView, ApiError> {
    meetings::require_meeting(&mut *conn, meeting_id).await?;
    let participants = db::participants::list_for_meeting(&mut *conn, meeting_id).await?;

    let name = validate::participant_name(&body.name)?;
    let emoji = validate::emoji_or_default(body.emoji.as_deref(), texts::DEFAULT_PARTICIPANT_EMOJI);
    let paid_by = match body.paid_by_id {
        Some(payer_id) => Some(validate::paid_by(&participants, None, payer_id)?),
        None => None,
    };

    let participant = db::participants::insert(&mut *conn, meeting_id, &name, &emoji).await?;
    db::log::append(
        &mut *conn,
        meeting_id,
        &texts::participant_joined(&participant.name),
    )
    .await?;

    if let Some(payer_id) = paid_by {
        db::participants::set_paid_by(&mut *conn, participant.id, Some(payer_id)).await?;
        db::log::append(
            &mut *conn,
            meeting_id,
            &texts::payer_assigned(&participant.name, name_of(&participants, payer_id)),
        )
        .await?;
    }

    meetings::load_view(conn, meeting_id).await
}

/// Имя участника из уже прочитанного состава встречи. Идентификатор к этому
/// моменту проверен, поэтому промах — ошибка в коде, а не во вводе.
fn name_of(participants: &[db::records::ParticipantRow], id: Uuid) -> &str {
    participants
        .iter()
        .find(|row| row.id == id)
        .map(|row| row.name.as_str())
        .expect("плательщик проверен валидацией")
}

/// `None` в поле означает «не менять». Позиция и цвет не меняются никогда:
/// они закреплены за участником с момента добавления.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateParticipant {
    pub name: Option<String>,
    pub emoji: Option<String>,
    /// `None` — не менять, `Some(None)` — снять плательщика,
    /// `Some(Some(id))` — назначить.
    #[serde(default, deserialize_with = "present")]
    pub paid_by_id: Option<Option<Uuid>>,
}

/// Отличает отсутствующее поле от `null`. Без этого serde схлопывает оба
/// случая в `None`, и снять плательщика было бы нельзя.
fn present<'de, D>(deserializer: D) -> Result<Option<Option<Uuid>>, D::Error>
where
    D: Deserializer<'de>,
{
    Option::<Uuid>::deserialize(deserializer).map(Some)
}

pub async fn update_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    participant_id: Uuid,
    body: UpdateParticipant,
) -> Result<MeetingView, ApiError> {
    let (current, participants) =
        require_participant(&mut *conn, meeting_id, participant_id).await?;

    // Запрос в слое данных требует оба значения: позиция и цвет остаются, а имя
    // с эмодзи перезаписываются целиком. Незаданное поле берём из текущей
    // строки — так частичная правка получается без второго запроса.
    let name = match body.name.as_deref() {
        Some(raw) => validate::participant_name(raw)?,
        None => current.name.clone(),
    };
    let emoji = validate::emoji_or_default(body.emoji.as_deref(), &current.emoji);
    let paid_by = match body.paid_by_id {
        None => current.paid_by,
        Some(None) => None,
        Some(Some(payer_id)) => Some(validate::paid_by(
            &participants,
            Some(current.id),
            payer_id,
        )?),
    };

    let updated = db::participants::update(&mut *conn, participant_id, &name, &emoji)
        .await?
        .ok_or(ApiError::NotFound)?;

    let profile_changed = updated.name != current.name || updated.emoji != current.emoji;
    let payer_changed = paid_by != current.paid_by;

    if payer_changed {
        db::participants::set_paid_by(&mut *conn, participant_id, paid_by).await?;
    }

    // Правка без изменений тоже пишет строку профиля — так было и до
    // плательщиков, и клиент на это не рассчитывает иначе.
    if profile_changed || !payer_changed {
        db::log::append(
            &mut *conn,
            meeting_id,
            &texts::participant_updated(&updated.name),
        )
        .await?;
    }

    if payer_changed {
        let line = match paid_by {
            Some(payer_id) => {
                texts::payer_assigned(&updated.name, name_of(&participants, payer_id))
            }
            None => texts::payer_removed(&updated.name),
        };
        db::log::append(&mut *conn, meeting_id, &line).await?;
    }

    meetings::load_view(conn, meeting_id).await
}

pub async fn remove_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    participant_id: Uuid,
) -> Result<MeetingView, ApiError> {
    // Имя нужно для строки лога, поэтому читается до удаления: после каскада
    // взять его негде.
    let (current, _) = require_participant(&mut *conn, meeting_id, participant_id).await?;
    let name = current.name.clone();

    db::participants::delete(&mut *conn, participant_id).await?;
    db::log::append(&mut *conn, meeting_id, &texts::participant_deleted(&name)).await?;

    meetings::load_view(conn, meeting_id).await
}

/// Участник, принадлежащий этой встрече, и весь её состав. Чужой — `404`,
/// а не `422`: по этому адресу его не существует, и различать «нет» и «есть,
/// но не здесь» клиенту незачем.
async fn require_participant(
    conn: &mut PgConnection,
    meeting_id: Uuid,
    participant_id: Uuid,
) -> Result<
    (
        db::records::ParticipantRow,
        Vec<db::records::ParticipantRow>,
    ),
    ApiError,
> {
    meetings::require_meeting(&mut *conn, meeting_id).await?;

    let participants = db::participants::list_for_meeting(&mut *conn, meeting_id).await?;
    let current = participants
        .iter()
        .find(|row| row.id == participant_id)
        .cloned()
        .ok_or(ApiError::NotFound)?;

    Ok((current, participants))
}

/// Ответ — `200`, а не `201`: тело описывает встречу, а не созданного участника,
/// и `Location` указывать некуда.
pub async fn create(
    State(pool): State<PgPool>,
    Path(meeting_id): Path<Uuid>,
    JsonBody(body): JsonBody<CreateParticipant>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = add_participant(&mut tx, meeting_id, body).await?;
    tx.commit().await?;

    Ok(Json(view))
}

pub async fn update(
    State(pool): State<PgPool>,
    Path((meeting_id, participant_id)): Path<(Uuid, Uuid)>,
    JsonBody(body): JsonBody<UpdateParticipant>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = update_participant(&mut tx, meeting_id, participant_id, body).await?;
    tx.commit().await?;

    Ok(Json(view))
}

pub async fn destroy(
    State(pool): State<PgPool>,
    Path((meeting_id, participant_id)): Path<(Uuid, Uuid)>,
) -> Result<Json<MeetingView>, ApiError> {
    let mut tx = pool.begin().await?;
    let view = remove_participant(&mut tx, meeting_id, participant_id).await?;
    tx.commit().await?;

    Ok(Json(view))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_null_and_id_payer_are_three_different_things() {
        let id = Uuid::from_u128(7);

        let missing: UpdateParticipant = serde_json::from_str("{}").expect("пустое тело");
        let null: UpdateParticipant = serde_json::from_str(r#"{"paidById":null}"#).expect("null");
        let set: UpdateParticipant =
            serde_json::from_str(&format!(r#"{{"paidById":"{id}"}}"#)).expect("id");

        assert_eq!(missing.paid_by_id, None);
        assert_eq!(null.paid_by_id, Some(None));
        assert_eq!(set.paid_by_id, Some(Some(id)));
    }
}
