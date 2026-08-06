import { useRef, useState, type DragEvent } from 'react'

import { ApiError } from '../../api/client'
import { useRemoveCover, useUploadCover } from '../../api/meetings'
import { coverGradient } from '../../domain/cover'
import { ImageError, compressImage } from '../../domain/imageFile'
import Button from '../ui/Button'
import styles from './CoverDropzone.module.css'

/**
 * Обложка встречи: перетаскивание и выбор файла.
 *
 * Клик по зоне открывает файловый диалог — в прототипе только `drop`, но на
 * телефоне бросить файл нечем. Отступление записано в спеке.
 */
export default function CoverDropzone({
  meetingId,
  emoji,
  hasCover,
  coverVersion,
}: {
  meetingId: string
  emoji: string
  hasCover: boolean
  coverVersion: number
}) {
  const upload = useUploadCover(meetingId)
  const remove = useRemoveCover(meetingId)
  const input = useRef<HTMLInputElement>(null)

  const [dragging, setDragging] = useState(false)
  const [problem, setProblem] = useState<string | null>(null)

  const busy = upload.isPending || remove.isPending

  const accept = async (file: File | undefined) => {
    if (!file) return

    setProblem(null)

    try {
      // Сжимаем до отправки: фотография с телефона иначе не пройдёт лимит.
      const blob = await compressImage(file)

      upload.mutate(blob, {
        onError: (error) =>
          setProblem(
            error instanceof ApiError ? error.humanMessage : 'Не удалось загрузить обложку',
          ),
      })
    } catch (error) {
      // Причину показываем как есть: «это точно картинка?» полезнее, чем
      // «ошибка загрузки».
      setProblem(error instanceof ImageError ? error.message : 'Не удалось обработать файл')
    }
  }

  const onDrop = (event: DragEvent) => {
    event.preventDefault()
    setDragging(false)
    void accept(event.dataTransfer.files[0])
  }

  const openPicker = () => {
    if (!busy) input.current?.click()
  }

  return (
    <div
      className={[
        styles.zone,
        dragging && styles.dragging,
        hasCover && styles.filled,
      ]
        .filter(Boolean)
        .join(' ')}
      style={{
        // Без обложки — та же заглушка, что у карточки этой встречи в списке.
        background: hasCover
          ? `center/cover url(/api/meetings/${meetingId}/cover?v=${coverVersion})`
          : coverGradient(meetingId),
      }}
      // Зона ведёт себя как кнопка: иначе обложку нельзя поставить
      // с клавиатуры вообще.
      role="button"
      tabIndex={0}
      aria-label={hasCover ? 'Заменить обложку встречи' : 'Загрузить обложку встречи'}
      onClick={openPicker}
      onKeyDown={(event) => {
        if (event.key === 'Enter' || event.key === ' ') {
          event.preventDefault()
          openPicker()
        }
      }}
      onDragOver={(event) => {
        event.preventDefault()
        setDragging(true)
      }}
      onDragLeave={() => setDragging(false)}
      onDrop={onDrop}
    >
      <input
        ref={input}
        className={styles.hiddenInput}
        type="file"
        accept="image/*"
        onChange={(event) => {
          void accept(event.target.files?.[0])
          // Сбрасываем значение: иначе повторный выбор того же файла
          // не вызовет `change`.
          event.target.value = ''
        }}
      />

      {!hasCover && (
        <div>
          <span className={styles.emoji} aria-hidden="true">
            {emoji}
          </span>
          <p className={styles.title}>Перетащите фото встречи</p>
          <p className={styles.note}>jpg, png — или нажмите, чтобы выбрать</p>
          {problem && <p className={styles.error}>{problem}</p>}
        </div>
      )}

      {hasCover && (
        <div className={styles.actions}>
          <Button
            size="small"
            onClick={(event) => {
              event.stopPropagation()
              openPicker()
            }}
          >
            Заменить
          </Button>
          <Button
            size="small"
            variant="danger"
            onClick={(event) => {
              // Клик по кнопке не должен ещё и открыть файловый диалог.
              event.stopPropagation()
              remove.mutate(undefined)
            }}
          >
            Удалить
          </Button>
        </div>
      )}

      {hasCover && problem && <p className={styles.error}>{problem}</p>}

      {busy && <div className={styles.busy}>Загружаем…</div>}
    </div>
  )
}
