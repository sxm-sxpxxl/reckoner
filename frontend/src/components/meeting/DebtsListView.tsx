import { useMemo, useState } from 'react'

import type { Participant, Transfer } from '../../api/types'
import { formatRubles } from '../../domain/format'
import Avatar from '../ui/Avatar'
import Button from '../ui/Button'
import styles from './DebtsListView.module.css'

interface DebtorGroup {
  debtor: Participant
  transfers: Transfer[]
  total: number
}

export default function DebtsListView({
  settlement,
  people,
  onSettle,
}: {
  settlement: Transfer[]
  people: Map<string, Participant>
  /** Открывает модалку перевода с подставленными сторонами и суммой. */
  onSettle: (transfer: Transfer) => void
}) {
  // По умолчанию раскрыты — как в прототипе. Свёрнутые запоминаем, а не
  // наоборот: иначе после каждой правки всё снова раскрывалось бы.
  const [collapsed, setCollapsed] = useState<Record<string, boolean>>({})

  const groups = useMemo(() => {
    const byDebtor = new Map<string, DebtorGroup>()

    for (const transfer of settlement) {
      const debtor = people.get(transfer.fromId)

      if (!debtor) continue

      const group = byDebtor.get(transfer.fromId) ?? { debtor, transfers: [], total: 0 }

      group.transfers.push(transfer)
      group.total += transfer.amountRubles
      byDebtor.set(transfer.fromId, group)
    }

    return [...byDebtor.values()]
  }, [settlement, people])

  return (
    <div className={`${styles.list} stagger`}>
      {groups.map((group) => {
        const open = !collapsed[group.debtor.id]

        return (
          <div key={group.debtor.id} className={styles.card}>
            <button
              type="button"
              className={styles.head}
              aria-expanded={open}
              onClick={() =>
                setCollapsed((current) => ({
                  ...current,
                  [group.debtor.id]: open,
                }))
              }
            >
              <Avatar
                emoji={group.debtor.emoji}
                colorIndex={group.debtor.colorIndex}
                name={group.debtor.name}
                size={38}
              />
              <span className={styles.who}>
                <span className={styles.name}>{group.debtor.name}</span>
                <span className={styles.count}>переводов: {group.transfers.length}</span>
              </span>
              <span className={styles.total}>{formatRubles(group.total)}</span>
              <span className={styles.chevron} aria-hidden="true">
                {open ? '▲' : '▼'}
              </span>
            </button>

            {open && (
              <div className={styles.rows}>
                {group.transfers.map((transfer) => {
                  const creditor = people.get(transfer.toId)

                  return (
                    <div key={`${transfer.fromId}-${transfer.toId}`} className={styles.row}>
                      <span className={styles.arrow} aria-hidden="true">
                        →
                      </span>
                      {creditor && (
                        <Avatar
                          emoji={creditor.emoji}
                          colorIndex={creditor.colorIndex}
                          name={creditor.name}
                          size={26}
                        />
                      )}
                      <span className={styles.rowName}>{creditor?.name ?? '—'}</span>
                      <span className={styles.rowAmount}>
                        {formatRubles(transfer.amountRubles)}
                      </span>
                      <Button size="small" onClick={() => onSettle(transfer)}>
                        Отдано
                      </Button>
                    </div>
                  )
                })}
              </div>
            )}
          </div>
        )
      })}
    </div>
  )
}
