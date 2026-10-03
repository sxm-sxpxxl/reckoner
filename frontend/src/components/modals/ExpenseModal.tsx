import { useMemo, useState, type FormEvent } from 'react'

import { ApiError } from '../../api/client'
import { useAddEntry, useUpdateEntry, type ShareBody } from '../../api/meetings'
import type { Entry, Meeting } from '../../api/types'
import { parseAmountExpression } from '../../domain/amountExpression'
import { formatRubles } from '../../domain/format'
import { previewSplit, splitStatus, type Pinned, type SplitStatus } from '../../domain/sharePreview'
import { payerOf } from '../../domain/wallets'
import AmountInput from '../ui/AmountInput'
import Button from '../ui/Button'
import Select from '../ui/Select'
import Modal from './Modal'
import ShareRow from './ShareRow'
import form from './MeetingFormModal.module.css'
import styles from './ExpenseModal.module.css'

/** Строка «Делим на». Сумма хранится текстом: пока человек печатает,
 *  «390 +» ещё не число, и превращать её в `NaN` на каждый символ нельзя. */
interface SplitRow {
  included: boolean
  text: string
}

const EVEN: SplitRow = { included: true, text: '' }

/** Строки формы из сохранённой разбивки: нет строки на сервере — пустое
 *  поле, `0` — выключен, остальное — вписанная сумма. */
function rowsFromEntry(entry: Entry | undefined, meeting: Meeting): Record<string, SplitRow> {
  const rows: Record<string, SplitRow> = {}

  for (const person of meeting.participants) rows[person.id] = EVEN

  for (const share of entry?.shares ?? []) {
    rows[share.participantId] =
      share.rubles === 0 ? { included: false, text: '' } : { included: true, text: String(share.rubles) }
  }

  return rows
}

/** Подпись под списком. `error` запрещает сохранение. */
function statusLine(status: SplitStatus, amount: number): { text: string; error: boolean } | null {
  switch (status.kind) {
    case 'plain':
      return null
    case 'rest':
      return { text: `Остаток ${formatRubles(status.rest)} — поровну на ${status.among}`, error: false }
    case 'adjusted':
      return status.difference > 0
        ? { text: `+${formatRubles(status.difference)} разложены пропорционально`, error: false }
        : { text: `−${formatRubles(-status.difference)} вычтены пропорционально`, error: false }
    case 'over':
      return { text: `Вписано ${formatRubles(status.pinned)} — больше, чем потрачено`, error: true }
    case 'far':
      return {
        text: `Вписано ${formatRubles(status.pinned)} из ${formatRubles(amount)} — проверьте суммы`,
        error: true,
      }
  }
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

  const people = meeting.participants

  const [payerId, setPayerId] = useState(entry?.payerId ?? people[0]?.id ?? '')
  const [amountText, setAmountText] = useState(entry ? String(entry.amountRubles) : '')
  const [description, setDescription] = useState(entry?.description ?? '')
  const [rows, setRows] = useState(() => rowsFromEntry(entry, meeting))

  const pending = add.isPending || update.isPending
  const failure = add.error ?? update.error
  const error = failure instanceof ApiError ? failure : null

  const ids = useMemo(() => people.map((person) => person.id), [people])
  // Участник мог появиться, пока модалка открыта: для него строки ещё нет.
  const rowOf = (id: string) => rows[id] ?? EVEN

  const amount = parseAmountExpression(amountText)
  const amountOk = Number.isFinite(amount.rubles) && amount.rubles > 0
  const total = amountOk ? amount.rubles : 0

  const parsed = useMemo(
    () => Object.fromEntries(ids.map((id) => [id, parseAmountExpression(rows[id]?.text ?? '')])),
    [ids, rows],
  )

  // Выключенный — ноль, вписанное — своё, пустое поле — ключа нет, то есть «поровну».
  const pinned = useMemo(() => {
    const result: Pinned = {}

    for (const id of ids) {
      const row = rows[id] ?? EVEN

      if (!row.included) result[id] = 0
      else if (Number.isFinite(parsed[id].rubles)) result[id] = parsed[id].rubles
    }

    return result
  }, [ids, rows, parsed])

  const preview = useMemo(() => previewSplit(total, ids, pinned), [total, ids, pinned])
  const line = amountOk ? statusLine(splitStatus(total, ids, pinned), total) : null

  const someInvalid = amount.invalid || ids.some((id) => rowOf(id).included && parsed[id].invalid)
  const rounded = amount.rounded || ids.some((id) => rowOf(id).included && parsed[id].rounded)
  const valid = amountOk && payerId !== '' && !someInvalid && !line?.error

  const setRow = (id: string, patch: Partial<SplitRow>) => {
    setRows((current) => ({ ...current, [id]: { ...(current[id] ?? EVEN), ...patch } }))
  }

  const submit = (event: FormEvent) => {
    event.preventDefault()
    if (!valid) return

    // Пустые поля в запрос не попадают: их отсутствие и есть «поровну».
    const shares: ShareBody[] = ids.flatMap((id) =>
      id in pinned ? [{ participantId: id, rubles: pinned[id] }] : [],
    )
    const body = { payerId, amountRubles: amount.rubles, description, shares }

    if (entry) {
      // `shares: []` — «снять разбивку», в отличие от «не трогать».
      update.mutate({ id: entry.id, ...body }, { onSuccess: onClose })

      return
    }

    add.mutate({ kind: 'expense', ...body }, { onSuccess: onClose })
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
          <span className={form.label}>Кто заплатил</span>

          <div className={styles.payerRow}>
            <Select
              className={styles.payerSelect}
              value={payerId}
              onChange={setPayerId}
              label="Кто заплатил"
              options={people.map((person) => ({
                value: person.id,
                label: `${person.emoji} ${person.name}`,
              }))}
            />

            <span className={styles.payerAmount}>
              <AmountInput
                className={`${form.input} ${styles.amountField} ${
                  error?.field === 'amountRubles' || amount.invalid ? form.invalid : ''
                }`}
                value={amountText}
                onChange={setAmountText}
                placeholder="0 ₽"
                label="Сумма расхода"
                autoFocus
              />
            </span>
          </div>

          {amountOk && amount.compound && (
            <p className={styles.sum}>= {formatRubles(amount.rubles)}</p>
          )}

          {rounded && (
            <p className={styles.roundingNote}>Копейки не учитываем — суммы округлены вниз</p>
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
            <span className={styles.hint}>пустое поле — поровну</span>
          </div>

          <div className={styles.sharesPanel}>
            {people.map((person) => {
              const row = rowOf(person.id)

              return (
                <ShareRow
                  key={person.id}
                  participant={person}
                  payer={payerOf(person, people)}
                  included={row.included}
                  text={row.text}
                  parsed={parsed[person.id]}
                  share={preview[person.id] ?? 0}
                  onToggle={() => setRow(person.id, { included: !row.included })}
                  onText={(text) => setRow(person.id, { text })}
                />
              )
            })}
          </div>

          {line && <p className={line.error ? form.error : styles.status}>{line.text}</p>}
        </div>

        {error && <p className={form.error}>{error.humanMessage}</p>}
      </form>
    </Modal>
  )
}
