import { useState } from 'react'
import { Link, useNavigate, useParams } from 'react-router-dom'

import { ApiError } from '../api/client'
import { useMeeting } from '../api/meetings'
import type { Entry, Participant, Transfer } from '../api/types'
import AppHeader from '../components/layout/AppHeader'
import Container from '../components/layout/Container'
import ChangeLog from '../components/meeting/ChangeLog'
import DebtsSection from '../components/meeting/DebtsSection'
import HistorySection from '../components/meeting/HistorySection'
import MeetingSummary from '../components/meeting/MeetingSummary'
import ParticipantsSection from '../components/meeting/ParticipantsSection'
import ExpenseModal from '../components/modals/ExpenseModal'
import MeetingFormModal from '../components/modals/MeetingFormModal'
import ParticipantModal from '../components/modals/ParticipantModal'
import TransferModal from '../components/modals/TransferModal'
import Button from '../components/ui/Button'
import styles from './MeetingPage.module.css'

/** Какая модалка открыта. Одно состояние вместо флага на каждую: двух модалок
 *  одновременно не бывает, а так их нельзя случайно открыть вместе. */
type OpenModal =
  | { kind: 'meeting' }
  | { kind: 'participant'; participant?: Participant }
  | { kind: 'expense'; entry?: Entry }
  | { kind: 'transfer'; prefill?: Transfer }
  | null

export default function MeetingPage() {
  const { id = '' } = useParams<{ id: string }>()
  const navigate = useNavigate()
  const meeting = useMeeting(id)
  const [modal, setModal] = useState<OpenModal>(null)
  const close = () => setModal(null)

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

  const data = meeting.data
  // Расход и перевод требуют, чтобы было кому платить.
  const hasPeople = data.participants.length > 0

  return (
    <div className={styles.page}>
      {header}
      <Container>
        <MeetingSummary meeting={data} onEdit={() => setModal({ kind: 'meeting' })} />

        <ParticipantsSection
          participants={data.participants}
          onAdd={() => setModal({ kind: 'participant' })}
          onEdit={(participant) => setModal({ kind: 'participant', participant })}
        />

        <div className={styles.columns}>
          <HistorySection
            meeting={data}
            onAddExpense={() =>
              hasPeople ? setModal({ kind: 'expense' }) : setModal({ kind: 'participant' })
            }
            onAddTransfer={() =>
              hasPeople ? setModal({ kind: 'transfer' }) : setModal({ kind: 'participant' })
            }
            onEditEntry={(entry) =>
              entry.kind === 'expense'
                ? setModal({ kind: 'expense', entry })
                : // Перевод правится удалением и повторным вводом: полей у него
                  // три, и отдельная форма правки не окупается.
                  undefined
            }
          />

          <DebtsSection
            meeting={data}
            onSettle={(transfer) => setModal({ kind: 'transfer', prefill: transfer })}
          />
        </div>

        <ChangeLog log={data.log} />
      </Container>

      {modal?.kind === 'meeting' && <MeetingFormModal meeting={data} onClose={close} />}

      {modal?.kind === 'participant' && (
        <ParticipantModal meetingId={id} participant={modal.participant} onClose={close} />
      )}

      {modal?.kind === 'expense' && (
        <ExpenseModal meeting={data} entry={modal.entry} onClose={close} />
      )}

      {modal?.kind === 'transfer' && (
        <TransferModal
          meeting={data}
          defaultFrom={modal.prefill?.fromId}
          defaultTo={modal.prefill?.toId}
          defaultAmount={modal.prefill?.amountRubles}
          onClose={close}
        />
      )}
    </div>
  )
}
