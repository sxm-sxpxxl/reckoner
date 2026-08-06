import { useMemo } from 'react'

import { useRemoveEntry } from '../../api/meetings'
import type { Entry, Meeting } from '../../api/types'
import Button from '../ui/Button'
import HistoryRow from './HistoryRow'
import styles from './HistorySection.module.css'

export default function HistorySection({
  meeting,
  onAddExpense,
  onAddTransfer,
  onEditEntry,
}: {
  meeting: Meeting
  onAddExpense: () => void
  onAddTransfer: () => void
  onEditEntry: (entry: Entry) => void
}) {
  const removeEntry = useRemoveEntry(meeting.id)
  const people = useMemo(
    () => new Map(meeting.participants.map((person) => [person.id, person])),
    [meeting.participants],
  )

  return (
    <section>
      <div className={styles.head}>
        <h2 className={styles.title}>История</h2>
        <div className={styles.actions}>
          <Button variant="primary" size="small" onClick={onAddExpense}>
            + Расход
          </Button>
          <Button size="small" onClick={onAddTransfer}>
            + Перевод
          </Button>
        </div>
      </div>

      {meeting.entries.length === 0 ? (
        <div className={styles.empty}>
          <span className={styles.emptyEmoji} aria-hidden="true">
            🧾
          </span>
          <h3 className={styles.emptyTitle}>Пока ни одной траты</h3>
          <p className={styles.emptyNote}>
            Добавьте первый расход — и внизу сразу появится, кто кому переводит
          </p>
          <Button variant="dark" onClick={onAddExpense}>
            Добавить расход
          </Button>
        </div>
      ) : (
        // Порядок задаёт сервер: новые сверху.
        <div className={`${styles.panel} stagger`}>
          {meeting.entries.map((entry) => (
            <HistoryRow
              key={entry.id}
              entry={entry}
              people={people}
              onEdit={() => onEditEntry(entry)}
              onRemove={() => removeEntry.mutate(entry.id)}
              removing={removeEntry.isPending && removeEntry.variables === entry.id}
            />
          ))}
        </div>
      )}
    </section>
  )
}
