export const AI_MODELS = [
  { value: 'claude-opus-4.8', label: 'Claude Opus 4.8 (1M) - 2.2x', recommended: false },
  { value: 'claude-opus-4.7', label: 'Claude Opus 4.7 (1M) - 2.2x', recommended: false },
  { value: 'claude-opus-4.6', label: 'Claude Opus 4.6 (1M) - 2.2x', recommended: false },
  { value: 'claude-opus-4.5', label: 'Claude Opus 4.5 (200K) - 2.2x', recommended: false },
  { value: 'claude-sonnet-5', label: 'Claude Sonnet 5 (1M) - 1.3x', recommended: true },
  { value: 'claude-sonnet-4.6', label: 'Claude Sonnet 4.6 (1M) - 1.3x', recommended: false },
  { value: 'claude-sonnet-4.5', label: 'Claude Sonnet 4.5 (200K) - 1.3x', recommended: false },
  { value: 'claude-sonnet-4', label: 'Claude Sonnet 4.0 (200K) - 1.3x', recommended: false },
  { value: 'auto', label: 'Auto (智能选择) - 1.0x', recommended: false },
  { value: 'gpt-5.6-sol', label: 'GPT-5.6 Sol (272K) - 2.4x', recommended: false },
  { value: 'gpt-5.6-terra', label: 'GPT-5.6 Terra (272K) - 1.2x', recommended: false },
  { value: 'gpt-5.6-luna', label: 'GPT-5.6 Luna (272K) - 0.6x', recommended: false },
  { value: 'glm-5', label: 'GLM-5 (200K) - 0.5x', recommended: false },
  { value: 'claude-haiku-4.5', label: 'Claude Haiku 4.5 (200K) - 0.4x', recommended: false },
  { value: 'deepseek-3.2', label: 'DeepSeek 3.2 (128K) - 0.25x', recommended: false },
  { value: 'minimax-m2.5', label: 'MiniMax M2.5 (200K) - 0.25x', recommended: false },
  { value: 'minimax-m2.1', label: 'MiniMax M2.1 (200K) - 0.15x', recommended: false },
  { value: 'qwen3-coder-next', label: 'Qwen3 Coder Next (256K) - 0.05x', recommended: false },
]

// Kiro IDE settings.json 通知键 -> app-settings.json 字段名
export const NOTIFICATION_SETTINGS_FIELD_MAP = {
  'kiroAgent.notifications.agent.actionRequired': 'notifyActionRequired',
  'kiroAgent.notifications.agent.failure': 'notifyFailure',
  'kiroAgent.notifications.agent.success': 'notifySuccess',
  'kiroAgent.notifications.billing': 'notifyBilling'
}

// ---- 通知 / 遥测开关（同属 Kiro IDE settings.json，已合并进 Kiro 设置页）----

export interface NotificationState {
  notifyActionRequired: boolean
  notifyFailure: boolean
  notifySuccess: boolean
  notifyBilling: boolean
}

export interface TelemetryState {
  telemetryContentCollection: boolean
  telemetryUsageAnalytics: boolean
  telemetryEditStats: boolean
  telemetryFeedback: boolean
  telemetryPromptLogging: boolean
  telemetryEditStatsDetails: boolean
  telemetryEditStatsDecorations: boolean
  telemetryEditStatsStatusBar: boolean
}

// 默认值对齐 Kiro 1.0：notify.failure / notify.success 默认 false
export const DEFAULT_NOTIFICATIONS: NotificationState = {
  notifyActionRequired: true,
  notifyFailure: false,
  notifySuccess: false,
  notifyBilling: true,
}

export const DEFAULT_TELEMETRY: TelemetryState = {
  telemetryContentCollection: false,
  telemetryUsageAnalytics: false,
  telemetryEditStats: false,
  telemetryFeedback: false,
  telemetryPromptLogging: false,
  telemetryEditStatsDetails: false,
  telemetryEditStatsDecorations: false,
  telemetryEditStatsStatusBar: false,
}

