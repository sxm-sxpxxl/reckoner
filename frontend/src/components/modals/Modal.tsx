import { useEffect, useId, useRef, type ReactNode } from 'react'
import styles from './Modal.module.css'

interface ModalProps {
  title: string
  onClose: () => void
  children: ReactNode
  /** Кнопки подвала. Слева — опциональная «Удалить», справа — «Отмена»
   *  и основное действие. */
  footer?: ReactNode
}

/**
 * Каркас модалки.
 *
 * Сверх прототипа здесь доступность, которой в нём нет: закрытие по Escape,
 * возврат фокуса на элемент, открывший модалку, и блокировка прокрутки фона.
 * Без последнего на телефоне вместо модалки прокручивается страница под ней.
 */
export default function Modal({ title, onClose, children, footer }: ModalProps) {
  const titleId = useId()
  const panelRef = useRef<HTMLDivElement>(null)

  useEffect(() => {
    const opener = document.activeElement as HTMLElement | null
    const { overflow } = document.body.style
    document.body.style.overflow = 'hidden'

    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') onClose()
    }

    document.addEventListener('keydown', onKeyDown)
    panelRef.current?.focus()

    return () => {
      document.removeEventListener('keydown', onKeyDown)
      document.body.style.overflow = overflow
      opener?.focus?.()
    }
  }, [onClose])

  return (
    <div className={styles.overlay} onMouseDown={onClose}>
      <div
        ref={panelRef}
        className={styles.panel}
        role="dialog"
        aria-modal="true"
        aria-labelledby={titleId}
        tabIndex={-1}
        // Клик внутри не должен закрывать. Слушаем mousedown, а не click:
        // иначе выделение текста, начатое в панели и отпущенное на фоне,
        // считалось бы кликом по фону и закрывало модалку.
        onMouseDown={(event) => event.stopPropagation()}
      >
        <div className={styles.head}>
          <h2 className={styles.title} id={titleId}>
            {title}
          </h2>
          <button type="button" className={styles.close} onClick={onClose} aria-label="Закрыть">
            ✕
          </button>
        </div>

        {children}

        {footer && <div className={styles.foot}>{footer}</div>}
      </div>
    </div>
  )
}
