type TauriInternals = {
  invoke?: (command: string, args?: Record<string, unknown>) => Promise<unknown>
}

export const isTauriRuntime = () =>
  typeof window !== 'undefined' && Boolean((window as any).__TAURI_INTERNALS__)

export const getAdminToken = () =>
  typeof window === 'undefined' ? '' : localStorage.getItem('kiro_admin_token') || ''

export const setAdminToken = (token: string) => {
  localStorage.setItem('kiro_admin_token', token.trim())
}

export const clearAdminToken = () => {
  localStorage.removeItem('kiro_admin_token')
}

export async function invoke<T = unknown>(command: string, args: Record<string, unknown> = {}): Promise<T> {
  const internals = typeof window !== 'undefined'
    ? (window as any).__TAURI_INTERNALS__ as TauriInternals | undefined
    : undefined

  if (internals?.invoke) {
    return internals.invoke(command, args) as Promise<T>
  }

  const response = await fetch(`/api/invoke/${encodeURIComponent(command)}`, {
    method: 'POST',
    headers: {
      'content-type': 'application/json',
      ...(getAdminToken() ? { authorization: `Bearer ${getAdminToken()}` } : {}),
    },
    body: JSON.stringify(args),
  })
  const body = await response.json().catch(() => ({}))
  if (!response.ok) {
    const message = body?.error?.message || body?.error || `请求失败 (${response.status})`
    throw new Error(String(message))
  }
  return body as T
}
