//! Суммы для человека. Используется только в текстах лога: их пишет сервер,
//! и клиент показывает строку как есть.

/// `8 400 ₽`. Разряды разделяет неразрывный пробел (U+00A0) — так же, как это
/// делает `Intl.NumberFormat('ru-RU')` на клиенте, и по той же причине: число
/// не должно разрываться переносом строки.
pub fn format_rubles(amount: i64) -> String {
    let digits = amount.unsigned_abs().to_string();
    let mut result = String::with_capacity(digits.len() + digits.len() / 3 + 4);

    if amount < 0 {
        // U+2212 MINUS SIGN, а не дефис: это математический минус.
        result.push('−');
    }

    for (index, digit) in digits.chars().enumerate() {
        if index > 0 && (digits.len() - index).is_multiple_of(3) {
            result.push('\u{a0}');
        }
        result.push(digit);
    }

    result.push('\u{a0}');
    result.push('₽');
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn groups_thousands_with_a_non_breaking_space() {
        // Разделитель — U+00A0, а не обычный пробел: иначе число могло бы
        // разорваться переносом строки посередине. В тестах он записан
        // экранированием, потому что глазами эти два пробела не отличить.
        assert_eq!(format_rubles(0), "0\u{a0}₽");
        assert_eq!(format_rubles(999), "999\u{a0}₽");
        assert_eq!(format_rubles(1000), "1\u{a0}000\u{a0}₽");
        assert_eq!(format_rubles(8400), "8\u{a0}400\u{a0}₽");
        assert_eq!(format_rubles(13700), "13\u{a0}700\u{a0}₽");
        assert_eq!(format_rubles(1234567), "1\u{a0}234\u{a0}567\u{a0}₽");
    }

    #[test]
    fn negative_amount_keeps_its_sign() {
        // В записях сумма всегда больше нуля, но баланс участника бывает
        // отрицательным. Потерять минус хуже, чем не уметь его печатать.
        assert_eq!(format_rubles(-225), "−225\u{a0}₽");
    }
}
