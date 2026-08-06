import { useMemo, useState, type FormEvent } from 'react'

import { ApiError } from '../../api/client'
import { useAddEntry, useUpdateEntry, type ShareBody } from '../../api/meetings'
import type { Entry, Meeting } from '../../api/types'
import { parseAmount } from '../../domain/format'
import { nextQuarters, previewShares, type Quarters } from '../../domain/sharePreview'
import Button from '../ui/Button'
import Modal from './Modal'
import ShareRow from './ShareRow'
import form from './MeetingFormModal.module.css'
import styles from './ExpenseModal.module.css'

const FULL = 4

/** Доли записи в вид `{ [participantId]: quarters }`. Отсутствие ключа
 *  на сервере означает полную долю — восстанавливаем это здесь. */
function quartersFromEntry(entry: Entry | undefined, meeting: Meeting): Quarters {
  const result: Quarters = {}

  for (const person of meeting.participants) result[person.id] = FULL
  for (const share of entry?.shares ?? []) result[share.participantId] = share.weightQuarters

  return result
}

export default function ExpenseModal({
  meeting,
  entry,
  onClose,
}: {
  meeting: Meeting
  /** Задана — правим существующий расход. */
  entry?: Entry
  onClose: () => void
}) {
  const add = useAddEntry(meeting.id)
  const update = useUpdateEntry(meeting.id)

  const [payerId, setPayerId] = useState(entry?.payerId ?? meeting.participants[0]?.id ?? '')
  const [amountText, setAmountText] = useState(entry ? String(entry.amountRubles) : '')
  const [description, setDescription] = useState(entry?.description ?? '')
  const [quarters, setQuarters] = useState<Quarters>(() => quartersFromEntry(entry, meeting))

  const parsed = parseAmount(amountText)
  const pending = add.isPending || update.isPending
  const failure = add.error ?? update.error
  const error = failure instanceof ApiError ? failure : null

  const ids = useMemo(() => meeting.participants.map((person) => person.id), [meeting.participants])
  const preview = useMemo(
    () => previewShares(Number.isFinite(parsed.rubles) ? Math.max(parsed.rubles, 0) : 0, ids, quarters),
    [parsed.rubles, ids, quarters],
  )

  const valid = Number.isFinite(parsed.rubles) && parsed.rubles > 0 && payerId !== ''

  const submit = (event: FormEvent) => {
    event.preventDefault()
    if (!valid) return

    // Полные доли в запрос не попадают: их отсутствие и есть полная доля.
    const shares: ShareBody[] = ids
      .filter((id) => (quarters[id] ?? FULL) < FULL)
      .map((id) => ({ participantId: id, weightQuarters: quarters[id] ?? FULL }))

    if (entry) {
      update.mutate(
        {
          id: entry.id,
          payerId,
          amountRubles: parsed.rubles,
          description,
          // `[]` — «снять все неполные доли», в отличие от «не трогать».
          shares,
        },
        { onSuccess: onClose },
      )

      return
    }

    add.mutate(
      {
        kind: 'expense',
        payerId,
        amountRubles: parsed.rubles,
        description,
        shares,
      },
      { onSuccess: onClose },
    )
  }

  return (
    <Modal
      title={entry ? 'Расход' : 'Новый расход'}
      onClose={onClose}
      footer={
        <>
          <span className={form.footSpacer} />
          <Button type="button" onClick={onClose}>
            Отмена
          </Button>
          <Button
            type="submit"
            form="expense-form"
            variant="primary"
            loading={pending}
            disabled={!valid}
          >
            {entry ? 'Сохранить' : 'Добавить'}
          </Button>
        </>
      }
    >
      <form id="expense-form" className={form.form} onSubmit={submit}>
        <div className={form.field}>
          <label className={form.label} htmlFor="expense-payer">
            Кто заплатил
          </label>
          <select
            id="expense-payer"
            className={form.input}
            value={payerId}
            onChange={(event) => setPayerId(event.target.value)}
          >
            {meeting.participants.map((person) => (
              <option key={person.id} value={person.id}>
                {person.emoji} {person.name}
              </option>
            ))}
          </select>
        </div>

        <div className={form.field}>
          <label className={form.label} htmlFor="expense-amount">
            Сумма ₽
          </label>
          <input
            id="expense-amount"
            className={`${form.input} ${styles.amountInput} ${
              error?.field === 'amountRubles' ? form.invalid : ''
            }`}
            inputMode="decimal"
            value={amountText}
            placeholder="0"
            autoFocus
            onChange={(event) => setAmountText(event.target.value)}
          />
          {parsed.rounded && (
            <p className={styles.roundingNote}>
              Копейки не учитываем — сохранится {parsed.rubles} ₽
            </p>
          )}
        </div>

        <div className={form.field}>
          <label className={form.label} htmlFor="expense-description">
            На что
          </label>
          <input
            id="expense-description"
            className={form.input}
            value={description}
            placeholder="Без описания"
            onChange={(event) => setDescription(event.target.value)}
          />
        </div>

        <div className={form.field}>
          <div className={styles.sharesHead}>
            <span className={form.label}>Делим на</span>
            <span className={styles.hint}>жмите на долю, чтобы уменьшить</span>
          </div>

          <div className={styles.sharesPanel}>
            {meeting.participants.map((person) => (
              <ShareRow
                key={person.id}
                participant={person}
                quarters={quarters[person.id] ?? FULL}
                amount={preview[person.id] ?? 0}
                onCycle={() =>
                  setQuarters((current) => ({
                    ...current,
                    [person.id]: nextQuarters(current[person.id] ?? FULL),
                  }))
                }
              />
            ))}
          </div>
        </div>

        {error && <p className={form.error}>{error.humanMessage}</p>}
      </form>
    </Modal>
  )
}
