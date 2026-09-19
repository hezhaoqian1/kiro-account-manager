import { startTransition, useCallback, useEffect, useMemo, useRef, useState } from 'react'
import { Play, Square, ScrollText, Copy, Zap, TestTube2, Network, AlertCircle } from 'lucide-react'
import { Alert as AlertPrimitive, AlertDescription, AlertTitle } from '@/components/ui/alert'
import { Button } from '@/components/ui/button'
import { configureProxyClients } from '../../../api/gatewayApi'
import { useApp } from '../../../hooks/useApp'
import { Stack, Group, Badge, Card, Text } from '@/components/shared/layout'
import {
  DialogRoot,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogBody
} from '@/components/shared/dialog'
import GatewayConfigComponent from './GatewayConfig'
import { RequestLogsDialog } from './RequestLogsDialog'
import { ApiPlaygroundDialog } from './ApiPlaygroundDialog'
import { RouteTestDialog } from './RouteTestDialog'
import { GatewayConfig, GatewayStatus } from './gatewayPageState'
import { ErrorHistoryEntry } from './gatewayPageUtils'

// 定义类型接口
interface Account {
  id: string
  email?: string
  userId?: string
}

interface Group {
  id: string
  name: string
}

interface ClientConfigResult {
  success: boolean
  error?: string
}
import {
  applyGatewayLocalOnlyChange,
  buildClientSamples,
  buildGatewayActionSummary,
  buildGatewayBaseUrl,
  buildGatewayClientRecipes,
  buildGatewayIntegrationSummary,
  buildGatewayRoutingSummary,
  buildGatewaySecuritySummary,
  countEffectiveClientApiKeys,
  createGatewayFieldErrors,
  formatGatewayAccountOptionLabel,
  formatGatewayTimestamp,
  getEffectiveClientApiKey,
  mergeErrorHistory,
  redactGatewayApiKey
} from './gatewayPageUtils'
import {
  buildGatewayConfigSnapshot,
  buildGatewayRuntimeSnapshot,
  buildGatewayStatusState,
  DEFAULT_GATEWAY_CONFIG,
  DEFAULT_GATEWAY_STATUS,
  loadGatewayPageData,
  openGatewayLogDir,
  saveGatewayConfig,
  startGateway,
  stopGateway,
  hydrateGatewayConfig
} from './gatewayPageState'
import { useGatewayPolling } from './useGatewayPolling'
import { isTauriRuntime } from '../../../compat/tauriCore'

function Alert(props: any) {
  return <AlertPrimitive {...props} />
}

function ThemedAlert({ title, children, ...props }: any) {
  return (
    <Alert {...props}>
      {title && <AlertTitle className={"text-foreground"}>{title}</AlertTitle>}
      <AlertDescription className={"text-muted-foreground"}>
        {children}
      </AlertDescription>
    </Alert>
  )
}

