import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

// https://vite.dev/config/
export default defineConfig({
  // GitHub Pages отдаёт сайт из подпапки репозитория. Ставится сразу, а не
  // перед деплоем: иначе пришлось бы переписывать все пути к ассетам и ссылки
  // роутера. Дев-сервер после этого живёт на /reckoner/ — как и прод.
  base: '/reckoner/',
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      devOptions: {
        enabled: true,
      },
      manifest: {
        name: 'Финальная расплата',
        // Короткое имя подписывает иконку на домашнем экране, и места там
        // на 12 символов: полное название система обрежет многоточием.
        short_name: 'Расплата',
        display: 'standalone',
        start_url: '/reckoner/',
        scope: '/reckoner/',
        // Интерфейс русский. По умолчанию плагин ставит `en`, и системе это
        // важно: от языка зависят переносы и голос экранного диктора.
        lang: 'ru',
        background_color: '#F2EEE5',
        theme_color: '#F2EEE5',
        icons: [
          // Знак «Ф» белым на #16150F — тот же, что в шапке приложения.
          // maskable отдельными файлами и с большими полями: система вырезает
          // из них круг или скруглённый квадрат по своему вкусу, и знак
          // из обычной иконки она обрезала бы по краям.
          {
            src: 'pwa-192x192.png',
            sizes: '192x192',
            type: 'image/png',
          },
          {
            src: 'pwa-512x512.png',
            sizes: '512x512',
            type: 'image/png',
          },
          {
            src: 'pwa-maskable-192x192.png',
            sizes: '192x192',
            type: 'image/png',
            purpose: 'maskable',
          },
          {
            src: 'pwa-maskable-512x512.png',
            sizes: '512x512',
            type: 'image/png',
            purpose: 'maskable',
          },
        ],
      },
    }),
  ],
  server: {
    // Явный IPv4: по умолчанию Vite биндится на localhost, который на Windows
    // разрешается в ::1, и dev-сервер становится недоступен по 127.0.0.1.
    // Поменяйте на true, чтобы открыть доступ по локальной сети (проверка с телефона).
    host: '127.0.0.1',
    proxy: {
      '/api': {
        // Тоже 127.0.0.1, а не localhost: backend слушает 0.0.0.0, то есть только
        // IPv4, и если Node разрешит localhost в ::1 — прокси упадёт с ECONNREFUSED.
        target: 'http://127.0.0.1:3000',
        changeOrigin: true,
      },
    },
  },
})
