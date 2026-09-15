import type React from 'react'
import { Switch } from '../../ui/switch'
import { rowShell } from './rowStyles'

interface SwitchRowProps {
  checked: boolean
  onCheckedChange: (v: boolean) => void
  icon?: React.ReactNode
  label: string
  hint?: string
  trailing?: React.ReactNode
  title?: string
}

/**
 * 紧凑开关行：左侧 switch + 图标 + 标签，右侧可选附加控件（select / 按钮）。
 * 比 ToggleRow 多支持图标、副标题、尾控件，用于配置项较丰富的场景。
 */
function SwitchRow({
  checked,
  onCheckedChange,
  icon,
  label,
  hint,
  trailing,
  title,
}: SwitchRowProps) {
  return (
    <div
      className={rowShell('default')}
      title={title}
    >
      <Switch checked={checked} onCheckedChange={onCheckedChange} />
      {icon && <span className="text-muted-foreground flex items-center">{icon}</span>}
      <span className="text-sm font-medium text-foreground">{label}</span>
      {hint && <span className="text-xs text-muted-foreground ml-1">{hint}</span>}
      {trailing && <div className="ml-auto flex items-center gap-2">{trailing}</div>}
    </div>
  )
}

export default SwitchRow
