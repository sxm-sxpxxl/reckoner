import type { ParticipantChip } from '../../api/types'
import Avatar from './Avatar'
import styles from './Avatar.module.css'

const MAX_SHOWN = 5

/**
 * Аватары внахлёст. Больше пяти не показываем — вместо шестого ставим «+N».
 *
 * Хендофф говорит «максимум 5», но молча обрезать список нельзя: у встречи
 * на девятерых карточка выглядела бы так же, как у встречи на пятерых.
 */
export default function AvatarStack({
  participants,
  size = 28,
}: {
  participants: ParticipantChip[]
  size?: number
}) {
  const shown = participants.slice(0, MAX_SHOWN)
  const hidden = participants.length - shown.length

  return (
    <span className={styles.stack}>
      {shown.map((person) => (
        <Avatar
          key={person.id}
          emoji={person.emoji}
          colorIndex={person.colorIndex}
          name={person.name}
          size={size}
          ringed
        />
      ))}
      {hidden > 0 && (
        <span
          className={styles.more}
          style={{ width: size, height: size }}
          title={participants
            .slice(MAX_SHOWN)
            .map((person) => person.name)
            .join(', ')}
        >
          +{hidden}
        </span>
      )}
    </span>
  )
}
