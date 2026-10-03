import type { Participant } from '../../api/types'
import { coveredBy, payerOf } from '../../domain/wallets'
import Button from '../ui/Button'
import ParticipantCard from './ParticipantCard'
import styles from './ParticipantsSection.module.css'

export default function ParticipantsSection({
  participants,
  onAdd,
  onEdit,
}: {
  participants: Participant[]
  onAdd: () => void
  onEdit: (participant: Participant) => void
}) {
  return (
    <section className={styles.section}>
      <div className={styles.head}>
        <h2 className={styles.title}>Участники</h2>
        <Button onClick={onAdd}>+ Участник</Button>
      </div>

      {participants.length === 0 ? (
        <div className={styles.empty}>
          Пока никого. Добавьте участников — и можно записывать расходы.
        </div>
      ) : (
        <div className={`${styles.grid} stagger`}>
          {participants.map((participant) => (
            <ParticipantCard
              key={participant.id}
              participant={participant}
              payer={payerOf(participant, participants)}
              covers={coveredBy(participant, participants)}
              onEdit={() => onEdit(participant)}
            />
          ))}
        </div>
      )}
    </section>
  )
}
