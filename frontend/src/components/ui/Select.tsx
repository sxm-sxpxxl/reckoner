import { useEffect, useId, useRef, useState } from 'react'

import styles from './Select.module.css'

export interface Option {
  value: string
  label: string
}

/**
 * Выпадающий список.
 *
 * Написан вместо нативного `<select>` по одной причине: список опций у него
 * рисует операционная система, и ни скругления, ни цвет подсветки из CSS
 * недоступны — в тёмной теме он оставался светлой прямоугольной панелью
 * с синим выделением.
 *
 * Цена решения: на телефоне пропадает системный барабан выбора, который для
 * пальца удобнее списка. Поэтому клавиатура и роли реализованы честно, а не
 * «как-нибудь»: без них замена нативного элемента была бы прямым ухудшением.
 */
export default function Select({
  options,
  value,
  onChange,
  label,
  className,
}: {
  options: Option[]
  value: string
  onChange: (value: string) => void
  /** Для программ чтения с экрана: у кнопки нет видимой подписи. */
  label: string
  className?: string
}) {
  const [open, setOpen] = useState(false)
  const [active, setActive] = useState(0)
  const wrap = useRef<HTMLDivElement>(null)
  const listId = useId()

  const selectedIndex = Math.max(
    0,
    options.findIndex((option) => option.value === value),
  )
  const current = options[selectedIndex]

  // Клик мимо закрывает. Слушаем на всём документе, потому что список может
  // перекрывать что угодно на странице.
  useEffect(() => {
    if (!open) return

    const onDown = (event: PointerEvent) => {
      if (!wrap.current?.contains(event.target as Node)) setOpen(false)
    }

    document.addEventListener('pointerdown', onDown)

    return () => document.removeEventListener('pointerdown', onDown)
  }, [open])

  const openAt = (index: number) => {
    setActive(index)
    setOpen(true)
  }

  const pick = (index: number) => {
    onChange(options[index].value)
    setOpen(false)
  }

  const onKeyDown = (event: React.KeyboardEvent) => {
    if (!open) {
      if (event.key === 'Enter' || event.key === ' ' || event.key === 'ArrowDown') {
        event.preventDefault()
        openAt(selectedIndex)
      }

      return
    }

    if (event.key === 'Escape') {
      setOpen(false)

      return
    }

    if (event.key === 'Enter' || event.key === ' ') {
      event.preventDefault()
      pick(active)

      return
    }

    if (event.key === 'ArrowDown') {
      event.preventDefault()
      setActive((index) => (index + 1) % options.length)
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault()
      setActive((index) => (index - 1 + options.length) % options.length)
    }

    if (event.key === 'Home') {
      event.preventDefault()
      setActive(0)
    }

    if (event.key === 'End') {
      event.preventDefault()
      setActive(options.length - 1)
    }
  }

  return (
    <div className={`${styles.wrap} ${className ?? ''}`} ref={wrap}>
      <button
        type="button"
        className={styles.trigger}
        role="combobox"
        aria-expanded={open}
        aria-controls={listId}
        aria-haspopup="listbox"
        aria-label={label}
        onClick={() => (open ? setOpen(false) : openAt(selectedIndex))}
        onKeyDown={onKeyDown}
      >
        {current?.label ?? ''}
      </button>
      <span className={styles.chevron} aria-hidden="true">
        ▼
      </span>

      {open && (
        <ul className={styles.list} id={listId} role="listbox" aria-label={label}>
          {options.map((option, index) => (
            <li
              key={option.value}
              role="option"
              aria-selected={index === selectedIndex}
              className={[
                styles.option,
                index === active && styles.active,
                index === selectedIndex && styles.selected,
              ]
                .filter(Boolean)
                .join(' ')}
              // `pointerdown`, а не `click`: закрытие по клику мимо слушает ту же
              // фазу, и на `click` список успевал бы закрыться до выбора.
              onPointerDown={(event) => {
                event.preventDefault()
                pick(index)
              }}
              onPointerEnter={() => setActive(index)}
            >
              {option.label}
            </li>
          ))}
        </ul>
      )}
    </div>
  )
}
