import type { Participant } from '../../api/types'
import { formatRubles } from '../../domain/format'
import Avatar from '../ui/Avatar'
import styles from './ShareRow.module.css'

/** Подписи четвертей. Индекс — число четвертей. */
const CHIP_LABELS = ['0', '¼', '½', '¾', '1']

export default function ShareRow({
  participant,
  quarters,
  amount,
  onCycle,
}: {
  participant: Participant
  quarters: number
  amount: number
  onCycle: () => void
}) {
  const excluded = quarters === 0
  const partial = quarters > 0 && quarters < 4

  return (
    <div className={styles.row}>
      <Avatar
        emoji={participant.emoji}
        colorIndex={participant.colorIndex}
        name={participant.name}
        size={30}
      />

      <span className={`${styles.name} ${excluded ? styles.muted : ''}`}>{participant.name}</span>

      <span className={`${styles.amount} ${excluded ? styles.muted : ''}`}>
        {excluded ? '—' : formatRubles(amount)}
      </span>

      {/* Кнопка, а не div: иначе долю не выставить с клавиатуры. */}
      <button
        type="button"
        className={`${styles.chip} ${partial ? styles.partial : ''} ${excluded ? styles.zero : ''}`}
        onClick={onCycle}
        aria-label={`Доля ${participant.name}: ${CHIP_LABELS[quarters]}. Нажмите, чтобы уменьшить`}
      >
        {CHIP_LABELS[quarters]}
      </button>
    </div>
  )
}
