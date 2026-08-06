import { useEffect, useState } from 'react'
import { ApiError, api } from './client'

/** Сколько ждём здоровья сервера, прежде чем показать экран пробуждения.
 *  Живой бэкенд отвечает за единицы миллисекунд, так что мелькнуть экран
 *  не должен. */
const PATIENCE_MS = 2000

/** Сколько всего готовы будить. Render поднимает уснувший инстанс
 *  за 30–60 секунд; дальше ждать бессмысленно — что-то сломано. */
const GIVE_UP_MS = 75_000

const POLL_MS = 2500

type WakeState = 'checking' | 'slow' | 'ready'

/**
 * Проверка живости бэкенда при старте приложения.
 *
 * Пока сервис спит, край Render отвечает `404` с `text/plain` — не наше
 * приложение, а его маршрутизатор (`x-render-routing: no-server`). Поэтому
 * одной попытки мало: первый запрос почти наверняка получит отказ, и если
 * на нём остановиться, экран пробуждения исчезнет через мгновение, а страница
 * покажет ошибку — ровно там, где надо было подождать.
 *
 * Запросы не отменяем и таймаутов не ставим: они дешёвые, а отмена заставила бы
 * начинать заново.
 *
 * Если за `GIVE_UP_MS` сервер так и не ответил — пускаем страницу дальше:
 * она покажет свою ошибку с кнопкой «Повторить». Вечный спиннер хуже, потому
 * что из него нет выхода.
 */
export function useServerWake(): WakeState {
  const [state, setState] = useState<WakeState>('checking')

  useEffect(() => {
    let alive = true

    const patience = setTimeout(() => {
      if (alive) setState((current) => (current === 'checking' ? 'slow' : current))
    }, PATIENCE_MS)

    const startedAt = Date.now()

    const probe = async () => {
      while (alive) {
        try {
          await api.get('/api/health')

          return
        } catch (error) {
          // Наши собственные ответы означают, что сервер жив и отвечает, —
          // будить некого. Повторяем только отказы края и сети.
          const worthRetrying =
            error instanceof ApiError && (error.kind === 'upstream' || error.kind === 'network')

          if (!worthRetrying || Date.now() - startedAt > GIVE_UP_MS) return

          await new Promise((resolve) => setTimeout(resolve, POLL_MS))
        }
      }
    }

    void probe().finally(() => {
      if (alive) setState('ready')
      clearTimeout(patience)
    })

    return () => {
      alive = false
      clearTimeout(patience)
    }
  }, [])

  return state
}
