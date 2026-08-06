import { Link, useParams } from 'react-router-dom'

import AppHeader from '../components/layout/AppHeader'
import Container from '../components/layout/Container'
import Button from '../components/ui/Button'

/**
 * Заглушка страницы встречи. Настоящая — план 5: обложка, сводка, участники,
 * история, «Кто кому должен» и лог.
 *
 * Нужна уже сейчас, потому что создание встречи по хендоффу сразу открывает
 * её, и без маршрута человек попадал бы на пустой экран без выхода.
 */
export default function MeetingPage() {
  const { id } = useParams<{ id: string }>()

  return (
    <>
      <AppHeader
        action={
          <Link to="/">
            <Button>← Все встречи</Button>
          </Link>
        }
      />
      <Container>
        <p style={{ marginTop: 34, color: 'var(--text-2)' }}>
          Страница встречи появится следующим шагом. Идентификатор: {id}
        </p>
      </Container>
    </>
  )
}
