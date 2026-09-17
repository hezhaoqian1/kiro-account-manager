import { useEffect } from 'react'
import { fetchGatewayStatus } from './gatewayPageState'
import { formatGatewayTimestamp } from './gatewayPageUtils'

interface UseGatewayPollingOptions {
  fallbackConfig: any
  onStatus: (data: { status: any; fallbackConfig: any; syncedAt: string }) => void
  statusInterval?: number
}

/**
 * 网关状态轮询。
 *
 * 曾有一段「请求日志轮询」分支，由 `activeTab === 'observability'` 门控，但唯一调用点
 * 写死 `activeTab: 'config'` 且未传 `onRequestLogs`，该分支永不执行 —— 已删除。
 * 请求日志现在由 `RequestLogsDialog` 在自身打开期间轮询。
 */
export function useGatewayPolling({
  fallbackConfig,
  onStatus,
  statusInterval = 2000
}: UseGatewayPollingOptions) {
  // 状态轮询
  useEffect(() => {
    let timer: NodeJS.Timeout | null = null
    let isActive = true

    const poll = () => {
      if (!isActive || document.hidden) {
        return
      }

      fetchGatewayStatus()
        .then((status) => {
          if (isActive) {
            onStatus({
              status,
              fallbackConfig,
              syncedAt: formatGatewayTimestamp()
            })
          }
        })
        .catch((error) => {
          console.error('[Gateway] Failed to fetch status:', error)
        })
    }

    // 立即执行一次
    poll()

    // 设置定时轮询
    timer = setInterval(poll, statusInterval)

    // 监听页面可见性变化
    const handleVisibilityChange = () => {
      if (document.hidden) {
        // 页面隐藏时清除定时器
        if (timer) {
          clearInterval(timer)
          timer = null
        }
      } else {
        // 页面可见时重新启动轮询
        if (!timer && isActive) {
          poll()
          timer = setInterval(poll, statusInterval)
        }
      }
    }

    document.addEventListener('visibilitychange', handleVisibilityChange)

    return () => {
      isActive = false
      if (timer) {
        clearInterval(timer)
      }
      document.removeEventListener('visibilitychange', handleVisibilityChange)
    }
  }, [fallbackConfig, onStatus, statusInterval])
}
