/** Форматирование для показа. Все суммы — целые рубли: копеек в приложении нет. */

const RUBLES = new Intl.NumberFormat('ru-RU', { maximumFractionDigits: 0 })

/**
 * Разбор введённой суммы.
 *
 * Пробелы и запятая допускаются — так их и вводят. Копеек в приложении нет,
 * поэтому дробная часть отбрасывается, а `rounded` позволяет сказать об этом
 * вслух вместо того, чтобы молча потерять половину суммы.
 *
 * `rubles` — `NaN`, если ввод не число: вызывающий проверяет через
 * `Number.isFinite`.
 */
export function parseAmount(raw: string): { rubles: number; rounded: boolean } {
  const normalized = raw.replace(/\s/g, '').replace(',', '.')

  // `Number('')` — это 0, а пустое поле суммой считать нельзя.
  if (normalized === '') return { rubles: NaN, rounded: false }

  const value = Number(normalized)

  if (!Number.isFinite(value)) return { rubles: NaN, rounded: false }

  const rubles = Math.floor(value)

  return { rubles, rounded: rubles !== value }
}

/** `13 700 ₽`, разряды через неразрывный пробел. */
export function formatRubles(amount: number): string {
  // `Intl` для ru-RU уже разделяет разряды неразрывным пробелом; между числом
  // и знаком рубля ставим такой же, чтобы сумма не рвалась переносом строки.
  return `${RUBLES.format(amount)} ₽`
}

/** `+4 975 ₽` / `−1 325 ₽` / `ровно` — подпись баланса. */
export function formatSigned(net: number): string {
  if (net === 0) return 'ровно'

  // U+2212 MINUS SIGN, а не дефис: рядом с плюсом дефис выглядит короче и ниже.
  // Знак ставим сами, поэтому форматируем модуль.
  const sign = net > 0 ? '+' : '−'

  return `${sign}${formatRubles(Math.abs(net))}`
}

const CARD_DATE = new Intl.DateTimeFormat('ru-RU', { day: 'numeric', month: 'long' })

/**
 * `23 июля` из `2026-07-23`.
 *
 * Дата разбирается по частям, а не через `new Date(iso)`: строка вида
 * `2026-07-23` трактуется как полночь UTC, и в зоне пользователя это может
 * оказаться предыдущий день. Встреча — календарная дата, а не момент времени.
 */
export function formatCardDate(heldOn: string): string {
  const [year, month, day] = heldOn.split('-').map(Number)

  return CARD_DATE.format(new Date(year, month - 1, day))
}

/**
 * `23.07, 21:05` — история и лог. Момент времени, показываем в зоне клиента.
 *
 * Запятая — по скриншотам 03 и 04; текст хендоффа даёт `дд.мм чч:мм` без неё,
 * но скриншоты объявлены high-fidelity, и в обоих местах запятая есть.
 */
export function formatLogTime(iso: string): string {
  const moment = new Date(iso)
  const pad = (value: number) => String(value).padStart(2, '0')

  return `${pad(moment.getDate())}.${pad(moment.getMonth() + 1)}, ${pad(moment.getHours())}:${pad(moment.getMinutes())}`
}
