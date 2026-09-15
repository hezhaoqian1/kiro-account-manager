import { Sun, Moon, Palette, Check, LayoutList, ZoomIn, Gauge, Monitor } from 'lucide-react'
import { Switch } from '@/components/ui/switch'
import { buildThemeOptions } from './settingsConstants'
import SectionCard from './SectionCard'
import { rowShell } from './rowStyles'

interface SettingsAppearanceProps {
  theme: string
  setTheme: (theme: string) => void
  density: string
  setDensity: (density: string) => void
  uiScale: number
  setUiScale: (scale: number) => void
  reduceMotion: boolean
  setReduceMotion: (enabled: boolean) => void
  t: (key: string) => string
}

function SettingsAppearance({ theme, setTheme, density, setDensity, uiScale, setUiScale, reduceMotion, setReduceMotion, t }: SettingsAppearanceProps) {
  const themeIconMap: Record<string, any> = { Sun, Moon, Palette, Monitor }
  const themeOptions = buildThemeOptions(t)
  const densityOptions = [
    { key: 'compact', name: t('settings.densityCompact') },
    { key: 'comfortable', name: t('settings.densityComfortable') },
    { key: 'spacious', name: t('settings.densitySpacious') },
  ]
  const scaleOptions = [90, 100, 110, 125]

  return (
    <div className="space-y-3">
      <SectionCard
        title={t('settings.theme')}
        accent="violet"
        icon={<Palette size={14} className="text-violet-500" />}
        desc={t('settings.themeDesc')}
      >
        <div className="grid grid-cols-2 sm:grid-cols-3 lg:grid-cols-4 xl:grid-cols-5 gap-2.5">
          {themeOptions.map((opt: any) => {
            const Icon = themeIconMap[opt.iconName]
            const isActive = theme === opt.key
            return (
              <button
                key={opt.key}
                onClick={() => setTheme(opt.key)}
                aria-pressed={isActive}
                className={`group relative overflow-hidden rounded-xl border transition-all duration-200 cursor-pointer focus:outline-none focus:ring-2 focus:ring-primary/30 ${
                  isActive
                    ? 'border-primary ring-1 ring-primary/30 shadow-md'
                    : 'border-border hover:border-primary/50 hover:shadow-sm'
                }`}
              >
                {/* 主题色预览条 */}
                <div className={`h-10 bg-gradient-to-br ${opt.color} flex items-center justify-center`}>
                  <Icon size={16} className="text-white drop-shadow" />
                </div>
                {/* 名称 */}
                <div className="px-2.5 py-1.5 bg-card flex items-center justify-between">
                  <span className="text-xs font-medium text-foreground truncate">{opt.name}</span>
                  {isActive && (
                    <div className="w-4 h-4 rounded-full flex items-center justify-center bg-primary flex-shrink-0">
                      <Check size={10} className="text-white" />
                    </div>
                  )}
                </div>
              </button>
            )
          })}
        </div>
      </SectionCard>

      <SectionCard
        title={t('settings.density')}
        accent="blue"
        icon={<LayoutList size={14} className="text-blue-500" />}
        desc={t('settings.densityDesc')}
      >
        <div className="grid grid-cols-3 gap-2.5">
          {densityOptions.map((opt) => {
            const isActive = density === opt.key
            return (
              <button
                key={opt.key}
                onClick={() => setDensity(opt.key)}
                aria-pressed={isActive}
                className={`group relative overflow-hidden rounded-xl border transition-all duration-200 cursor-pointer focus:outline-none focus:ring-2 focus:ring-primary/30 ${
                  isActive
                    ? 'border-primary ring-1 ring-primary/30 shadow-md'
                    : 'border-border hover:border-primary/50 hover:shadow-sm'
                }`}
              >
                <div className="px-3 py-2.5 bg-card flex items-center justify-between">
                  <span className="text-xs font-medium text-foreground truncate">{opt.name}</span>
                  {isActive && (
                    <div className="w-4 h-4 rounded-full flex items-center justify-center bg-primary flex-shrink-0">
                      <Check size={10} className="text-white" />
                    </div>
                  )}
                </div>
              </button>
            )
          })}
        </div>
      </SectionCard>

      <SectionCard
        title={t('settings.uiScale')}
        accent="green"
        icon={<ZoomIn size={14} className="text-green-500" />}
        desc={t('settings.uiScaleDesc')}
      >
        <div className="grid grid-cols-2 sm:grid-cols-4 gap-2.5">
          {scaleOptions.map((scale) => {
            const isActive = uiScale === scale
            return (
              <button
                key={scale}
                onClick={() => setUiScale(scale)}
                aria-pressed={isActive}
                className={`group relative overflow-hidden rounded-xl border transition-all duration-200 cursor-pointer focus:outline-none focus:ring-2 focus:ring-primary/30 ${
                  isActive
                    ? 'border-primary ring-1 ring-primary/30 shadow-md'
                    : 'border-border hover:border-primary/50 hover:shadow-sm'
                }`}
              >
                <div className="px-3 py-2.5 bg-card flex items-center justify-center">
                  <span className="text-xs font-medium text-foreground">{scale}%</span>
                </div>
              </button>
            )
          })}
        </div>
      </SectionCard>

      <SectionCard
        title={t('settings.reduceMotion')}
        accent="amber"
        icon={<Gauge size={14} className="text-amber-500" />}
        desc={t('settings.reduceMotionDesc')}
      >
        <div className={`${rowShell('default')} justify-between`}>
          <span className="text-sm">{t('settings.reduceMotionLabel')}</span>
          <Switch checked={!!reduceMotion} onCheckedChange={(checked: boolean) => setReduceMotion(checked)} />
        </div>
      </SectionCard>
    </div>
  )
}

export default SettingsAppearance
