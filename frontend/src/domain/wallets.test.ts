import { describe, expect, it } from 'vitest'
import type { Participant } from '../api/types'
import { coveredBy, payerOf, walletName } from './wallets'

function person(id: string, paidById: string | null = null): Participant {
  return {
    id,
    name: id,
    emoji: '🦊',
    colorIndex: 0,
    position: 0,
    contributedRubles: 0,
    netRubles: 0,
    paidById,
  }
}

describe('payerOf', () => {
  it('у того, кто платит сам, плательщика нет', () => {
    const people = [person('Женя')]

    expect(payerOf(people[0], people)).toBeUndefined()
  })

  it('находит плательщика', () => {
    const people = [person('Аня', 'Женя'), person('Женя')]

    expect(payerOf(people[0], people)?.id).toBe('Женя')
  })

  it('связь с тем, за кого самого платят, не действует — как на сервере', () => {
    const people = [person('А', 'Б'), person('Б', 'В'), person('В')]

    expect(payerOf(people[0], people)).toBeUndefined()
    expect(payerOf(people[1], people)?.id).toBe('В')
  })

  it('связь с посторонним не действует', () => {
    const people = [person('Аня', 'кто-то')]

    expect(payerOf(people[0], people)).toBeUndefined()
  })
})

describe('coveredBy и walletName', () => {
  const people = [person('Женя'), person('Аня', 'Женя'), person('Катя'), person('Настя', 'Женя')]

  it('перечисляет оплачиваемых в порядке встречи', () => {
    expect(coveredBy(people[0], people).map((other) => other.id)).toEqual(['Аня', 'Настя'])
  })

  it('подписывает кошелёк', () => {
    expect(walletName(people[0], people)).toBe('Женя + Аня, Настя')
    expect(walletName(people[2], people)).toBe('Катя')
  })
})
