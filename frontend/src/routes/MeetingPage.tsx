import { useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'

import { ApiError } from '../api/client'
import { useMeeting } from '../api/meetings'
import AppHeader from '../components/layout/AppHeader'
import Container from '../components/layout/Container'
import MeetingSummary from '../components/meeting/MeetingSummary'
import MeetingFormModal from '../components/modals/MeetingFormModal'
import Button from '../components/ui/Button'
import styles from './MeetingPage.module.css'

export default function MeetingPage() {
  const { id = '' } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const meeting = useMeeting(id)
  const [editing, setEditing] = useState(false)

  const header = (
    <AppHeader
      action={
        <Link to="/">
          <Button>← Все встречи</Button>
        </Link>
      }
    />
  )

  if (meeting.isPending) {
    return (
      <div className={styles.page}>
        {header}
        <Container>
          <div className={styles.skeleton} />
        </Container>
      </div>
    )
  }

  if (meeting.isError) {
    const notFound = meeting.error instanceof ApiError && meeting.error.status === 404

    return (
      <div className={styles.page}>
        {header}
        <Container>
          <div className={styles.state}>
            <p className={styles.stateText}>
              {notFound
                ? 'Встреча не найдена — возможно, её удалили.'
                : 'Не удалось загрузить встречу.'}
            </p>
            {notFound ? (
              <Button variant="dark" onClick={() => navigate('/')}>
                Все встречи
              </Button>
            ) : (
              <Button variant="dark" onClick={() => meeting.refetch()}>
                Повторить
              </Button>
            )}
          </div>
        </Container>
      </div>
    )
  }

  return (
    <div className={styles.page}>
      {header}
      <Container>
        <MeetingSummary meeting={meeting.data} onEdit={() => setEditing(true)} />
      </Container>

      {editing && <MeetingFormModal meeting={meeting.data} onClose={() => setEditing(false)} />}
    </div>
  )
}
