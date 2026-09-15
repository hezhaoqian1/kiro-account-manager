import React, { useState, useEffect, useMemo } from 'react'
import { Plus, Dice6, Copy, Trash2, Pencil, Check, ToggleLeft, ToggleRight } from 'lucide-react'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Switch } from '@/components/ui/switch'
import {
  DialogRoot,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogBody,
  DialogFooter
} from '@/components/shared/dialog'
import { cn } from '@/lib/utils'
import { useApp } from '@/hooks/useApp'
import { toast } from 'sonner'
import { GatewayConfig } from './gatewayPageState'
import { useDialog } from '@/contexts/DialogContext'

interface ApiKeysDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  clientApiKeysText: string
  setConfig: React.Dispatch<React.SetStateAction<GatewayConfig>>
  onSave?: () => void
}

type ApiKeyItem = { name: string; key: string; enabled: boolean }

const DISABLED_PREFIX = '#disabled#'

const parseApiKeys = (text: string): ApiKeyItem[] =>
  (text || '')
    .split(/[\n,]+/)
    .map(k => k.trim())
    .filter(Boolean)
    .map(rawKey => {
      const enabled = !rawKey.startsWith(DISABLED_PREFIX)
      const rest = enabled ? rawKey : rawKey.substring(DISABLED_PREFIX.length)
      const colonIdx = rest.indexOf(':')
      const hasName = colonIdx > 0 && !rest.startsWith('sk-')

      return {
        name: hasName ? rest.substring(0, colonIdx) : '',
        key: hasName ? rest.substring(colonIdx + 1) : rest,
        enabled
      }
    })

const serializeApiKeys = (keys: ApiKeyItem[]) =>
  keys
    .map(({ name, key, enabled }) => `${enabled ? '' : DISABLED_PREFIX}${name ? `${name}:` : ''}${key}`)
    .join('\n')

// 生成标准 OpenAI 风格 API Key: sk-{48位大小写字母+数字}
const createApiKey = () => {
  const chars = 'ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789'
  const array = new Uint8Array(48)
  globalThis.crypto.getRandomValues(array)
  const random = Array.from(array, b => chars[b % chars.length]).join('')
  return `sk-${random}`
}

const maskApiKey = (key: string) =>
  key.length > 16 ? `${key.substring(0, 7)}${'•'.repeat(8)}${key.slice(-4)}` : key

