import { useState, useMemo } from 'react'
import { Button } from '@/components/ui/button'
import { DialogRoot, DialogContent, DialogHeader, DialogTitle, DialogBody } from '@/components/shared/dialog'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Label } from '@/components/ui/label'
import { Textarea } from '@/components/ui/textarea'
import { Play, Copy, RotateCw } from 'lucide-react'
import { Group } from '@/components/shared/layout'
import { useApp } from '../../../hooks/useApp'

interface Account {
  id: string
  email?: string
  userId?: string
  label?: string
  status?: string
  enabled?: boolean
  groupId?: string
}

export interface GatewayRoutingConfig {
  accountMode: string
  accountId?: string | null
  groupId?: string | null
  poolAccountIds?: string[]
}

interface ApiPlaygroundDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  accounts: Account[]
  gatewayBaseUrl: string
  gatewayApiKey: string
  routing?: GatewayRoutingConfig
}

type Protocol = 'anthropic' | 'openai-chat' | 'openai-responses'

const PROTOCOL_LABELS: Record<Protocol, string> = {
  'anthropic': 'Anthropic Messages (/v1/messages)',
  'openai-chat': 'OpenAI Chat (/v1/chat/completions)',
  'openai-responses': 'OpenAI Responses (/v1/responses)'
}

const DEFAULT_REQUESTS: Record<Protocol, any> = {
  'anthropic': {
    model: 'claude-sonnet-4-5-20250929',
    max_tokens: 1024,
    messages: [
      {
        role: 'user',
        content: '你好'
      }
    ],
    stream: true
  },
  'openai-chat': {
    model: 'claude-sonnet-4-5-20250929',
    messages: [
      {
        role: 'user',
        content: '你好'
      }
    ],
    stream: true,
    temperature: 0.7,
    top_p: 1
  },
  'openai-responses': {
    model: 'claude-sonnet-4-5-20250929',
    input: [
      {
        role: 'user',
        content: '你好'
      }
    ],
    stream: true,
    temperature: 0.7
  }
}

const QUICK_TEMPLATES: Record<string, { labelKey: string; getRequest: (protocol: Protocol) => any }> = {
  'simple': {
    labelKey: 'gateway.tplSimple',
    getRequest: (protocol: Protocol) => DEFAULT_REQUESTS[protocol]
  },
  'multi-turn': {
    labelKey: 'gateway.tplMultiTurn',
    getRequest: (protocol: Protocol) => {
      const base = DEFAULT_REQUESTS[protocol]
      if (protocol === 'openai-responses') {
        return {
          ...base,
          input: [
            { role: 'user', content: '你好，我是小明' },
            { role: 'assistant', content: '你好小明！很高兴认识你。' },
            { role: 'user', content: '我叫什么名字？' }
          ]
        }
      }
      return {
        ...base,
        messages: [
          { role: 'user', content: '你好，我是小明' },
          { role: 'assistant', content: '你好小明！很高兴认识你。' },
          { role: 'user', content: '我叫什么名字？' }
        ]
      }
    }
  },
  'non-stream': {
    labelKey: 'gateway.tplNonStream',
    getRequest: (protocol: Protocol) => ({
      ...DEFAULT_REQUESTS[protocol],
      stream: false
    })
  }
}

const UNAVAILABLE_STATUSES = new Set([
  'banned', '封禁', '已封禁',
  'invalid', '失效', '已失效', 'Token已失效',
  'expired', '过期', '已过期',
])

function isAccountSelectable(account: Account): boolean {
  if (account.enabled === false) return false
  if (account.status && UNAVAILABLE_STATUSES.has(account.status)) return false
  return true
}

function matchesRouting(account: Account, routing?: GatewayRoutingConfig): boolean {
  if (!routing) return true
  switch (routing.accountMode) {
    case 'single':
      return !routing.accountId || account.id === routing.accountId
    case 'group':
      return !routing.groupId || account.groupId === routing.groupId
    case 'pool':
      return !routing.poolAccountIds?.length || routing.poolAccountIds.includes(account.id)
    default:
      return true
  }
}

function accountLabel(account: Account): string {
  return account.email || account.userId || account.label || account.id
}

