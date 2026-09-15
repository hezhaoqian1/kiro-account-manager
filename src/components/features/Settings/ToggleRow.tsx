import { Switch } from '../../ui/switch'
import { rowShell } from './rowStyles'

interface ToggleRowProps {
  checked: boolean
  onChange: (v: boolean) => Promise<void> | void
  label: string
}

/**
 * 紧凑型开关行，用于 Settings 各 tab 的批量布尔配置（Agent / 通知 / 遥测）。
 */
function ToggleRow({ checked, onChange, label }: ToggleRowProps) {
  return (
    <label className={`${rowShell('compact')} cursor-pointer`}>
      <Switch checked={checked} onCheckedChange={onChange} />
      <span className="text-xs text-foreground">{label}</span>
    </label>
  )
}

export default ToggleRow
