import Button from '../ui/Button'
import styles from './MeetingsEmpty.module.css'

export default function MeetingsEmpty({ onCreate }: { onCreate: () => void }) {
  return (
    <div className={styles.empty}>
      <span className={styles.emoji} aria-hidden="true">
        🍃
      </span>
      <h2 className={styles.title}>Ничего не нашлось</h2>
      <p className={styles.note}>Поменяйте фильтры или заведите первую встречу</p>
      <Button variant="dark" onClick={onCreate}>
        Создать встречу
      </Button>
    </div>
  )
}
