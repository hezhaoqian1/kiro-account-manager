export async function writeTextFile(path: string, contents: string): Promise<void> {
  if (typeof window !== 'undefined' && !path) {
    const blob = new Blob([contents], { type: 'application/json' })
    const url = URL.createObjectURL(blob)
    const anchor = document.createElement('a')
    anchor.href = url
    anchor.download = 'kiro-accounts.json'
    anchor.click()
    URL.revokeObjectURL(url)
  }
}
