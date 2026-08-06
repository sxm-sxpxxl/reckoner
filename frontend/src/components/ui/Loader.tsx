import styles from './Loader.module.css'

/**
 * Индикатор загрузки.
 *
 * Заменил скелетоны-плитки: на тёмной теме их шиммер бежал между двумя
 * поверхностями, отличающимися на четыре единицы яркости, и выглядел как
 * три мёртвых прямоугольника.
 */
export default function Loader({ label = 'Загружаем' }: { label?: string }) {
  return (
    <div className={styles.wrap} role="status" aria-live="polite">
      <span className={styles.ring} aria-hidden="true" />
      <p className={styles.label}>{label}</p>
    </div>
  )
}
