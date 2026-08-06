import { useState, type FormEvent } from 'react'

import { ApiError } from '../../api/client'
import { useAddEntry } from '../../api/meetings'
import type { Meeting } from '../../api/types'
import { parseAmount } from '../../domain/format'
import Button from '../ui/Button'
import Select from '../ui/Select'
import Modal from './Modal'
import form from './MeetingFormModal.module.css'
import styles from './ExpenseModal.module.css'

/**
 * Перевод в счёт долга.
 *
 * Значения по умолчанию приходят от кнопки «Отдано» в блоке долгов: там уже
 * известно, кто кому и сколько.
 */
export default function TransferModal({
  meeting,
  defaultFrom,
  defaultTo,
  defaultAmount,
  onClose,
}: {
  meeting: Meeting
  defaultFrom?: string
  defaultTo?: string
  defaultAmount?: number
  onClose: () => void
}) {
  const add = useAddEntry(meeting.id)

  const [fromId, setFromId] = useState(defaultFrom ?? meeting.participants[0]?.id ?? '')
  const [toId, setToId] = useState(defaultTo ?? meeting.participants[1]?.id ?? '')
  const [amountText, setAmountText] = useState(defaultAmount ? String(defaultAmount) : '')

  const parsed = parseAmount(amountText)
  const error = add.error instanceof ApiError ? add.error : null

  // Проверяем сами, не дожидаясь `422`: ответ сервера тут ничего не добавит,
  // а показать несовпадение можно сразу.
  const sameParty = fromId !== '' && fromId === toId
  const valid = Number.isFinite(parsed.rubles) && parsed.rubles > 0 && !sameParty

  const submit = (event: FormEvent) => {
    event.preventDefault()
    if (!valid) return

    add.mutate(
      { kind: 'transfer', payerId: fromId, recipientId: toId, amountRubles: parsed.rubles },
      { onSuccess: onClose },
    )
  }

  return (
    <Modal
      title="Перевод"
      onClose={onClose}
      footer={
        <>
          <span className={form.footSpacer} />
          <Button type="button" onClick={onClose}>
            Отмена
          </Button>
          <Button
            type="submit"
            form="transfer-form"
            variant="primary"
            loading={add.isPending}
            disabled={!valid}
          >
            Добавить
          </Button>
        </>
      }
    >
      <form id="transfer-form" className={form.form} onSubmit={submit}>
        <div className={form.field}>
          <span className={form.label}>
            Кто переводит
          </span>
          <Select
            value={fromId}
            onChange={setFromId}
            label="Кто переводит"
            options={meeting.participants.map((person) => ({
              value: person.id,
              label: `${person.emoji} ${person.name}`,
            }))}
          />
        </div>

        <div className={form.field}>
          <span className={form.label}>
            Кому переводит
          </span>
          <Select
            className={sameParty || error?.field === 'recipientId' ? form.invalid : undefined}
            value={toId}
            onChange={setToId}
            label="Кому переводит"
            options={meeting.participants.map((person) => ({
              value: person.id,
              label: `${person.emoji} ${person.name}`,
            }))}
          />
          {sameParty && <p className={form.error}>Перевод себе ничего не меняет</p>}
        </div>

        <div className={form.field}>
          <label className={form.label} htmlFor="transfer-amount">
            Сумма ₽
          </label>
          <input
            id="transfer-amount"
            className={`${form.input} ${styles.amountInput}`}
            inputMode="decimal"
            value={amountText}
            placeholder="0"
            onChange={(event) => setAmountText(event.target.value)}
          />
          {parsed.rounded && (
            <p className={styles.roundingNote}>
              Копейки не учитываем — сохранится {parsed.rubles} ₽
            </p>
          )}
        </div>

        {error && <p className={form.error}>{error.humanMessage}</p>}
      </form>
    </Modal>
  )
}
