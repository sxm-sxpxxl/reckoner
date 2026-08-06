/** Ответы API. Форма зафиксирована в backend/src/api/view.rs и проверена
 *  интеграционными тестами; менять здесь в одностороннем порядке нельзя. */

export type MeetingStatus = 'settled' | 'attention' | 'alarm' | 'no-participants'

export type EntryKind = 'expense' | 'transfer'

/** Участник в карточке списка: аватар и имя, без денег. */
export interface ParticipantChip {
  id: string
  name: string
  emoji: string
  colorIndex: number
}

export interface Participant extends ParticipantChip {
  position: number
  /** «внёс N ₽»: только оплаченные расходы, отправленные переводы не в счёт. */
  contributedRubles: number
  /** Плюс — должны ему, минус — должен он. */
  netRubles: number
}

export interface Share {
  participantId: string
  /** 0..3. Полной доли здесь не бывает: её отсутствие и есть полная доля. */
  weightQuarters: number
}

export interface Entry {
  id: string
  kind: EntryKind
  payerId: string
  recipientId: string | null
  amountRubles: number
  description: string
  occurredAt: string
  shares: Share[]
  sharedByAll: boolean
}

export interface Transfer {
  fromId: string
  toId: string
  amountRubles: number
}

export interface Totals {
  spentRubles: number
  perPersonRubles: number
  pendingTransfers: number
}

export interface LogRecord {
  id: number
  text: string
  createdAt: string
}

/** Карточка списка. */
export interface MeetingCard {
  id: string
  title: string
  description: string
  emoji: string
  heldOn: string
  hasCover: boolean
  coverVersion: number
  totalRubles: number
  participants: ParticipantChip[]
  pendingTransfers: number
  status: MeetingStatus
}

/** Встреча целиком — ответ чтения и любой мутации. */
export interface Meeting {
  id: string
  title: string
  description: string
  emoji: string
  heldOn: string
  hasCover: boolean
  coverVersion: number
  createdAt: string
  participants: Participant[]
  entries: Entry[]
  settlement: Transfer[]
  totals: Totals
  status: MeetingStatus
  log: LogRecord[]
}

export type SortMode = 'date-desc' | 'date-asc' | 'total-desc' | 'open-first'

export interface ListFilters {
  q?: string
  participant?: string
  sort?: SortMode
}

export interface CreateMeetingBody {
  title?: string
  description?: string
  emoji?: string
  heldOn?: string
}

export interface UpdateMeetingBody {
  title?: string
  description?: string
  emoji?: string
  heldOn?: string
}

/** Единая форма отказа: см. backend/src/api/error.rs. */
export interface ApiErrorBody {
  error: string
  field?: string
  message?: string
}
