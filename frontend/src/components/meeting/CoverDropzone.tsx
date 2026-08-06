import styles from './CoverDropzone.module.css'

/**
 * Обложка встречи.
 *
 * Пока только показывает: загрузка файла — отдельный этап вместе с серверными
 * ручками, сжатием через canvas и `ETag`. Поэтому зона не принимает `drop`
 * и курсор остаётся обычным — активная зона, которая молча ничего не делает,
 * обманывает сильнее, чем честно неактивная.
 */
export default function CoverDropzone({
  meetingId,
  emoji,
  hasCover,
  coverVersion,
}: {
  meetingId: string
  emoji: string
  hasCover: boolean
  coverVersion: number
}) {
  if (hasCover) {
    return (
      <div
        className={`${styles.zone} ${styles.filled}`}
        style={{ backgroundImage: `url(/api/meetings/${meetingId}/cover?v=${coverVersion})` }}
        role="img"
        aria-label="Обложка встречи"
      />
    )
  }

  return (
    <div className={styles.zone}>
      <div>
        <span className={styles.emoji} aria-hidden="true">
          {emoji}
        </span>
        <p className={styles.title}>Перетащите фото встречи</p>
        <p className={styles.note}>jpg, png — просто бросьте сюда</p>
      </div>
    </div>
  )
}
