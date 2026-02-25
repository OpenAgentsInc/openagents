use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::codex_threads::CodexThreadStore;
use crate::config::Config;
use crate::domain_store::{DomainStore, RuntimeDriverOverrideRecord};

#[derive(Debug, Clone, Copy, Serialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum RuntimeDriver {
    ControlService,
    RuntimeService,
}

impl RuntimeDriver {
    pub fn as_str(self) -> &'static str {
        match self {
            RuntimeDriver::ControlService => "control_service",
            RuntimeDriver::RuntimeService => "runtime_service",
        }
    }

    pub fn parse(value: &str) -> Option<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "control_service" | "control" => Some(RuntimeDriver::ControlService),
            "runtime_service" | "runtime" => Some(RuntimeDriver::RuntimeService),
            _ => None,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeShadowStatus {
    pub enabled: bool,
    pub sample_rate: f64,
    pub max_capture_bytes: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeRoutingStatus {
    pub default_driver: RuntimeDriver,
    pub forced_driver: Option<RuntimeDriver>,
    pub force_control_service: bool,
    pub overrides_enabled: bool,
    pub canary_user_percent: u8,
    pub canary_autopilot_percent: u8,
    pub shadow: RuntimeShadowStatus,
    pub overrides: Vec<RuntimeDriverOverrideRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeShadowDecision {
    pub enabled: bool,
    pub mirrored: bool,
    pub sample_rate: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub shadow_driver: Option<RuntimeDriver>,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuntimeRoutingDecision {
    pub driver: RuntimeDriver,
    pub reason: String,
    pub default_driver: RuntimeDriver,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub forced_driver: Option<RuntimeDriver>,
    pub force_control_service: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub user_override: Option<RuntimeDriver>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autopilot_override: Option<RuntimeDriver>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autopilot_binding_driver: Option<RuntimeDriver>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub canary_scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub autopilot_id: Option<String>,
    pub shadow: RuntimeShadowDecision,
}

#[derive(Debug, Clone)]
pub struct RuntimeRoutingResolveInput {
    pub user_id: String,
    pub thread_id: String,
    pub autopilot_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct RuntimeRoutingService {
    config: RuntimeRoutingConfig,
}

#[derive(Debug, Clone)]
struct RuntimeRoutingConfig {
    default_driver: RuntimeDriver,
    forced_driver: Option<RuntimeDriver>,
    force_control_service: bool,
    overrides_enabled: bool,
    canary_user_percent: u8,
    canary_autopilot_percent: u8,
    canary_seed: String,
    shadow_enabled: bool,
    shadow_sample_rate: f64,
    shadow_max_capture_bytes: u64,
}

impl RuntimeRoutingService {
    pub fn from_config(config: &Config) -> Self {
        Self {
            config: RuntimeRoutingConfig::from_config(config),
        }
    }

    pub async fn status(&self, domain_store: &DomainStore) -> RuntimeRoutingStatus {
        let overrides = domain_store
            .list_runtime_driver_overrides()
            .await
            .unwrap_or_default();

        RuntimeRoutingStatus {
            default_driver: self.config.default_driver,
            forced_driver: self.config.forced_driver,
            force_control_service: self.config.force_control_service,
            overrides_enabled: self.config.overrides_enabled,
            canary_user_percent: self.config.canary_user_percent,
            canary_autopilot_percent: self.config.canary_autopilot_percent,
            shadow: RuntimeShadowStatus {
                enabled: self.config.shadow_enabled,
                sample_rate: self.config.shadow_sample_rate,
                max_capture_bytes: self.config.shadow_max_capture_bytes,
            },
            overrides,
        }
    }

    pub async fn resolve(
        &self,
        domain_store: &DomainStore,
        thread_store: &CodexThreadStore,
        input: RuntimeRoutingResolveInput,
    ) -> RuntimeRoutingDecision {
        let user_id = input.user_id.trim().to_string();
        let thread_id = input.thread_id.trim().to_string();

        let autopilot_id = match input
            .autopilot_id
            .and_then(|value| normalize_optional_string(Some(value)))
        {
            Some(value) => Some(value),
            None => thread_store.autopilot_id_for_thread(&thread_id).await,
        };

        if self.config.force_control_service {
            return self.decision(
                RuntimeDriver::ControlService,
                "force_control_service",
                autopilot_id,
                None,
                None,
                None,
                None,
                &user_id,
                &thread_id,
            );
        }

        if let Some(forced_driver) = self.config.forced_driver {
            return self.decision(
                forced_driver,
                "force_driver",
                autopilot_id,
                None,
                None,
                None,
                None,
                &user_id,
                &thread_id,
            );
        }

        if self.config.overrides_enabled {
            let user_override = domain_store
                .find_active_runtime_driver_override("user", &user_id)
                .await
                .ok()
                .flatten()
                .and_then(|row| parse_driver(&row.driver));
            if let Some(user_override) = user_override {
                return self.decision(
                    user_override,
                    "user_override",
                    autopilot_id,
                    Some(user_override),
                    None,
                    None,
                    None,
                    &user_id,
                    &thread_id,
                );
            }

            if let Some(ap_id) = autopilot_id.as_deref() {
                let autopilot_override = domain_store
                    .find_active_runtime_driver_override("autopilot", ap_id)
                    .await
                    .ok()
                    .flatten()
                    .and_then(|row| parse_driver(&row.driver));
                if let Some(autopilot_override) = autopilot_override {
                    return self.decision(
                        autopilot_override,
                        "autopilot_override",
                        autopilot_id,
                        None,
                        Some(autopilot_override),
                        None,
                        None,
                        &user_id,
                        &thread_id,
                    );
                }
            }
        }

        if let Some(ap_id) = autopilot_id.as_deref() {
            let binding_driver = domain_store
                .find_primary_autopilot_binding_driver(ap_id)
                .await
                .ok()
                .flatten()
                .and_then(|driver| parse_driver(&driver));
            if let Some(binding_driver) = binding_driver {
                return self.decision(
                    binding_driver,
                    "autopilot_binding",
                    autopilot_id,
                    None,
                    None,
                    Some(binding_driver),
                    None,
                    &user_id,
                    &thread_id,
                );
            }
        }

        if self.config.canary_user_percent > 0
            && in_canary(
                &self.config.canary_seed,
                "user",
                &user_id,
                self.config.canary_user_percent,
            )
        {
            return self.decision(
                RuntimeDriver::RuntimeService,
                "user_canary",
                autopilot_id,
                None,
                None,
                None,
                Some("user".to_string()),
                &user_id,
                &thread_id,
            );
        }

        if let Some(ap_id) = autopilot_id.as_deref()
            && self.config.canary_autopilot_percent > 0
            && in_canary(
                &self.config.canary_seed,
                "autopilot",
                ap_id,
                self.config.canary_autopilot_percent,
            )
        {
            return self.decision(
                RuntimeDriver::RuntimeService,
                "autopilot_canary",
                autopilot_id,
                None,
                None,
                None,
                Some("autopilot".to_string()),
                &user_id,
                &thread_id,
            );
        }

        self.decision(
            self.config.default_driver,
            "default_driver",
            autopilot_id,
            None,
            None,
            None,
            None,
            &user_id,
            &thread_id,
        )
    }

    fn decision(
        &self,
        driver: RuntimeDriver,
        reason: &str,
        autopilot_id: Option<String>,
        user_override: Option<RuntimeDriver>,
        autopilot_override: Option<RuntimeDriver>,
        autopilot_binding_driver: Option<RuntimeDriver>,
        canary_scope: Option<String>,
        user_id: &str,
        thread_id: &str,
    ) -> RuntimeRoutingDecision {
        let mirrored = self.shadow_mirror_enabled(user_id, thread_id, driver);
        RuntimeRoutingDecision {
            driver,
            reason: reason.to_string(),
            default_driver: self.config.default_driver,
            forced_driver: self.config.forced_driver,
            force_control_service: self.config.force_control_service,
            user_override,
            autopilot_override,
            autopilot_binding_driver,
            canary_scope,
            autopilot_id,
            shadow: RuntimeShadowDecision {
                enabled: self.config.shadow_enabled,
                mirrored,
                sample_rate: self.config.shadow_sample_rate,
                shadow_driver: if mirrored {
                    Some(RuntimeDriver::RuntimeService)
                } else {
                    None
                },
            },
        }
    }

    fn shadow_mirror_enabled(
        &self,
        user_id: &str,
        thread_id: &str,
        primary_driver: RuntimeDriver,
    ) -> bool {
        if !self.config.shadow_enabled || primary_driver != RuntimeDriver::ControlService {
            return false;
        }

        let sample_rate = self.config.shadow_sample_rate.clamp(0.0, 1.0);
        if sample_rate <= 0.0 {
            return false;
        }
        if sample_rate >= 1.0 {
            return true;
        }

        let key = format!("{thread_id}|{user_id}");
        let hash = Sha256::digest(key.as_bytes());
        let mut first = [0u8; 4];
        first.copy_from_slice(&hash[..4]);
        let bucket = u32::from_be_bytes(first) % 10_000;
        bucket < (sample_rate * 10_000.0).round() as u32
    }
}

impl RuntimeRoutingConfig {
    fn from_config(config: &Config) -> Self {
        let default_driver =
            parse_driver(&config.runtime_driver).unwrap_or(RuntimeDriver::ControlService);
        let forced_driver = config
            .runtime_force_driver
            .as_deref()
            .and_then(parse_driver);
        let canary_seed = normalize_optional_string(Some(config.runtime_canary_seed.clone()))
            .unwrap_or_else(|| "runtime-canary-v1".to_string());
        Self {
            default_driver,
            forced_driver,
            force_control_service: config.runtime_force_control_service,
            overrides_enabled: config.runtime_overrides_enabled,
            canary_user_percent: config.runtime_canary_user_percent.min(100),
            canary_autopilot_percent: config.runtime_canary_autopilot_percent.min(100),
            canary_seed,
            shadow_enabled: config.runtime_shadow_enabled,
            shadow_sample_rate: config.runtime_shadow_sample_rate,
            shadow_max_capture_bytes: config.runtime_shadow_max_capture_bytes,
        }
    }
}

fn parse_driver(value: &str) -> Option<RuntimeDriver> {
    RuntimeDriver::parse(value)
}

fn normalize_optional_string(value: Option<String>) -> Option<String> {
    value
        .map(|raw| raw.trim().to_string())
        .filter(|raw| !raw.is_empty())
}

fn in_canary(seed: &str, scope: &str, scope_id: &str, percent: u8) -> bool {
    if percent == 0 {
        return false;
    }
    if percent >= 100 {
        return true;
    }

    let hash_input = format!("{seed}|{scope}|{scope_id}");
    let digest = Sha256::digest(hash_input.as_bytes());
    let mut first = [0u8; 4];
    first.copy_from_slice(&digest[..4]);
    (u32::from_be_bytes(first) % 100) < u32::from(percent)
}

#[cfg(test)]
mod tests {
    use super::RuntimeDriver;

    #[test]
    fn runtime_driver_parse_is_rust_label_authoritative() {
        assert_eq!(
            RuntimeDriver::parse("control_service"),
            Some(RuntimeDriver::ControlService)
        );
        assert_eq!(
            RuntimeDriver::parse("runtime_service"),
            Some(RuntimeDriver::RuntimeService)
        );
        assert_eq!(RuntimeDriver::parse("legacy"), None);
        assert_eq!(RuntimeDriver::parse("laravel"), None);
        assert_eq!(RuntimeDriver::parse("openagents.com"), None);
        assert_eq!(RuntimeDriver::parse("elixir"), None);
    }

    #[test]
    fn runtime_driver_as_str_uses_rust_era_labels() {
        assert_eq!(RuntimeDriver::ControlService.as_str(), "control_service");
        assert_eq!(RuntimeDriver::RuntimeService.as_str(), "runtime_service");
    }
}
