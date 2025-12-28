//! Marketplace Integration
//!
//! Integrates with the OpenAgents marketplace for skill licensing, usage tracking,
//! and billing. Uses NIP-SA skill events (kinds 39220, 39221) for license verification.

use crate::hooks::{Hook, HookResult, ToolCall, ToolOutput};
use async_trait::async_trait;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::sync::{Arc, RwLock};
use std::time::{SystemTime, UNIX_EPOCH};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillLicenseInfo {
    pub skill_id: String,
    pub skill_name: String,
    pub version: String,
    pub agent_pubkey: String,
    pub capabilities: Vec<String>,
    pub granted_at: u64,
    pub expires_at: Option<u64>,
    pub price_sats: u64,
}

impl SkillLicenseInfo {
    pub fn is_expired(&self) -> bool {
        if let Some(expires) = self.expires_at {
            let now = SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .map(|d| d.as_secs())
                .unwrap_or(0);
            now >= expires
        } else {
            false
        }
    }

    pub fn has_capability(&self, capability: &str) -> bool {
        self.capabilities.iter().any(|c| c == capability)
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SkillUsageRecord {
    pub skill_id: String,
    pub agent_pubkey: String,
    pub timestamp: u64,
    pub tokens_used: u64,
    pub cost_sats: u64,
}

pub trait LicenseStore: Send + Sync {
    fn get_license(&self, skill_id: &str, agent_pubkey: &str) -> Option<SkillLicenseInfo>;
    fn store_license(&self, license: SkillLicenseInfo) -> Result<(), String>;
    fn list_licenses(&self, agent_pubkey: &str) -> Vec<SkillLicenseInfo>;
    fn revoke_license(&self, skill_id: &str, agent_pubkey: &str) -> Result<bool, String>;
}

pub trait UsageTracker: Send + Sync {
    fn record_usage(&self, record: SkillUsageRecord) -> Result<(), String>;
    fn get_usage(&self, skill_id: &str, agent_pubkey: &str) -> Vec<SkillUsageRecord>;
    fn total_cost(&self, agent_pubkey: &str) -> u64;
}

#[derive(Default)]
pub struct InMemoryLicenseStore {
    licenses: RwLock<HashMap<(String, String), SkillLicenseInfo>>,
}

impl InMemoryLicenseStore {
    pub fn new() -> Self {
        Self::default()
    }
}

impl LicenseStore for InMemoryLicenseStore {
    fn get_license(&self, skill_id: &str, agent_pubkey: &str) -> Option<SkillLicenseInfo> {
        let guard = self.licenses.read().ok()?;
        guard
            .get(&(skill_id.to_string(), agent_pubkey.to_string()))
            .cloned()
    }

    fn store_license(&self, license: SkillLicenseInfo) -> Result<(), String> {
        let mut guard = self.licenses.write().map_err(|e| e.to_string())?;
        guard.insert(
            (license.skill_id.clone(), license.agent_pubkey.clone()),
            license,
        );
        Ok(())
    }

    fn list_licenses(&self, agent_pubkey: &str) -> Vec<SkillLicenseInfo> {
        let guard = match self.licenses.read() {
            Ok(g) => g,
            Err(_) => return vec![],
        };
        guard
            .values()
            .filter(|l| l.agent_pubkey == agent_pubkey)
            .cloned()
            .collect()
    }

    fn revoke_license(&self, skill_id: &str, agent_pubkey: &str) -> Result<bool, String> {
        let mut guard = self.licenses.write().map_err(|e| e.to_string())?;
        Ok(guard
            .remove(&(skill_id.to_string(), agent_pubkey.to_string()))
            .is_some())
    }
}

#[derive(Default)]
pub struct InMemoryUsageTracker {
    records: RwLock<Vec<SkillUsageRecord>>,
}

impl InMemoryUsageTracker {
    pub fn new() -> Self {
        Self::default()
    }
}

impl UsageTracker for InMemoryUsageTracker {
    fn record_usage(&self, record: SkillUsageRecord) -> Result<(), String> {
        let mut guard = self.records.write().map_err(|e| e.to_string())?;
        guard.push(record);
        Ok(())
    }

    fn get_usage(&self, skill_id: &str, agent_pubkey: &str) -> Vec<SkillUsageRecord> {
        let guard = match self.records.read() {
            Ok(g) => g,
            Err(_) => return vec![],
        };
        guard
            .iter()
            .filter(|r| r.skill_id == skill_id && r.agent_pubkey == agent_pubkey)
            .cloned()
            .collect()
    }

    fn total_cost(&self, agent_pubkey: &str) -> u64 {
        let guard = match self.records.read() {
            Ok(g) => g,
            Err(_) => return 0,
        };
        guard
            .iter()
            .filter(|r| r.agent_pubkey == agent_pubkey)
            .map(|r| r.cost_sats)
            .sum()
    }
}

pub struct SkillLicenseHook {
    store: Arc<dyn LicenseStore>,
    agent_pubkey: String,
    skill_tool_mapping: HashMap<String, String>,
}

impl SkillLicenseHook {
    pub fn new(store: Arc<dyn LicenseStore>, agent_pubkey: String) -> Self {
        Self {
            store,
            agent_pubkey,
            skill_tool_mapping: HashMap::new(),
        }
    }

    pub fn map_tool_to_skill(mut self, tool_name: &str, skill_id: &str) -> Self {
        self.skill_tool_mapping
            .insert(tool_name.to_string(), skill_id.to_string());
        self
    }

    fn get_skill_for_tool(&self, tool_name: &str) -> Option<&String> {
        self.skill_tool_mapping.get(tool_name)
    }
}

#[async_trait]
impl Hook for SkillLicenseHook {
    fn name(&self) -> &str {
        "skill-license"
    }

    async fn before_tool(&self, call: &mut ToolCall) -> HookResult {
        if let Some(skill_id) = self.get_skill_for_tool(&call.name) {
            match self.store.get_license(skill_id, &self.agent_pubkey) {
                Some(license) => {
                    if license.is_expired() {
                        return HookResult::Block {
                            message: format!(
                                "License for skill '{}' has expired",
                                license.skill_name
                            ),
                        };
                    }
                }
                None => {
                    return HookResult::Block {
                        message: format!(
                            "No license found for skill '{}'. Purchase required.",
                            skill_id
                        ),
                    };
                }
            }
        }
        HookResult::Continue
    }
}

pub struct SkillUsageHook {
    tracker: Arc<dyn UsageTracker>,
    agent_pubkey: String,
    skill_tool_mapping: HashMap<String, String>,
    pricing: HashMap<String, SkillPricing>,
}

#[derive(Debug, Clone)]
pub struct SkillPricing {
    pub per_call_sats: Option<u64>,
    pub per_1k_input_sats: Option<u64>,
    pub per_1k_output_sats: Option<u64>,
}

impl Default for SkillPricing {
    fn default() -> Self {
        Self {
            per_call_sats: Some(10),
            per_1k_input_sats: None,
            per_1k_output_sats: None,
        }
    }
}

impl SkillPricing {
    pub fn per_call(sats: u64) -> Self {
        Self {
            per_call_sats: Some(sats),
            per_1k_input_sats: None,
            per_1k_output_sats: None,
        }
    }

    pub fn per_token(input_per_1k: u64, output_per_1k: u64) -> Self {
        Self {
            per_call_sats: None,
            per_1k_input_sats: Some(input_per_1k),
            per_1k_output_sats: Some(output_per_1k),
        }
    }

    pub fn calculate_cost(&self, input_tokens: u64, output_tokens: u64) -> u64 {
        let mut cost = self.per_call_sats.unwrap_or(0);

        if let Some(rate) = self.per_1k_input_sats {
            cost += (input_tokens * rate) / 1000;
        }
        if let Some(rate) = self.per_1k_output_sats {
            cost += (output_tokens * rate) / 1000;
        }

        cost
    }
}

impl SkillUsageHook {
    pub fn new(tracker: Arc<dyn UsageTracker>, agent_pubkey: String) -> Self {
        Self {
            tracker,
            agent_pubkey,
            skill_tool_mapping: HashMap::new(),
            pricing: HashMap::new(),
        }
    }

    pub fn map_tool_to_skill(mut self, tool_name: &str, skill_id: &str) -> Self {
        self.skill_tool_mapping
            .insert(tool_name.to_string(), skill_id.to_string());
        self
    }

    pub fn set_pricing(mut self, skill_id: &str, pricing: SkillPricing) -> Self {
        self.pricing.insert(skill_id.to_string(), pricing);
        self
    }
}

#[async_trait]
impl Hook for SkillUsageHook {
    fn name(&self) -> &str {
        "skill-usage"
    }

    async fn after_tool(&self, call: &ToolCall, output: &mut ToolOutput) -> HookResult {
        if let Some(skill_id) = self.skill_tool_mapping.get(&call.name) {
            let pricing = self.pricing.get(skill_id).cloned().unwrap_or_default();

            let tokens_used = output.content.len() as u64;
            let cost = pricing.calculate_cost(0, tokens_used);

            let record = SkillUsageRecord {
                skill_id: skill_id.clone(),
                agent_pubkey: self.agent_pubkey.clone(),
                timestamp: SystemTime::now()
                    .duration_since(UNIX_EPOCH)
                    .map(|d| d.as_secs())
                    .unwrap_or(0),
                tokens_used,
                cost_sats: cost,
            };

            if let Err(e) = self.tracker.record_usage(record) {
                tracing::warn!("Failed to record skill usage: {}", e);
            }
        }
        HookResult::Continue
    }
}

pub struct MarketplaceIntegration {
    license_store: Arc<dyn LicenseStore>,
    usage_tracker: Arc<dyn UsageTracker>,
    agent_pubkey: String,
}

impl MarketplaceIntegration {
    pub fn new(
        license_store: Arc<dyn LicenseStore>,
        usage_tracker: Arc<dyn UsageTracker>,
        agent_pubkey: String,
    ) -> Self {
        Self {
            license_store,
            usage_tracker,
            agent_pubkey,
        }
    }

    pub fn with_defaults(agent_pubkey: String) -> Self {
        Self {
            license_store: Arc::new(InMemoryLicenseStore::new()),
            usage_tracker: Arc::new(InMemoryUsageTracker::new()),
            agent_pubkey,
        }
    }

    pub fn add_license(&self, license: SkillLicenseInfo) -> Result<(), String> {
        self.license_store.store_license(license)
    }

    pub fn check_license(&self, skill_id: &str) -> Option<SkillLicenseInfo> {
        self.license_store.get_license(skill_id, &self.agent_pubkey)
    }

    pub fn list_licenses(&self) -> Vec<SkillLicenseInfo> {
        self.license_store.list_licenses(&self.agent_pubkey)
    }

    pub fn total_spend(&self) -> u64 {
        self.usage_tracker.total_cost(&self.agent_pubkey)
    }

    pub fn create_license_hook(&self, tool_skill_map: HashMap<String, String>) -> SkillLicenseHook {
        let mut hook = SkillLicenseHook::new(self.license_store.clone(), self.agent_pubkey.clone());
        for (tool, skill) in tool_skill_map {
            hook = hook.map_tool_to_skill(&tool, &skill);
        }
        hook
    }

    pub fn create_usage_hook(
        &self,
        tool_skill_map: HashMap<String, String>,
        pricing: HashMap<String, SkillPricing>,
    ) -> SkillUsageHook {
        let mut hook = SkillUsageHook::new(self.usage_tracker.clone(), self.agent_pubkey.clone());
        for (tool, skill) in tool_skill_map {
            hook = hook.map_tool_to_skill(&tool, &skill);
        }
        for (skill, price) in pricing {
            hook = hook.set_pricing(&skill, price);
        }
        hook
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn create_test_license(skill_id: &str, expires: Option<u64>) -> SkillLicenseInfo {
        SkillLicenseInfo {
            skill_id: skill_id.to_string(),
            skill_name: format!("{}-skill", skill_id),
            version: "1.0.0".to_string(),
            agent_pubkey: "test-agent".to_string(),
            capabilities: vec!["execute".to_string()],
            granted_at: 1700000000,
            expires_at: expires,
            price_sats: 1000,
        }
    }

    #[test]
    fn test_license_not_expired() {
        let license = create_test_license("skill-1", None);
        assert!(!license.is_expired());
    }

    #[test]
    fn test_license_expired() {
        let license = create_test_license("skill-1", Some(1));
        assert!(license.is_expired());
    }

    #[test]
    fn test_license_has_capability() {
        let license = create_test_license("skill-1", None);
        assert!(license.has_capability("execute"));
        assert!(!license.has_capability("admin"));
    }

    #[test]
    fn test_in_memory_license_store() {
        let store = InMemoryLicenseStore::new();
        let license = create_test_license("skill-1", None);

        store.store_license(license.clone()).unwrap();

        let retrieved = store.get_license("skill-1", "test-agent");
        assert!(retrieved.is_some());
        assert_eq!(retrieved.unwrap().skill_id, "skill-1");
    }

    #[test]
    fn test_license_store_list() {
        let store = InMemoryLicenseStore::new();
        store
            .store_license(create_test_license("skill-1", None))
            .unwrap();
        store
            .store_license(create_test_license("skill-2", None))
            .unwrap();

        let licenses = store.list_licenses("test-agent");
        assert_eq!(licenses.len(), 2);
    }

    #[test]
    fn test_license_store_revoke() {
        let store = InMemoryLicenseStore::new();
        store
            .store_license(create_test_license("skill-1", None))
            .unwrap();

        let revoked = store.revoke_license("skill-1", "test-agent").unwrap();
        assert!(revoked);

        let retrieved = store.get_license("skill-1", "test-agent");
        assert!(retrieved.is_none());
    }

    #[test]
    fn test_usage_tracker() {
        let tracker = InMemoryUsageTracker::new();

        let record = SkillUsageRecord {
            skill_id: "skill-1".to_string(),
            agent_pubkey: "test-agent".to_string(),
            timestamp: 1700000000,
            tokens_used: 1000,
            cost_sats: 50,
        };

        tracker.record_usage(record).unwrap();

        let usage = tracker.get_usage("skill-1", "test-agent");
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].tokens_used, 1000);
    }

    #[test]
    fn test_usage_tracker_total_cost() {
        let tracker = InMemoryUsageTracker::new();

        for i in 0..3 {
            let record = SkillUsageRecord {
                skill_id: format!("skill-{}", i),
                agent_pubkey: "test-agent".to_string(),
                timestamp: 1700000000 + i as u64,
                tokens_used: 100,
                cost_sats: 10,
            };
            tracker.record_usage(record).unwrap();
        }

        assert_eq!(tracker.total_cost("test-agent"), 30);
    }

    #[test]
    fn test_skill_pricing_per_call() {
        let pricing = SkillPricing::per_call(100);
        assert_eq!(pricing.calculate_cost(0, 0), 100);
        assert_eq!(pricing.calculate_cost(1000, 500), 100);
    }

    #[test]
    fn test_skill_pricing_per_token() {
        let pricing = SkillPricing::per_token(10, 20);
        let cost = pricing.calculate_cost(2000, 1000);
        assert_eq!(cost, 20 + 20);
    }

    #[test]
    fn test_marketplace_integration() {
        let integration = MarketplaceIntegration::with_defaults("test-agent".to_string());

        let license = create_test_license("skill-1", None);
        integration.add_license(license).unwrap();

        let retrieved = integration.check_license("skill-1");
        assert!(retrieved.is_some());

        let all = integration.list_licenses();
        assert_eq!(all.len(), 1);
    }

    #[tokio::test]
    async fn test_skill_license_hook_allows_licensed() {
        let store = Arc::new(InMemoryLicenseStore::new());
        store
            .store_license(create_test_license("skill-1", None))
            .unwrap();

        let hook = SkillLicenseHook::new(store, "test-agent".to_string())
            .map_tool_to_skill("premium_tool", "skill-1");

        let mut call = ToolCall {
            name: "premium_tool".to_string(),
            parameters: HashMap::new(),
            session_id: "session-1".to_string(),
        };

        let result = hook.before_tool(&mut call).await;
        assert!(matches!(result, HookResult::Continue));
    }

    #[tokio::test]
    async fn test_skill_license_hook_blocks_unlicensed() {
        let store = Arc::new(InMemoryLicenseStore::new());

        let hook = SkillLicenseHook::new(store, "test-agent".to_string())
            .map_tool_to_skill("premium_tool", "skill-1");

        let mut call = ToolCall {
            name: "premium_tool".to_string(),
            parameters: HashMap::new(),
            session_id: "session-1".to_string(),
        };

        let result = hook.before_tool(&mut call).await;
        assert!(matches!(result, HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_skill_license_hook_blocks_expired() {
        let store = Arc::new(InMemoryLicenseStore::new());
        store
            .store_license(create_test_license("skill-1", Some(1)))
            .unwrap();

        let hook = SkillLicenseHook::new(store, "test-agent".to_string())
            .map_tool_to_skill("premium_tool", "skill-1");

        let mut call = ToolCall {
            name: "premium_tool".to_string(),
            parameters: HashMap::new(),
            session_id: "session-1".to_string(),
        };

        let result = hook.before_tool(&mut call).await;
        assert!(matches!(result, HookResult::Block { .. }));
    }

    #[tokio::test]
    async fn test_skill_usage_hook_tracks() {
        let tracker = Arc::new(InMemoryUsageTracker::new());

        let hook = SkillUsageHook::new(tracker.clone(), "test-agent".to_string())
            .map_tool_to_skill("premium_tool", "skill-1")
            .set_pricing("skill-1", SkillPricing::per_call(50));

        let call = ToolCall {
            name: "premium_tool".to_string(),
            parameters: HashMap::new(),
            session_id: "session-1".to_string(),
        };

        let mut output = ToolOutput {
            content: "result data".to_string(),
            is_error: false,
        };

        hook.after_tool(&call, &mut output).await;

        let usage = tracker.get_usage("skill-1", "test-agent");
        assert_eq!(usage.len(), 1);
        assert_eq!(usage[0].cost_sats, 50);
    }

    #[tokio::test]
    async fn test_skill_license_hook_ignores_unmapped_tools() {
        let store = Arc::new(InMemoryLicenseStore::new());

        let hook = SkillLicenseHook::new(store, "test-agent".to_string())
            .map_tool_to_skill("premium_tool", "skill-1");

        let mut call = ToolCall {
            name: "free_tool".to_string(),
            parameters: HashMap::new(),
            session_id: "session-1".to_string(),
        };

        let result = hook.before_tool(&mut call).await;
        assert!(matches!(result, HookResult::Continue));
    }
}
