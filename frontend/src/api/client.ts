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
      headers: init?.body ? { 'content-type': 'application/json' } : undefined,
    })
  } catch {
    // Сеть не дошла: сервер спит, интернета нет, CORS. Отличить нельзя —
    // `fetch` во всех этих случаях бросает одинаково.
    throw new ApiError(0, 'network', 'Сервер не отвечает')
  }

  if (response.status === 204) return undefined as T

  const text = await response.text()
  const body: unknown = text ? JSON.parse(text) : null

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
  post: <T>(path: string, body?: unknown) =>
    request<T>(path, { method: 'POST', body: JSON.stringify(body ?? {}) }),
  patch: <T>(path: string, body: unknown) =>
    request<T>(path, { method: 'PATCH', body: JSON.stringify(body) }),
  delete: <T>(path: string) => request<T>(path, { method: 'DELETE' }),
}
