import styles from './WakeScreen.module.css'

/**
 * Экран на время пробуждения бэкенда.
 *
 * Бесплатный Render засыпает через 15 минут простоя, и первый запрос после сна
 * висит 30–60 секунд. Без этого экрана друг увидит белую страницу и решит, что
 * сайт сломался.
 */
export default function WakeScreen() {
  return (
    <div className={styles.screen}>
      <div className={styles.body}>
        <div className={styles.spinner} role="status" aria-label="Загрузка" />
        <h1 className={styles.title}>Будим сервер</h1>
        <p className={styles.note}>
          Это займёт около минуты — бесплатный хостинг засыпает, когда на сайт долго никто
          не заходит.
        </p>
      </div>
    </div>
  )
}
