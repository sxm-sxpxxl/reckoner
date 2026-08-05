import { defineConfig } from 'vite'
import react from '@vitejs/plugin-react'
import { VitePWA } from 'vite-plugin-pwa'

// https://vite.dev/config/
export default defineConfig({
  plugins: [
    react(),
    VitePWA({
      registerType: 'autoUpdate',
      devOptions: {
        enabled: true,
      },
      manifest: {
        name: 'Reckoner',
        short_name: 'Reckoner',
        display: 'standalone',
        start_url: '/',
        background_color: '#ffffff',
        theme_color: '#1e64c8',
        icons: [
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
