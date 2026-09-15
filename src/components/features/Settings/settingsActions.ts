import { invoke } from '@tauri-apps/api/core'

const buildSettingsErrorMessage = (t, err, titleKey = 'settings.saveFailed') => {
  const title = t(titleKey)
  return {
    title,
    message: `${title}: ${err}`}
}

export const persistAppSettings = async ({
  updates,
  notifyChange = false,
  updateAppSettings,
  emitFn,
  showError,
  t}) => {
  try {
    const nextSettings = await updateAppSettings(updates)
    if (!nextSettings) {
      await showError(t('settings.saveFailed'), t('settings.saveFailed'))
      return null
    }
    if (notifyChange) {
      await emitFn('settings-changed')
    }
    await emitFn('app-settings-changed', nextSettings)
    return nextSettings
  } catch (err) {
    const errorInfo = buildSettingsErrorMessage(t, err)
    await showError(errorInfo.title, errorInfo.message)
    return null
  }
}

export const runKiroCommandWithAppSettings = async ({
  command,
  commandArgs,
  appSettingsUpdates,
  notifyChange = false,
  persistSettings,
  showError,
  t}) => {
  try {
    await invoke(command, commandArgs)
    if (appSettingsUpdates) {
      return await persistSettings({
        updates: appSettingsUpdates,
        notifyChange})
    }
    return true
  } catch (err) {
    const errorInfo = buildSettingsErrorMessage(t, err)
    await showError(errorInfo.title, errorInfo.message)
    return null
  }
}

/**
 * 工厂：构造 `setState + 持久化到 appSettings` 的 boolean handler。
 * 用于 Settings tab 中大量「拨开关 → 同步落盘」的场景。
 */
export const makeAppBoolToggle = (
  setter: (v: boolean) => void,
  field: string,
  save: (updates: any, notifyChange?: boolean) => Promise<any>,
  notifyChange = false,
) => async (checked: boolean) => {
  setter(checked)
  await save({ [field]: checked }, notifyChange)
}

/**
 * 工厂：构造 `setState + 调用 Kiro IDE 命令 + 同步 appSettings` 的 boolean handler。
 */
export const makeKiroBoolToggle = (
  setter: (v: boolean) => void,
  runCmd: (command: string, args: any, updates?: any, notifyChange?: boolean) => Promise<any>,
  command: string,
  appField: string | null = null,
  argName: string = 'enabled',
) => async (checked: boolean) => {
  setter(checked)
  const updates = appField ? { [appField]: checked } : null
  await runCmd(command, { [argName]: checked }, updates)
}

