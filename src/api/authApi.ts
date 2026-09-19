// 登录相关 API 调用
import { invoke } from '@tauri-apps/api/core'

// 获取支持的登录提供商
export function getSupportedProviders() {
  return invoke<string[]>('get_supported_providers')
}

// 发起 Kiro 登录（social 传 { provider }，Enterprise 额外传 startUrl/region）
export function kiroLogin<T = unknown>(params: { provider: string; startUrl?: string; region?: string }) {
  return invoke<T>('kiro_login', params)
}

// 取消进行中的登录
export function cancelKiroLogin() {
  return invoke('cancel_kiro_login')
}
