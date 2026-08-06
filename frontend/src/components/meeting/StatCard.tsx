import styles from './StatCard.module.css'

export default function StatCard({
  label,
  value,
  color,
}: {
  label: string
  value: string
  /** Цвет значения. У «Осталось переводов» он статусный. */
  color?: string
}) {
  return (
    <div className={styles.card}>
      <div className={styles.label}>{label}</div>
      <div className={styles.value} style={{ color }}>
        {value}
      </div>
    </div>
  )
}
