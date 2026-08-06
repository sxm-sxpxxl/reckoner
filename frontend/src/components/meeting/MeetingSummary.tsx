import type { Meeting } from '../../api/types'
import { formatCardDate, formatRubles } from '../../domain/format'
import { STATUS_TONES, pageStatusLabel } from '../../domain/statusTone'
import Button from '../ui/Button'
import CoverDropzone from './CoverDropzone'
import StatCard from './StatCard'
import styles from './MeetingSummary.module.css'

export default function MeetingSummary({
  meeting,
  onEdit,
}: {
  meeting: Meeting
  onEdit: () => void
}) {
  const tone = STATUS_TONES[meeting.status]

  return (
    <div className={styles.head}>
      <CoverDropzone
        meetingId={meeting.id}
        emoji={meeting.emoji}
        hasCover={meeting.hasCover}
        coverVersion={meeting.coverVersion}
      />

      <div className={styles.side}>
        <div className={styles.topRow}>
          <span
            className={styles.badge}
            style={{ color: tone.text, background: tone.bg, borderColor: tone.border }}
          >
            {pageStatusLabel(meeting.status, meeting.totals.pendingTransfers)}
          </span>
          <span className={styles.date}>{formatCardDate(meeting.heldOn)}</span>

          {/* Кнопка с подписью, а не бледный карандаш у заголовка: правка
              встречи — не редкая операция, и её надо видеть, не наводя мышь. */}
          <Button size="small" onClick={onEdit} className={styles.edit}>
            ✎ Изменить
          </Button>
        </div>

        <h1 className={styles.title}>{meeting.title}</h1>

        {meeting.description && <p className={styles.description}>{meeting.description}</p>}

        <div className={styles.stats}>
          <StatCard label="Потратили" value={formatRubles(meeting.totals.spentRubles)} />
          <StatCard label="На человека" value={formatRubles(meeting.totals.perPersonRubles)} />
          <StatCard
            label="Осталось переводов"
            value={String(meeting.totals.pendingTransfers)}
            color={tone.text}
          />
        </div>
      </div>
    </div>
  )
}