// key = Kiro IDE settings.json 中的通知键；field = 本地 state 字段
export const NOTIFICATION_ROWS: { key: string; label: string; field: keyof NotificationState }[] = [
  { key: 'kiroAgent.notifications.agent.actionRequired', label: 'settings.notifyActionRequired', field: 'notifyActionRequired' },
  { key: 'kiroAgent.notifications.agent.failure', label: 'settings.notifyFailure', field: 'notifyFailure' },
  { key: 'kiroAgent.notifications.agent.success', label: 'settings.notifySuccess', field: 'notifySuccess' },
  { key: 'kiroAgent.notifications.billing', label: 'settings.notifyBilling', field: 'notifyBilling' },
]

// ideKey = Kiro IDE settings.json 中的遥测键；field = 本地 state 字段（与 app-settings 同名，可直接映射）
export const TELEMETRY_ROWS: { ideKey: string; label: string; field: keyof TelemetryState }[] = [
  { ideKey: 'telemetry.dataSharingAndPromptLogging.contentCollectionForServiceImprovement', label: 'settings.telemetryContentCollection', field: 'telemetryContentCollection' },
  { ideKey: 'telemetry.dataSharingAndPromptLogging.usageAnalyticsAndPerformanceMetrics', label: 'settings.telemetryUsageAnalytics', field: 'telemetryUsageAnalytics' },
  { ideKey: 'telemetry.editStats.enabled', label: 'settings.telemetryEditStats', field: 'telemetryEditStats' },
  { ideKey: 'telemetry.feedback.enabled', label: 'settings.telemetryFeedback', field: 'telemetryFeedback' },
  { ideKey: 'telemetry.dataSharingAndPromptLogging.promptLogging', label: 'settings.telemetryPromptLogging', field: 'telemetryPromptLogging' },
  { ideKey: 'telemetry.editStats.details.enabled', label: 'settings.telemetryEditStatsDetails', field: 'telemetryEditStatsDetails' },
  { ideKey: 'telemetry.editStats.showDecorations', label: 'settings.telemetryEditStatsDecorations', field: 'telemetryEditStatsDecorations' },
  { ideKey: 'telemetry.editStats.showStatusBar', label: 'settings.telemetryEditStatsStatusBar', field: 'telemetryEditStatsStatusBar' },
]

export const buildThemeOptions = (t) => [
  // 跟随系统：由 next-themes 按 prefers-color-scheme 解析为 light / dark
  { key: 'system', name: t('settings.themeSystem') || 'Follow system', iconName: 'Monitor', color: 'from-slate-400 to-slate-600' },
  { key: 'light', name: t('settings.light') || 'Light', iconName: 'Sun', color: 'from-blue-400 to-blue-600' },
  { key: 'dark', name: t('settings.dark') || 'Dark', iconName: 'Moon', color: 'from-gray-700 to-gray-900' },
  { key: 'purple', name: t('settings.purple') || 'Purple', iconName: 'Palette', color: 'from-purple-500 to-purple-700' },
  { key: 'green', name: t('settings.green') || 'Green', iconName: 'Palette', color: 'from-emerald-500 to-emerald-700' },
  { key: 'tech', name: t('settings.tech') || 'Tech Blue', iconName: 'Palette', color: 'from-blue-500 to-cyan-500' },
  { key: 'dark-one', name: t('settings.darkOne') || 'Dark One', iconName: 'Moon', color: 'from-slate-700 to-gray-900' },
  { key: 'business', name: t('settings.business') || 'Business', iconName: 'Palette', color: 'from-amber-500 to-yellow-600' },
  { key: 'sunset', name: t('settings.sunset') || 'Sunset', iconName: 'Palette', color: 'from-orange-400 to-red-500' },
  { key: 'ocean', name: t('settings.ocean') || 'Ocean', iconName: 'Palette', color: 'from-cyan-400 to-blue-500' },
  { key: 'rose', name: t('settings.rose') || 'Rose', iconName: 'Palette', color: 'from-pink-400 to-rose-500' },
  { key: 'aurora', name: t('settings.aurora') || 'Aurora', iconName: 'Palette', color: 'from-teal-400 to-emerald-500' },
  { key: 'midnight', name: t('settings.midnight') || 'Midnight', iconName: 'Moon', color: 'from-gray-900 via-yellow-700 to-black' },
  { key: 'forest', name: t('settings.forest') || 'Forest', iconName: 'Palette', color: 'from-green-600 to-emerald-900' },
  { key: 'sakura', name: t('settings.sakura') || 'Sakura', iconName: 'Palette', color: 'from-pink-200 to-rose-400' },
]
