import { useState } from 'react'

import { applyTheme, readTheme, type Theme } from '../../domain/theme'
import styles from './ThemeToggle.module.css'

export default function ThemeToggle() {
  const [theme, setTheme] = useState<Theme>(readTheme)

  const next: Theme = theme === 'dark' ? 'light' : 'dark'

  return (
    <button
      type="button"
      className={styles.toggle}
      aria-label={next === 'light' ? 'Светлая тема' : 'Тёмная тема'}
      title={next === 'light' ? 'Светлая тема' : 'Тёмная тема'}
      onClick={() => {
        applyTheme(next)
        setTheme(next)
      }}
    >
      {theme === 'dark' ? '☀' : '☾'}
    </button>
  )
}
