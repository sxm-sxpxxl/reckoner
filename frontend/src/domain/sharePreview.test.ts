import { describe, expect, it } from 'vitest'
import { previewShares } from './sharePreview'

/** Участники в порядке `position`, как их отдаёт сервер. */
const four = ['a', 'b', 'c', 'd']

describe('previewShares', () => {
  it('делит поровну, когда доли не заданы', () => {
    expect(previewShares(400, four, {})).toEqual({ a: 100, b: 100, c: 100, d: 100 })
  })

  it('раздаёт остаток по наибольшей дробной части, тай-брейк по порядку', () => {
    // 100 на трёх → 34 / 33 / 33: лишний рубль уходит первому по позиции.
    expect(previewShares(100, ['a', 'b', 'c'], {})).toEqual({ a: 34, b: 33, c: 33 })
  })

  it('учитывает четверти', () => {
    // Веса 1, ½, ½ на 100 ₽ → 50 / 25 / 25.
    expect(previewShares(100, ['a', 'b', 'c'], { b: 2, c: 2 })).toEqual({ a: 50, b: 25, c: 25 })
  })

  it('нулевая доля не платит ничего', () => {
    // Веса 1, ½, 0, 1 на 100 ₽ → 40 / 20 / 0 / 40.
    expect(previewShares(100, four, { b: 2, c: 0 })).toEqual({ a: 40, b: 20, c: 0, d: 40 })
  })

  it('если у всех доля 0 — считаем всех по одной', () => {
    // Так же поступает сервер: иначе расход не разделить вообще.
    expect(previewShares(90, ['a', 'b', 'c'], { a: 0, b: 0, c: 0 })).toEqual({
      a: 30,
      b: 30,
      c: 30,
    })
  })

  it('сумма долей всегда равна расходу', () => {
    // Тот же инвариант, что проверяет property-тест в Rust: без него план
    // переводов не закрылся бы в ноль.
    for (const amount of [1, 7, 13, 999, 13700]) {
      const shares = previewShares(amount, four, { c: 1 })
      const total = Object.values(shares).reduce((sum, value) => sum + value, 0)

      expect(total).toBe(amount)
    }
  })
})
