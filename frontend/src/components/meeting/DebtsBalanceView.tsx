import type { Participant } from '../../api/types'
import { formatSigned } from '../../domain/format'
import styles from './DebtsBalanceView.module.css'

/** Минимальная видимая длина полосы: иначе баланс в один рубль на фоне
 *  тысяч выглядел бы как ноль. */
const MIN_PERCENT = 1.2

export default function DebtsBalanceView({
  participants,
  nameOf,
}: {
  participants: Participant[]
  /** Подпись кошелька: «Женя + Аня». */
  nameOf: (person: Participant) => string
}) {
  const scale = Math.max(...participants.map((person) => Math.abs(person.netRubles)), 1)

  return (
    <div className={`${styles.panel} stagger`}>
      {participants.map((person) => {
        const net = person.netRubles
        const width = net === 0 ? 0 : Math.max((Math.abs(net) / scale) * 50, MIN_PERCENT)
        const color = net > 0 ? 'var(--ok-accent)' : 'var(--debt)'

        return (
          <div key={person.id} className={styles.row}>
            <div className={styles.head}>
              <span className={styles.name}>
                {person.emoji} {nameOf(person)}
              </span>
              <span
                className={styles.value}
                style={{
                  color: net > 0 ? 'var(--ok-text)' : net < 0 ? 'var(--debt)' : 'var(--text-3)',
                }}
              >
                {formatSigned(net)}
              </span>
            </div>

            <div className={styles.track}>
              <span className={styles.axis} aria-hidden="true" />
              {width > 0 && (
                <span
                  className={styles.bar}
                  // Полоса растёт от центра: долг влево, профицит вправо.
                  style={
                    net < 0
                      ? { right: '50%', width: `${width}%`, background: color }
                      : { left: '50%', width: `${width}%`, background: color }
                  }
                />
              )}
            </div>
          </div>
        )
      })}

      <p className={styles.note}>Слева — кто должен, справа — кому должны</p>
    </div>
  )
}
