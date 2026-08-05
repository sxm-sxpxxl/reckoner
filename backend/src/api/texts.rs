//! Тексты лога «Что менялось» и значения по умолчанию.
//!
//! Все формулировки — в настоящем времени и без указания рода: поля пола
//! у участника нет, а «Настя добавил» — баг, а не мелочь. Собираются на сервере
//! и в той же транзакции пишутся в базу, поэтому разойтись с данными не могут.

use super::money::format_rubles;

/// Название встречи, созданной без названия: в дизайне встреча заводится одной
/// кнопкой, а назвать её можно потом.
pub const DEFAULT_MEETING_TITLE: &str = "Новая встреча";

/// Эмодзи новой встречи — как в прототипе.
pub const DEFAULT_MEETING_EMOJI: &str = "✨";

/// Эмодзи участника, если клиент не прислал своё. Значение — первое в палитре
/// прототипа, чтобы сервер и выбор в модалке не расходились.
pub const DEFAULT_PARTICIPANT_EMOJI: &str = "🐻";

/// Описание расхода, если поле пустое. Прототип делает то же самое при вводе,
/// но источник истины — сервер.
pub const DEFAULT_EXPENSE_DESCRIPTION: &str = "Без описания";

pub const MEETING_CREATED: &str = "Встреча создана";
pub const MEETING_EDITED: &str = "Встреча отредактирована";

pub fn participant_joined(name: &str) -> String {
    format!("{name} присоединяется к встрече")
}

pub fn participant_updated(name: &str) -> String {
    format!("Профиль участника обновлён: {name}")
}

pub fn participant_deleted(name: &str) -> String {
    format!("Участник удалён: {name}")
}

pub fn expense_added(payer: &str, description: &str, amount: i64) -> String {
    format!(
        "{payer} добавляет расход «{description}» — {}",
        format_rubles(amount)
    )
}

/// Один текст на все переводы. Спека содержала второй, отдельный для кнопки
/// «Отдано» в блоке долгов, но это тот же `POST /entries` с `kind: "transfer"`:
/// сервер эти случаи не различает.
pub fn transfer_added(payer: &str, recipient: &str, amount: i64) -> String {
    format!("{payer} переводит {} → {recipient}", format_rubles(amount))
}

pub fn entry_updated(amount: i64) -> String {
    format!("Запись изменена: {}", format_rubles(amount))
}

pub fn entry_deleted(amount: i64) -> String {
    format!("Удалена запись на {}", format_rubles(amount))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn participant_lines_do_not_mark_gender() {
        // Настоящее время — единственная причина, по которой эти строки вообще
        // собираются на сервере: «Настя присоединилась» и «Егор присоединился»
        // требовали бы поля пола, которого нет.
        assert_eq!(
            participant_joined("Настя"),
            "Настя присоединяется к встрече"
        );
        assert_eq!(
            participant_updated("Настя"),
            "Профиль участника обновлён: Настя"
        );
        assert_eq!(participant_deleted("Егор"), "Участник удалён: Егор");
    }

    #[test]
    fn expense_line_names_payer_description_and_amount() {
        assert_eq!(
            expense_added("Настя", "Продукты на все дни", 8400),
            "Настя добавляет расход «Продукты на все дни» — 8\u{a0}400\u{a0}₽"
        );
    }

    #[test]
    fn transfer_line_points_at_the_recipient() {
        // Один текст на все переводы, включая кнопку «Отдано» в блоке долгов:
        // это тот же `POST /entries` с `kind: "transfer"`, и сервер отличить
        // их не может.
        assert_eq!(
            transfer_added("Настя", "Влад", 3425),
            "Настя переводит 3\u{a0}425\u{a0}₽ → Влад"
        );
    }

    #[test]
    fn entry_lines_name_the_amount() {
        assert_eq!(entry_updated(8400), "Запись изменена: 8\u{a0}400\u{a0}₽");
        assert_eq!(entry_deleted(2100), "Удалена запись на 2\u{a0}100\u{a0}₽");
    }
}
