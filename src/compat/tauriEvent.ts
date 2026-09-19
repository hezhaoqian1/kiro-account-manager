import { invoke, isTauriRuntime } from './tauriCore'

export type UnlistenFn = () => void

export async function listen<T = unknown>(event: string, handler: (event: { payload: T }) => void): Promise<UnlistenFn> {
  if (isTauriRuntime()) {
    const internals = (window as any).__TAURI_INTERNALS__
    if (internals?.listen) return internals.listen(event, handler)
  }
  void event
  void handler
  return () => undefined
}

export async function emit(event: string, payload?: unknown): Promise<void> {
  if (!isTauriRuntime()) return
  const internals = (window as any).__TAURI_INTERNALS__
  if (internals?.emit) {
    await internals.emit(event, payload)
  } else {
    void invoke
  }
}
