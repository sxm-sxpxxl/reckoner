import type { MeetingStatus } from '../api/types'

/**
 * Соответствие статуса цвету и подписи. Единственное место, где это записано:
 * статус красит рамку карточки, бейдж и число в статкарточке, и разъехаться
 * они не должны.
 */
export interface StatusTone {
  /** Подпись бейджа. `pendingTransfers` нужен для «Осталось N». */
  label: (pendingTransfers: number) => string
  text: string
  bg: string
  border: string
}

export const STATUS_TONES: Record<MeetingStatus, StatusTone> = {
  settled: {
    label: () => 'Все в расчёте',
    text: 'var(--ok-text)',
    bg: 'var(--ok-bg)',
    border: 'var(--ok-border)',
  },
  attention: {
    label: (pending) => `Осталось ${pending}`,
    text: 'var(--warn-text)',
    bg: 'var(--warn-bg)',
    border: 'var(--warn-border)',
  },
  alarm: {
    label: (pending) => `Осталось ${pending}`,
    text: 'var(--alarm-text)',
    bg: 'var(--alarm-bg)',
    border: 'var(--alarm-border)',
  },
  'no-participants': {
    label: () => 'Нет участников',
    text: 'var(--text-3)',
    bg: 'var(--surface-2)',
    border: 'var(--border)',
  },
}

/**
 * Подпись бейджа на странице встречи.
 *
 * На карточке списка она короче — «Осталось 3», здесь «Осталось переводов: 3».
 * Так на скриншотах 01 и 02: подпись зависит от места, поэтому живёт отдельно
 * от `STATUS_TONES.label`, а цвета у них общие.
 */
export function pageStatusLabel(status: MeetingStatus, pendingTransfers: number): string {
  if (status === 'settled') return 'Все в расчёте'
  if (status === 'no-participants') return 'Нет участников'

  return `Осталось переводов: ${pendingTransfers}`
}
