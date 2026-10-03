use uuid::Uuid;

/// Идентификатор участника. Newtype, чтобы его нельзя было спутать
/// с идентификатором встречи или записи.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ParticipantId(pub Uuid);

impl From<Uuid> for ParticipantId {
    fn from(value: Uuid) -> Self {
        Self(value)
    }
}

/// Участник встречи. Домену нужны идентификатор, порядок добавления —
/// `position` делает раздачу остатка рублей детерминированной — и связь
/// «платит за».
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Participant {
    pub id: ParticipantId,
    pub position: i32,
    /// Кто платит за участника: другой участник той же встречи. `None` —
    /// платит сам за себя. Как это влияет на расчёт — `wallets.rs`.
    pub paid_by: Option<ParticipantId>,
}

/// Запись — это либо расход (потрачено на встрече, делится между участниками),
/// либо перевод от одного участника другому в счёт погашения долга.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum EntryKind {
    Expense,
    Transfer,
}

/// Явная доля участника в расходе, в рублях. `0` — участник из расхода
/// исключён. Участник без такой записи делит остаток поровну.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct FixedShare {
    pub participant_id: ParticipantId,
    pub rubles: i64,
}

/// Расход или перевод. Суммы — целые рубли, всегда больше нуля;
/// это гарантирует слой API.
///
/// Тип один в один повторяет строку таблицы `entries` вместе с её долями,
/// поэтому `recipient_id` заполнен только у перевода, а `fixed` у перевода
/// всегда пусто. Эти инварианты проверяются CHECK-constraint'ами в схеме
/// базы, а не типами: домен получает уже провалидированные строки.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Entry {
    pub kind: EntryKind,
    pub payer_id: ParticipantId,
    /// Заполнен только у перевода.
    pub recipient_id: Option<ParticipantId>,
    pub amount: i64,
    /// Явные доли расхода: вписанные суммы и исключённые участники (`0`).
    /// Кого здесь нет, тот делит остаток поровну. У перевода всегда пусто.
    pub fixed: Vec<FixedShare>,
}

/// Факты встречи — всё, что домену нужно для расчёта.
#[derive(Debug, Clone, Copy)]
pub struct MeetingFacts<'a> {
    pub participants: &'a [Participant],
    pub entries: &'a [Entry],
}
