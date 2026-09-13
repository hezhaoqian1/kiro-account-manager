//! 负载均衡模块：多种策略、健康检查、加权轮询与故障转移。

// 负载均衡模块
// 提供多种负载均衡策略，包括健康检查、加权轮询、故障转移等

use crate::core::account::Account;
use rand::{distributions::WeightedIndex, prelude::*, seq::SliceRandom};
use std::{
    collections::HashMap,
    sync::Arc,
    time::{Duration, Instant},
};
use tokio::sync::RwLock;

mod types;
mod balancer;

pub use types::*;
pub use balancer::*;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_account_health_score() {
        let mut health = AccountHealth::new("test".to_string());

        // 新账号，默认满分
        assert_eq!(health.health_score(), 100);

        // 记录成功
        health.record_success(100);
        assert!(health.health_score() >= 90);

        // 记录失败（有成功记录时不会标记为不健康，但分数会下降）
        health.record_failure();
        health.record_failure();
        health.record_failure();
        // 有 1 次成功 + 3 次失败 = 25% 成功率，仍然 is_healthy（因为有成功记录）
        assert!(health.is_healthy);
        assert!(health.health_score() < 50);

        // 全新账号，纯失败场景
        let mut health2 = AccountHealth::new("test2".to_string());
        health2.record_failure();
        health2.record_failure();
        health2.record_failure();
        assert!(!health2.is_healthy);
        assert_eq!(health2.health_score(), 0);
    }

    #[tokio::test]
    async fn test_load_balancer_round_robin() {
        let lb = LoadBalancer::new(LoadBalancerStrategy::RoundRobin);
        let accounts = vec![
            Account {
                id: "1".to_string(),
                email: Some("test1@example.com".to_string()),
                password: None,
                label: "Account 1".to_string(),
                status: "active".to_string(),
                added_at: "2024-01-01".to_string(),
                access_token: Some("token1".to_string()),
                refresh_token: None,
                expires_at: None,
                provider: None,
                user_id: None,
                auth_method: None,
                client_id: None,
                client_secret: None,
                region: Some("us-east-1".to_string()),
                client_id_hash: None,
                sso_session_id: None,
                id_token: None,
                start_url: None,
                profile_arn: None,
                token_endpoint: None,
                issuer_url: None,
                scopes: None,
                usage_data: None,
                group_id: None,
                tag_links: vec![],
                machine_id: None,
                available_models_cache: None,
                failure_count: 0,
                last_failure_at: None,
                disabled_reason: None,
                success_count: 0,
                enabled: true,
                proxy_config: None,
            },
            Account {
                id: "2".to_string(),
                email: Some("test2@example.com".to_string()),
                password: None,
                label: "Account 2".to_string(),
                status: "active".to_string(),
                added_at: "2024-01-01".to_string(),
                access_token: Some("token2".to_string()),
                refresh_token: None,
                expires_at: None,
                provider: None,
                user_id: None,
                auth_method: None,
                client_id: None,
                client_secret: None,
                region: Some("us-east-1".to_string()),
                client_id_hash: None,
                sso_session_id: None,
                id_token: None,
                start_url: None,
                profile_arn: None,
                token_endpoint: None,
                issuer_url: None,
                scopes: None,
                usage_data: None,
                group_id: None,
                tag_links: vec![],
                machine_id: None,
                available_models_cache: None,
                failure_count: 0,
                last_failure_at: None,
                disabled_reason: None,
                success_count: 0,
                enabled: true,
                proxy_config: None,
            },
        ];

        let acc1 = lb.select_account(&accounts).await.unwrap();
        let acc2 = lb.select_account(&accounts).await.unwrap();
        let acc3 = lb.select_account(&accounts).await.unwrap();

        assert_eq!(acc1.id, "1");
        assert_eq!(acc2.id, "2");
        assert_eq!(acc3.id, "1"); // 轮回到第一个
    }
}
