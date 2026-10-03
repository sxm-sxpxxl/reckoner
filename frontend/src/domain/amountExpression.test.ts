import { describe, expect, it } from 'vitest'
import { parseAmountExpression } from './amountExpression'

describe('parseAmountExpression', () => {
  it('складывает позиции чека', () => {
    expect(parseAmountExpression('390 + 1200 + 624')).toEqual({
      rubles: 2214,
      rounded: false,
      compound: true,
      invalid: false,
    })
  })

  it('понимает разряды через пробел и запятую в дробной части', () => {
    expect(parseAmountExpression('1 250,50 + 99,50')).toEqual({
      rubles: 1350,
      rounded: false,
      compound: true,
      invalid: false,
    })
  })

  it('не теряет копейки на плавающей точке', () => {
    // В float 0.1 + 0.2 + 0.7 — это 0.9999999999999999, и округление вниз дало бы 0.
    expect(parseAmountExpression('0,1 + 0,2 + 0,7').rubles).toBe(1)
  })

  it('округляет вниз один раз — итог', () => {
    expect(parseAmountExpression('0,6 + 0,6')).toEqual({
      rubles: 1,
      rounded: true,
      compound: true,
      invalid: false,
    })
  })

  it('одно число — обычная сумма', () => {
    expect(parseAmountExpression('11996')).toEqual({
      rubles: 11996,
      rounded: false,
      compound: false,
      invalid: false,
    })
  })

  it('висящий плюс во время набора — ещё не ошибка', () => {
    expect(parseAmountExpression('390 + ')).toEqual({
      rubles: 390,
      rounded: false,
      compound: false,
      invalid: false,
    })
  })

  it('пустое поле — пусто, а не ноль', () => {
    for (const raw of ['', '   ', '+']) {
      const parsed = parseAmountExpression(raw)

      expect(Number.isNaN(parsed.rubles)).toBe(true)
      expect(parsed.invalid).toBe(false)
    }
  })

  it('мусор делает поле невалидным', () => {
    for (const raw of ['390 + абв', '-5', '12,5,3', '1e3']) {
      const parsed = parseAmountExpression(raw)

      expect(Number.isNaN(parsed.rubles)).toBe(true)
      expect(parsed.invalid).toBe(true)
    }
  })
})
