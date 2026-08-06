import { useState } from 'react'

import { isMuted, playSting, setMuted } from '../../domain/sting'
import styles from './SoundToggle.module.css'

/**
 * Включение приветственного сигнала.
 *
 * Нажатие — это и есть жест пользователя, поэтому при включении сигнал можно
 * сразу проиграть: человек слышит, на что подписался.
 */
export default function SoundToggle() {
  const [muted, setLocal] = useState(isMuted)

  return (
    <button
      type="button"
      className={styles.toggle}
      aria-pressed={!muted}
      aria-label={muted ? 'Включить звук' : 'Выключить звук'}
      title={muted ? 'Включить звук' : 'Выключить звук'}
      onClick={() => {
        const next = !muted

        setMuted(next)
        setLocal(next)

        if (!next) void playSting()
      }}
    >
      {muted ? '🔇' : '🔊'}
    </button>
  )
}
