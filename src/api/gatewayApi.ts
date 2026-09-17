// 网关（反代）相关 API 调用
import { invoke } from '@tauri-apps/api/core'

export function getGatewayConfig<T = any>() {
  return invoke<T>('get_gateway_config')
}

export function getGatewayStatus<T = any>() {
  return invoke<T>('get_gateway_status')
}

export function saveGatewayConfig(config: any) {
  return invoke('save_gateway_config', { config })
}

export function startGateway<T = any>(config: any) {
  return invoke<T>('start_gateway', { config })
}

export function stopGateway() {
  return invoke('stop_gateway')
}

export function getGatewayLogDir() {
  return invoke<string>('get_gateway_log_dir')
}

export function openGatewayLogDir() {
  return invoke<string>('open_gateway_log_dir')
}

export function getGatewayRequestLogs<T = any[]>(limit = 120) {
  return invoke<T>('get_gateway_request_logs', { limit })
}

export function clearGatewayRequestLogs() {
  return invoke('clear_gateway_request_logs')
}

export function getGatewayRequestStats<T = any>() {
  return invoke<T>('get_gateway_request_stats')
}

// 按模型 / 端点维度聚合的统计（与 request_stats 同源，均读 LogStore）
export function getGatewayModelStats<T = any[]>() {
  return invoke<T>('get_gateway_model_stats')
}

export function getGatewayEndpointStats<T = any[]>() {
  return invoke<T>('get_gateway_endpoint_stats')
}

export function getCacheStats<T = any>() {
  return invoke<T>('get_cache_stats')
}

export function clearAllCache() {
  return invoke('clear_all_cache')
}

/** 清理过期缓存，返回清理条数 */
export function cleanupExpiredCache() {
  return invoke<number>('cleanup_expired_cache')
}

/** 清除指定会话的缓存 */
export function clearSessionCache(sessionId: string) {
  return invoke('clear_session_cache', { sessionId })
}

/**
 * 网关运行时账号健康度。
 * 键为账号 ID；网关未启动时后端返回错误，调用方应视为「未运行」而非真错误。
 */
export function getAllAccountHealth<T = Record<string, any>>() {
  return invoke<T>('get_all_account_health')
}

/** 重置单个账号的健康状态 */
export function resetAccountHealth(accountId: string) {
  return invoke('reset_account_health', { accountId })
}

/** 清理超过 1 小时未检查的健康状态条目 */
export function cleanupStaleHealth() {
  return invoke('cleanup_stale_health')
}

// 获取可用模型列表
export function getAvailableModels() {
  return invoke<string[]>('get_available_models')
}

/** 按当前 2API 配置模拟负载均衡选号（不发起真实上游请求） */
export function testRouteConfig(config: any) {
  return invoke<{
    matched_accounts: string[]
    selected_account: string | null
    error: string | null
  }>('test_route_config', { config })
}

// 为选中的客户端写入反代配置
export function configureProxyClients(args: {
  clients: string[]
  host: string
  port: number
  apiKey: string
}) {
  return invoke<any[]>('configure_proxy_clients', args)
}
