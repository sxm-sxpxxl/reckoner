import { useMemo, useState } from 'react'

import type { Meeting, Transfer } from '../../api/types'
import SegmentedControl from '../ui/SegmentedControl'
import DebtsBalanceView from './DebtsBalanceView'
import DebtsListView from './DebtsListView'
import DebtsTableView from './DebtsTableView'
import SettledPanel from './SettledPanel'
import styles from './DebtsSection.module.css'

type View = 'people' | 'table' | 'balance'

const SEGMENTS = [
  { value: 'people' as const, label: 'Списком' },
  { value: 'table' as const, label: 'Таблицей' },
  { value: 'balance' as const, label: 'Баланс' },
]

export default function DebtsSection({
  meeting,
  onSettle,
}: {
  meeting: Meeting
  onSettle: (transfer: Transfer) => void
}) {
  const [view, setView] = useState<View>('people')
  const people = useMemo(
    () => new Map(meeting.participants.map((person) => [person.id, person])),
    [meeting.participants],
  )

  const settled = meeting.settlement.length === 0 && meeting.entries.length > 0

  return (
    <section>
      <div className={styles.head}>
        <h2 className={styles.title}>Кто кому должен</h2>
        {!settled && (
          <SegmentedControl
            segments={SEGMENTS}
            value={view}
            onChange={setView}
            label="Представление долгов"
          />
        )}
      </div>

      {settled ? (
        <SettledPanel />
      ) : meeting.entries.length === 0 ? (
        <div className={styles.empty}>
          Пока нечего делить. Добавьте расход — и здесь появится, кто кому переводит.
        </div>
      ) : view === 'people' ? (
        <DebtsListView settlement={meeting.settlement} people={people} onSettle={onSettle} />
      ) : view === 'table' ? (
        <DebtsTableView settlement={meeting.settlement} participants={meeting.participants} />
      ) : (
        <DebtsBalanceView participants={meeting.participants} />
      )}
    </section>
  )
}
