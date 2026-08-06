/**
 * Живое превью долей расхода.
 *
 * Повторяет алгоритм из `backend/src/domain/shares.rs`, и это дублирование
 * осознанное: в модалке расхода сумма против каждого имени обновляется на
 * каждый ввод, а записи ещё нет — сходить за ней на сервер нельзя.
 *
 * Источник истины при этом остаётся сервер: превью показывает, а сохраняет
 * и пересчитывает он. Расхождение поймают тесты — они берут те же числа, что
 * доменные тесты в Rust.
 */

/** Полная доля в четвертях. Столько же в `domain::FULL_QUARTERS`. */
const FULL_QUARTERS = 4

/** Доли по идентификатору участника. Отсутствие ключа — полная доля. */
export type Quarters = Record<string, number>

/** Цикл по клику на чип доли: 1 → ¾ → ½ → ¼ → 0 → 1, как в хендоффе. */
export function nextQuarters(current: number): number {
  return current === 0 ? FULL_QUARTERS : current - 1
}

export function previewShares(
  amount: number,
  /** Участники в порядке `position` — он и есть тай-брейк при раздаче остатка. */
  participantIds: string[],
  quarters: Quarters,
): Record<string, number> {
  const weights = participantIds.map((id) => quarters[id] ?? FULL_QUARTERS)
  const totalWeight = weights.reduce((sum, weight) => sum + weight, 0)

  // Если исключили всех, делить не по чему. Считаем всех по полной доле —
  // так же поступает сервер.
  const effective = totalWeight === 0 ? weights.map(() => FULL_QUARTERS) : weights
  const effectiveTotal = totalWeight === 0 ? FULL_QUARTERS * weights.length : totalWeight

  const shares: Record<string, number> = {}

  if (effectiveTotal === 0) {
    for (const id of participantIds) shares[id] = 0

    return shares
  }

  // Целая часть каждому, остаток рублей — тем, у кого дробная часть больше.
  // Сравниваем не дроби, а числители: `amount * w` целое, поэтому остаток
  // от деления и есть дробная часть, только без потери точности.
  const remainders: { id: string; remainder: number; index: number }[] = []
  let distributed = 0

  participantIds.forEach((id, index) => {
    const numerator = amount * effective[index]
    const whole = Math.floor(numerator / effectiveTotal)

    shares[id] = whole
    distributed += whole
    remainders.push({ id, remainder: numerator % effectiveTotal, index })
  })

  remainders.sort((left, right) =>
    // Больший остаток вперёд; при равных — меньшая позиция.
    right.remainder === left.remainder
      ? left.index - right.index
      : right.remainder - left.remainder,
  )

  let leftover = amount - distributed

  for (const entry of remainders) {
    if (leftover <= 0) break
    // Участник с нулевым весом не должен получить рубль остатка: он не делит
    // этот расход вовсе.
    if (effective[entry.index] === 0) continue

    shares[entry.id] += 1
    leftover -= 1
  }

  return shares
}
