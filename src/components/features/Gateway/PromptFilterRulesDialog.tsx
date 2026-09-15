import React, { useState, useEffect } from 'react'
import { Plus, Trash2, Filter } from 'lucide-react'
import { Badge } from '@/components/ui/badge'
import { Button } from '@/components/ui/button'
import { Input } from '@/components/ui/input'
import { Label } from '@/components/ui/label'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '@/components/ui/select'
import { Switch } from '@/components/ui/switch'
import { Textarea } from '@/components/ui/textarea'
import {
  DialogRoot,
  DialogContent,
  DialogHeader,
  DialogTitle,
  DialogDescription,
  DialogBody,
  DialogFooter
} from '@/components/shared/dialog'
import { toast } from 'sonner'
import { useApp } from '../../../hooks/useApp'
import { PromptFilterRule } from './gatewayPageState'

// 预置过滤规则
const PRESET_RULES = [
  {
    nameKey: 'gateway.presetFilterGitStatus',
    ruleType: 'lines-containing',
    matchPattern: 'git status',
    replace: ''
  },
  {
    nameKey: 'gateway.presetFilterRecentCommits',
    ruleType: 'lines-containing',
    matchPattern: 'Recent commits:',
    replace: ''
  },
  {
    nameKey: 'gateway.presetFilterKnowledgeCutoff',
    ruleType: 'lines-containing',
    matchPattern: 'Assistant knowledge cutoff',
    replace: ''
  },
  {
    nameKey: 'gateway.presetFilterBillingHeader',
    ruleType: 'lines-containing',
    matchPattern: 'x-anthropic-billing-header:',
    replace: ''
  },
  {
    nameKey: 'gateway.presetFilterFastMode',
    ruleType: 'regex',
    matchPattern: '<fast_mode_info>.*?</fast_mode_info>',
    replace: ''
  },
  {
    nameKey: 'gateway.presetFilterProjectPath',
    ruleType: 'lines-containing',
    matchPattern: '.claude/projects/',
    replace: ''
  }
]

interface PromptFilterRulesDialogProps {
  open: boolean
  onOpenChange: (open: boolean) => void
  promptFilterRules: PromptFilterRule[]
  setField: (key: string, value: PromptFilterRule[] | string | boolean) => void
  onSave?: () => void
}

