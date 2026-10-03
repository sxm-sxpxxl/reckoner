import { describe, expect, it } from 'vitest'
import { previewSplit, splitStatus, type Pinned } from './sharePreview'

/** Участники в порядке `position`, как их отдаёт сервер. */
const three = ['a', 'b', 'c']
const four = ['a', 'b', 'c', 'd']

/** Ресторан из спеки — те же числа, что в `backend/src/domain/shares.rs`. */
const restaurant = ['katya', 'alexey', 'nastya', 'anya', 'zhenya', 'veronika']

describe('previewSplit', () => {
  it('без вписанных сумм делит поровну', () => {
    expect(previewSplit(400, four, {})).toEqual({ a: 100, b: 100, c: 100, d: 100 })
  })

  it('раздаёт остаток по порядку', () => {
    expect(previewSplit(100, three, {})).toEqual({ a: 34, b: 33, c: 33 })
  })

  it('вписанные платят ровно своё, остальные — остаток поровну', () => {
    expect(previewSplit(1001, four, { a: 400 })).toEqual({ a: 400, b: 201, c: 200, d: 200 })
  })

  it('ноль исключает из расхода', () => {
    expect(previewSplit(101, three, { c: 0 })).toEqual({ a: 51, b: 50, c: 0 })
  })

  it('пустым ничего, если вписанное покрывает расход', () => {
    expect(previewSplit(1000, three, { a: 600, b: 400 })).toEqual({ a: 600, b: 400, c: 0 })
  })

  it('вписано у всех и сходится — ровно вписанное', () => {
    expect(previewSplit(100, three, { a: 50, b: 30, c: 20 })).toEqual({ a: 50, b: 30, c: 20 })
  })

  it('скидку вычитает пропорционально', () => {
    expect(previewSplit(1000, three, { a: 700, b: 400, c: 100 })).toEqual({ a: 584, b: 333, c: 83 })
  })

  it('вписано больше расхода при пустых — пропорция, пустым ноль', () => {
    expect(previewSplit(1000, three, { a: 800, b: 400 })).toEqual({ a: 667, b: 333, c: 0 })
  })

  it('исключены все — делим на всех', () => {
    expect(previewSplit(100, three, { a: 0, b: 0, c: 0 })).toEqual({ a: 34, b: 33, c: 33 })
  })

  it('ресторан: шесть рублей раскладываются пропорционально', () => {
    expect(
      previewSplit(11996, restaurant, {
        katya: 2214,
        alexey: 3440,
        nastya: 0,
        anya: 0,
        zhenya: 3430,
        veronika: 2906,
      }),
    ).toEqual({ katya: 2215, alexey: 3442, nastya: 0, anya: 0, zhenya: 3432, veronika: 2907 })
  })

  it('ресторан: пара может вписать свою часть как угодно', () => {
    expect(
      previewSplit(11996, restaurant, {
        katya: 2214,
        alexey: 1720,
        nastya: 1720,
        anya: 0,
        zhenya: 3430,
        veronika: 2906,
      }),
    ).toEqual({ katya: 2215, alexey: 1721, nastya: 1721, anya: 0, zhenya: 3432, veronika: 2907 })
  })

  it('ресторан: пустое поле забирает остаток', () => {
    expect(
      previewSplit(11996, restaurant, { alexey: 3440, nastya: 0, anya: 0, zhenya: 3430, veronika: 2906 }),
    ).toEqual({ katya: 2220, alexey: 3440, nastya: 0, anya: 0, zhenya: 3430, veronika: 2906 })
  })

  it('сумма долей всегда равна расходу', () => {
    const cases: Pinned[] = [{}, { a: 400 }, { a: 0, b: 7 }, { a: 300, b: 300, c: 300, d: 100 }]

    for (const pinned of cases) {
      for (const amount of [1, 7, 999, 1000, 13700]) {
        const shares = previewSplit(amount, four, pinned)
        const total = Object.values(shares).reduce((sum, value) => sum + value, 0)

        expect(total).toBe(amount)
      }
    }
  })
})

describe('splitStatus', () => {
  it('ничего не вписано — сказать нечего', () => {
    expect(splitStatus(1000, four, {})).toEqual({ kind: 'plain' })
  })

  it('только исключённые — это обычное деление поровну', () => {
    expect(splitStatus(1000, four, { c: 0 })).toEqual({ kind: 'plain' })
  })

  it('вписано не у всех — остаток на пустых', () => {
    expect(splitStatus(1000, four, { a: 400 })).toEqual({ kind: 'rest', rest: 600, among: 3 })
  })

  it('вписано больше расхода при пустых — ошибка', () => {
    expect(splitStatus(1000, three, { a: 1200 })).toEqual({ kind: 'over', pinned: 1200 })
  })

  it('ресторан: шесть рублей разложены', () => {
    expect(
      splitStatus(11996, restaurant, {
        katya: 2214,
        alexey: 3440,
        nastya: 0,
        anya: 0,
        zhenya: 3430,
        veronika: 2906,
      }),
    ).toEqual({ kind: 'adjusted', difference: 6 })
  })

  it('скидка — разница отрицательная', () => {
    expect(splitStatus(1000, three, { a: 700, b: 400, c: 100 })).toEqual({
      kind: 'adjusted',
      difference: -200,
    })
  })

  it('четверть — граница, дальше — ошибка', () => {
    expect(splitStatus(1000, three, { a: 750, b: 0, c: 0 })).toEqual({
      kind: 'adjusted',
      difference: 250,
    })
    expect(splitStatus(1000, three, { a: 749, b: 0, c: 0 })).toEqual({ kind: 'far', pinned: 749 })
  })

  it('сходится ровно — сказать нечего', () => {
    expect(splitStatus(100, three, { a: 50, b: 30, c: 20 })).toEqual({ kind: 'plain' })
  })

  it('исключены все — сказать нечего', () => {
    expect(splitStatus(100, three, { a: 0, b: 0, c: 0 })).toEqual({ kind: 'plain' })
  })
})
