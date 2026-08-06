/// <reference types="vite/client" />

interface ImportMetaEnv {
  /** Адрес бэкенда. В дев-режиме пуст: `/api` проксирует Vite. */
  readonly VITE_API_BASE_URL?: string
}

interface ImportMeta {
  readonly env: ImportMetaEnv
}