function PromptFilterRulesDialog({ open, onOpenChange, promptFilterRules, setField, onSave }: PromptFilterRulesDialogProps) {
  const { t } = useApp()
  const rules = promptFilterRules || []

  const [newRuleName, setNewRuleName] = useState('')
  const [newRuleType, setNewRuleType] = useState('lines-containing')
  const [newMatchPattern, setNewMatchPattern] = useState('')
  const [newReplace, setNewReplace] = useState('')

  // 弹窗关闭时清理表单输入
  useEffect(() => {
    if (!open) {
      setNewRuleName('')
      setNewRuleType('lines-containing')
      setNewMatchPattern('')
      setNewReplace('')
    }
  }, [open])

  const handleToggle = (idx: number, checked: boolean) => {
    const updated = [...rules]
    updated[idx] = { ...updated[idx], enabled: checked }
    setField('promptFilterRules', updated)
  }

  const handleDelete = (idx: number) => {
    setField('promptFilterRules', rules.filter((_: PromptFilterRule, i: number) => i !== idx))
  }

  const handleAdd = () => {
    if (!newRuleName.trim() || !newMatchPattern.trim()) return

    const newRule = {
      id: crypto.randomUUID(),
      name: newRuleName.trim(),
      enabled: true,
      ruleType: newRuleType,
      matchPattern: newMatchPattern.trim(),
      replace: newRuleType === 'regex' ? newReplace : ''
    }
    setField('promptFilterRules', [...rules, newRule])
    setNewRuleName('')
    setNewMatchPattern('')
    setNewReplace('')
    toast.success(t('gateway.filterRuleAdded', { name: newRule.name }))
  }

  const handlePreset = () => {
    const existingPatterns = new Set(rules.map((r: PromptFilterRule) => r.matchPattern))
    const newRules = PRESET_RULES
      .filter(p => !existingPatterns.has(p.matchPattern))
      .map(p => ({
        id: crypto.randomUUID(),
        name: t(p.nameKey),
        enabled: true,
        ruleType: p.ruleType,
        matchPattern: p.matchPattern,
        replace: p.replace
      }))
    if (newRules.length > 0) {
      setField('promptFilterRules', [...rules, ...newRules])
      toast.success(t('gateway.presetRulesLoaded', { count: newRules.length }))
    } else {
      toast.info(t('gateway.allPresetRulesExist'))
    }
  }

  const handleSave = async () => {
    if (onSave) {
      await onSave()
    }
    onOpenChange(false)
  }

  return (
    <DialogRoot open={open} onOpenChange={onOpenChange}>
      <DialogContent maxWidth="960px" className="max-h-[85vh]">
        <DialogHeader>
          <DialogTitle>{t('gateway.promptFilterRules')}</DialogTitle>
          <DialogDescription>
            {t('gateway.customRegexOrKeywordFilterRules')}
          </DialogDescription>
        </DialogHeader>

        {/* 滚动容器 */}
        <DialogBody className="space-y-4 pr-1 pt-2">
          {/* 现有规则列表 */}
          {rules.length > 0 && (
            <div className="space-y-2">
              <Label className="text-sm font-medium">{t('gateway.configuredRules', { count: rules.length })}</Label>
              <div className="space-y-2 max-h-64 overflow-y-auto border rounded-lg p-3 bg-muted/20">
                {rules.map((rule: any, idx: number) => (
                  <div key={rule.id || idx} className="flex items-start gap-3 p-3 rounded-lg border bg-background">
                    <Switch
                      checked={rule.enabled}
                      onCheckedChange={(checked: boolean) => handleToggle(idx, checked)}
                      className="mt-1"
                    />
                    <div className="flex-1 min-w-0 space-y-1">
                      <div className="flex items-center gap-2">
                        <span className="font-medium text-sm">{rule.name}</span>
                        <Badge variant="outline" className="text-xs">
                          {rule.ruleType === 'regex' ? t('gateway.regex') : t('gateway.containKeywords')}
                        </Badge>
                      </div>
                      <div className="text-xs text-muted-foreground font-mono break-all">
                        {t('gateway.match')}: {rule.matchPattern}
                      </div>
                      {rule.ruleType === 'regex' && rule.replace && (
                        <div className="text-xs text-muted-foreground font-mono break-all">
                          {t('gateway.replace')}: {rule.replace}
                        </div>
                      )}
                    </div>
                    <Button
                      size="sm"
                      variant="ghost"
                      className="h-8 w-8 p-0 text-destructive hover:text-destructive"
                      onClick={() => handleDelete(idx)}
                    >
                      <Trash2 size={14} />
                    </Button>
                  </div>
                ))}
              </div>
            </div>
          )}

          {/* 添加新规则 */}
          <div className="space-y-3 border rounded-lg p-4 bg-muted/10">
            <Label className="text-sm font-medium">{t('gateway.newRuleSection')}</Label>
            <div className="grid grid-cols-2 gap-3">
              <div className="space-y-1.5">
                <Label className="text-xs text-muted-foreground">{t('gateway.ruleName')}</Label>
                <Input
                  value={newRuleName}
                  onChange={(e) => setNewRuleName(e.target.value)}
                  placeholder={t('gateway.ruleNamePlaceholder')}
                />
              </div>
              <div className="space-y-1.5">
                <Label className="text-xs text-muted-foreground">{t('gateway.ruleType')}</Label>
                <Select value={newRuleType} onValueChange={setNewRuleType}>
                  <SelectTrigger>
                    <SelectValue />
                  </SelectTrigger>
                  <SelectContent>
                    <SelectItem value="lines-containing">{t('gateway.ruleTypeLinesContaining')}</SelectItem>
                    <SelectItem value="regex">{t('gateway.ruleTypeRegex')}</SelectItem>
                  </SelectContent>
                </Select>
              </div>
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">{t('gateway.matchPattern')}</Label>
              <Textarea
                value={newMatchPattern}
                onChange={(e) => setNewMatchPattern(e.target.value)}
                placeholder={t('gateway.matchPatternPlaceholder')}
                rows={2}
                className="font-mono text-xs"
              />
            </div>
            <div className="space-y-1.5">
              <Label className="text-xs text-muted-foreground">{t('gateway.replaceContentRegexEmpty')}</Label>
              <Input
                value={newReplace}
                onChange={(e) => setNewReplace(e.target.value)}
                placeholder={t('gateway.replacePlaceholderEmpty')}
                className="font-mono text-xs"
                disabled={newRuleType !== 'regex'}
              />
            </div>
            <div className="flex gap-2">
              <Button
                size="sm"
                onClick={handleAdd}
                className="flex-1"
                disabled={!newRuleName.trim() || !newMatchPattern.trim()}
              >
                <Plus size={14} className="mr-1" />
                {t('gateway.addRule')}
              </Button>
              <Button size="sm" variant="outline" onClick={handlePreset}>
                <Filter size={14} className="mr-1" />
                {t('gateway.addPresetRules')}
              </Button>
            </div>
          </div>
        </DialogBody>

        {/* 底部操作 */}
        <DialogFooter>
          <Button variant="outline" onClick={() => onOpenChange(false)}>
            {t('gateway.cancel')}
          </Button>
          <Button onClick={handleSave}>
            {t('gateway.saveConfig')}
          </Button>
        </DialogFooter>
      </DialogContent>
    </DialogRoot>
  )
}

export default PromptFilterRulesDialog