export function ApiPlaygroundDialog({
  open,
  onOpenChange,
  accounts,
  gatewayBaseUrl,
  gatewayApiKey,
  routing,
}: ApiPlaygroundDialogProps) {
  const { t } = useApp()
  const [protocol, setProtocol] = useState<Protocol>('anthropic')
  const [selectedAccount, setSelectedAccount] = useState<string>('')
  const [requestBody, setRequestBody] = useState<string>(JSON.stringify(DEFAULT_REQUESTS['anthropic'], null, 2))
  const [response, setResponse] = useState<string>('')
  const [error, setError] = useState<string>('')
  const [loading, setLoading] = useState(false)
  const [streamingChunks, setStreamingChunks] = useState<string[]>([])
  const [activeTemplate, setActiveTemplate] = useState('simple')

  // 仅展示当前网关路由配置下可用的账号
  const availableAccounts = useMemo(() => {
    return accounts.filter(acc => isAccountSelectable(acc) && matchesRouting(acc, routing))
  }, [accounts, routing])

  const routingHint = useMemo(() => {
    if (!routing) return t('gateway.routingAuto')
    switch (routing.accountMode) {
      case 'single':
        return t('gateway.routingSingle')
      case 'group':
        return t('gateway.routingGroup')
      case 'pool':
        return t('gateway.routingPool', { count: routing.poolAccountIds?.length || 0 })
      default:
        return routing.accountMode
    }
  }, [routing])

  const handleProtocolChange = (value: Protocol) => {
    setProtocol(value)
    setRequestBody(JSON.stringify(DEFAULT_REQUESTS[value], null, 2))
    setResponse('')
    setError('')
    setStreamingChunks([])
    setActiveTemplate('simple')
  }

  const handleTemplateSelect = (templateKey: string) => {
    const template = QUICK_TEMPLATES[templateKey]
    if (template) {
      setRequestBody(JSON.stringify(template.getRequest(protocol), null, 2))
      setActiveTemplate(templateKey)
    }
  }

  const handleFormatJson = () => {
    try {
      const parsed = JSON.parse(requestBody)
      setRequestBody(JSON.stringify(parsed, null, 2))
      setError('')
    } catch (e: any) {
      setError(t('gateway.jsonFormatError', { message: e.message }))
    }
  }

  const handleCopyResponse = async () => {
    try {
      await navigator.clipboard.writeText(response || streamingChunks.join('\n'))
    } catch (e) {
      console.error('Copy failed:', e)
    }
  }

  const getEndpointPath = (): string => {
    switch (protocol) {
      case 'anthropic':
        return '/v1/messages'
      case 'openai-chat':
        return '/v1/chat/completions'
      case 'openai-responses':
        return '/v1/responses'
    }
  }

  const handleSendRequest = async () => {
    setLoading(true)
    setResponse('')
    setError('')
    setStreamingChunks([])

    try {
      const body = JSON.parse(requestBody)
      const endpoint = `${gatewayBaseUrl}${getEndpointPath()}`

      const headers: Record<string, string> = {
        'Content-Type': 'application/json',
        'Authorization': `Bearer ${gatewayApiKey}`,
      }

      // Anthropic 需要特殊 header
      if (protocol === 'anthropic') {
        headers['anthropic-version'] = '2023-06-01'
      }

      // 指定账号：后端会在当前路由可用集合内强制使用该账号
      if (selectedAccount) {
        headers['x-account-id'] = selectedAccount
      }

      const res = await fetch(endpoint, {
        method: 'POST',
        headers,
        body: JSON.stringify(body),
      })

      if (!res.ok) {
        const errorText = await res.text()
        setError(`HTTP ${res.status}: ${errorText}`)
        return
      }

      // 判断是否流式响应
      const contentType = res.headers.get('content-type') || ''
      if (body.stream && contentType.includes('text/event-stream')) {
        // 流式响应处理
        const reader = res.body?.getReader()
        const decoder = new TextDecoder()
        const chunks: string[] = []

        if (reader) {
          let done = false
          while (!done) {
            const { value, done: readerDone } = await reader.read()
            done = readerDone
            if (value) {
              const chunk = decoder.decode(value, { stream: true })
              chunks.push(chunk)
              setStreamingChunks([...chunks])
            }
          }
        }
      } else {
        // 普通 JSON 响应
        const json = await res.json()
        setResponse(JSON.stringify(json, null, 2))
      }
    } catch (e: any) {
      setError(t('gateway.requestFailed', { message: e.message }))
    } finally {
      setLoading(false)
    }
  }

  return (
    <DialogRoot open={open} onOpenChange={onOpenChange}>
      <DialogContent maxWidth="min(94vw, 1280px)" className="h-[85vh] flex flex-col glass-card">
        <DialogHeader className="pb-4 border-b border-border/40">
          <DialogTitle className="flex items-center gap-2.5 text-base font-semibold">
            <div className="flex items-center justify-center h-7 w-7 rounded-lg bg-primary/10">
              <Play className="w-3.5 h-3.5 text-primary" />
            </div>
            {t('gateway.apiPlayground')}
            <span className="ml-auto inline-flex items-center gap-1.5 rounded-full bg-muted/60 border border-border/50 px-2.5 py-0.5 text-[10px] font-mono text-muted-foreground">
              <span className="h-1.5 w-1.5 rounded-full bg-green-500/80 animate-pulse"></span>
              {getEndpointPath()}
            </span>
          </DialogTitle>
        </DialogHeader>
        <DialogBody className="flex-1 overflow-hidden flex flex-col gap-4 pt-4">
          {/* 配置区域 */}
          <div className="rounded-xl border border-border/40 bg-muted/20 p-3">
            <div className="grid grid-cols-1 lg:grid-cols-[minmax(0,1fr)_minmax(0,1fr)_auto] gap-3 items-end">
              {/* 协议选择 */}
              <div className="space-y-1.5">
                <Label className="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">{t('gateway.protocol')}</Label>
                <Select value={protocol} onValueChange={(value) => handleProtocolChange(value as Protocol)}>
                  <SelectTrigger className="h-9 text-sm bg-background/80 border-border/50">
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    {Object.entries(PROTOCOL_LABELS).map(([key, label]) => (
                      <SelectItem key={key} value={key}>
                        {label}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
              </div>

              {/* 账号选择 */}
              <div className="space-y-1.5">
                <Label className="text-[11px] font-medium text-muted-foreground uppercase tracking-wide">
                  {t('gateway.specifiedAccount')}
                  <span className="ml-1.5 normal-case tracking-normal font-normal opacity-70">
                    · {routingHint}
                  </span>
                </Label>
                <Select
                  value={selectedAccount || '__default__'}
                  onValueChange={(value) => setSelectedAccount(value === '__default__' ? '' : value)}
                >
                  <SelectTrigger className="h-9 text-sm bg-background/80 border-border/50">
                    <SelectValue placeholder={t('gateway.autoSelect')} />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="__default__">{t('gateway.autoSelectLb')}</SelectItem>
                    {availableAccounts.map(acc => (
                      <SelectItem key={acc.id} value={acc.id}>
                        {accountLabel(acc)}
                      </SelectItem>
                    ))}
                  </SelectContent>
                </Select>
                {availableAccounts.length === 0 && (
                  <p className="text-[10px] text-amber-600 dark:text-amber-400">
                    {t('gateway.noAvailableAccountsForRouting')}
                  </p>
                )}
              </div>

              {/* 快速模板 */}
              <div className="flex flex-wrap gap-1.5">
                {Object.entries(QUICK_TEMPLATES).map(([key, { labelKey }]) => (
                  <Button
                    key={key}
                    variant="outline"
                    size="sm"
                    className={`h-9 text-xs px-2.5 border-border/50 transition-colors ${activeTemplate === key
                        ? 'bg-primary/10 text-primary border-primary/40 shadow-sm'
                        : 'bg-background/80 hover:bg-primary/5 hover:border-primary/40 hover:text-primary'
                      }`}
                    onClick={() => handleTemplateSelect(key)}
                    disabled={loading}
                  >
                    {t(labelKey)}
                  </Button>
                ))}
              </div>
            </div>
          </div>

          {/* 请求/响应编辑器 */}
          <div className="flex-1 grid grid-cols-1 lg:grid-cols-2 gap-3 min-h-0">
            {/* 左侧：请求配置 */}
            <div className="flex flex-col min-h-0 rounded-xl border border-border/50 overflow-hidden shadow-sm">
              <div className="flex items-center justify-between px-3 py-2 bg-gradient-to-r from-muted/40 to-muted/20 border-b border-border/40">
                <div className="flex items-center gap-2">
                  <span className="h-2 w-2 rounded-full bg-blue-500/70"></span>
                  <span className="text-xs font-medium text-foreground">{t('gateway.requestBody')}</span>
                  <span className="text-[10px] text-muted-foreground">JSON</span>
                </div>
                <Button variant="ghost" size="sm" className="h-6 text-[11px] px-2 hover:text-primary" onClick={handleFormatJson} disabled={loading}>
                  {t('gateway.format')}
                </Button>
              </div>
              <Textarea
                value={requestBody}
                onChange={(e) => {
                  setRequestBody(e.target.value)
                  setActiveTemplate('custom')
                }}
                className="flex-1 font-mono text-[11px] leading-relaxed resize-none border-0 rounded-none focus-visible:ring-0 bg-background/60"
                placeholder={t('gateway.inputJsonPlaceholder')}
                disabled={loading}
              />
            </div>

            {/* 右侧：响应展示 */}
            <div className="flex flex-col min-h-0 rounded-xl border border-border/50 overflow-hidden shadow-sm">
              <div className="flex items-center justify-between px-3 py-2 bg-gradient-to-r from-muted/40 to-muted/20 border-b border-border/40">
                <div className="flex items-center gap-2">
                  {loading ? (
                    <span className="h-2 w-2 rounded-full bg-primary animate-pulse"></span>
                  ) : response || streamingChunks.length > 0 ? (
                    <span className="h-2 w-2 rounded-full bg-green-500/70"></span>
                  ) : (
                    <span className="h-2 w-2 rounded-full bg-muted-foreground/30"></span>
                  )}
                  <span className="text-xs font-medium text-foreground">
                    {t('gateway.responseResult')}
                  </span>
                  {loading && <span className="text-[10px] text-primary animate-pulse">{t('gateway.receiving')}</span>}
                  {selectedAccount && (
                    <span className="text-[10px] text-muted-foreground font-mono truncate max-w-[180px]">
                      x-account-id={selectedAccount.slice(0, 8)}…
                    </span>
                  )}
                </div>
                {(response || streamingChunks.length > 0) && (
                  <Button variant="ghost" size="sm" className="h-6 text-[11px] px-2 hover:text-primary" onClick={handleCopyResponse}>
                    <Copy className="h-3 w-3 mr-1" />
                    {t('gateway.copy')}
                  </Button>
                )}
              </div>
              <Textarea
                value={streamingChunks.length > 0 ? streamingChunks.join('\n') : response}
                readOnly
                className="flex-1 font-mono text-[11px] leading-relaxed resize-none border-0 rounded-none focus-visible:ring-0 bg-muted/10"
                placeholder={t('gateway.responsePlaceholder')}
              />
            </div>
          </div>

          {/* 错误提示 */}
          {error && (
            <div className="rounded-xl border border-destructive/30 bg-destructive/5 px-3.5 py-2.5 flex items-start gap-2">
              <span className="h-4 w-4 rounded-full bg-destructive/20 flex items-center justify-center shrink-0 mt-0.5">
                <span className="h-1.5 w-1.5 rounded-full bg-destructive"></span>
              </span>
              <p className="text-xs text-destructive leading-relaxed break-all">{error}</p>
            </div>
          )}

          {/* 底部操作栏 */}
          <div className="flex items-center justify-between pt-3 border-t border-border/40">
            <div className="flex items-center gap-2 text-[11px] text-muted-foreground">
              <span className="font-mono truncate max-w-[560px]">{gatewayBaseUrl}{getEndpointPath()}</span>
            </div>
            <Group gap="xs">
              <Button variant="outline" size="sm" className="h-8 px-3" onClick={() => onOpenChange(false)} disabled={loading}>
                {t('gateway.close')}
              </Button>
              <Button
                size="sm"
                className="h-8 px-4 bg-primary hover:bg-primary/90 text-primary-foreground shadow-sm"
                onClick={handleSendRequest}
                disabled={loading || !requestBody.trim()}
              >
                {loading ? (
                  <>
                    <RotateCw className="h-3.5 w-3.5 mr-1.5 animate-spin" />
                    {t('gateway.requesting')}
                  </>
                ) : (
                  <>
                    <Play className="h-3.5 w-3.5 mr-1.5" />
                    {t('gateway.sendRequest')}
                  </>
                )}
              </Button>
            </Group>
          </div>
        </DialogBody>
      </DialogContent>
    </DialogRoot>
  )
}
