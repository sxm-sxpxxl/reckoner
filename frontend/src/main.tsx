import { StrictMode } from 'react'
import { createRoot } from 'react-dom/client'

// Сабсеты: кириллица и латиница. Интерфейс русский, но имена и описания
// пользователь пишет какие хочет.
import '@fontsource/onest/cyrillic-400.css'
import '@fontsource/onest/cyrillic-500.css'
import '@fontsource/onest/cyrillic-600.css'
import '@fontsource/onest/cyrillic-700.css'
import '@fontsource/onest/latin-400.css'
import '@fontsource/onest/latin-500.css'
import '@fontsource/onest/latin-600.css'
import '@fontsource/onest/latin-700.css'
import '@fontsource/unbounded/cyrillic-600.css'
import '@fontsource/unbounded/cyrillic-700.css'
import '@fontsource/unbounded/latin-600.css'
import '@fontsource/unbounded/latin-700.css'
import '@fontsource/jetbrains-mono/cyrillic-500.css'
import '@fontsource/jetbrains-mono/cyrillic-700.css'
import '@fontsource/jetbrains-mono/latin-500.css'
import '@fontsource/jetbrains-mono/latin-700.css'

import './styles/tokens.css'
import './styles/base.css'
import App from './App'

createRoot(document.getElementById('root')!).render(
  <StrictMode>
    <App />
  </StrictMode>,
)
