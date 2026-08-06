/**
 * Тема оформления.
 *
 * Выбор личный и хранится только в браузере: на сервер не уходит и другим
 * участникам встречи не передаётся. Общая тема была бы странной — приложение
 * без авторизации, «своей» настройки у человека там просто нет места.
 */

export type Theme = 'dark' | 'light'

const STORAGE_KEY = 'reckoner:theme'

/** Тёмная по умолчанию: она и есть основная, светлая — альтернатива. */
const DEFAULT: Theme = 'dark'

export function readTheme(): Theme {
  try {
    const saved = localStorage.getItem(STORAGE_KEY)

    if (saved === 'light' || saved === 'dark') return saved
  } catch {
    // Приватный режим может запретить `localStorage` — берём умолчание.
  }

  return DEFAULT
}

/**
 * Ставит атрибут на `<html>` и запоминает выбор.
 *
 * Вызывается ещё до отрисовки React (см. `main.tsx`): иначе на каждой загрузке
 * у выбравшего светлую тему моргала бы тёмная.
 */
export function applyTheme(theme: Theme): void {
  document.documentElement.dataset.theme = theme

  // Цвет строки состояния в мобильных браузерах и в установленной PWA.
  const meta = document.querySelector('meta[name="theme-color"]')

  meta?.setAttribute('content', theme === 'light' ? '#F2EEE5' : '#0E1013')

  try {
    localStorage.setItem(STORAGE_KEY, theme)
  } catch {
    // Не сохранилось — в этой сессии тема всё равно применена.
  }
}
