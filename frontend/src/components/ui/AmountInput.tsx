import { useRef } from 'react'

import styles from './AmountInput.module.css'

/**
 * Поле суммы, в котором можно складывать позиции: «390 + 1200 + 624».
 *
 * На цифровой клавиатуре телефона плюса нет, поэтому, пока фокус в поле,
 * рядом стоит кнопка «+». Видимость — через `:focus-within` в CSS, а не через
 * состояние: кнопка всегда в DOM, и клик по ней не теряется из-за того, что
 * поле успело потерять фокус и кнопка исчезла раньше клика.
 */
export default function AmountInput({
  value,
  onChange,
  placeholder,
  label,
  className,
  autoFocus,
}: {
  value: string
  onChange: (value: string) => void
  placeholder?: string
  /** Для программ чтения с экрана: у поля нет видимой подписи. */
  label: string
  /** Классы самого поля: внешний вид задаёт форма, в которой оно стоит. */
  className?: string
  autoFocus?: boolean
}) {
  const input = useRef<HTMLInputElement>(null)

  return (
    <span className={styles.wrap}>
      <input
        ref={input}
        className={className}
        inputMode="decimal"
        value={value}
        placeholder={placeholder}
        aria-label={label}
        autoFocus={autoFocus}
        onChange={(event) => onChange(event.target.value)}
      />

      <button
        type="button"
        className={styles.plus}
        // С клавиатуры плюс просто печатают — кнопка в порядке табуляции не нужна.
        tabIndex={-1}
        aria-label="Добавить позицию"
        // Иначе нажатие забрало бы фокус у поля, и клавиатура телефона спряталась бы.
        onMouseDown={(event) => event.preventDefault()}
        onClick={() => {
          onChange(`${value.trimEnd()} + `)
          input.current?.focus()
        }}
      >
        +
      </button>
    </span>
  )
}
