import type { Entry, Participant, Share } from '../../api/types'
import { formatLogTime, formatRubles } from '../../domain/format'
import Avatar from '../ui/Avatar'
import styles from './HistoryRow.module.css'

/** Приписка к описанию расхода, если он делится не поровну на всех. */
function splitNote(shares: Share[]): string {
  if (shares.some((share) => share.rubles > 0)) return ' · точные суммы'
  if (shares.length > 0) return ' · делят не все'

  return ''
}

export default function HistoryRow({
  entry,
  people,
  onEdit,
  onRemove,
  removing,
}: {
  entry: Entry
  /** Участники по идентификатору — строка называет плательщика и получателя. */
  people: Map<string, Participant>
  onEdit: () => void
  onRemove: () => void
  removing: boolean
}) {
  const payer = people.get(entry.payerId)
  const recipient = entry.recipientId ? people.get(entry.recipientId) : undefined
  const isTransfer = entry.kind === 'transfer'

  const who = recipient ? `${payer?.name ?? '—'} → ${recipient.name}` : (payer?.name ?? '—')
  const note = isTransfer ? 'перевод в счёт долга' : entry.description + splitNote(entry.shares)

  return (
    <div className={styles.row}>
      {/* Клик по строке открывает правку — наше добавление к хендоффу: там
          запись можно только удалить и завести заново, потеряв разбивку. */}
      <button
        type="button"
        className={styles.main}
        onClick={onEdit}
        aria-label={`Редактировать: ${who}, ${formatRubles(entry.amountRubles)}`}
      >
        {payer && (
          <Avatar emoji={payer.emoji} colorIndex={payer.colorIndex} name={payer.name} size={36} />
        )}

        <span className={styles.body}>
          <span className={styles.who}>{who}</span>
          <span className={styles.note}>{note}</span>
        </span>

        <span className={styles.right}>
          <span
            className={styles.amount}
            style={{ color: isTransfer ? 'var(--ok-text)' : 'var(--ink)' }}
          >
            {isTransfer ? '↳ ' : ''}
            {formatRubles(entry.amountRubles)}
          </span>
          <span className={styles.time}>{formatLogTime(entry.occurredAt)}</span>
        </span>
      </button>

      <button
        type="button"
        className={styles.remove}
        onClick={onRemove}
        disabled={removing}
        aria-label="Удалить запись"
        title="Удалить"
      >
        ✕
      </button>
    </div>
  )
}
