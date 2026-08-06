import { QueryClient, QueryClientProvider } from '@tanstack/react-query'
import { BrowserRouter, Route, Routes } from 'react-router-dom'

import { useServerWake } from './api/health'
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
      retry: 1,
    },
  },
})

function Shell() {
  const wake = useServerWake()

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
