use super::settle::Transfer;

/// Статус встречи. Цвет карточки и текст бейджа в интерфейсе — производные
/// от него: зелёный, жёлтый, красный.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MeetingStatus {
    /// Участников ещё нет.
    NoParticipants,
    /// Все в расчёте, переводов не осталось.
    Settled,
    /// Остался один или два перевода.
    Attention(usize),
    /// Осталось три и более переводов.
    Alarm(usize),
}

impl MeetingStatus {
    /// Спека: нет участников → `NoParticipants`; 0 переводов → `Settled`;
    /// 1–2 → `Attention`; 3 и больше → `Alarm`.
    pub fn from_plan(participant_count: usize, plan: &[Transfer]) -> Self {
        if participant_count == 0 {
            return Self::NoParticipants;
        }

        match plan.len() {
            0 => Self::Settled,
            length @ 1..=2 => Self::Attention(length),
            length => Self::Alarm(length),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::domain::testing::participant;

    fn plan(length: usize) -> Vec<Transfer> {
        let from = participant(0);
        let to = participant(1);
        (0..length)
            .map(|_| Transfer {
                from: from.id,
                to: to.id,
                amount: 1,
            })
            .collect()
    }

    #[test]
    fn reports_no_participants_regardless_of_plan() {
        assert_eq!(
            MeetingStatus::from_plan(0, &plan(0)),
            MeetingStatus::NoParticipants
        );
        // Отсутствие участников важнее длины плана. В базе такой встречи не
        // бывает, но приоритет проверок должен быть закреплён.
        assert_eq!(
            MeetingStatus::from_plan(0, &plan(4)),
            MeetingStatus::NoParticipants
        );
    }

    #[test]
    fn empty_plan_means_settled() {
        assert_eq!(
            MeetingStatus::from_plan(5, &plan(0)),
            MeetingStatus::Settled
        );
    }

    #[test]
    fn one_or_two_transfers_need_attention() {
        assert_eq!(
            MeetingStatus::from_plan(3, &plan(1)),
            MeetingStatus::Attention(1)
        );
        assert_eq!(
            MeetingStatus::from_plan(3, &plan(2)),
            MeetingStatus::Attention(2)
        );
    }

    #[test]
    fn three_or_more_transfers_raise_alarm() {
        assert_eq!(
            MeetingStatus::from_plan(4, &plan(3)),
            MeetingStatus::Alarm(3)
        );
        assert_eq!(
            MeetingStatus::from_plan(9, &plan(7)),
            MeetingStatus::Alarm(7)
        );
    }
}
