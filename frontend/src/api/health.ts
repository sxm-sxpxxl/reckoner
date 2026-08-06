import { useEffect, useState } from 'react'
import { api } from './client'

/** Сколько ждём здоровья сервера, прежде чем показать экран пробуждения.
 *  Живой бэкенд отвечает за единицы миллисекунд, так что мелькнуть экран
 *  не должен. */
const PATIENCE_MS = 2000

type WakeState = 'checking' | 'slow' | 'ready'

/**
 * Проверка живости бэкенда при старте приложения.
 *
 * Запрос не отменяем и таймаутов на него не ставим: спящий Render всё равно
 * ответит, а отмена заставила бы начать заново. Таймер только переключает
 * картинку.
 *
 * Если `/api/health` отказал совсем — считаем состояние готовым и пускаем
 * страницу дальше: она покажет свою ошибку с кнопкой «Повторить». Вечный
 * спиннер здесь был бы хуже, потому что из него нет выхода.
 */
export function useServerWake(): WakeState {
  const [state, setState] = useState<WakeState>('checking')

  useEffect(() => {
    let alive = true

    const timer = setTimeout(() => {
      if (alive) setState((current) => (current === 'checking' ? 'slow' : current))
    }, PATIENCE_MS)

    api
      .get('/api/health')
      .catch(() => undefined)
      .finally(() => {
        if (alive) setState('ready')
        clearTimeout(timer)
      })

    return () => {
      alive = false
      clearTimeout(timer)
    }
  }, [])

  return state
}
