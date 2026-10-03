import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { api } from './client'
import type {
  CreateMeetingBody,
  ListFilters,
  Meeting,
  MeetingCard,
  UpdateMeetingBody,
} from './types'

/** Ключ включает фильтры целиком: иначе смена сортировки переиспользовала бы
 *  чужой ответ и список показывал бы прежний порядок. */
export const meetingKeys = {
  list: (filters: ListFilters) => ['meetings', filters] as const,
  one: (id: string) => ['meeting', id] as const,
}

function toQueryString(filters: ListFilters): string {
  const params = new URLSearchParams()

  if (filters.q) params.set('q', filters.q)
  if (filters.participant) params.set('participant', filters.participant)
  if (filters.sort) params.set('sort', filters.sort)

  const query = params.toString()

  return query ? `?${query}` : ''
}

/**
 * Список встреч.
 *
 * Фильтрация и сортировка идут на сервер, а не по загруженному массиву: только
 * так `open-first` и `total-desc` считаются по настоящим данным, а не по тому
 * куску, что оказался на клиенте.
 */
export function useMeetings(filters: ListFilters) {
  return useQuery({
    queryKey: meetingKeys.list(filters),
    queryFn: () => api.get<MeetingCard[]>(`/api/meetings${toQueryString(filters)}`),
  })
}

export function useMeeting(id: string) {
  return useQuery({
    queryKey: meetingKeys.one(id),
    queryFn: () => api.get<Meeting>(`/api/meetings/${id}`),
  })
}

/**
 * Общий хвост всех мутаций встречи.
 *
 * Каждая ручка отвечает встречей целиком — с пересчитанными балансами, планом
 * переводов и статусом. Поэтому ответ кладём прямо в кэш: второго запроса
 * не нужно, и UI не может разойтись с тем, что записано. Список инвалидируем —
 * в нём меняются сумма, статус и состав участников.
 */
function useMeetingMutation<TBody>(meetingId: string, send: (body: TBody) => Promise<Meeting>) {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: send,
    onSuccess: (meeting) => {
      queryClient.setQueryData(meetingKeys.one(meetingId), meeting)
      queryClient.invalidateQueries({ queryKey: ['meetings'] })
    },
  })
}

export function useCreateMeeting() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (body: CreateMeetingBody) => api.post<Meeting>('/api/meetings', body),
    onSuccess: (meeting) => {
      queryClient.setQueryData(meetingKeys.one(meeting.id), meeting)
      queryClient.invalidateQueries({ queryKey: ['meetings'] })
    },
  })
}

export function useUpdateMeeting(id: string) {
  return useMeetingMutation(id, (body: UpdateMeetingBody) =>
    api.patch<Meeting>(`/api/meetings/${id}`, body),
  )
}

export function useDeleteMeeting() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (id: string) => api.delete<void>(`/api/meetings/${id}`),
    onSuccess: (_result, id) => {
      queryClient.removeQueries({ queryKey: meetingKeys.one(id) })
      queryClient.invalidateQueries({ queryKey: ['meetings'] })
    },
  })
}

export interface ParticipantBody {
  name?: string
  emoji?: string
}

export function useAddParticipant(meetingId: string) {
  return useMeetingMutation(meetingId, (body: ParticipantBody) =>
    api.post<Meeting>(`/api/meetings/${meetingId}/participants`, body),
  )
}

export function useUpdateParticipant(meetingId: string) {
  return useMeetingMutation(
    meetingId,
    ({ id, ...body }: ParticipantBody & { id: string }) =>
      api.patch<Meeting>(`/api/meetings/${meetingId}/participants/${id}`, body),
  )
}

export function useRemoveParticipant(meetingId: string) {
  return useMeetingMutation(meetingId, (id: string) =>
    api.delete<Meeting>(`/api/meetings/${meetingId}/participants/${id}`),
  )
}

export interface ShareBody {
  participantId: string
  rubles: number
}

export interface EntryBody {
  kind: 'expense' | 'transfer'
  payerId: string
  recipientId?: string | null
  amountRubles: number
  description?: string
  occurredAt?: string
  shares?: ShareBody[]
}

export function useAddEntry(meetingId: string) {
  return useMeetingMutation(meetingId, (body: EntryBody) =>
    api.post<Meeting>(`/api/meetings/${meetingId}/entries`, body),
  )
}

export interface EntryPatchBody {
  payerId?: string
  recipientId?: string
  amountRubles?: number
  description?: string
  occurredAt?: string
  shares?: ShareBody[]
}

export function useUpdateEntry(meetingId: string) {
  return useMeetingMutation(
    meetingId,
    ({ id, ...body }: EntryPatchBody & { id: string }) =>
      api.patch<Meeting>(`/api/meetings/${meetingId}/entries/${id}`, body),
  )
}

export function useUploadCover(meetingId: string) {
  return useMeetingMutation(meetingId, (blob: Blob) =>
    api.putBlob<Meeting>(`/api/meetings/${meetingId}/cover`, blob),
  )
}

export function useRemoveCover(meetingId: string) {
  return useMeetingMutation(meetingId, () =>
    api.delete<Meeting>(`/api/meetings/${meetingId}/cover`),
  )
}

export function useRemoveEntry(meetingId: string) {
  return useMeetingMutation(meetingId, (id: string) =>
    api.delete<Meeting>(`/api/meetings/${meetingId}/entries/${id}`),
  )
}
