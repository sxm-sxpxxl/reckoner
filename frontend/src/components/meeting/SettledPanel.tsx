import styles from './SettledPanel.module.css'

/** Показывается вместо всех трёх представлений, когда переводов не осталось. */
export default function SettledPanel() {
  return (
    <div className={styles.panel}>
      <span className={styles.emoji} aria-hidden="true">
        🎉
      </span>
      <h3 className={styles.title}>Все в расчёте</h3>
      <p className={styles.note}>Встреча закрыта на равных. Можно планировать следующую.</p>
    </div>
  )
}
