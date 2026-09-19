import { FormEvent, useState } from 'react'
import { LockKeyhole } from 'lucide-react'
import { invoke, setAdminToken } from '@/compat/tauriCore'
import { Button } from '../ui/button'
import { Input } from '../ui/input'

interface WebAdminLoginProps {
  onAuthenticated: () => void
}

export default function WebAdminLogin({ onAuthenticated }: WebAdminLoginProps) {
  const [token, setToken] = useState('')
  const [error, setError] = useState('')
  const [loading, setLoading] = useState(false)

  const submit = async (event: FormEvent) => {
    event.preventDefault()
    if (!token.trim() || loading) return
    setLoading(true)
    setError('')
    try {
      setAdminToken(token)
      await invoke('get_current_user')
      onAuthenticated()
    } catch (reason: any) {
      setAdminToken('')
      setError(reason?.message || '管理员令牌无效')
    } finally {
      setLoading(false)
    }
  }

  return (
    <div className="h-screen w-full flex items-center justify-center bg-background px-6">
      <form onSubmit={submit} className="w-full max-w-sm space-y-5 rounded-xl border border-border bg-card p-7 shadow-xl">
        <div className="flex items-center gap-3">
          <div className="flex h-10 w-10 items-center justify-center rounded-lg bg-primary/15 text-primary">
            <LockKeyhole size={20} />
          </div>
          <div>
            <h1 className="text-lg font-semibold">Kiro Account Manager</h1>
            <p className="text-sm text-muted-foreground">管理员登录</p>
          </div>
        </div>
        <Input
          type="password"
          value={token}
          onChange={(event) => setToken(event.target.value)}
          placeholder="ADMIN_TOKEN"
          autoComplete="current-password"
          autoFocus
        />
        {error && <p className="text-sm text-destructive">{error}</p>}
        <Button type="submit" className="w-full" disabled={loading || !token.trim()}>
          {loading ? '验证中…' : '登录管理后台'}
        </Button>
      </form>
    </div>
  )
}
