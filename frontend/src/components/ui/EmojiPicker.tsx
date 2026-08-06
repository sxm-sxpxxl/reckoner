import { AVATAR_EMOJI } from '../../domain/avatars'
import styles from './EmojiPicker.module.css'

export default function EmojiPicker({
  value,
  onChange,
}: {
  value: string
  onChange: (emoji: string) => void
}) {
  return (
    <div className={styles.grid} role="radiogroup" aria-label="Аватар участника">
      {AVATAR_EMOJI.map((emoji) => (
        <button
          key={emoji}
          type="button"
          role="radio"
          aria-checked={emoji === value}
          aria-label={emoji}
          className={`${styles.option} ${emoji === value ? styles.picked : ''}`}
          onClick={() => onChange(emoji)}
        >
          {emoji}
        </button>
      ))}
    </div>
  )
}
