import { createContext, useContext, useState, useEffect, ReactNode } from 'react'
import { getAppSettings, saveAppSettings } from '../api/settingsApi'
import { listen, UnlistenFn } from '@tauri-apps/api/event'

export interface AppSettings {
  lockModel: boolean;
  lockedModel: string | null;
  autoRefresh: boolean;
  autoRefreshInterval: number;
  browserPath: string;
  privacyMode: boolean;
  autoSwitchEnabled: boolean;
  autoSwitchThreshold: number;
  autoSwitchInterval: number;
  switchTarget: 'ide' | 'cli' | 'both';
  enableCodebaseIndexing: boolean;
  enableTabAutocomplete: boolean;
  usageSummary: boolean;
  enableDebugLogs: boolean;
  notifyActionRequired: boolean;
  notifyFailure: boolean;
  notifySuccess: boolean;
  notifyBilling: boolean;
  referenceTracker: boolean;
  configureMcp: 'Enabled' | 'Disabled' | string;
  telemetryContentCollection: boolean;
  telemetryUsageAnalytics: boolean;
  telemetryEditStats: boolean;
  telemetryFeedback: boolean;
  telemetryPromptLogging: boolean;
  telemetryEditStatsDetails: boolean;
  telemetryEditStatsDecorations: boolean;
  telemetryEditStatsStatusBar: boolean;
  appProxyMode: 'followKiro' | 'disabled' | string;
  density: 'compact' | 'comfortable' | 'spacious' | string;
  uiScale: number;
  reduceMotion: boolean;
}

interface AppSettingsContextValue {
  settings: AppSettings | null;
  loading: boolean;
  updateSettings: (updates: Partial<AppSettings>) => Promise<AppSettings | null>;
  reload: () => Promise<void>;
}

const AppSettingsContext = createContext<AppSettingsContextValue | null>(null)

// 默认设置
const DEFAULT_SETTINGS: AppSettings = {
  lockModel: false,
  lockedModel: null,
  autoRefresh: true,
  autoRefreshInterval: 50,
  browserPath: '',
  privacyMode: true,
  autoSwitchEnabled: false,
  autoSwitchThreshold: 1,
  autoSwitchInterval: 5,
  switchTarget: 'ide',
  // 默认值对齐 Kiro 1.0：enableCodebaseIndexing / enableTabAutocomplete / notify.failure / notify.success 默认 false
  enableCodebaseIndexing: false,
  enableTabAutocomplete: false,
  usageSummary: true,
  enableDebugLogs: false,
  notifyActionRequired: true,
  notifyFailure: false,
  notifySuccess: false,
  notifyBilling: true,
  referenceTracker: false,
  configureMcp: 'Enabled',
  telemetryContentCollection: false,
  telemetryUsageAnalytics: false,
  telemetryEditStats: false,
  telemetryFeedback: false,
  telemetryPromptLogging: false,
  telemetryEditStatsDetails: false,
  telemetryEditStatsDecorations: false,
  telemetryEditStatsStatusBar: false,
  appProxyMode: 'followKiro',
  density: 'comfortable',
  uiScale: 100,
  reduceMotion: false
}

export function AppSettingsProvider({ children }: { children: ReactNode }) {
  const [settings, setSettings] = useState<AppSettings | null>(null)
  const [loading, setLoading] = useState(true)

  // 加载设置
  const loadSettings = async () => {
    try {
      const appSettings = await getAppSettings<AppSettings>()
      setSettings(appSettings || DEFAULT_SETTINGS)
    } catch (err) {
      console.error('[AppSettings] 加载失败:', err)
      setSettings(DEFAULT_SETTINGS)
    } finally {
      setLoading(false)
    }
  }

  // 更新设置
  const updateSettings = async (updates: Partial<AppSettings>) => {
    try {
      await saveAppSettings(updates)
      let nextSettings: AppSettings | null = null
      setSettings(prev => {
        nextSettings = { ...(prev || DEFAULT_SETTINGS), ...updates }
        return nextSettings
      })
      return nextSettings
    } catch (err) {
      console.error('[AppSettings] 保存失败:', err)
      return null
    }
  }

  useEffect(() => {
    loadSettings()

    let unlisten: UnlistenFn | null = null

    const setupListener = async () => {
      unlisten = await listen<AppSettings | null>('app-settings-changed', (event) => {
        if (event.payload) {
          setSettings(event.payload)
        } else {
          loadSettings()
        }
      })
    }

    setupListener()

    return () => {
      if (unlisten) unlisten()
    }
  }, [])

  // 外观：写到 <html> 上，由 CSS 驱动（密度 / 缩放 / 动效）
  useEffect(() => {
    const root = document.documentElement
    root.dataset.density = settings?.density || 'comfortable'
    root.style.setProperty('--ui-scale', String((settings?.uiScale || 100) / 100))
    root.dataset.reduceMotion = settings?.reduceMotion ? 'true' : 'false'
  }, [settings?.density, settings?.uiScale, settings?.reduceMotion])

  return (
    <AppSettingsContext.Provider value={{ settings, loading, updateSettings, reload: loadSettings }}>
      {children}
    </AppSettingsContext.Provider>
  )
}

export function useAppSettings() {
  const context = useContext(AppSettingsContext)
  if (context === null) {
    throw new Error('useAppSettings must be used within AppSettingsProvider')
  }
  return context
}
