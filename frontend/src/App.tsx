import { useEffect, useState } from 'react'
import './App.css'

type HealthStatus =
  | { state: 'loading' }
  | { state: 'ok'; body: unknown }
  | { state: 'error'; message: string }

function App() {
  const [health, setHealth] = useState<HealthStatus>({ state: 'loading' })

  useEffect(() => {
    fetch('/api/health')
      .then((res) => {
        if (!res.ok) throw new Error(`HTTP ${res.status}`)
        return res.json()
      })
      .then((body) => setHealth({ state: 'ok', body }))
      .catch((err) =>
        setHealth({ state: 'error', message: String(err?.message ?? err) }),
      )
  }, [])

  return (
    <main className="status-card">
      <h1>reckoner</h1>
      <p className="hint">
        Проверка связи с backend: <code>GET /api/health</code>
      </p>
      {health.state === 'loading' && <p className="status status-loading">Проверка…</p>}
      {health.state === 'ok' && (
        <p className="status status-ok">
          ok — {JSON.stringify(health.body)}
        </p>
      )}
      {health.state === 'error' && (
        <p className="status status-error">Ошибка: {health.message}</p>
      )}
    </main>
  )
}

export default App
