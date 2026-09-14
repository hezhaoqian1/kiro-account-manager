import { useEffect, useState } from 'react'
import { Bot, ShieldCheck, FlaskConical } from 'lucide-react'
import { Input } from '../../ui/input'
import { Textarea } from '../../ui/textarea'
import { Select, SelectContent, SelectItem, SelectTrigger, SelectValue } from '../../ui/select'
import { Label } from '../../ui/label'
import SectionCard from './SectionCard'
import ToggleRow from './ToggleRow'
import { useDialog } from '../../../contexts/DialogContext'
import { getKiroSettings, setKiroAgentSetting } from '../../../api/settingsApi'

// Kiro 1.0 Agent 高级设置面板。
//
// 这些键与 IDE settings.json **双向同步**（IDE 优先 + 回写 app-settings.json），由后端
// `get_kiro_settings_inner` / `sync_to_app_settings` 统一处理，本面板只负责读写。
// 覆盖 Kiro 1.0 configuration 里、旧版设置页未暴露的键：
//   toolCardDisplayMode / terminalCommandTimeout / agentIgnoreFiles / artifacts.autoOpenPanel
//   / trust.defaultPattern / trust.defaultScope / autoApproveAgentCommands
//   / experiments.cloudConfig / experiments.workspaceManager / editorActions.prompts
// 默认值均对齐 Kiro 的 configuration 声明（抠自 IDE 产物）。
//
// 本组件由 SettingsKiro 渲染，位于 kiroAgent.* 分组内。mcpApprovedEnvVars 归到
// 「配置 MCP」卡片、kiro.startupMode 归到「Agent 行为」卡片，均不在此面板。
//
// 注意：遥测类开关（telemetry.*）不在这里——它们与「遥测与隐私」卡片里的同类键一起走
// app-settings 同步，见 SettingsKiro。

interface AdvancedState {
  toolCardDisplayMode: string
  terminalCommandTimeout: string // 字符串承接输入框，空串 = 未设置（写 null 删除键）
  agentIgnoreFiles: string // 每行一个
  artifactsAutoOpenPanel: boolean
  trustDefaultPattern: string
  trustDefaultScope: string
  autoApproveAgentCommands: string // 每行一个
  experimentsCloudConfig: boolean
  experimentsWorkspaceManager: boolean
  editorActionsPrompts: string // JSON 文本
}

const DEFAULTS: AdvancedState = {
  toolCardDisplayMode: 'collapseOnComplete',
  terminalCommandTimeout: '',
  agentIgnoreFiles: '',
  artifactsAutoOpenPanel: true,
  trustDefaultPattern: 'base',
  trustDefaultScope: 'workspace',
  autoApproveAgentCommands: '',
  experimentsCloudConfig: false,
  experimentsWorkspaceManager: false,
  editorActionsPrompts: '',
}

const lines = (v: string) => v.split('\n').map(s => s.trim()).filter(Boolean)

const TOOL_CARD_MODES = ['collapseOnComplete', 'alwaysExpanded'] as const
const TRUST_PATTERNS = ['full', 'partial', 'base'] as const
const TRUST_SCOPES = ['user', 'workspace', 'session'] as const

