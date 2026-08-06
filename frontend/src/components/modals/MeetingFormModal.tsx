import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'

import { ApiError } from '../../api/client'
import { useCreateMeeting, useDeleteMeeting, useUpdateMeeting } from '../../api/meetings'
import type { Meeting } from '../../api/types'
import { MEETING_EMOJI } from '../../domain/emoji'
import Button from '../ui/Button'
import EmojiPicker from '../ui/EmojiPicker'
import ConfirmDelete from './ConfirmDelete'
import Modal from './Modal'
import styles from './MeetingFormModal.module.css'

/** Сегодня в виде `2026-08-06`. Через `toISOString` нельзя: он переводит
 *  в UTC, и вечером в Москве подставилось бы завтрашнее число. */
function today(): string {
  const now = new Date()
  const pad = (value: number) => String(value).padStart(2, '0')

  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`
}

/**
 * Форма встречи: одна и та же на создание и на правку.
 *
 * Правка — наше добавление к хендоффу: там встреча всегда рождается с эмодзи ✨,
 * и поменять его, название или дату негде.
 */
export default function MeetingFormModal({
  meeting,
  onClose,
}: {
  meeting?: Meeting
  onClose: () => void
}) {
  const navigate = useNavigate()
  const create = useCreateMeeting()
  const update = useUpdateMeeting(meeting?.id ?? '')
  const remove = useDeleteMeeting()

  const [title, setTitle] = useState(meeting?.title ?? '')
  const [description, setDescription] = useState(meeting?.description ?? '')
  const [emoji, setEmoji] = useState(meeting?.emoji ?? '✨')
  const [heldOn, setHeldOn] = useState(meeting?.heldOn ?? today())
  const [confirming, setConfirming] = useState(false)

  const pending = create.isPending || update.isPending
  const failure = create.error ?? update.error
  const error = failure instanceof ApiError ? failure : null

  const submit = (event: FormEvent) => {
    event.preventDefault()

    if (meeting) {
      update.mutate({ title, description, emoji, heldOn }, { onSuccess: onClose })

      return
    }

    create.mutate(
      { title, description, emoji, heldOn },
      // Как в хендоффе: созданная встреча сразу открывается.
      { onSuccess: (created) => navigate(`/meetings/${created.id}`) },
    )
  }

  if (confirming && meeting) {
    return (
      <ConfirmDelete
        title="Удалить встречу?"
        text="Исчезнут все участники, расходы и переводы. Отменить это будет нельзя."
        loading={remove.isPending}
        onCancel={() => setConfirming(false)}
        onConfirm={() =>
          remove.mutate(meeting.id, {
            onSuccess: () => navigate('/'),
          })
        }
      />
    )
  }

  return (
    <Modal
      title={meeting ? 'Встреча' : 'Новая встреча'}
      onClose={onClose}
      footer={
        <>
          {meeting && (
            <Button type="button" variant="danger" onClick={() => setConfirming(true)}>
              Удалить
            </Button>
          )}
          <span className={styles.footSpacer} />
          <Button type="button" onClick={onClose}>
            Отмена
          </Button>
          <Button type="submit" form="meeting-form" variant="primary" loading={pending}>
            {meeting ? 'Сохранить' : 'Создать'}
          </Button>
        </>
      }
    >
      <form id="meeting-form" className={styles.form} onSubmit={submit}>
        <div className={styles.field}>
          <label className={styles.label} htmlFor="meeting-title">
            Название
          </label>
          <input
            id="meeting-title"
            className={`${styles.input} ${error?.field === 'title' ? styles.invalid : ''}`}
            value={title}
            placeholder="Новая встреча"
            onChange={(event) => setTitle(event.target.value)}
          />
        </div>

        <div className={styles.field}>
          <span className={styles.label}>Эмодзи</span>
          <EmojiPicker
            options={MEETING_EMOJI}
            value={emoji}
            onChange={setEmoji}
            label="Эмодзи встречи"
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label} htmlFor="meeting-date">
            Дата встречи
          </label>
          <input
            id="meeting-date"
            className={styles.input}
            type="date"
            value={heldOn}
            onChange={(event) => setHeldOn(event.target.value)}
          />
        </div>

        <div className={styles.field}>
          <label className={styles.label} htmlFor="meeting-description">
            Описание
          </label>
          <textarea
            id="meeting-description"
            className={styles.textarea}
            rows={3}
            value={description}
            onChange={(event) => setDescription(event.target.value)}
          />
        </div>

        {error && <p className={styles.error}>{error.humanMessage}</p>}
      </form>
    </Modal>
  )
}
