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