export default function KiroAgentAdvancedPanel({ t }: { t: (key: string) => string }) {
  const { showError } = useDialog()
  const [s, setS] = useState<AdvancedState>(DEFAULTS)

  useEffect(() => {
    let cancelled = false
    getKiroSettings<any>()
      .then(v => {
        if (cancelled || !v) return
        setS({
          toolCardDisplayMode: v.toolCardDisplayMode ?? 'collapseOnComplete',
          terminalCommandTimeout: v.terminalCommandTimeout == null ? '' : String(v.terminalCommandTimeout),
          agentIgnoreFiles: (v.agentIgnoreFiles ?? []).join('\n'),
          artifactsAutoOpenPanel: v.artifactsAutoOpenPanel ?? true,
          trustDefaultPattern: v.trustDefaultPattern ?? 'base',
          trustDefaultScope: v.trustDefaultScope ?? 'workspace',
          autoApproveAgentCommands: (v.autoApproveAgentCommands ?? []).join('\n'),
          experimentsCloudConfig: v.experimentsCloudConfig ?? false,
          experimentsWorkspaceManager: v.experimentsWorkspaceManager ?? false,
          editorActionsPrompts: v.editorActionsPrompts ? JSON.stringify(v.editorActionsPrompts, null, 2) : '',
        })
      })
      .catch(() => {})
    return () => {
      cancelled = true
    }
  }, [])

  const patch = (p: Partial<AdvancedState>) => setS(prev => ({ ...prev, ...p }))

  const apply = async (key: string, value: unknown) => {
    try {
      await setKiroAgentSetting(key, value)
    } catch (err) {
      await showError(t('settings.saveFailed'), `${t('settings.saveFailed')}: ${err}`)
    }
  }

  const applyTimeout = async () => {
    const raw = s.terminalCommandTimeout.trim()
    if (raw === '') {
      await apply('kiroAgent.terminalCommandTimeout', null)
      return
    }
    const val = Number(raw)
    if (!Number.isFinite(val) || val < 0) {
      await showError(t('settings.saveFailed'), t('settings.agentNumberInvalid'))
      return
    }
    await apply('kiroAgent.terminalCommandTimeout', Math.floor(val))
  }

  const applyEditorPrompts = async () => {
    const raw = s.editorActionsPrompts.trim()
    if (raw === '') {
      await apply('kiroAgent.editorActions.prompts', {})
      return
    }
    try {
      const parsed = JSON.parse(raw)
      if (typeof parsed !== 'object' || parsed === null || Array.isArray(parsed)) {
        throw new Error('expect a JSON object')
      }
      await apply('kiroAgent.editorActions.prompts', parsed)
    } catch (err) {
      await showError(t('settings.saveFailed'), `${t('settings.agentJsonInvalid')}: ${err}`)
    }
  }

  return (
    <div className="space-y-3">
      {/* === Agent 行为（扩展）=== */}
      <SectionCard
        title={t('settings.agentAdvanced')}
        accent="blue"
        icon={<Bot size={14} className="text-blue-500" />}
        desc={t('settings.agentAdvancedDesc')}
      >
        <div className="space-y-3">
          <div>
            <Label className="block text-[11px] text-muted-foreground mb-1">
              {t('settings.agentToolCardDisplayMode')}
            </Label>
            <Select
              value={s.toolCardDisplayMode}
              onValueChange={v => {
                patch({ toolCardDisplayMode: v })
                void apply('kiroAgent.toolCardDisplayMode', v)
              }}
            >
              <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
              <SelectContent>
                {TOOL_CARD_MODES.map(m => (
                  <SelectItem key={m} value={m}>{t(`settings.agentToolCard_${m}`)}</SelectItem>
                ))}
              </SelectContent>
            </Select>
            <p className="text-[11px] text-muted-foreground mt-1">{t('settings.agentToolCardDisplayModeDesc')}</p>
          </div>

          <div>
            <Label className="block text-[11px] text-muted-foreground mb-1">
              {t('settings.agentTerminalCommandTimeout')}
            </Label>
            <Input
              type="number"
              min={0}
              value={s.terminalCommandTimeout}
              onChange={e => patch({ terminalCommandTimeout: e.target.value })}
              onBlur={applyTimeout}
              placeholder={t('settings.agentTerminalCommandTimeoutPlaceholder')}
              className="h-8 text-xs"
            />
            <p className="text-[11px] text-muted-foreground mt-1">{t('settings.agentTerminalCommandTimeoutDesc')}</p>
          </div>

          <ToggleRow
            checked={s.artifactsAutoOpenPanel}
            onChange={v => {
              patch({ artifactsAutoOpenPanel: v })
              void apply('kiroAgent.artifacts.autoOpenPanel', v)
            }}
            label={t('settings.agentArtifactsAutoOpen')}
          />

          <div>
            <Label className="block text-[11px] text-muted-foreground mb-1">
              {t('settings.agentIgnoreFiles')}
            </Label>
            <Textarea
              value={s.agentIgnoreFiles}
              onChange={e => patch({ agentIgnoreFiles: e.target.value })}
              onBlur={() => apply('kiroAgent.agentIgnoreFiles', lines(s.agentIgnoreFiles))}
              placeholder=".gitignore"
              className="font-mono text-xs"
              rows={2}
            />
            <p className="text-[11px] text-muted-foreground mt-1">{t('settings.agentIgnoreFilesDesc')}</p>
          </div>
        </div>
      </SectionCard>

      {/* === 实验与高级 === */}
      <SectionCard
        title={t('settings.agentExperiments')}
        accent="amber"
        icon={<FlaskConical size={14} className="text-amber-500" />}
        desc={t('settings.agentExperimentsDesc')}
      >
        <div className="space-y-3">
          <ToggleRow
            checked={s.experimentsCloudConfig}
            onChange={v => {
              patch({ experimentsCloudConfig: v })
              void apply('kiroAgent.experiments.cloudConfig', v)
            }}
            label={t('settings.agentExperimentCloudConfig')}
          />
          <ToggleRow
            checked={s.experimentsWorkspaceManager}
            onChange={v => {
              patch({ experimentsWorkspaceManager: v })
              void apply('kiroAgent.experiments.workspaceManager', v)
            }}
            label={t('settings.agentExperimentWorkspaceManager')}
          />

          <div>
            <Label className="block text-[11px] text-muted-foreground mb-1">
              {t('settings.agentEditorActionsPrompts')}
            </Label>
            <Textarea
              value={s.editorActionsPrompts}
              onChange={e => patch({ editorActionsPrompts: e.target.value })}
              onBlur={applyEditorPrompts}
              placeholder={'{\n  "explain": "..."\n}'}
              className="font-mono text-xs"
              rows={3}
            />
            <p className="text-[11px] text-muted-foreground mt-1">{t('settings.agentEditorActionsPromptsDesc')}</p>
          </div>
        </div>
      </SectionCard>

      {/* === 信任与自动批准 ===
          放在本面板最后：紧随其后的「权限规则」(PermissionsPanel) 属于同一主题
          （agent 能否不经询问就动手），两张卡相邻更符合心智模型。 */}
      <SectionCard
        title={t('settings.agentTrust')}
        accent="red"
        icon={<ShieldCheck size={14} className="text-red-500" />}
        desc={t('settings.agentTrustDesc')}
      >
        <div className="space-y-3">
          <div className="grid grid-cols-1 sm:grid-cols-2 gap-2">
            <div>
              <Label className="block text-[11px] text-muted-foreground mb-1">
                {t('settings.agentTrustDefaultPattern')}
              </Label>
              <Select
                value={s.trustDefaultPattern}
                onValueChange={v => {
                  patch({ trustDefaultPattern: v })
                  void apply('kiroAgent.trust.defaultPattern', v)
                }}
              >
                <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {TRUST_PATTERNS.map(p => (
                    <SelectItem key={p} value={p}>{t(`settings.agentTrustPattern_${p}`)}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
            <div>
              <Label className="block text-[11px] text-muted-foreground mb-1">
                {t('settings.agentTrustDefaultScope')}
              </Label>
              <Select
                value={s.trustDefaultScope}
                onValueChange={v => {
                  patch({ trustDefaultScope: v })
                  void apply('kiroAgent.trust.defaultScope', v)
                }}
              >
                <SelectTrigger className="h-8 text-xs"><SelectValue /></SelectTrigger>
                <SelectContent>
                  {TRUST_SCOPES.map(sc => (
                    <SelectItem key={sc} value={sc}>{t(`settings.agentTrustScope_${sc}`)}</SelectItem>
                  ))}
                </SelectContent>
              </Select>
            </div>
          </div>

          <div>
            <Label className="block text-[11px] text-muted-foreground mb-1">
              {t('settings.agentAutoApproveCommands')}
            </Label>
            <Textarea
              value={s.autoApproveAgentCommands}
              onChange={e => patch({ autoApproveAgentCommands: e.target.value })}
              onBlur={() => apply('kiroAgent.autoApproveAgentCommands', lines(s.autoApproveAgentCommands))}
              placeholder="git *"
              className="font-mono text-xs"
              rows={2}
            />
            <p className="text-[11px] text-muted-foreground mt-1">{t('settings.agentAutoApproveCommandsDesc')}</p>
          </div>
        </div>
      </SectionCard>
    </div>
  )
}
