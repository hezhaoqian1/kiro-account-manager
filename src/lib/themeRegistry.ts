/**
 * 主题注册表 —— 全站唯一事实来源。
 *
 * 背景：此前主题有两份互相矛盾的清单：
 *  - `components/features/Layout/index.tsx` 的 `themeIcons`
 *    （含 `theme1`/`theme2` 两个**幽灵主题**——`index.css` 里没有任何 `[data-theme='theme1']` 规则；
 *      且缺 `system`/`aurora`/`sakura`）
 *  - `Settings/settingsConstants.ts` 的 `buildThemeOptions`
 *    （含 aurora/sakura，与 `index.css` 完全对齐）
 *
 * 后果：点侧栏主题按钮在 aurora/sakura 下 `indexOf` 得 -1 → 跳回 light；反向又能切到
 * 不存在的 theme1/theme2，导致 `<html data-theme>` 匹配不到任何 CSS，主题丢失。
 *
 * 现改为：本文件为唯一来源，`index.css` 的 `[data-theme=...]` 与本表一一对应；
 * Layout（侧栏轮转）与 Settings（外观页列表）都从这里派生。
 */

export type ThemeIconName = 'Monitor' | 'Sun' | 'Moon' | 'Palette'

export interface ThemeOption {
  /** 与 `index.css` 的 `[data-theme='<key>']` 一致 */
  key: string
  /** i18n key */
  nameKey: string
  /** t() 缺失时的兜底文案 */
  fallbackName: string
  iconName: ThemeIconName
  /** Tailwind 渐变类，用于外观页的预览条 */
  color: string
}

export const THEME_REGISTRY: ThemeOption[] = [
  { key: 'system', nameKey: 'settings.themeSystem', fallbackName: 'Follow system', iconName: 'Monitor', color: 'from-slate-400 to-slate-600' },
  { key: 'light', nameKey: 'settings.light', fallbackName: 'Light', iconName: 'Sun', color: 'from-blue-400 to-blue-600' },
  { key: 'dark', nameKey: 'settings.dark', fallbackName: 'Dark', iconName: 'Moon', color: 'from-gray-700 to-gray-900' },
  { key: 'purple', nameKey: 'settings.purple', fallbackName: 'Purple', iconName: 'Palette', color: 'from-purple-500 to-purple-700' },
  { key: 'green', nameKey: 'settings.green', fallbackName: 'Green', iconName: 'Palette', color: 'from-emerald-500 to-emerald-700' },
  { key: 'tech', nameKey: 'settings.tech', fallbackName: 'Tech Blue', iconName: 'Palette', color: 'from-blue-500 to-cyan-500' },
  { key: 'dark-one', nameKey: 'settings.darkOne', fallbackName: 'Dark One', iconName: 'Moon', color: 'from-slate-700 to-gray-900' },
  { key: 'business', nameKey: 'settings.business', fallbackName: 'Business', iconName: 'Palette', color: 'from-amber-500 to-yellow-600' },
  { key: 'sunset', nameKey: 'settings.sunset', fallbackName: 'Sunset', iconName: 'Palette', color: 'from-orange-400 to-red-500' },
  { key: 'ocean', nameKey: 'settings.ocean', fallbackName: 'Ocean', iconName: 'Palette', color: 'from-cyan-400 to-blue-500' },
  { key: 'rose', nameKey: 'settings.rose', fallbackName: 'Rose', iconName: 'Palette', color: 'from-pink-400 to-rose-500' },
  { key: 'aurora', nameKey: 'settings.aurora', fallbackName: 'Aurora', iconName: 'Palette', color: 'from-teal-400 to-emerald-500' },
  { key: 'midnight', nameKey: 'settings.midnight', fallbackName: 'Midnight', iconName: 'Moon', color: 'from-gray-900 via-yellow-700 to-black' },
  { key: 'forest', nameKey: 'settings.forest', fallbackName: 'Forest', iconName: 'Palette', color: 'from-green-600 to-emerald-900' },
  { key: 'sakura', nameKey: 'settings.sakura', fallbackName: 'Sakura', iconName: 'Palette', color: 'from-pink-200 to-rose-400' },
]

/** 主题 key 顺序，用于侧栏轮转等场景 */
export const THEME_KEYS: string[] = THEME_REGISTRY.map(o => o.key)

/** 查主题；找不到返回 undefined（调用方需兜底，避免出现幽灵主题） */
export const findTheme = (key: string | undefined) =>
  THEME_REGISTRY.find(o => o.key === key)
