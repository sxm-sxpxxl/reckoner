import type { ApiErrorBody } from './types'

/** В дев-режиме пусто: Vite проксирует `/api` на бэкенд (см. vite.config.ts).
 *  В проде — полный адрес сервиса на Render, задаётся при сборке. */
const BASE = import.meta.env.VITE_API_BASE_URL ?? ''

/**
 * Отказ API в разобранном виде.
 *
 * Сервер отвечает единой формой на все ошибки, поэтому и клиенту хватает
 * одного класса. `field` есть только у `422` и указывает, какой инпут
 * подсветить.
 */
export class ApiError extends Error {
  readonly status: number
  readonly kind: string
  readonly field?: string

  constructor(status: number, kind: string, message: string, field?: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
    this.kind = kind
    this.field = field
  }

  /** Текст для человека. Разработческие подробности сюда не попадают. */
  get humanMessage(): string {
    if (this.kind === 'validation') return this.message
    if (this.kind === 'not-found') return 'Встреча не найдена — возможно, её удалили.'
    if (this.kind === 'network') return 'Сервер не отвечает. Проверьте соединение.'

    return 'Что-то пошло не так. Попробуйте ещё раз.'
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  let response: Response

  try {
    response = await fetch(BASE + path, {
      ...init,
      // Заголовок из `init` важнее: у обложки тело — блоб, и подставить ему
      // `application/json` значило бы соврать о типе. JSON-методы своего
      // заголовка не передают, поэтому им достаётся значение по умолчанию.
      headers: init?.headers ?? (init?.body ? { 'content-type': 'application/json' } : undefined),
    })
  } catch {
    // Сеть не дошла: сервер спит, интернета нет, CORS. Отличить нельзя —
    // `fetch` во всех этих случаях бросает одинаково.
    throw new ApiError(0, 'network', 'Сервер не отвечает')
  }

  if (response.status === 204) return undefined as T

  const text = await response.text()
  let body: unknown = null

  if (text) {
    try {
      body = JSON.parse(text)
    } catch {
      // Наше API отвечает JSON всегда, на любой ошибке. Текст в теле означает,
      // что отвечали не мы: так выглядит край Render, пока инстанс
      // просыпается, — `404 Not Found` с `text/plain` и заголовком
      // `x-render-routing: no-server`. Раньше `JSON.parse` здесь просто падал,
      // и наружу летел SyntaxError вместо понятной ошибки.
      throw new ApiError(response.status, 'upstream', 'Сервер ещё просыпается')
    }
  }

  if (!response.ok) {
    const error = (body ?? {}) as Partial<ApiErrorBody>

    throw new ApiError(
      response.status,
      error.error ?? 'unknown',
      error.message ?? 'Запрос не прошёл',
      error.field,
    )
  }

  return body as T
}

export const api = {
  get: <T>(path: string) => request<T>(path),
  /** Сырое тело — для обложек. `content-type` берётся у самого блоба;
   *  сервер всё равно определяет тип по байтам, но заголовок не должен врать. */
  putBlob: <T>(path: string, blob: Blob) =>
    request<T>(path, {
      method: 'PUT',
      body: blob,
      headers: { 'content-type': blob.type || 'application/octet-stream' },
    }),
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: 'POST', body: JSON.stringify(body ?? {}) }),
  patch: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'PATCH', body: JSON.stringify(body) }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
}
