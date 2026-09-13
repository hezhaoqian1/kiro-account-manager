// Deep Link 回调处理
// 处理 kiro-account-manager://kiro.kiroAgent/authenticate-success?code=xxx&state=xxx 格式的 OAuth 回调

use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, Mutex};
use std::time::Duration;

const DEEP_LINK_SCHEME: &str = "kiro";
const DEEP_LINK_REDIRECT_URI: &str = "kiro.kiroAgent/authenticate-success";

/// OAuth 回调结果（state 已在 `handle_deep_link` 中验证）
#[derive(Debug, Clone)]
pub struct OAuthCallbackResult {
    pub code: String,
}

/// 回调结果类型别名
type CallbackResult = Result<OAuthCallbackResult, String>;
/// 回调接收器类型别名
type CallbackReceiver = Arc<Mutex<Option<Receiver<CallbackResult>>>>;
/// 待处理发送器类型别名
type PendingSender = Mutex<Option<(String, Sender<CallbackResult>)>>;

/// Deep Link OAuth 回调等待器
pub struct DeepLinkCallbackWaiter {
    result_rx: CallbackReceiver,
    timeout: Duration,
}

impl DeepLinkCallbackWaiter {
    /// 获取 `redirect_uri` (根据环境自动选择协议)
    pub fn get_redirect_uri() -> String {
        format!("{}://{}", DEEP_LINK_SCHEME, DEEP_LINK_REDIRECT_URI)
    }

    /// 获取当前环境的协议名称
    pub fn get_protocol_scheme() -> &'static str {
        DEEP_LINK_SCHEME
    }

    /// 等待回调结果
    pub fn wait_for_callback(&self) -> Result<OAuthCallbackResult, String> {
        // 锁中毒时恢复 guard 而非 panic(M5):这些锁保护的只是一个 Option 槽,
        // 持锁线程 panic 不会破坏其内部不变量,直接取回数据继续即可。否则一次 panic
        // 会永久毒化该锁,让此后所有 login/deep_link 全部 panic 直到重启应用。
        let rx = self
            .result_rx
            .lock()
            .unwrap_or_else(|poisoned| {
                log::warn!("[deep_link] result_rx mutex poisoned, recovering guard");
                poisoned.into_inner()
            })
            .take()
            .ok_or("Callback channel already consumed")?;

        match rx.recv_timeout(self.timeout) {
            Ok(result) => result,
            Err(_) => Err("OAuth callback timeout (5 minutes)".to_string()),
        }
    }
}
/// 全局回调发送器存储
static PENDING_SENDER: std::sync::OnceLock<PendingSender> = std::sync::OnceLock::new();

/// 初始化 deep link 处理器（应用启动时调用）
pub fn init() {
    PENDING_SENDER.get_or_init(|| Mutex::new(None));
}

/// 注册一个新的回调等待器，返回接收端
pub fn register_waiter(state: &str) -> DeepLinkCallbackWaiter {
    let (tx, rx) = mpsc::channel();

    // 存储发送端
    let storage = PENDING_SENDER.get_or_init(|| Mutex::new(None));
    // 锁中毒时恢复 guard 而非 panic(M5),见 wait_for_callback 注释。
    let mut guard = storage.lock().unwrap_or_else(|poisoned| {
        log::warn!("[deep_link] pending sender mutex poisoned, recovering guard");
        poisoned.into_inner()
    });
    if let Some((_state, previous_tx)) = guard.take() {
        let _ = previous_tx.send(Err("登录已取消".to_string()));
    }
    *guard = Some((state.to_string(), tx));

    DeepLinkCallbackWaiter {
        result_rx: Arc::new(Mutex::new(Some(rx))),
        timeout: Duration::from_secs(300),
    }
}
/// 取消当前等待中的 deep link 登录
pub fn cancel_waiter() -> bool {
    let Some(storage) = PENDING_SENDER.get() else {
        return false;
    };

    // 锁中毒时恢复 guard 而非 panic(M5),见 wait_for_callback 注释。
    let mut guard = storage.lock().unwrap_or_else(|poisoned| {
        log::warn!("[deep_link] pending sender mutex poisoned, recovering guard");
        poisoned.into_inner()
    });
    let Some((_state, tx)) = guard.take() else {
        return false;
    };
    let _ = tx.send(Err("登录已取消".to_string()));
    true
}

