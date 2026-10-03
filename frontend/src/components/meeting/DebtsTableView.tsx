import { useMemo } from 'react'

import type { Participant, Transfer } from '../../api/types'
import styles from './DebtsTableView.module.css'

/**
 * Матрица «Должник ↓ / кому →».
 *
 * Настоящая `<table>` со `scope`, а не сетка из div'ов: это таблица данных,
 * и экранный диктор должен читать её как таблицу — иначе непонятно, кто кому
 * должен.
 */
export default function DebtsTableView({
  settlement,
  participants,
  nameOf,
}: {
  settlement: Transfer[]
  participants: Participant[]
  /** Подпись кошелька: «Женя + Аня». */
  nameOf: (person: Participant) => string
}) {
  const amounts = useMemo(() => {
    const byPair = new Map<string, number>()

    for (const transfer of settlement) {
      byPair.set(`${transfer.fromId}→${transfer.toId}`, transfer.amountRubles)
    }

    return byPair
  }, [settlement])

  return (
    <div className={styles.scroller}>
      <table className={styles.table}>
        <caption className="visually-hidden" style={{ position: 'absolute', left: -9999 }}>
          Кто кому должен: строки — должники, столбцы — получатели
        </caption>
        <thead>
          <tr>
            <th className={styles.corner} scope="col">
              Должник
            </th>
            {participants.map((person) => (
              <th key={person.id} className={styles.colHead} scope="col" title={nameOf(person)}>
                <span aria-hidden="true">{person.emoji}</span>
                <span style={{ position: 'absolute', left: -9999 }}>{nameOf(person)}</span>
              </th>
            ))}
          </tr>
        </thead>
        <tbody className="stagger">
          {participants.map((debtor) => (
            <tr key={debtor.id}>
              <th className={styles.rowHead} scope="row">
                {debtor.emoji} {nameOf(debtor)}
              </th>
              {participants.map((creditor) => {
                if (debtor.id === creditor.id) {
                  return (
                    <td key={creditor.id} className={`${styles.cell} ${styles.none}`}>
                      ·
                    </td>
                  )
                }

                const amount = amounts.get(`${debtor.id}→${creditor.id}`)

                return (
                  <td
                    key={creditor.id}
                    className={`${styles.cell} ${amount ? styles.debt : styles.none}`}
                  >
                    {/* Сумма без «₽»: знак в каждой ячейке матрицы — шум. */}
                    {amount ? amount.toLocaleString('ru-RU') : '—'}
                  </td>
                )
              })}
            </tr>
          ))}
        </tbody>
      </table>
    </div>
  )
}
