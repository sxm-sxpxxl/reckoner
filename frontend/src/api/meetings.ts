import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query'

import { api } from './client'
import type { CreateMeetingBody, ListFilters, Meeting, MeetingCard } from './types'

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

export function useCreateMeeting() {
  const queryClient = useQueryClient()

  return useMutation({
    mutationFn: (body: CreateMeetingBody) => api.post<Meeting>('/api/meetings', body),
    onSuccess: (meeting) => {
      // Ответ мутации — уже посчитанная встреча целиком, поэтому кладём её
      // в кэш вместо повторного запроса.
      queryClient.setQueryData(meetingKeys.one(meeting.id), meeting)
      queryClient.invalidateQueries({ queryKey: ['meetings'] })
    },
  })
}
