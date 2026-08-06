import { useState } from 'react'

import { isSoundOn, setSoundOn, startTheme, stopTheme } from '../../domain/theme-music'
import styles from './ThemeToggle.module.css'

/**
 * Включение фоновой темы.
 *
 * Нажатие — это и есть жест пользователя, поэтому включение сразу запускает
 * музыку: браузер уже не возражает.
 */
export default function SoundToggle() {
  const [on, setOn] = useState(isSoundOn)

  return (
    <button
      type="button"
      className={styles.toggle}
      aria-pressed={on}
      aria-label={on ? 'Выключить музыку' : 'Включить музыку'}
      title={on ? 'Выключить музыку' : 'Включить музыку'}
      onClick={() => {
        const next = !on

        setSoundOn(next)
        setOn(next)

        if (next) void startTheme()
        else stopTheme()
      }}
    >
      {on ? '🔊' : '🔇'}
    </button>
  )
}
