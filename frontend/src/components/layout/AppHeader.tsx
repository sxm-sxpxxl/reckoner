import type { ReactNode } from 'react'
import { Link } from 'react-router-dom'
import styles from './AppHeader.module.css'

interface AppHeaderProps {
  /** Правая часть шапки. На списке — «+ Новая встреча», на встрече —
   *  «← Все встречи». Слот, а не условие внутри шапки: страница знает, что ей
   *  нужно, а шапка про страницы знать не должна. */
  action?: ReactNode
}

export default function AppHeader({ action }: AppHeaderProps) {
  return (
    <header className={styles.header}>
      <div className={styles.inner}>
        <Link to="/" className={styles.brand}>
          <span className={styles.mark} aria-hidden="true">
            Ф
          </span>
          <span className={styles.wordmark}>Фонд встреч</span>
        </Link>
        {action}
      </div>
    </header>
  )
}
