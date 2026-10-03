import type { Participant } from '../../api/types'
import type { ParsedAmount } from '../../domain/amountExpression'
import { formatRubles } from '../../domain/format'
import AmountInput from '../ui/AmountInput'
import Avatar from '../ui/Avatar'
import form from './MeetingFormModal.module.css'
import styles from './ShareRow.module.css'

/**
 * Строка «Делим на»: участвует ли человек и сколько с него.
 *
 * Пустое поле — человек делит остаток поровну, его доля видна серым
 * плейсхолдером. Вписанное число — ровно его доля, если правило пропорции её
 * не сдвинуло; тогда под полем стоит итог.
 */
export default function ShareRow({
  participant,
  payer,
  included,
  text,
  parsed,
  share,
  onToggle,
  onText,
}: {
  participant: Participant
  /** Кто платит за участника — подпись «платит Женя». */
  payer?: Participant
  included: boolean
  text: string
  parsed: ParsedAmount
  /** Итоговая доля из превью. */
  share: number
  onToggle: () => void
  onText: (text: string) => void
}) {
  const pinned = included && Number.isFinite(parsed.rubles)
  // Под полем — что вышло из ввода: сумма позиций и сдвиг пропорцией.
  const sum = pinned && parsed.compound ? `= ${formatRubles(parsed.rubles)}` : ''
  const moved = pinned && share !== parsed.rubles ? `→ ${formatRubles(share)}` : ''
  const result = [sum, moved].filter(Boolean).join(' ')

  return (
    <div className={`${styles.row} ${included ? '' : styles.excluded}`}>
      {/* Кнопка, а не div: участие должно переключаться и с клавиатуры. */}
      <button
        type="button"
        className={styles.who}
        onClick={onToggle}
        aria-pressed={included}
        aria-label={`Участие: ${participant.name}`}
      >
        <Avatar
          emoji={participant.emoji}
          colorIndex={participant.colorIndex}
          name={participant.name}
          size={30}
        />
        <span className={styles.text}>
          <span className={styles.name}>{participant.name}</span>
          {payer && <span className={styles.caption}>платит {payer.name}</span>}
        </span>
      </button>

      {included ? (
        <span className={styles.money}>
          <AmountInput
            className={`${form.input} ${styles.input} ${parsed.invalid ? form.invalid : ''}`}
            value={text}
            onChange={onText}
            placeholder={formatRubles(share)}
            label={`Сумма: ${participant.name}`}
          />
          {result && <span className={styles.result}>{result}</span>}
        </span>
      ) : (
        <span className={styles.dash}>—</span>
      )}
    </div>
  )
}
