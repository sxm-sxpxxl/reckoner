/**
 * Живое превью разбивки расхода.
 *
 * Повторяет `split_amount` и `check_split` из `backend/src/domain/shares.rs`,
 * и это дублирование осознанное: в модалке расхода доля против каждого имени
 * обновляется на каждый ввод, а записи ещё нет — сходить за ней на сервер нельзя.
 *
 * Источник истины — сервер: превью показывает, а сохраняет и пересчитывает он.
 * Расхождение поймают тесты: они берут те же числа, что доменные тесты в Rust.
 */

/** Вписанные суммы по участнику: `0` — исключён. Нет ключа — делит остаток поровну. */
export type Pinned = Record<string, number>

export function previewSplit(
  amount: number,
  /** Участники в порядке `position` — он и есть тай-брейк при раздаче остатка. */
  participantIds: string[],
  pinned: Pinned,
): Record<string, number> {
  if (participantIds.length === 0) return {}

  const fixed = participantIds.map((id) => pinned[id])
  const total = fixed.reduce<number>((sum, value) => sum + (value ?? 0), 0)
  const someoneEven = fixed.some((value) => value === undefined)

  // Правило 1: вписанные платят своё, остаток — поровну между остальными.
  if (someoneEven && total <= amount) {
    const rest = distribute(
      amount - total,
      participantIds,
      fixed.map((value) => (value === undefined ? 1 : 0)),
    )

    return Object.fromEntries(
      participantIds.map((id, index) => [id, rest[id] + (fixed[index] ?? 0)]),
    )
  }

  // Правило 2: пропорционально вписанному; у кого суммы нет — ноль.
  if (total > 0) return distribute(amount, participantIds, fixed.map((value) => value ?? 0))

  // Правило 3: исключены все — делим на всех поровну.
  return distribute(amount, participantIds, participantIds.map(() => 1))
}

/**
 * Что сказать под списком «Делим на». `over` и `far` запрещают сохранение —
 * сервер ответил бы на них 422.
 */
export type SplitStatus =
  | { kind: 'plain' }
  | { kind: 'rest'; rest: number; among: number }
  | { kind: 'adjusted'; difference: number }
  | { kind: 'over'; pinned: number }
  | { kind: 'far'; pinned: number }

export function splitStatus(amount: number, participantIds: string[], pinned: Pinned): SplitStatus {
  const fixed = participantIds.map((id) => pinned[id])
  const total = fixed.reduce<number>((sum, value) => sum + (value ?? 0), 0)
  const even = fixed.filter((value) => value === undefined).length
  const anyPositive = fixed.some((value) => value !== undefined && value > 0)

  if (even > 0) {
    if (total > amount) return { kind: 'over', pinned: total }

    // Если ничего не вписано, это обычное деление поровну, и говорить нечего.
    return anyPositive ? { kind: 'rest', rest: amount - total, among: even } : { kind: 'plain' }
  }

  if (total === 0) return { kind: 'plain' }
  if (4 * Math.abs(amount - total) > amount) return { kind: 'far', pinned: total }

  return total === amount ? { kind: 'plain' } : { kind: 'adjusted', difference: amount - total }
}

/**
 * Целая часть каждому, остаток по рублю тем, у кого больше дробная часть,
 * при равенстве — по порядку. `BigInt`, потому что `amount × вес` с вписанными
 * суммами в вес может выйти за 2^53, и тогда остатки сравнивались бы неточно.
 */
function distribute(amount: number, ids: string[], weights: number[]): Record<string, number> {
  const total = weights.reduce((sum, weight) => sum + BigInt(weight), 0n)
  const shares: Record<string, number> = {}
  const remainders: { id: string; remainder: bigint; index: number }[] = []
  let distributed = 0n

  ids.forEach((id, index) => {
    const numerator = BigInt(amount) * BigInt(weights[index])
    const whole = numerator / total

    shares[id] = Number(whole)
    distributed += whole
    remainders.push({ id, remainder: numerator % total, index })
  })

  remainders.sort((left, right) =>
    left.remainder === right.remainder
      ? left.index - right.index
      : right.remainder > left.remainder
        ? 1
        : -1,
  )

  let leftover = BigInt(amount) - distributed

  for (const entry of remainders) {
    if (leftover <= 0n) break

    shares[entry.id] += 1
    leftover -= 1n
  }

  return shares
}
