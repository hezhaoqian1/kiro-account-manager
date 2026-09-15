/**
 * 设置页「配置行」的统一外壳样式。
 *
 * 背景：此前 SwitchRow / ToggleRow 各自维护一套内边距、圆角与底色，SettingsGeneral
 * 的「切换目标」、SettingsKiro 的 appProxyMode、SettingsAppearance 的 reduceMotion
 * 又各手写了一套，导致同一页面并存 5 种行样式（字号 xs/sm、圆角 md/lg、底色 card/muted
 * 都不一致）。这里收敛为两种尺寸变体，所有行统一从此取值。
 *
 * 交互语义不变：是否「整行可点」仍由各组件自行决定（ToggleRow 用 label 包裹故可点，
 * SwitchRow 用 div 且可能含尾控件故不可点），外壳本身不包含 cursor-pointer 等交互样式。
 */
export type RowSize = 'default' | 'compact'

const SHELL: Record<RowSize, string> = {
  /** 整宽行：可含尾控件，行高较舒展 */
  default:
    'flex items-center gap-2 px-3 py-2 rounded-lg border border-border bg-card hover:bg-muted/40 transition-colors',
  /** 网格内密集排布：行高与字号更紧凑 */
  compact:
    'flex items-center gap-2 px-2.5 py-1.5 rounded-md border border-border bg-card hover:bg-muted/40 transition-colors',
}

export const rowShell = (size: RowSize = 'default') => SHELL[size]
