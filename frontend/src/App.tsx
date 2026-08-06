import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { useEffect } from 'react'
import { BrowserRouter, Route, Routes } from 'react-router-dom'

import { ApiError } from './api/client'
import { useServerWake } from './api/health'
import { playSting } from './domain/sting'
import WakeScreen from './components/layout/WakeScreen'
import MeetingPage from './routes/MeetingPage'
import MeetingsListPage from './routes/MeetingsListPage'

const queryClient = new QueryClient({
  defaultOptions: {
    queries: {
      // Мутация возвращает встречу целиком и кладётся прямо в кэш, поэтому
      // перезапрашивать сразу после неё нечего. Возврат фокуса — другое дело:
      // за это время встречу мог поменять друг.
      refetchOnWindowFocus: true,
      staleTime: 30_000,
      // Повторяем только то, что имеет смысл повторять. На `404` встречи нет
      // и не появится, на `422` тело не станет валиднее — повтор лишь удваивает
      // ожидание, и человек всё это время смотрит на пустой скелет вместо
      // «встреча не найдена».
      retry: (failureCount, error) => {
        if (error instanceof ApiError && error.status >= 400 && error.status < 500) {
          return false
        }

        return failureCount < 1
      },
    },
  },
})

/**
 * Играет приветственный сигнал на первое действие пользователя.
 *
 * Не на загрузку страницы: браузеры не дают воспроизводить звук до жеста, и
 * «звук при открытии» просто не сработал бы. Первый клик или нажатие клавиши —
 * ближайший к открытию момент, когда это разрешено.
 */
function useWelcomeSting() {
  useEffect(() => {
    const fire = () => void playSting()

    // `once` — сигнал один на загрузку страницы, а не на каждый клик.
    window.addEventListener('pointerdown', fire, { once: true })
    window.addEventListener('keydown', fire, { once: true })

    return () => {
      window.removeEventListener('pointerdown', fire)
      window.removeEventListener('keydown', fire)
    }
  }, [])
}

function Shell() {
  const wake = useServerWake()

  useWelcomeSting()

  // Экран пробуждения показываем только пока сервер молчит дольше секунды-двух.
  if (wake === 'slow') return <WakeScreen />

  return (
    <Routes>
      <Route path="/" element={<MeetingsListPage />} />
      <Route path="/meetings/:id" element={<MeetingPage />} />
    </Routes>
  )
}

export default function App() {
  return (
    <QueryClientProvider client={queryClient}>
      {/* basename — потому что Pages отдаёт сайт из подпапки; без него роутер
          не узнает собственные ссылки. */}
      <BrowserRouter basename="/reckoner">
        <Shell />
      </BrowserRouter>
    </QueryClientProvider>
  )
}
