import type { Participant } from '../api/types'

/**
 * Кто фактически платит за участника.
 *
 * Повторяет `wallet_of` из `backend/src/domain/wallets.rs`: связь действует,
 * только если плательщик есть во встрече, это не сам участник и за самого
 * плательщика никто не платит. Иначе участник платит сам, и сервер считает
 * его баланс отдельно — показывать его надо так же.
 */
export function payerOf(person: Participant, participants: Participant[]): Participant | undefined {
  if (person.paidById === null) return undefined

  const payer = participants.find((other) => other.id === person.paidById)

  if (!payer || payer.id === person.id || payer.paidById !== null) return undefined

  return payer
}

/** Те, за кого платит участник, в порядке встречи. */
export function coveredBy(person: Participant, participants: Participant[]): Participant[] {
  return participants.filter((other) => payerOf(other, participants)?.id === person.id)
}

/** Подпись кошелька: «Женя + Аня, Настя» — или просто имя. */
export function walletName(person: Participant, participants: Participant[]): string {
  const covered = coveredBy(person, participants)

  return covered.length === 0
    ? person.name
    : `${person.name} + ${covered.map((other) => other.name).join(', ')}`
}
