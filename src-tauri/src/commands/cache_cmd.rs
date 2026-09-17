/// 缓存管理命令
///
/// 提供缓存的查询、清理和统计功能
use crate::gateway::response_cache::CacheStats;
use crate::state::AppState;
use tauri::State;

/// 获取缓存统计信息
#[tauri::command]
pub async fn get_cache_stats(state: State<'_, AppState>) -> Result<CacheStats, String> {
    let response_cache = {
        let guard = state
            .gateway
            .lock()
            .map_err(|_| "获取 gateway 状态失败".to_string())?;

        if let Some(runtime) = guard.as_ref() {
            Some(runtime.response_cache.clone())
        } else {
            None
        }
    };

    if let Some(cache_arc) = response_cache {
        let cache = cache_arc.lock().await;
        Ok(cache.stats())
    } else {
        Ok(CacheStats {
            delta_cache_size: 0,
            lru_cache_size: 0,
            persistent_cache_enabled: false,
        })
    }
}

/// 清除所有缓存
#[tauri::command]
pub async fn clear_all_cache(state: State<'_, AppState>) -> Result<(), String> {
    let response_cache = {
        let guard = state
            .gateway
            .lock()
            .map_err(|_| "获取 gateway 状态失败".to_string())?;

        if let Some(runtime) = guard.as_ref() {
            runtime.response_cache.clone()
        } else {
            return Err("Gateway 未运行".to_string());
        }
    };

    let mut cache = response_cache.lock().await;
    cache.clear_all();
    Ok(())
}

/// 清除会话缓存
#[tauri::command]
pub async fn clear_session_cache(
    state: State<'_, AppState>,
    session_id: String,
) -> Result<(), String> {
    let response_cache = {
        let guard = state
            .gateway
            .lock()
            .map_err(|_| "获取 gateway 状态失败".to_string())?;

        if let Some(runtime) = guard.as_ref() {
            runtime.response_cache.clone()
        } else {
            return Err("Gateway 未运行".to_string());
        }
    };

    let mut cache = response_cache.lock().await;
    cache.clear_session(&session_id);
    Ok(())
}

/// 清理过期缓存
#[tauri::command]
pub async fn cleanup_expired_cache(state: State<'_, AppState>) -> Result<usize, String> {
    let response_cache = {
        let guard = state
            .gateway
            .lock()
            .map_err(|_| "获取 gateway 状态失败".to_string())?;

        if let Some(runtime) = guard.as_ref() {
            runtime.response_cache.clone()
        } else {
            return Err("Gateway 未运行".to_string());
        }
    };

    let mut cache = response_cache.lock().await;
    cache
        .cleanup_expired()
        .map_err(|e| format!("清理过期缓存失败: {}", e))
}
