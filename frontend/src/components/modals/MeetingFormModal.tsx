import { useState, type FormEvent } from 'react'
import { useNavigate } from 'react-router-dom'

import { ApiError } from '../../api/client'
import { useCreateMeeting } from '../../api/meetings'
import Button from '../ui/Button'
import Modal from './Modal'
import styles from './MeetingFormModal.module.css'

/** Сегодня в виде `2026-08-06`. Через `toISOString` нельзя: он переводит
 *  в UTC, и вечером в Москве подставилось бы завтрашнее число. */
function today(): string {
  const now = new Date()
  const pad = (value: number) => String(value).padStart(2, '0')

  return `${now.getFullYear()}-${pad(now.getMonth() + 1)}-${pad(now.getDate())}`
}

export default function MeetingFormModal({ onClose }: { onClose: () => void }) {
  const navigate = useNavigate()
  const create = useCreateMeeting()

  const [title, setTitle] = useState('')
  const [description, setDescription] = useState('')
  const [emoji, setEmoji] = useState('✨')
  const [heldOn, setHeldOn] = useState(today())

  const error = create.error instanceof ApiError ? create.error : null

  const submit = (event: FormEvent) => {
    event.preventDefault()

    create.mutate(
      { title, description, emoji, heldOn },
      {
        // Как в хендоффе: созданная встреча сразу открывается.
        onSuccess: (meeting) => navigate(`/meetings/${meeting.id}`),
      },
    )
  }

  return (
    <Modal
      title="Новая встреча"
      onClose={onClose}
      footer={
        <>
          <span className={styles.footSpacer} />
          <Button type="button" onClick={onClose}>
            Отмена
          </Button>
          <Button type="submit" form="meeting-form" variant="primary" loading={create.isPending}>
            Создать
          </Button>
        </>
      }
    >
      <form id="meeting-form" className={styles.form} onSubmit={submit}>
        <div className={styles.row}>
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
            <label className={styles.label} htmlFor="meeting-emoji">
              Эмодзи
            </label>
            <input
              id="meeting-emoji"
              className={`${styles.input} ${styles.emojiInput}`}
              value={emoji}
              onChange={(event) => setEmoji(event.target.value)}
            />
          </div>
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
