import { useEffect, useMemo, useState } from 'react'

import { useMeetings } from '../api/meetings'
import { ApiError } from '../api/client'
import type { SortMode } from '../api/types'
import AppHeader from '../components/layout/AppHeader'
import Container from '../components/layout/Container'
import Loader from '../components/ui/Loader'
import MeetingCard from '../components/meetings/MeetingCard'
import MeetingFilters from '../components/meetings/MeetingFilters'
import MeetingsEmpty from '../components/meetings/MeetingsEmpty'
import MeetingFormModal from '../components/modals/MeetingFormModal'
import Button from '../components/ui/Button'
import styles from './MeetingsListPage.module.css'

/** Пауза перед запросом поиска: без неё каждая буква уходит в Neon. */
const SEARCH_DEBOUNCE_MS = 300

export default function MeetingsListPage() {
  const [search, setSearch] = useState('')
  const [debouncedSearch, setDebouncedSearch] = useState('')
  const [participant, setParticipant] = useState('')
  const [sort, setSort] = useState<SortMode>('date-desc')

  useEffect(() => {
    const timer = setTimeout(() => setDebouncedSearch(search), SEARCH_DEBOUNCE_MS)

    return () => clearTimeout(timer)
  }, [search])

  const meetings = useMeetings({
    q: debouncedSearch || undefined,
    participant: participant || undefined,
    sort,
  })

  // Имена для селекта — из нефильтрованного списка. Если брать их из текущей
  // выдачи, то, выбрав участника, мы вычистили бы из селекта все прочие имена
  // и вернуться к «Все участники» было бы можно, а к другому имени — нет.
  const allMeetings = useMeetings({})
  const names = useMemo(() => {
    const unique = new Set<string>()

    for (const meeting of allMeetings.data ?? []) {
      for (const person of meeting.participants) unique.add(person.name)
    }

    return [...unique].sort((left, right) => left.localeCompare(right, 'ru'))
  }, [allMeetings.data])

  const [creating, setCreating] = useState(false)
  const startCreating = () => setCreating(true)

  return (
    <div className={styles.page}>
      <AppHeader
        action={
          <Button variant="primary" onClick={startCreating}>
            + Новая встреча
          </Button>
        }
      />

      <Container>
        <div className={styles.intro}>
          <h1 className={styles.h1}>Кто, кому и сколько</h1>
          <p className={styles.lead}>
            Скидываемся, гуляем, а потом считает приложение. Заходите в любую встречу и добавляйте
            расходы — переводы пересчитаются сами.
          </p>
        </div>

        <MeetingFilters
          search={search}
          onSearchChange={setSearch}
          participant={participant}
          onParticipantChange={setParticipant}
          sort={sort}
          onSortChange={setSort}
          names={names}
        />

        {meetings.isPending && <Loader label="Ищем встречи" />}

        {meetings.isError && (
          <div className={styles.error}>
            <p className={styles.errorText}>
              {meetings.error instanceof ApiError
                ? meetings.error.humanMessage
                : 'Не удалось загрузить встречи.'}
            </p>
            <Button variant="dark" onClick={() => meetings.refetch()}>
              Повторить
            </Button>
          </div>
        )}

        {meetings.isSuccess &&
          (meetings.data.length === 0 ? (
            <MeetingsEmpty onCreate={startCreating} />
          ) : (
            <div className={styles.grid}>
              {meetings.data.map((meeting) => (
                <MeetingCard key={meeting.id} meeting={meeting} />
              ))}
            </div>
          ))}
      </Container>

      {creating && <MeetingFormModal onClose={() => setCreating(false)} />}
    </div>
  )
}
