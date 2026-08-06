import styles from './EmojiPicker.module.css'

/**
 * Выбор эмодзи из готового набора.
 *
 * Именно выбор, а не ввод: в текстовом поле пролезал любой текст, и «Эмодзи»
 * превращалось во второе название.
 */
export default function EmojiPicker({
  options,
  value,
  onChange,
  label,
}: {
  options: string[]
  value: string
  onChange: (emoji: string) => void
  label: string
}) {
  // Текущее значение показываем даже если его нет в наборе: у встреч, созданных
  // когда эмодзи вводили текстом, стоит что угодно, и без этой строки правка
  // выглядела бы так, будто ничего не выбрано, а любое сохранение молча
  // подменяло бы эмодзи.
  const shown = options.includes(value) ? options : [value, ...options]

  return (
    <div className={styles.grid} role="radiogroup" aria-label={label}>
      {shown.map((emoji) => (
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
