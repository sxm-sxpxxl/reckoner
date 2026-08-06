import type { MeetingStatus } from '../../api/types'
import { STATUS_TONES } from '../../domain/statusTone'
import styles from './StatusBadge.module.css'

export default function StatusBadge({
  status,
  pendingTransfers,
}: {
  status: MeetingStatus
  pendingTransfers: number
}) {
  const tone = STATUS_TONES[status]

  return (
    <span
      className={styles.badge}
      style={{ color: tone.text, background: tone.bg, borderColor: tone.border }}
    >
      {tone.label(pendingTransfers)}
    </span>
  )
}
