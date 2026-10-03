/**
 * Разбор поля суммы, в котором можно складывать: «390 + 1200 + 624».
 *
 * Счёт в ресторане приходит по позициям, и складывать их в уме перед вводом —
 * ровно та работа, которую приложение должно забрать себе.
 *
 * Пробелы допускаются где угодно, десятичный разделитель — запятая или точка.
 * Считаем в копейках и округляем вниз один раз — итог, а не каждую позицию:
 * «99,50 + 0,50» — это ровно 100, а не 99.
 */

export interface ParsedAmount {
  /** Итог в целых рублях. `NaN`, если поле пустое или в нём не число. */
  rubles: number
  /** У итога была дробная часть, и она отброшена. */
  rounded: boolean
  /** Слагаемых больше одного — итог стоит показать под полем. */
  compound: boolean
  /** В поле есть что-то кроме чисел и плюсов. Пустое поле невалидным не считается. */
  invalid: boolean
}

/** Число без знака: «1250», «1250,5», «0.99». */
const TERM = /^\d+(?:[.,]\d+)?$/

const EMPTY: ParsedAmount = { rubles: NaN, rounded: false, compound: false, invalid: false }

export function parseAmountExpression(raw: string): ParsedAmount {
  // Пустые слагаемые — это висящий плюс во время набора («390 +»), а не ошибка.
  const terms = raw
    .split('+')
    .map((term) => term.replace(/\s/g, ''))
    .filter((term) => term !== '')

  if (terms.length === 0) return EMPTY

  const compound = terms.length > 1

  if (!terms.every((term) => TERM.test(term))) {
    return { rubles: NaN, rounded: false, compound, invalid: true }
  }

  const kopecks = terms.reduce((sum, term) => sum + toKopecks(term), 0)

  return {
    rubles: Math.floor(kopecks / 100),
    rounded: kopecks % 100 !== 0,
    compound,
    invalid: false,
  }
}

/** «1250,5» → 125050. Цифры после второй дробной — меньше копейки, их отбрасываем. */
function toKopecks(term: string): number {
  const [whole, fraction = ''] = term.replace(',', '.').split('.')

  return Number(whole) * 100 + Number(fraction.slice(0, 2).padEnd(2, '0'))
}
