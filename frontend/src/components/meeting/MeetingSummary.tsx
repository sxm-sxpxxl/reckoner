import type { Meeting } from '../../api/types'
import { formatCardDate, formatRubles } from '../../domain/format'
import { STATUS_TONES, pageStatusLabel } from '../../domain/statusTone'
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
        </div>

        <div className={styles.titleRow}>
          <h1 className={styles.title}>{meeting.title}</h1>
          <button
            type="button"
            className={styles.edit}
            onClick={onEdit}
            aria-label="Редактировать встречу"
            title="Редактировать встречу"
          >
            ✎
          </button>
        </div>

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
