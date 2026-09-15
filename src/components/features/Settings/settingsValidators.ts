export const isValidProxy = (url) => {
  if (!url) return true
  try {
    const urlObj = new URL(url)
    return ['http:', 'https:', 'socks5:', 'socks5h:', 'socks4:'].includes(urlObj.protocol)
  } catch {
    return false
  }
}

export const isValidBrowserPath = (path) => {
  if (!path) return true
  const hasValidSuffix = /\.(exe|cmd|bat|sh|app)($|\s|")/i.test(path)
  const isQuoted = path.includes('"')
  return hasValidSuffix || isQuoted || path.includes('/')
}
