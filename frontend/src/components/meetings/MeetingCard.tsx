import { Link } from 'react-router-dom'

import { apiUrl } from '../../api/client'
import type { MeetingCard as MeetingCardData } from '../../api/types'
import { coverGradient } from '../../domain/cover'
import { formatCardDate } from '../../domain/format'
import { STATUS_TONES } from '../../domain/statusTone'
import AvatarStack from '../ui/AvatarStack'
import Money from '../ui/Money'
import StatusBadge from './StatusBadge'
import styles from './MeetingCard.module.css'

export default function MeetingCard({ meeting }: { meeting: MeetingCardData }) {
  const tone = STATUS_TONES[meeting.status]
  const cover = meeting.hasCover
    ? `url(${apiUrl(`/api/meetings/${meeting.id}/cover?v=${meeting.coverVersion}`)})`
    : coverGradient(meeting.id)

  return (
    // Ссылка, а не div с onClick: иначе встречу нельзя открыть в новой вкладке
    // и скопировать адрес, а поделиться ссылкой на встречу — смысл роутинга.
    <Link
      to={`/meetings/${meeting.id}`}
      className={styles.card}
      style={{ borderColor: tone.border }}
    >
      <div className={styles.cover} style={{ background: cover }}>
        <span className={styles.emoji} aria-hidden="true">
          {meeting.emoji}
        </span>
        <StatusBadge status={meeting.status} pendingTransfers={meeting.pendingTransfers} />
      </div>

      <div className={styles.body}>
        <div className={styles.titleRow}>
          <h2 className={styles.title}>{meeting.title}</h2>
          <span className={styles.date}>{formatCardDate(meeting.heldOn)}</span>
        </div>

        <div className={styles.footRow}>
          <AvatarStack participants={meeting.participants} />
          <Money amount={meeting.totalRubles} size={16} />
        </div>
      </div>
    </Link>
  )
}