export function ApiKeysDialog({ open, onOpenChange, clientApiKeysText, setConfig, onSave }: ApiKeysDialogProps) {
  const { t } = useApp()
  const { showConfirm } = useDialog()
  const [localKeys, setLocalKeys] = useState<ApiKeyItem[]>([])
  const [hasInitialized, setHasInitialized] = useState(false)
  const [editingIdx, setEditingIdx] = useState<number | null>(null)
  const [editingKey, setEditingKey] = useState('')
  const [copiedIdx, setCopiedIdx] = useState<number | null>(null)

  const enabledCount = useMemo(() => localKeys.filter(k => k.enabled).length, [localKeys])

  useEffect(() => {
    if (!open) {
      setHasInitialized(false)
      setEditingIdx(null)
      return
    }
    if (hasInitialized) return
    setLocalKeys(parseApiKeys(clientApiKeysText))
    setHasInitialized(true)
  }, [open, clientApiKeysText, hasInitialized])

  const writeConfig = (keysToSave: ApiKeyItem[]) => {
    setConfig((prev: any) => ({
      ...prev,
      clientApiKeysText: serializeApiKeys(keysToSave),
      apiKey: keysToSave.find(k => k.enabled)?.key || keysToSave[0]?.key || ''
    }))
  }

  const commitKeys = (updated: ApiKeyItem[]) => {
    setLocalKeys(updated)
    writeConfig(updated)
  }

  const patchKey = (idx: number, patch: Partial<ApiKeyItem>) => {
    commitKeys(localKeys.map((item, i) => i === idx ? { ...item, ...patch } : item))
  }

  const generateKey = () => {
    const idx = localKeys.length + 1
    const next = [...localKeys, { name: `Key ${idx}`, key: createApiKey(), enabled: true }]
    commitKeys(next)
    toast.success(t('gatewayKeys.generated'))
  }

  const addKey = () => {
    const idx = localKeys.length + 1
    const key = createApiKey()
    const next = [...localKeys, { name: `Key ${idx}`, key, enabled: true }]
    commitKeys(next)
    setEditingIdx(next.length - 1)
    setEditingKey(key)
    toast.success(t('gatewayKeys.addedEditable'))
  }

  const startEdit = (idx: number) => {
    setEditingIdx(idx)
    setEditingKey(localKeys[idx].key)
  }

  const confirmEdit = (idx = editingIdx) => {
    if (idx === null || !editingKey.trim()) return
    patchKey(idx, { key: editingKey.trim() })
    setEditingIdx(null)
    setEditingKey('')
  }

  const handleDelete = async (idx: number) => {
    const confirmed = await showConfirm(t('gatewayKeys.deleteConfirmMessage'), t('gatewayKeys.deleteConfirmTitle'))
    if (!confirmed) return
    commitKeys(localKeys.filter((_, i) => i !== idx))
    toast.success(t('gatewayKeys.deleted'))
  }

  const handleCopy = async (keyText: string, idx: number) => {
    try {
      await navigator.clipboard.writeText(keyText)
      setCopiedIdx(idx)
      toast.success(t('gatewayKeys.copied'))
      setTimeout(() => setCopiedIdx(null), 1500)
    } catch {
      toast.error(t('gatewayKeys.copyFailed'))
    }
  }

  const toggleAll = (enabled: boolean) => {
    commitKeys(localKeys.map(k => ({ ...k, enabled })))
    toast.success(enabled ? t('gatewayKeys.enableAllDone') : t('gatewayKeys.disableAllDone'))
  }

  const handleSave = () => {
    let finalKeys = [...localKeys]
    if (editingIdx !== null && editingKey.trim()) {
      finalKeys[editingIdx] = { ...finalKeys[editingIdx], key: editingKey.trim() }
    }
    writeConfig(finalKeys)
    setEditingIdx(null)
    onSave?.()
    onOpenChange(false)
  }

  const handleCancel = () => {
    // 取消时恢复到打开时的状态，不保存
    setLocalKeys(parseApiKeys(clientApiKeysText))
    setEditingIdx(null)
    onOpenChange(false)
  }

  return (
    <DialogRoot open={open} onOpenChange={(v) => { if (!v) handleCancel() }}>
      <DialogContent maxWidth="800px" className="max-h-[85vh]">
        <DialogHeader>
          <DialogTitle>{t('gatewayKeys.title')}</DialogTitle>
          <DialogDescription>
            {t('gatewayKeys.description', { enabled: enabledCount, total: localKeys.length })}
          </DialogDescription>
        </DialogHeader>

        <DialogBody className="flex flex-col gap-3 pt-2 min-h-0">
          {/* 工具栏 */}
          <div className="flex items-center gap-2">
            <Button size="sm" variant="default" onClick={generateKey} className="h-8 text-xs gap-1.5">
              <Dice6 size={13} /> {t('gatewayKeys.generate')}
            </Button>
            <Button size="sm" variant="outline" onClick={addKey} className="h-8 text-xs gap-1.5">
              <Plus size={13} /> {t('gatewayKeys.addManually')}
            </Button>
            <div className="ml-auto flex gap-1">
              <Button size="sm" variant="ghost" onClick={() => toggleAll(true)} className="h-7 text-[10px] gap-1" disabled={localKeys.length === 0}>
                <ToggleRight size={12} /> {t('gatewayKeys.enableAll')}
              </Button>
              <Button size="sm" variant="ghost" onClick={() => toggleAll(false)} className="h-7 text-[10px] gap-1" disabled={localKeys.length === 0}>
                <ToggleLeft size={12} /> {t('gatewayKeys.disableAll')}
              </Button>
            </div>
          </div>

          {/* Key 列表 */}
          <div className="border rounded-lg overflow-hidden max-h-[400px] overflow-y-auto">
            {localKeys.length === 0 ? (
              <div className="p-8 text-center text-sm text-muted-foreground">
                {t('gatewayKeys.emptyHint')}
              </div>
            ) : (
              <table className="w-full text-xs">
                <thead className="bg-muted/30 sticky top-0">
                  <tr>
                    <th className="p-2 text-left font-medium w-10">{t('gatewayKeys.colEnabled')}</th>
                    <th className="p-2 text-left font-medium w-28">{t('gatewayKeys.colName')}</th>
                    <th className="p-2 text-left font-medium">{t('gatewayKeys.colKey')}</th>
                    <th className="p-2 text-right font-medium w-24">{t('gatewayKeys.colActions')}</th>
                  </tr>
                </thead>
                <tbody>
                  {localKeys.map((item, idx) => (
                    <tr
                      key={`${item.key}-${idx}`}
                      className={cn(
                        'border-t hover:bg-muted/20 transition-colors',
                        !item.enabled && 'opacity-50'
                      )}
                    >
                      <td className="p-2">
                        <Switch
                          checked={item.enabled}
                          onCheckedChange={(checked) => patchKey(idx, { enabled: checked })}
                        />
                      </td>
                      <td className="p-2">
                        <Input
                          value={item.name}
                          onChange={(e) => patchKey(idx, { name: e.target.value })}
                          className="h-7 text-xs"
                          placeholder={t('gatewayKeys.namePlaceholder')}
                        />
                      </td>
                      <td className="p-2">
                        {editingIdx === idx ? (
                          <Input
                            value={editingKey}
                            onChange={(e) => setEditingKey(e.target.value)}
                            className="h-7 text-xs font-mono"
                            autoFocus
                            onKeyDown={(e) => { if (e.key === 'Enter') confirmEdit(idx) }}
                            onBlur={() => confirmEdit(idx)}
                          />
                        ) : (
                          <code className="text-xs font-mono bg-muted/40 px-2 py-1 rounded inline-block max-w-full truncate">
                            {maskApiKey(item.key)}
                          </code>
                        )}
                      </td>
                      <td className="p-2">
                        <div className="flex items-center justify-end gap-0.5">
                          {editingIdx === idx ? (
                            <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={() => confirmEdit(idx)}>
                              <Check size={12} className="text-green-600" />
                            </Button>
                          ) : (
                            <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={() => startEdit(idx)}>
                              <Pencil size={11} className="text-muted-foreground" />
                            </Button>
                          )}
                          <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={() => handleCopy(item.key, idx)}>
                            {copiedIdx === idx ? <Check size={12} className="text-green-600" /> : <Copy size={11} className="text-muted-foreground" />}
                          </Button>
                          <Button size="sm" variant="ghost" className="h-6 w-6 p-0" onClick={() => handleDelete(idx)}>
                            <Trash2 size={11} className="text-red-500" />
                          </Button>
                        </div>
                      </td>
                    </tr>
                  ))}
                </tbody>
              </table>
            )}
          </div>
        </DialogBody>

        <DialogFooter>
          <Button variant="outline" onClick={handleCancel}>{t('gatewayKeys.cancel')}</Button>
          <Button onClick={handleSave}>{t('gatewayKeys.save')}</Button>
        </DialogFooter>
      </DialogContent>
    </DialogRoot>
  )
}

export default ApiKeysDialog
