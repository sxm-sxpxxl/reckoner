import Button from '../ui/Button'
import Modal from './Modal'
import styles from './ConfirmDelete.module.css'

/**
 * Подтверждение удаления.
 *
 * Свой диалог, а не нативный `confirm()`: тот на iOS блокирует страницу поверх
 * уже открытой модалки и выглядит чужеродно. Заодно здесь можно объяснить
 * последствия — а у удаления участника они неочевидные.
 */
export default function ConfirmDelete({
  title,
  text,
  loading = false,
  onConfirm,
  onCancel,
}: {
  title: string
  text: string
  loading?: boolean
  onConfirm: () => void
  onCancel: () => void
}) {
  return (
    <Modal
      title={title}
      onClose={onCancel}
      footer={
        <>
          <span className={styles.spacer} />
          <Button type="button" onClick={onCancel}>
            Отмена
          </Button>
          <Button type="button" variant="danger" loading={loading} onClick={onConfirm}>
            Удалить
          </Button>
        </>
      }
    >
      <p className={styles.text}>{text}</p>
    </Modal>
  )
}
