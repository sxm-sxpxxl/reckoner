import type { SortMode } from '../../api/types'
import Select from '../ui/Select'
import styles from './MeetingFilters.module.css'

const SORT_LABELS: Record<SortMode, string> = {
  'date-desc': 'Сначала новые',
  'date-asc': 'Сначала старые',
  'total-desc': 'По сумме ↓',
  'open-first': 'Сначала незакрытые',
}

interface MeetingFiltersProps {
  search: string
  onSearchChange: (value: string) => void
  participant: string
  onParticipantChange: (value: string) => void
  sort: SortMode
  onSortChange: (value: SortMode) => void
  /** Имена для селекта. Собираются из нефильтрованного запроса, иначе фильтр
   *  по участнику вычистил бы из списка все остальные имена и снять его было
   *  бы нечем. */
  names: string[]
}

export default function MeetingFilters({
  search,
  onSearchChange,
  participant,
  onParticipantChange,
  sort,
  onSortChange,
  names,
}: MeetingFiltersProps) {
  return (
    <div className={styles.filters}>
      <input
        className={styles.search}
        type="search"
        value={search}
        placeholder="Поиск по встречам"
        aria-label="Поиск по встречам"
        onChange={(event) => onSearchChange(event.target.value)}
      />

      <Select
        className={styles.select}
        value={participant}
        onChange={onParticipantChange}
        label="Фильтр по участнику"
        options={[
          { value: '', label: 'Все участники' },
          ...names.map((name) => ({ value: name, label: name })),
        ]}
      />

      <Select
        className={styles.select}
        value={sort}
        onChange={(value) => onSortChange(value as SortMode)}
        label="Сортировка"
        options={Object.entries(SORT_LABELS).map(([value, label]) => ({ value, label }))}
      />
    </div>
  )
}
