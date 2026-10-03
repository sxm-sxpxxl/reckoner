import { describe, expect, it } from 'vitest'
import { formatCardDate, formatLogTime, formatRubles, formatSigned, parseAmount } from './format'

// Неразрывный пробел записан escape-последовательностью намеренно: от обычного
// его глазами не отличить, и тест с обычным пробелом молча проверял бы не то.
// Тот же разделитель, что в текстах лога на сервере (backend/src/api/money.rs) —
// иначе одна сумма выглядела бы по-разному в истории и в логе.
const NBSP = '\u00A0'

describe('formatRubles', () => {
  it('разделяет разряды неразрывным пробелом', () => {
    expect(formatRubles(0)).toBe(`0${NBSP}₽`)
    expect(formatRubles(999)).toBe(`999${NBSP}₽`)
    expect(formatRubles(13700)).toBe(`13${NBSP}700${NBSP}₽`)
    expect(formatRubles(1234567)).toBe(`1${NBSP}234${NBSP}567${NBSP}₽`)
  })
})

describe('formatSigned', () => {
  it('ставит плюс кредитору и минус должнику', () => {
    // Знак ставим сами, а форматируем модуль, поэтому какой минус подставил бы
    // `Intl` — неважно. Это и причина так делать: у ru-RU он U+2212, и зависеть
    // от версии ICU не хочется.
    expect(formatSigned(4975)).toBe(`+4${NBSP}975${NBSP}₽`)
    expect(formatSigned(-1325)).toBe(`−1${NBSP}325${NBSP}₽`)
  })

  it('нулевой баланс называет словом, а не нулём', () => {
    expect(formatSigned(0)).toBe('ровно')
  })
})

describe('formatCardDate', () => {
  it('печатает день и месяц без года', () => {
    // heldOn приходит как «2026-07-23» — календарная дата без времени,
    // и разбирать её как момент времени нельзя: в зоне восточнее UTC
    // `new Date('2026-07-23')` даёт 23 июля 03:00, а западнее — 22 июля.
    expect(formatCardDate('2026-07-23')).toBe('23 июля')
    expect(formatCardDate('2026-08-03')).toBe('3 августа')
    expect(formatCardDate('2026-01-01')).toBe('1 января')
  })
})

describe('formatLogTime', () => {
  it('печатает дату и время через запятую', () => {
    // occurredAt — момент времени в UTC, показываем его в зоне пользователя.
    // Дата строится локальной и через toISOString уходит в UTC, поэтому тест
    // не зависит от того, в какой зоне его запустили.
    const moment = new Date(2026, 6, 23, 21, 5).toISOString()

    // Запятая — по скриншотам 03 и 04, где она есть и в истории, и в логе.
    // Текст хендоффа говорит `дд.мм чч:мм` без неё, но скриншоты высокой
    // точности, и им верим.
    expect(formatLogTime(moment)).toBe('23.07, 21:05')
  })
})

describe('parseAmount', () => {
  it('отбрасывает копейки и говорит об этом', () => {
    expect(parseAmount('1 250,50')).toEqual({ rubles: 1250, rounded: true })
    expect(parseAmount('3425')).toEqual({ rubles: 3425, rounded: false })
  })

  it('пустое поле — не число', () => {
    expect(Number.isNaN(parseAmount('').rubles)).toBe(true)
  })
})
