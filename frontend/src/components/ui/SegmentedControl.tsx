import { useRef } from 'react'
import styles from './SegmentedControl.module.css'

export interface Segment<T extends string> {
  value: T
  label: string
}

/**
 * Переключатель представления.
 *
 * Настоящий `tablist` со стрелками, а не набор кнопок: это переключение вида,
 * и с клавиатуры оно должно работать так же, как мышью.
 */
export default function SegmentedControl<T extends string>({
  segments,
  value,
  onChange,
  label,
}: {
  segments: Segment<T>[]
  value: T
  onChange: (value: T) => void
  label: string
}) {
  const container = useRef<HTMLDivElement>(null)

  const move = (delta: number) => {
    const index = segments.findIndex((segment) => segment.value === value)
    const next = (index + delta + segments.length) % segments.length

    onChange(segments[next].value)
    // Фокус переезжает на выбранный сегмент, иначе стрелки перестают работать
    // после первого нажатия.
    container.current?.querySelectorAll('button')[next]?.focus()
  }

  return (
    <div className={styles.control} role="tablist" aria-label={label} ref={container}>
      {segments.map((segment) => (
        <button
          key={segment.value}
          type="button"
          role="tab"
          aria-selected={segment.value === value}
          tabIndex={segment.value === value ? 0 : -1}
          className={`${styles.segment} ${segment.value === value ? styles.active : ''}`}
          onClick={() => onChange(segment.value)}
          onKeyDown={(event) => {
            if (event.key === 'ArrowRight') move(1)
            if (event.key === 'ArrowLeft') move(-1)
          }}
        >
          {segment.label}
        </button>
      ))}
    </div>
  )
}
