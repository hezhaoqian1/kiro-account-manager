export async function getVersion(): Promise<string> {
  const version = (window as any).__TAURI_INTERNALS__?.metadata?.version
  return version || 'web'
}
