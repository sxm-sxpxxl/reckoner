import { describe, expect, it } from 'vitest'
import { fitWithin } from './imageFile'

describe('fitWithin', () => {
  it('ужимает по большей стороне, сохраняя пропорции', () => {
    expect(fitWithin(3000, 2000, 1600)).toEqual({ width: 1600, height: 1067 })
    expect(fitWithin(1000, 4000, 1600)).toEqual({ width: 400, height: 1600 })
  })

  it('не растягивает то, что и так меньше предела', () => {
    // Иначе фото 800×600 превратилось бы в размытые 1600×1200 и стало бы
    // весить больше исходника без всякой пользы.
    expect(fitWithin(800, 600, 1600)).toEqual({ width: 800, height: 600 })
  })

  it('квадрат остаётся квадратом', () => {
    expect(fitWithin(2400, 2400, 1600)).toEqual({ width: 1600, height: 1600 })
  })

  it('никогда не возвращает нулевую сторону', () => {
    // Панорама 4000×10 при пределе 1600 даёт высоту 4 — округление вниз
    // до нуля сделало бы canvas невалидным.
    const fitted = fitWithin(4000, 10, 1600)

    expect(fitted.width).toBe(1600)
    expect(fitted.height).toBeGreaterThan(0)
  })
})