function GatewayPage() {
  const { t } = useApp()

  const [config, setConfig] = useState<GatewayConfig>(DEFAULT_GATEWAY_CONFIG)
  const [status, setStatus] = useState<GatewayStatus>(DEFAULT_GATEWAY_STATUS)
  const [errorHistory, setErrorHistory] = useState<ErrorHistoryEntry[]>([])
  const [accounts, setAccounts] = useState<Account[]>([])
  const [groups, setGroups] = useState<Group[]>([])
  const [loading, setLoading] = useState(false)
  const [saving, setSaving] = useState(false)
  const [copySuccess, setCopySuccess] = useState('')
  const [logDir, setLogDir] = useState('')
  const [showRequestLogs, setShowRequestLogs] = useState(false)
  const [showApiPlayground, setShowApiPlayground] = useState(false)
  const [showRouteTest, setShowRouteTest] = useState(false)
  const [savedConfigSnapshot, setSavedConfigSnapshot] = useState(() => buildGatewayConfigSnapshot(DEFAULT_GATEWAY_CONFIG))
  const [appliedRuntimeSnapshot, setAppliedRuntimeSnapshot] = useState<any>(null)
  const [lastStatusSyncAt, setLastStatusSyncAt] = useState('-')
  const [showClientConfig, setShowClientConfig] = useState(false)
  const [clientConfigLoading, setClientConfigLoading] = useState(false)
  const [clientConfigResults, setClientConfigResults] = useState<ClientConfigResult[]>([])
  const [selectedClients, setSelectedClients] = useState<string[]>(['claudeCode'])
  // 「其他 OpenAI 兼容客户端」不在后端一键写入白名单内，只能复制配置，
  // 故与 selectedClients（后端 clients 参数）分开管理
  const [showOtherClient, setShowOtherClient] = useState(false)

  const hasConfiguredClients = useMemo(
    () => clientConfigResults.some(r => r.success),
    [clientConfigResults]
  )

  const accountOptions = useMemo(
    () => accounts.map(account => ({
      value: account.id,
      label: formatGatewayAccountOptionLabel(account),
      account, // 传递原始 account 数据，供账号池对话框使用
    })),
    [accounts]
  )

  const groupOptions = useMemo(
    () => groups.map(group => ({ value: group.id, label: group.name })),
    [groups]
  )

  const fieldErrors = useMemo(() => createGatewayFieldErrors(config), [config])
  const hasFieldErrors = Object.keys(fieldErrors).length > 0
  const configSnapshot = useMemo(() => buildGatewayConfigSnapshot(config), [config])
  const runtimeSnapshot = useMemo(() => buildGatewayRuntimeSnapshot(config), [config])
  const hasUnsavedChanges = configSnapshot !== savedConfigSnapshot
  const hasRuntimeChanges = !!status.running && !!appliedRuntimeSnapshot && runtimeSnapshot !== appliedRuntimeSnapshot

  // 自动保存 + 自动重启（防抖 1.5 秒）
  const autoSaveTimer = useRef<NodeJS.Timeout | null>(null)
  const isInitialLoad = useRef(true)
  useEffect(() => {
    // 跳过初始加载
    if (isInitialLoad.current) {
      isInitialLoad.current = false
      return
    }
    if (!hasUnsavedChanges) return
    if (autoSaveTimer.current) clearTimeout(autoSaveTimer.current)
    autoSaveTimer.current = setTimeout(async () => {
      try {
        await saveGatewayConfig(config)
        setSavedConfigSnapshot(buildGatewayConfigSnapshot(config))
        if (status.running) {
          await stopGateway()
          const st = await startGateway(config)
          const nextStatus = buildGatewayStatusState(st, st, config)
          setStatus(nextStatus)
          setAppliedRuntimeSnapshot(nextStatus.runtimeConfig ? buildGatewayRuntimeSnapshot(nextStatus.runtimeConfig) : buildGatewayRuntimeSnapshot(config))
          setLastStatusSyncAt(formatGatewayTimestamp())
        }
      } catch (e) {
        pushError(e)
      }
    }, 1500)
    return () => { if (autoSaveTimer.current) clearTimeout(autoSaveTimer.current) }
  }, [configSnapshot])

  const effectiveConfig = useMemo(
    () => (status.running && status.runtimeConfig ? status.runtimeConfig : config),
    [status.running, status.runtimeConfig, config]
  )
  const effectiveBaseUrl = useMemo(
    () => buildGatewayBaseUrl(
      effectiveConfig.host,
      effectiveConfig.port,
      effectiveConfig.localOnly,
      !isTauriRuntime() && typeof window !== 'undefined' ? window.location.origin : '',
    ),
    [effectiveConfig.host, effectiveConfig.port, effectiveConfig.localOnly]
  )
  const actionSummary = useMemo(
    () => buildGatewayActionSummary({ running: status.running, isDirty: hasUnsavedChanges, hasUnsavedChanges, hasRuntimeChanges, hasFieldErrors }),
    [status.running, hasUnsavedChanges, hasRuntimeChanges, hasFieldErrors]
  )
  const effectiveSecuritySummary = useMemo(
    () => buildGatewaySecuritySummary({ config: effectiveConfig }),
    [effectiveConfig]
  )
  const integrationSummary = useMemo(
    () => buildGatewayIntegrationSummary({
      baseUrl: effectiveBaseUrl,
      apiKey: effectiveConfig.clientApiKeysText || effectiveConfig.apiKey,
      clientApiKeysText: effectiveConfig.clientApiKeysText || effectiveConfig.apiKey,
      logDir,
      errorHistory
    }),
    [effectiveBaseUrl, effectiveConfig.clientApiKeysText, effectiveConfig.apiKey, logDir, errorHistory]
  )
  // 客户端接入配方：弹窗里两张可写客户端卡 + 「其他客户端」块的唯一数据源。
  const clientRecipes = useMemo(
    () => buildGatewayClientRecipes({
      baseUrl: effectiveBaseUrl,
      keyMaterial: effectiveConfig.clientApiKeysText || effectiveConfig.apiKey,
      samples: buildClientSamples(effectiveBaseUrl, effectiveConfig.clientApiKeysText || effectiveConfig.apiKey),
    }),
    [effectiveBaseUrl, effectiveConfig.clientApiKeysText, effectiveConfig.apiKey]
  )
  const writableClientRecipes = clientRecipes.filter(r => r.writable)
  const otherClientRecipe = clientRecipes.find(r => !r.writable)
  const effectiveRoutingSummary = useMemo(() => buildGatewayRoutingSummary({
    config: effectiveConfig,
    counts: {
      accounts: accounts.length,
      groups: groups.length
    },
    selectedLabels: {
      single: accountOptions.find(item => item.value === effectiveConfig.accountId)?.label,
      group: groupOptions.find(item => item.value === effectiveConfig.groupId)?.label
    }
  }), [effectiveConfig, accounts.length, groups.length, accountOptions, groupOptions])

  const latestErrorEntry = useMemo(
    () => errorHistory[0] || null,
    [errorHistory]
  )

  const consoleHighlights = useMemo(() => ([
    {
      id: 'current-entry',
      label: t('gateway.currentEntry'),
      value: effectiveBaseUrl
    },
    {
      id: 'client-key',
      // 显示掩码后的「生效」key：label 说的是密钥，就该给密钥；
      // 无生效 key 时退回状态文案（如「未配置客户端 Key」）
      label: t('gateway.clientKey'),
      value: effectiveSecuritySummary.primaryKeyMasked || effectiveSecuritySummary.apiKeyState
    },
    {
      id: 'routing-mode',
      label: t('gateway.routingMode'),
      value: effectiveRoutingSummary.modeLabel
    },
  ]), [
    effectiveBaseUrl,
    effectiveSecuritySummary.apiKeyState,
    effectiveRoutingSummary.modeLabel,
    t,
  ])

  const pollingFallbackConfig = useMemo(
    () => ({
      host: config.host,
      port: config.port
    }),
    [config.host, config.port]
  )

  const pushError = (msg: any) => {
    const normalized = String(msg?.message || msg || '').trim()
    if (!normalized) return
    setErrorHistory(prev => mergeErrorHistory(prev, normalized, formatGatewayTimestamp(), 8))
  }

  const loadAll = useCallback(async () => {
    setLoading(true)
    try {
      const { gatewayConfig, gatewayStatus, accounts: accountList, groups: groupList, logDir: gatewayLogDir } = await loadGatewayPageData()

      const nextConfig = hydrateGatewayConfig(gatewayConfig)
      const nextStatus = buildGatewayStatusState(gatewayStatus, gatewayConfig, nextConfig)
      const runtimeConfig = gatewayStatus?.running && nextStatus.runtimeConfig ? nextStatus.runtimeConfig : null
      setConfig(nextConfig)
      setSavedConfigSnapshot(buildGatewayConfigSnapshot(nextConfig))
      setAppliedRuntimeSnapshot(runtimeConfig ? buildGatewayRuntimeSnapshot(runtimeConfig) : null)
      setStatus(nextStatus)
      setLastStatusSyncAt(formatGatewayTimestamp())
      setAccounts(accountList)
      setGroups(groupList)
      setLogDir(gatewayLogDir)

      if (gatewayStatus?.lastError) {
        pushError(gatewayStatus.lastError)
      }
    } catch (e) {
      pushError(e)
    } finally {
      setLoading(false)
    }
  }, [])

  useEffect(() => {
    startTransition(() => {
      loadAll()
    })
  }, [loadAll])

  const handleStatusPoll = useCallback(({ status: nextStatus, fallbackConfig, syncedAt }: any) => {
    const nextState = buildGatewayStatusState(nextStatus, nextStatus, fallbackConfig)
    setStatus(nextState)
    setAppliedRuntimeSnapshot(nextState.running && nextState.runtimeConfig
      ? buildGatewayRuntimeSnapshot(nextState.runtimeConfig)
      : null)
    setLastStatusSyncAt(syncedAt)
    if (nextStatus?.lastError) {
      pushError(nextStatus.lastError)
    }
  }, [])

  useGatewayPolling({
    fallbackConfig: pollingFallbackConfig,
    onStatus: handleStatusPoll
  })

  const setField = (key: string, value: any) => setConfig(prev => ({ ...prev, [key]: value }))

  const createGeneratedApiKey = () => {
    const random = crypto?.randomUUID?.().replace(/-/g, '') || `${Date.now()}${Math.random().toString(36).slice(2)}`
    return `sk-${random}`
  }

  const handleRefresh = async () => {
    await loadAll()
  }

  const handleClearErrors = () => {
    setErrorHistory([])
  }

  const handleGenerateApiKey = () => {
    setConfig(prev => {
      const generatedKey = createGeneratedApiKey()
      const existingKeys = String(prev.clientApiKeysText || prev.apiKey || '').trim()
      const clientApiKeysText = existingKeys ? `${existingKeys}\n${generatedKey}` : generatedKey
      return {
        ...prev,
        apiKey: generatedKey,
        clientApiKeysText
      }
    })
  }

  const handleOpenLogDir = async () => {
    try {
      const dir = await openGatewayLogDir()
      setLogDir(String(dir || ''))
    } catch (e) {
      pushError(e)
    }
  }

  const guardInvalidConfig = () => {
    if (!hasFieldErrors) {
      return false
    }
    pushError(t('gateway.fixFormErrors'))
    return true
  }

  // 静默保存（Dialog 关闭时用，不校验、不重启）
  const handleSilentSave = async () => {
    try {
      await saveGatewayConfig(config)
      setSavedConfigSnapshot(buildGatewayConfigSnapshot(config))
    } catch (e) {
      pushError(e)
    }
  }

  const handleSave = async () => {
    if (guardInvalidConfig()) return
    setSaving(true)
    try {
      await saveGatewayConfig(config)
      setSavedConfigSnapshot(buildGatewayConfigSnapshot(config))
      // 保存成功后，如果网关正在运行则自动重启使配置生效
      if (status.running) {
        await stopGateway()
        const st = await startGateway(config)
        const nextStatus = buildGatewayStatusState(st, st, config)
        setStatus(nextStatus)
        setAppliedRuntimeSnapshot(nextStatus.runtimeConfig ? buildGatewayRuntimeSnapshot(nextStatus.runtimeConfig) : buildGatewayRuntimeSnapshot(config))
        setLastStatusSyncAt(formatGatewayTimestamp())
      }
    } catch (e) {
      pushError(e)
    } finally {
      setSaving(false)
    }
  }

  const handleStart = async () => {
    if (guardInvalidConfig()) return
    setSaving(true)
    try {
      const st = await startGateway(config)
      const nextStatus = buildGatewayStatusState(st, st, config)
      setStatus(nextStatus)
      setAppliedRuntimeSnapshot(nextStatus.runtimeConfig ? buildGatewayRuntimeSnapshot(nextStatus.runtimeConfig) : buildGatewayRuntimeSnapshot(config))
      setLastStatusSyncAt(formatGatewayTimestamp())
    } catch (e) {
      pushError(e)
    } finally {
      setSaving(false)
    }
  }

  const handleRestart = async () => {
    if (guardInvalidConfig()) return
    setSaving(true)
    try {
      if (status.running) {
        await stopGateway()
      }
      const st = await startGateway(config)
      const nextStatus = buildGatewayStatusState(st, st, config)
      setStatus(nextStatus)
      setAppliedRuntimeSnapshot(nextStatus.runtimeConfig ? buildGatewayRuntimeSnapshot(nextStatus.runtimeConfig) : buildGatewayRuntimeSnapshot(config))
      setLastStatusSyncAt(formatGatewayTimestamp())
    } catch (e) {
      pushError(e)
    } finally {
      setSaving(false)
    }
  }

  const handleStop = async () => {
    setSaving(true)
    try {
      await stopGateway()
      setStatus(prev => ({ ...prev, running: false }))
      setAppliedRuntimeSnapshot(null)
      setLastStatusSyncAt(formatGatewayTimestamp())
    } catch (e) {
      pushError(e)
    } finally {
      setSaving(false)
    }
  }

  const handleAutoStartToggle = async (checked: boolean) => {
    setField('enabled', checked)

    // 延迟执行，确保 setField 先更新状态
    setTimeout(async () => {
      try {
        // 先保存配置
        await saveGatewayConfig({ ...config, enabled: checked })
        setSavedConfigSnapshot(buildGatewayConfigSnapshot({ ...config, enabled: checked }))

        // 如果勾选自动启动且配置有效，立即启动2API
        if (checked && !hasFieldErrors) {
          setSaving(true)
          const st = await startGateway({ ...config, enabled: checked })
          const nextStatus = buildGatewayStatusState(st, st, { ...config, enabled: checked })
          setStatus(nextStatus)
          setAppliedRuntimeSnapshot(nextStatus.runtimeConfig ? buildGatewayRuntimeSnapshot(nextStatus.runtimeConfig) : buildGatewayRuntimeSnapshot({ ...config, enabled: checked }))
          setLastStatusSyncAt(formatGatewayTimestamp())
          setSaving(false)
        } else if (!checked && status.running) {
          // 如果取消自动启动且2API正在运行，停止2API
          setSaving(true)
          await stopGateway()
          setStatus(prev => ({ ...prev, running: false }))
          setAppliedRuntimeSnapshot(null)
          setLastStatusSyncAt(formatGatewayTimestamp())
          setSaving(false)
        }
      } catch (e) {
        pushError(e)
        setSaving(false)
      }
    }, 100)
  }

  const copyText = async (text: string, successMessage: string) => {
    try {
      await navigator.clipboard.writeText(text)
      setCopySuccess(successMessage)
      setTimeout(() => setCopySuccess(''), 1600)
    } catch (e) {
      pushError(e)
    }
  }

  const handleConfigureClients = async () => {
    setClientConfigLoading(true)
    try {
      const apiKey = getEffectiveClientApiKey(effectiveConfig.clientApiKeysText || effectiveConfig.apiKey)
      const results = await configureProxyClients({
        clients: selectedClients,
        host: effectiveConfig.host,
        port: effectiveConfig.port,
        apiKey,
      })
      setClientConfigResults(results)
    } catch (e) {
      pushError(e)
    } finally {
      setClientConfigLoading(false)
    }
  }

  return (
      <div className={`h-full overflow-y-auto p-6 glass-main`}>
        <div className="mb-4 flex items-center gap-3 animate-slide-in-left">
          <div className="w-10 h-10 rounded-xl bg-gradient-to-br from-primary/80 to-primary flex items-center justify-center shadow-md ring-1 ring-primary/20 flex-shrink-0">
            <Network size={20} className="text-primary-foreground" />
          </div>
          <div className="flex flex-col min-w-0">
            <h1 className="text-lg font-semibold text-foreground leading-tight">{t('nav.gateway')}</h1>
            <p className="text-sm text-muted-foreground leading-tight truncate">{t('gateway.gatewayDescription')}</p>
          </div>
        </div>
        <Stack gap="sm">
          <Card className={`glass-card border border-border rounded-xl p-3`}>
            <Stack gap="sm">
              <Group justify="space-between" align="center">
                <Group gap="xs">
                  <Text fw={700} className="text-foreground text-base">{t('gateway.kiroApiReverseProxy')}</Text>
                  {!status.running ? (
                    <Button
                      size="sm"
                      onClick={handleStart}
                      disabled={hasFieldErrors || saving || loading}
                      className="bg-green-500 hover:bg-green-600 text-white h-7 px-2.5 text-xs"
                    >
                      <Play size={12} className="mr-1" />
                      {t('gateway.start')}
                    </Button>
                  ) : (
                    <Button
                      size="sm"
                      onClick={handleStop}
                      disabled={saving || loading}
                      className="bg-red-500 hover:bg-red-600 text-white h-7 px-2.5 text-xs"
                    >
                      <Square size={12} className="mr-1" />
                      {t('gateway.stop')}
                    </Button>
                  )}
                  <Badge color={status.running ? 'green' : 'gray'}>{status.running ? t('gateway.running') : t('gateway.stopped')}</Badge>
                </Group>
                <Group gap="xs">
                  <Button variant="outline" size="sm" className="h-7 px-2.5 text-xs" onClick={() => setShowRequestLogs(true)} disabled={!status.running}>
                    <ScrollText size={12} className="mr-1" />
                    {t('gateway.requestLogs')}
                  </Button>
                  <Button variant="outline" size="sm" className="h-7 px-2.5 text-xs" onClick={() => setShowRouteTest(true)}>
                    <Network size={12} className="mr-1" />
                    {t('gateway.routeAllocationTest')}
                  </Button>
                  <Button variant="outline" size="sm" className="h-7 px-2.5 text-xs" onClick={() => setShowApiPlayground(true)} disabled={!status.running}>
                    <TestTube2 size={12} className="mr-1" />
                    {t('gateway.apiPlayground')}
                  </Button>
                </Group>
              </Group>

              <div className="grid grid-cols-3 gap-2">
                {consoleHighlights.map((item) => (
                  <div key={item.id} className="border rounded-lg p-2 group relative">
                    <Text size="xs" className={"text-muted-foreground"}>{item.label}</Text>
                    <div className="flex items-center gap-2">
                      <Text fw={700} className={"text-foreground text-sm flex-1 truncate"}>{item.value}</Text>
                      {(item.id === 'current-entry' || item.id === 'client-key') && (
                        <Button
                          size="sm"
                          variant="ghost"
                          className="h-6 w-6 p-0 opacity-0 group-hover:opacity-100 transition-opacity"
                          onClick={() => {
                            const text = item.id === 'current-entry'
                              ? effectiveBaseUrl
                              : (effectiveConfig.clientApiKeysText || effectiveConfig.apiKey || '').split('\n')[0]?.trim()
                            copyText(text, t('gateway.copiedItem', { label: item.label }))
                          }}
                        >
                          <Copy size={12} className="text-muted-foreground" />
                        </Button>
                      )}
                    </div>
                  </div>
                ))}
              </div>
            </Stack>
          </Card>

          <GatewayConfigComponent
            config={config}
            fieldErrors={fieldErrors}
            setField={setField}
            accountOptions={accountOptions}
            groupOptions={groupOptions}
            setConfig={setConfig}
            applyGatewayLocalOnlyChange={applyGatewayLocalOnlyChange}
            createGeneratedApiKey={createGeneratedApiKey}
            handleSaveConfig={handleSilentSave}
            handleAutoStartToggle={handleAutoStartToggle}
            onShowClientConfig={() => setShowClientConfig(true)}
            hasConfiguredClients={hasConfiguredClients}
          />

          <RequestLogsDialog open={showRequestLogs} onOpenChange={setShowRequestLogs} logLevel={config.logLevel} onLogLevelChange={(v) => setField('logLevel', v)} logRequests={config.logRequests} onLogRequestsChange={(v) => setField('logRequests', v)} onSave={handleSilentSave} />

          {/* 快速配置客户端弹窗 */}
          <DialogRoot open={showClientConfig} onOpenChange={setShowClientConfig}>
            <DialogContent maxWidth="680px" className="max-h-[88vh]">
              <DialogHeader className="">
                <DialogTitle className="">{t('gateway.quickClientConfig')}</DialogTitle>
                <DialogDescription className="">
                  {t('gateway.quickClientConfigDesc')}
                </DialogDescription>
              </DialogHeader>

              <DialogBody className="flex flex-col gap-4 pt-2 overflow-y-auto">
                {/* 无生效密钥警告：网关能启动，但写入后客户端全部 401 */}
                {countEffectiveClientApiKeys(effectiveConfig.clientApiKeysText || effectiveConfig.apiKey) === 0 && (
                  <div className="flex items-start gap-2 rounded-lg border border-amber-500/40 bg-amber-500/10 px-3 py-2 text-[11px] text-amber-700 dark:text-amber-400">
                    <AlertCircle size={13} className="mt-0.5 shrink-0" />
                    <span>{t('gateway.noEffectiveKeyWarn')}</span>
                  </div>
                )}

                {/* 客户端选择：每张卡直接给出该客户端要填的 Base URL 与鉴权头，
                    这两项无法从彼此推断，不给就会出现「写入成功但 401」。
                    数据统一来自 buildGatewayClientRecipes（唯一真相源）。 */}
                <div className="flex flex-col gap-2">
                  {writableClientRecipes.map(client => (
                    <div
                      key={client.id}
                      onClick={() => setSelectedClients(prev =>
                        prev.includes(client.id) ? prev.filter(c => c !== client.id) : [...prev, client.id]
                      )}
                      className={`rounded-xl border px-3 py-2.5 cursor-pointer transition-all ${selectedClients.includes(client.id)
                          ? 'border-primary bg-primary/5 shadow-sm'
                          : 'border-border bg-muted/20 hover:bg-muted/40'
                        }`}
                    >
                      <div className="flex items-center gap-2 min-w-0">
                        <Text size="sm" fw={600} className="text-foreground truncate">{client.label}</Text>
                        <span className="shrink-0 rounded-full bg-primary/15 px-1.5 py-0.5 text-[9px] font-medium text-primary">
                          {client.protocol}
                        </span>
                        <span className="ml-auto shrink-0 font-mono text-[10px] text-muted-foreground truncate" title={client.configPath}>
                          {client.configPath}
                        </span>
                      </div>
                      {/* 两个关键字段并排：Base URL 与鉴权头。
                          两者无法互相推导，且填错就 401，所以放在同一行直接对照。 */}
                      <div className="mt-1.5 grid grid-cols-2 gap-x-3 gap-y-1 text-[10px]">
                        <div className="flex items-center gap-1 min-w-0">
                          <span className="shrink-0 text-muted-foreground">{t('gateway.baseUrlLabel')}</span>
                          <code className="truncate font-mono text-foreground" title={client.baseUrl}>{client.baseUrl}</code>
                        </div>
                        <div className="flex items-center gap-1 min-w-0">
                          <span className="shrink-0 text-muted-foreground">{t('gateway.authHeaderLabel')}</span>
                          <code className="truncate font-mono text-foreground" title={client.authHeader}>{client.authHeader}</code>
                        </div>
                        <div className="flex items-center gap-1 min-w-0">
                          <span className="shrink-0 text-muted-foreground">{t('gateway.endpointLabel')}</span>
                          <code className="truncate font-mono text-muted-foreground" title={client.endpoint}>{client.endpoint}</code>
                        </div>
                        <div className="flex items-center gap-1 min-w-0">
                          <span className="shrink-0 text-muted-foreground">{t('gateway.clientKey')}</span>
                          <code className="truncate font-mono text-foreground" title={client.keyMasked || '-'}>{client.keyMasked || '-'}</code>
                        </div>
                      </div>
                    </div>
                  ))}
                </div>

                {/* 其他客户端：不在后端一键写入白名单内，只给可复制的完整配置。
                    没有这一段，长尾客户端（Cherry Studio / Cline / Continue 等）无处可去。 */}
                <div className="rounded-xl border border-border bg-muted/20">
                  <button
                    onClick={() => setShowOtherClient(v => !v)}
                    className="flex w-full items-center justify-between px-3 py-2 text-left cursor-pointer"
                  >
                    <div className="flex items-center gap-2">
                      <span className="text-sm font-medium text-foreground">{t('gateway.otherClient')}</span>
                      <span className="rounded-full bg-muted px-1.5 py-0.5 text-[9px] font-medium text-muted-foreground">
                        {t('gateway.otherClientHint')}
                      </span>
                    </div>
                    <span className="text-[10px] text-muted-foreground">{showOtherClient ? '▾' : '▸'}</span>
                  </button>

                  {showOtherClient && (
                    <div className="flex flex-col gap-2 border-t border-border px-3 py-2.5">
                      <div className="grid grid-cols-2 gap-2 text-[11px]">
                        <div className="flex flex-col gap-0.5">
                          <span className="text-[10px] text-muted-foreground">{t('gateway.baseUrlLabel')}</span>
                          <code className="truncate rounded bg-background/60 px-1.5 py-1 font-mono" title={otherClientRecipe?.baseUrl}>
                            {otherClientRecipe?.baseUrl}
                          </code>
                        </div>
                        <div className="flex flex-col gap-0.5">
                          <span className="text-[10px] text-muted-foreground">{t('gateway.authHeaderLabel')}</span>
                          <code className="truncate rounded bg-background/60 px-1.5 py-1 font-mono">
                            {otherClientRecipe?.authHeader}
                          </code>
                        </div>
                        <div className="flex flex-col gap-0.5">
                          <span className="text-[10px] text-muted-foreground">{t('gateway.endpointLabel')}</span>
                          <code className="truncate rounded bg-background/60 px-1.5 py-1 font-mono">
                            {otherClientRecipe?.endpoint}
                          </code>
                        </div>
                        <div className="flex flex-col gap-0.5">
                          <span className="text-[10px] text-muted-foreground">{t('gateway.clientKey')}</span>
                          <code className="truncate rounded bg-background/60 px-1.5 py-1 font-mono">
                            {otherClientRecipe?.keyMasked || '-'}
                          </code>
                        </div>
                      </div>
                      <Button
                        size="sm"
                        variant="outline"
                        className="h-7 w-full text-xs"
                        onClick={() => copyText(
                          otherClientRecipe?.copySample ?? '',
                          t('gateway.copiedItem', { label: t('gateway.otherClient') })
                        )}
                      >
                        <Copy size={11} className="mr-1" />
                        {t('gateway.copyCurl')}
                      </Button>
                    </div>
                  )}
                </div>

                {/* 配置预览：直接渲染 recipe.configPreview（= 一键写入时实际写入的完整文本），
                    不再手搓 env 行——历史上手写版与真实写入内容对不上
                    （如 Claude Code 实际写 ANTHROPIC_AUTH_TOKEN 而非 API_KEY）。 */}
                <div className="bg-muted/30 border border-border rounded-xl p-3">
                  <Text size="xs" className="text-muted-foreground mb-2">{t('gateway.configToWrite')}</Text>
                  <div className="flex flex-col gap-2 font-mono text-[11px]">
                    {selectedClients.map((id) => {
                      const recipe = writableClientRecipes.find(r => r.id === id)
                      if (!recipe) return null
                      return (
                        <div key={id} className="flex flex-col gap-0.5 p-2 rounded-lg bg-muted/30">
                          <span className="text-muted-foreground text-[10px] font-sans">{recipe.label} → {recipe.configPath}</span>
                          <pre className="whitespace-pre-wrap break-all text-foreground">{recipe.writeConfig}</pre>
                        </div>
                      )
                    })}
                  </div>
                </div>

                {/* 执行按钮 */}
                <Button
                  onClick={handleConfigureClients}
                  disabled={selectedClients.length === 0 || clientConfigLoading}
                  className="w-full bg-primary hover:bg-primary/90 text-primary-foreground"
                >
                  <Zap size={16} className="mr-1" />
                  {clientConfigLoading ? t('gateway.configuring') : t('gateway.oneClickConfigClients', { count: selectedClients.length })}
                </Button>

                {/* 结果展示 */}
                {clientConfigResults.length > 0 && (
                  <div className="flex flex-col gap-2">
                    {clientConfigResults.map((result: any, idx: number) => (
                      <div key={idx} className={`p-3 rounded-lg border ${result.success ? 'border-green-500/30 bg-green-500/5' : 'border-red-500/30 bg-red-500/5'}`}>
                        <Text size="sm" fw={600} className={result.success ? 'text-green-600' : 'text-red-500'}>
                          {result.success ? '✓' : '✗'} {result.client}
                        </Text>
                        {result.success && result.paths?.length > 0 && (
                          <Text size="xs" className="text-muted-foreground font-mono mt-1">
                            {result.paths.join(', ')}
                          </Text>
                        )}
                        {result.error && (
                          <Text size="xs" className="text-red-500 mt-1">{result.error}</Text>
                        )}
                      </div>
                    ))}
                  </div>
                )}
              </DialogBody>
            </DialogContent>
          </DialogRoot>

          <RouteTestDialog
            open={showRouteTest}
            onOpenChange={setShowRouteTest}
            config={effectiveConfig}
          />

          {/* API Playground */}
          <ApiPlaygroundDialog
            open={showApiPlayground}
            onOpenChange={setShowApiPlayground}
            accounts={accounts}
            gatewayBaseUrl={effectiveBaseUrl}
            gatewayApiKey={getEffectiveClientApiKey(effectiveConfig.clientApiKeysText || effectiveConfig.apiKey)}
            routing={{
              accountMode: effectiveConfig.accountMode,
              accountId: effectiveConfig.accountId,
              groupId: effectiveConfig.groupId,
              poolAccountIds: effectiveConfig.poolAccountIds,
            }}
          />
        </Stack>
      </div>
  )
}

export default GatewayPage
