import type { Participant } from '../../api/types'
import { formatRubles } from '../../domain/format'
import Avatar from '../ui/Avatar'
import styles from './ParticipantCard.module.css'

/**
 * Подпись баланса без указания рода.
 *
 * В хендоффе «ему должны N ₽» и «должен N ₽» — на скриншоте это даёт
 * «Настя должен», а поля пола у участника нет и не будет.
 */
function balanceLine(net: number): { text: string; color: string } {
  if (net > 0) return { text: `должны ${formatRubles(net)}`, color: 'var(--ok-text)' }
  if (net < 0) return { text: `долг ${formatRubles(-net)}`, color: 'var(--debt)' }

  return { text: 'в расчёте', color: 'var(--text-3)' }
}

export default function ParticipantCard({
  participant,
  payer,
  covers,
  onEdit,
}: {
  participant: Participant
  /** Кто платит за участника — тогда вместо баланса «платит Женя». */
  payer?: Participant
  /** За кого платит участник — подпись «+ Аня» рядом с именем. */
  covers: Participant[]
  onEdit: () => void
}) {
  const balance = payer
    ? { text: `платит ${payer.name}`, color: 'var(--text-3)' }
    : balanceLine(participant.netRubles)

  return (
    <div className={styles.card}>
      <Avatar
        emoji={participant.emoji}
        colorIndex={participant.colorIndex}
        name={participant.name}
        size={42}
      />

      <div className={styles.body}>
        <div className={styles.name}>
          {participant.name}
          {covers.length > 0 && (
            <span className={styles.covers}> + {covers.map((person) => person.name).join(', ')}</span>
          )}
        </div>
        <div className={styles.contributed}>внёс {formatRubles(participant.contributedRubles)}</div>
        <div
          className={`${styles.balance} ${payer ? styles.payer : ''}`}
          style={{ color: balance.color }}
        >
          {balance.text}
        </div>
      </div>

      {/* Кнопка, а не вся карточка: карточка ничего больше не делает, и клик
          мимо карандаша не должен открывать модалку. */}
      <button
        type="button"
        className={styles.edit}
        onClick={onEdit}
        aria-label={`Редактировать участника ${participant.name}`}
        title="Редактировать"
      >
        ✎
      </button>
    </div>
  )
}