/// 将 deep link 中的 `/app/callback` 映射到应用内的 `/callback`
pub fn get_app_callback_route(url: &str) -> Option<String> {
    let parsed = url::Url::parse(url).ok()?;

    if parsed.scheme() != DeepLinkCallbackWaiter::get_protocol_scheme() {
        return None;
    }

    if parsed.path() != "/app/callback" {
        return None;
    }

    let mut route = "/callback".to_string();
    if let Some(query) = parsed.query() {
        route.push('?');
        route.push_str(query);
    }

    Some(route)
}
/// 处理 deep link URL（由 main.rs 调用）
/// 返回 (是否处理成功, 是否需要导航到 /callback)
pub fn handle_deep_link(url: &str) -> (bool, bool) {
    log::info!("[deep_link] Received URL: {}", url);

    let Some(storage) = PENDING_SENDER.get() else {
        log::warn!("[deep_link] PENDING_SENDER not initialized");
        return (false, false);
    };

    // 锁中毒时恢复 guard 而非 panic(M5),见 wait_for_callback 注释。
    let mut guard = storage.lock().unwrap_or_else(|poisoned| {
        log::warn!("[deep_link] pending sender mutex poisoned, recovering guard");
        poisoned.into_inner()
    });
    let Some((expected_state, tx)) = guard.take() else {
        log::warn!("[deep_link] No pending login waiter");
        return (false, false);
    };

    log::info!(
        "[deep_link] Processing callback with expected state: {}",
        expected_state
    );

    // 解析 URL
    let parsed = match url::Url::parse(url) {
        Ok(u) => u,
        Err(e) => {
            let _ = tx.send(Err(format!("Invalid URL: {e}")));
            return (false, false);
        }
    };

    // 检查协议是否匹配当前环境
    let expected_scheme = DeepLinkCallbackWaiter::get_protocol_scheme();
    if parsed.scheme() != expected_scheme {
        *guard = Some((expected_state, tx)); // 放回去
        return (false, false);
    };

    // 提取参数
    let params: std::collections::HashMap<_, _> = parsed.query_pairs().collect();

    // 检查错误
    if let Some(error) = params.get("error") {
        let desc = params.get("error_description").map_or_else(
            || "Unknown error".to_string(),
            std::string::ToString::to_string,
        );
        let _ = tx.send(Err(format!("OAuth error: {error} - {desc}")));
        return (true, true); // 错误也需要导航到 /callback 显示错误
    }

    let Some(code) = params.get("code") else {
        let _ = tx.send(Err("Missing code parameter".to_string()));
        return (true, true);
    };
    let code = code.to_string();

    let Some(state) = params.get("state") else {
        let _ = tx.send(Err("Missing state parameter".to_string()));
        return (true, true);
    };
    let state = state.to_string();

    // 验证 state
    if state != expected_state {
        let _ = tx.send(Err("State mismatch - possible CSRF attack".to_string()));
        return (true, true);
    }

    let _ = tx.send(Ok(OAuthCallbackResult { code }));
    (true, true) // 成功处理，需要导航到 /callback
}

#[cfg(test)]
mod tests {
    use super::{handle_deep_link, register_waiter, DeepLinkCallbackWaiter};
    use std::time::Duration;

    /// 进程级全局状态 `PENDING_SENDER` 的串行守卫。
    ///
    /// 只有下面两个测试会写它，但它们互不隔离：`register_waiter` 会「顶掉并取消」
    /// 上一个等待器。并行执行时，`registering_new_waiter_cancels_previous_waiter`
    /// 可能在 `handle_deep_link_keeps_waiter_when_scheme_does_not_match` 注册之后、
    /// 调用 `handle_deep_link` 之前插进来，把它的发送端换成另一个 state，
    /// 于是该测试要么 `wait_for_callback()` 收到「登录已取消」、
    /// 要么因 state 不匹配而失败（只在满载跑全量测试时偶发，单独跑这 3 个测试
    /// 因为窗口太窄几乎复现不出来）。持有该守卫即可让它们串行执行。
    static DEEP_LINK_TEST_MUTEX: std::sync::Mutex<()> = std::sync::Mutex::new(());

    /// 获取全局状态测试守卫；锁中毒时恢复 guard（与生产代码同一套约定）。
    fn lock_global_state() -> std::sync::MutexGuard<'static, ()> {
        DEEP_LINK_TEST_MUTEX
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    #[test]
    fn deep_link_scheme_matches_registered_tauri_scheme() {
        let config: serde_json::Value = serde_json::from_str(include_str!("../../tauri.conf.json"))
            .expect("tauri config should parse");
        let scheme = config["plugins"]["deep-link"]["desktop"]["schemes"][0]
            .as_str()
            .expect("deep-link scheme should exist");

        assert_eq!(DeepLinkCallbackWaiter::get_protocol_scheme(), scheme);
        assert!(
            DeepLinkCallbackWaiter::get_redirect_uri().starts_with(&format!("{scheme}://")),
            "redirect uri should use registered scheme"
        );
        assert!(
            DeepLinkCallbackWaiter::get_redirect_uri().contains("/authenticate-success"),
            "redirect uri should keep callback path for social/idc compatibility"
        );
    }

    #[test]
    fn registering_new_waiter_cancels_previous_waiter() {
        let _guard = lock_global_state();
        let mut first = register_waiter("first-state");
        first.timeout = Duration::from_millis(20);
        let _second = register_waiter("second-state");

        let result = first.wait_for_callback();

        assert!(matches!(result, Err(message) if message == "登录已取消"));
    }

    #[test]
    fn handle_deep_link_keeps_waiter_when_scheme_does_not_match() {
        let _guard = lock_global_state();
        let waiter = register_waiter("expected-state");

        assert!(!handle_deep_link("wrong-scheme://callback?code=ok&state=expected-state").0);

        let handled = handle_deep_link(
            "kiro://kiro.kiroAgent/authenticate-success?code=ok&state=expected-state",
        );
        assert!(handled.0);
        assert!(handled.1);
        assert_eq!(
            waiter
                .wait_for_callback()
                .expect("callback should succeed")
                .code,
            "ok"
        );
    }
}
