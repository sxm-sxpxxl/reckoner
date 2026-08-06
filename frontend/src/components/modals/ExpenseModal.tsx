import { useMemo, useState, type FormEvent } from 'react'

import { ApiError } from '../../api/client'
import { useAddEntry, useUpdateEntry, type ShareBody } from '../../api/meetings'
import type { Entry, Meeting } from '../../api/types'
import { formatRubles, parseAmount } from '../../domain/format'
import { nextQuarters, previewShares, type Quarters } from '../../domain/sharePreview'
import Button from '../ui/Button'
import Modal from './Modal'
import Select from '../ui/Select'
import ShareRow from './ShareRow'
import form from './MeetingFormModal.module.css'
import styles from './ExpenseModal.module.css'

const FULL = 4

/** Одна строка «кто заплатил». Сумма хранится текстом: пока человек печатает,
 *  «1 2» ещё не число, и превращать её в `NaN` на каждый символ нельзя. */
interface PayerRow {
  participantId: string
  amountText: string
}

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
  /** Задана — правим существующий расход. Тогда плательщик один: правка
   *  относится к одной записи. */
  entry?: Entry
  onClose: () => void
}) {
  const add = useAddEntry(meeting.id)
  const update = useUpdateEntry(meeting.id)

  const [payers, setPayers] = useState<PayerRow[]>(() => [
    {
      participantId: entry?.payerId ?? meeting.participants[0]?.id ?? '',
      amountText: entry ? String(entry.amountRubles) : '',
    },
  ])
  const [description, setDescription] = useState(entry?.description ?? '')
  const [quarters, setQuarters] = useState<Quarters>(() => quartersFromEntry(entry, meeting))
  const [problem, setProblem] = useState<string | null>(null)

  const pending = add.isPending || update.isPending
  const failure = add.error ?? update.error
  const error = failure instanceof ApiError ? failure : null

  const ids = useMemo(() => meeting.participants.map((person) => person.id), [meeting.participants])

  const parsedPayers = payers.map((row) => ({ ...row, ...parseAmount(row.amountText) }))
  const filled = parsedPayers.filter((row) => Number.isFinite(row.rubles) && row.rubles > 0)

  // Доли считаются от общей суммы, и это не приближение: каждая запись делится
  // теми же долями, поэтому сумма долей по всем записям равна делению итога.
  const total = filled.reduce((sum, row) => sum + row.rubles, 0)
  const rounded = parsedPayers.some((row) => row.rounded)

  const preview = useMemo(
    () => previewShares(total, ids, quarters),
    [total, ids, quarters],
  )

  const valid = filled.length > 0 && filled.every((row) => row.participantId !== '')

  const setRow = (index: number, patch: Partial<PayerRow>) => {
    setPayers((current) => current.map((row, at) => (at === index ? { ...row, ...patch } : row)))
  }

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    if (!valid) return

    setProblem(null)

    // Полные доли в запрос не попадают: их отсутствие и есть полная доля.
    const shares: ShareBody[] = ids
      .filter((id) => (quarters[id] ?? FULL) < FULL)
      .map((id) => ({ participantId: id, weightQuarters: quarters[id] ?? FULL }))

    if (entry) {
      const only = filled[0]

      update.mutate(
        {
          id: entry.id,
          payerId: only.participantId,
          amountRubles: only.rubles,
          description,
          // `[]` — «снять все неполные доли», в отличие от «не трогать».
          shares,
        },
        { onSuccess: onClose },
      )

      return
    }

    // По записи на каждого плательщика, последовательно. Не параллельно:
    // каждая отвечает встречей целиком, и одновременные ответы затирали бы
    // друг друга в кэше, а порядок в истории стал бы случайным.
    try {
      for (const row of filled) {
        await add.mutateAsync({
          kind: 'expense',
          payerId: row.participantId,
          amountRubles: row.rubles,
          description,
          shares,
        })
      }

      onClose()
    } catch (cause) {
      // Записи, успевшие сохраниться, остаются — они настоящие. Говорим об этом
      // прямо, иначе человек повторит ввод и получит дубли.
      setProblem(
        cause instanceof ApiError
          ? `${cause.humanMessage}. Часть расходов могла сохраниться — проверьте историю.`
          : 'Не удалось сохранить все расходы — проверьте историю.',
      )
    }
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
            {entry ? 'Сохранить' : filled.length > 1 ? `Добавить ${filled.length}` : 'Добавить'}
          </Button>
        </>
      }
    >
      <form id="expense-form" className={form.form} onSubmit={submit}>
        <div className={form.field}>
          <div className={styles.sharesHead}>
            <span className={form.label}>Кто заплатил</span>
            {filled.length > 1 && (
              <span className={styles.payerTotal}>всего {formatRubles(total)}</span>
            )}
          </div>

          {payers.map((row, index) => (
            <div key={index} className={styles.payerRow}>
              <Select
                className={styles.payerSelect}
                value={row.participantId}
                onChange={(participantId) => setRow(index, { participantId })}
                label={`Плательщик ${index + 1}`}
                options={meeting.participants.map((person) => ({
                  value: person.id,
                  label: `${person.emoji} ${person.name}`,
                }))}
              />

              <input
                className={`${form.input} ${styles.payerAmount} ${
                  error?.field === 'amountRubles' ? form.invalid : ''
                }`}
                inputMode="decimal"
                value={row.amountText}
                placeholder="0 ₽"
                aria-label={`Сумма ${index + 1}`}
                autoFocus={index === 0}
                onChange={(event) => setRow(index, { amountText: event.target.value })}
              />

              {payers.length > 1 && (
                <button
                  type="button"
                  className={styles.dropPayer}
                  aria-label={`Убрать плательщика ${index + 1}`}
                  onClick={() => setPayers((current) => current.filter((_, at) => at !== index))}
                >
                  ✕
                </button>
              )}
            </div>
          ))}

          {/* Правка меняет одну запись, поэтому второй плательщик там
              не имеет смысла: это была бы уже другая запись. */}
          {!entry && payers.length < meeting.participants.length && (
            <button
              type="button"
              className={styles.addPayer}
              onClick={() =>
                setPayers((current) => [
                  ...current,
                  { participantId: meeting.participants[current.length]?.id ?? '', amountText: '' },
                ])
              }
            >
              + Ещё платил кто-то
            </button>
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

        {problem && <p className={form.error}>{problem}</p>}
        {!problem && error && <p className={form.error}>{error.humanMessage}</p>}
      </form>
    </Modal>
  )
}
