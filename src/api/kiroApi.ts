// Kiro 本地 token 与机器码 API 调用
import { invoke } from '@tauri-apps/api/core'

// 读取 Kiro IDE 当前登录的本地 token
export function getKiroLocalToken<T = any>() {
  return invoke<T>('get_kiro_local_token')
}

// 生成新的机器码
export function generateMachineGuid() {
  return invoke<string>('generate_machine_guid')
}

// 设置自定义机器码
export function setCustomMachineGuid(newGuid: string) {
  return invoke('set_custom_machine_guid', { newGuid })
}

// 获取系统机器码
export function getSystemMachineGuid<T = any>() {
  return invoke<T>('get_system_machine_guid')
}

// 重置系统机器码，返回新机器码
export function resetSystemMachineGuid() {
  return invoke<string>('reset_system_machine_guid')
}

// 以管理员身份重启应用（机器码重置等需要提权的场景）。
// 后端会先把当前数据目录通过 --data-dir= 传给提权实例，避免以其他管理员账号
// 提权时 %APPDATA% 漂移导致读不到账号数据。
export function restartAsAdmin() {
  return invoke<void>('restart_as_admin')
}
