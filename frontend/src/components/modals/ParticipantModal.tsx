import { useState, type FormEvent } from 'react'

import { ApiError } from '../../api/client'
import { useAddParticipant, useRemoveParticipant, useUpdateParticipant } from '../../api/meetings'
import type { Participant } from '../../api/types'
import { AVATAR_EMOJI } from '../../domain/emoji'
import Button from '../ui/Button'
import EmojiPicker from '../ui/EmojiPicker'
import Select from '../ui/Select'
import ConfirmDelete from './ConfirmDelete'
import Modal from './Modal'
import styles from './MeetingFormModal.module.css'

/** Одна форма на добавление и правку: поля те же, отличается только подвал. */
export default function ParticipantModal({
  meetingId,
  participant,
  people,
  onClose,
}: {
  meetingId: string
  participant?: Participant
  /** Все участники встречи — из них выбирают того, кто платит. */
  people: Participant[]
  onClose: () => void
}) {
  const add = useAddParticipant(meetingId)
  const update = useUpdateParticipant(meetingId)
  const remove = useRemoveParticipant(meetingId)

  const [name, setName] = useState(participant?.name ?? '')
  const [emoji, setEmoji] = useState(participant?.emoji ?? AVATAR_EMOJI[0])
  const [paidById, setPaidById] = useState(participant?.paidById ?? '')

  // За того, кто сам платит за других, назначить плательщика нельзя: вышла бы
  // цепочка, а их сервер не принимает.
  const covers = participant ? people.filter((person) => person.paidById === participant.id) : []

  // Плательщиком может быть любой, кроме самого человека и тех, за кого уже
  // платят, — по той же причине.
  const payerOptions = [
    { value: '', label: 'Платит за себя' },
    ...people
      .filter((person) => person.id !== participant?.id && person.paidById === null)
      .map((person) => ({ value: person.id, label: `${person.emoji} ${person.name}` })),
  ]
  const [confirming, setConfirming] = useState(false)

  const pending = add.isPending || update.isPending
  const failure = add.error ?? update.error
  const error = failure instanceof ApiError ? failure : null

  const submit = (event: FormEvent) => {
    event.preventDefault()

    const body = { name, emoji, paidById: paidById === '' ? null : paidById }

    if (participant) {
      update.mutate({ id: participant.id, ...body }, { onSuccess: onClose })

      return
    }

    add.mutate(body, { onSuccess: onClose })
  }

  if (confirming && participant) {
    return (
      <ConfirmDelete
        title="Удалить участника?"
        // Каскад в базе уносит все записи, где он плательщик или получатель.
        // Об этом надо предупредить: иначе человек стирает половину истории,
        // рассчитывая убрать одну карточку.
        text={`Вместе с ${participant.name} исчезнут все расходы и переводы, где он участвует. Суммы пересчитаются.${
          covers.length > 0
            ? ` За себя снова платят: ${covers.map((person) => person.name).join(', ')}.`
            : ''
        }`}
        loading={remove.isPending}
        onCancel={() => setConfirming(false)}
        onConfirm={() => remove.mutate(participant.id, { onSuccess: onClose })}
      />
    )
  }

  return (
    <Modal
      title={participant ? 'Участник' : 'Новый участник'}
      onClose={onClose}
      footer={
        <>
          {participant && (
            <Button type="button" variant="danger" onClick={() => setConfirming(true)}>
              Удалить
            </Button>
          )}
          <span className={styles.footSpacer} />
          <Button type="button" onClick={onClose}>
            Отмена
          </Button>
          <Button type="submit" form="participant-form" variant="primary" loading={pending}>
            {participant ? 'Сохранить' : 'Добавить'}
          </Button>
        </>
      }
    >
      <form id="participant-form" className={styles.form} onSubmit={submit}>
        <div className={styles.field}>
          <label className={styles.label} htmlFor="participant-name">
            Имя
          </label>
          <input
            id="participant-name"
            className={`${styles.input} ${error?.field === 'name' ? styles.invalid : ''}`}
            value={name}
            autoFocus
            onChange={(event) => setName(event.target.value)}
          />
        </div>

        <div className={styles.field}>
          <span className={styles.label}>Аватар</span>
          <EmojiPicker
            options={AVATAR_EMOJI}
            value={emoji}
            onChange={setEmoji}
            label="Аватар участника"
          />
        </div>

        <div className={styles.field}>
          <span className={styles.label}>Кто платит</span>

          {covers.length > 0 ? (
            <>
              <div className={`${styles.input} ${styles.readonly}`}>Платит за себя</div>
              <p className={styles.hint}>
                Уже платит за других: {covers.map((person) => person.name).join(', ')}
              </p>
            </>
          ) : (
            <Select
              value={paidById}
              onChange={setPaidById}
              label="Кто платит"
              options={payerOptions}
            />
          )}
        </div>

        {error && <p className={styles.error}>{error.humanMessage}</p>}
      </form>
    </Modal>
  )
}
