//! Narrow shared provider substrate for reusable provider runtime semantics.
//!
//! This crate intentionally owns only product-agnostic provider domain logic:
//! backend health, launch product derivation, inventory controls/models, and
//! provider lifecycle derivation. App-specific UX, execution snapshots, and
//! orchestration stay in app crates.

mod admin;
mod sandbox;
mod sandbox_execution;

pub use admin::*;
pub use sandbox::*;
pub use sandbox_execution::*;

use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBackendKind {
    #[serde(alias = "ollama")]
    GptOss,
    AppleFoundationModels,
}

impl ProviderBackendKind {
    pub const fn label(self) -> &'static str {
        match self {
            Self::GptOss => "local GPT-OSS runtime",
            Self::AppleFoundationModels => "Apple Foundation Models bridge",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderMode {
    Offline,
    Connecting,
    Online,
    Degraded,
}

impl ProviderMode {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Connecting => "connecting",
            Self::Online => "online",
            Self::Degraded => "degraded",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderIngressMode {
    Offline,
    Preview,
    Connecting,
    Online,
    Degraded,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderBlocker {
    IdentityMissing,
    WalletError,
    SkillTrustUnavailable,
    CreditLaneUnavailable,
    GptOssUnavailable,
    GptOssModelUnavailable,
    AppleFoundationModelsUnavailable,
    AppleFoundationModelsModelUnavailable,
}

impl ProviderBlocker {
    pub const fn code(self) -> &'static str {
        match self {
            Self::IdentityMissing => "IDENTITY_MISSING",
            Self::WalletError => "WALLET_ERROR",
            Self::SkillTrustUnavailable => "SKL_TRUST_UNAVAILABLE",
            Self::CreditLaneUnavailable => "AC_CREDIT_UNAVAILABLE",
            Self::GptOssUnavailable => "GPT_OSS_UNAVAILABLE",
            Self::GptOssModelUnavailable => "GPT_OSS_MODEL_UNAVAILABLE",
            Self::AppleFoundationModelsUnavailable => "APPLE_FM_UNAVAILABLE",
            Self::AppleFoundationModelsModelUnavailable => "APPLE_FM_MODEL_UNAVAILABLE",
        }
    }

    pub const fn detail(self) -> &'static str {
        match self {
            Self::IdentityMissing => "Nostr identity is not ready",
            Self::WalletError => "Spark wallet reports an error",
            Self::SkillTrustUnavailable => "SKL trust gate is not trusted",
            Self::CreditLaneUnavailable => "AC credit lane is not available",
            Self::GptOssUnavailable => "Local GPT-OSS backend is unavailable",
            Self::GptOssModelUnavailable => "No local GPT-OSS serving model is ready",
            Self::AppleFoundationModelsUnavailable => {
                "Apple Foundation Models backend is unavailable"
            }
            Self::AppleFoundationModelsModelUnavailable => {
                "Apple Foundation Models is not ready to serve inference"
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderFailureClass {
    Relay,
    Execution,
    Payment,
    Reconciliation,
}

impl ProviderFailureClass {
    pub const fn label(self) -> &'static str {
        match self {
            Self::Relay => "relay",
            Self::Execution => "execution",
            Self::Payment => "payment",
            Self::Reconciliation => "reconciliation",
        }
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderBackendHealth {
    pub reachable: bool,
    pub ready: bool,
    pub configured_model: Option<String>,
    pub ready_model: Option<String>,
    pub available_models: Vec<String>,
    pub last_error: Option<String>,
    pub last_action: Option<String>,
    pub availability_message: Option<String>,
    pub latency_ms_p50: Option<u64>,
}

impl ProviderBackendHealth {
    pub const fn is_ready(&self) -> bool {
        self.ready
    }
}

#[derive(Clone, Debug, Default, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAvailability {
    #[serde(alias = "ollama")]
    pub gpt_oss: ProviderBackendHealth,
    pub apple_foundation_models: ProviderBackendHealth,
    pub sandbox: ProviderSandboxAvailability,
}

impl ProviderAvailability {
    pub fn active_inference_backend(&self) -> Option<ProviderBackendKind> {
        if self.apple_foundation_models.is_ready() {
            Some(ProviderBackendKind::AppleFoundationModels)
        } else if self.gpt_oss.is_ready() {
            Some(ProviderBackendKind::GptOss)
        } else {
            None
        }
    }

    pub fn execution_backend_label(&self) -> &'static str {
        self.active_inference_backend()
            .map(ProviderBackendKind::label)
            .unwrap_or("no active inference backend")
    }

    pub fn product_visible(&self, product: ProviderComputeProduct) -> bool {
        match product {
            ProviderComputeProduct::GptOssInference
            | ProviderComputeProduct::AppleFoundationModelsInference => true,
            ProviderComputeProduct::GptOssEmbeddings => false,
            ProviderComputeProduct::SandboxContainerExec => self
                .sandbox
                .has_declared_execution_class(ProviderSandboxExecutionClass::ContainerExec),
            ProviderComputeProduct::SandboxPythonExec => self
                .sandbox
                .has_declared_execution_class(ProviderSandboxExecutionClass::PythonExec),
            ProviderComputeProduct::SandboxNodeExec => self
                .sandbox
                .has_declared_execution_class(ProviderSandboxExecutionClass::NodeExec),
            ProviderComputeProduct::SandboxPosixExec => self
                .sandbox
                .has_declared_execution_class(ProviderSandboxExecutionClass::PosixExec),
        }
    }

    pub fn product_backend_ready(&self, product: ProviderComputeProduct) -> bool {
        match product {
            ProviderComputeProduct::GptOssInference => self.gpt_oss.is_ready(),
            ProviderComputeProduct::GptOssEmbeddings => false,
            ProviderComputeProduct::AppleFoundationModelsInference => {
                self.apple_foundation_models.is_ready()
            }
            ProviderComputeProduct::SandboxContainerExec => self
                .sandbox
                .backend_ready_for_class(ProviderSandboxExecutionClass::ContainerExec),
            ProviderComputeProduct::SandboxPythonExec => self
                .sandbox
                .backend_ready_for_class(ProviderSandboxExecutionClass::PythonExec),
            ProviderComputeProduct::SandboxNodeExec => self
                .sandbox
                .backend_ready_for_class(ProviderSandboxExecutionClass::NodeExec),
            ProviderComputeProduct::SandboxPosixExec => self
                .sandbox
                .backend_ready_for_class(ProviderSandboxExecutionClass::PosixExec),
        }
    }

    pub fn capability_summary_for_product(&self, product: ProviderComputeProduct) -> String {
        match product {
            ProviderComputeProduct::SandboxContainerExec => self
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::ContainerExec)
                .unwrap_or_else(|| product.capability_summary_base().to_string()),
            ProviderComputeProduct::SandboxPythonExec => self
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::PythonExec)
                .unwrap_or_else(|| product.capability_summary_base().to_string()),
            ProviderComputeProduct::SandboxNodeExec => self
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::NodeExec)
                .unwrap_or_else(|| product.capability_summary_base().to_string()),
            ProviderComputeProduct::SandboxPosixExec => self
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::PosixExec)
                .unwrap_or_else(|| product.capability_summary_base().to_string()),
            ProviderComputeProduct::GptOssInference
            | ProviderComputeProduct::GptOssEmbeddings
            | ProviderComputeProduct::AppleFoundationModelsInference => {
                product.capability_summary(self)
            }
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderComputeProduct {
    GptOssInference,
    GptOssEmbeddings,
    AppleFoundationModelsInference,
    SandboxContainerExec,
    SandboxPythonExec,
    SandboxNodeExec,
    SandboxPosixExec,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderProductDescriptor {
    pub product_id: String,
    pub compute_family: String,
    pub backend_family: String,
    pub sandbox_execution_class: Option<String>,
}

impl ProviderComputeProduct {
    pub const fn all() -> [Self; 6] {
        [
            Self::GptOssInference,
            Self::AppleFoundationModelsInference,
            Self::SandboxContainerExec,
            Self::SandboxPythonExec,
            Self::SandboxNodeExec,
            Self::SandboxPosixExec,
        ]
    }

    pub const fn product_id(self) -> &'static str {
        match self {
            Self::GptOssInference => "gpt_oss.text_generation",
            Self::GptOssEmbeddings => "gpt_oss.embeddings",
            Self::AppleFoundationModelsInference => "apple_foundation_models.text_generation",
            Self::SandboxContainerExec => "sandbox.container.exec",
            Self::SandboxPythonExec => "sandbox.python.exec",
            Self::SandboxNodeExec => "sandbox.node.exec",
            Self::SandboxPosixExec => "sandbox.posix.exec",
        }
    }

    pub const fn display_label(self) -> &'static str {
        match self {
            Self::GptOssInference => "GPT-OSS inference",
            Self::GptOssEmbeddings => "GPT-OSS embeddings",
            Self::AppleFoundationModelsInference => "Apple FM inference",
            Self::SandboxContainerExec => "Sandbox container exec",
            Self::SandboxPythonExec => "Sandbox python exec",
            Self::SandboxNodeExec => "Sandbox node exec",
            Self::SandboxPosixExec => "Sandbox posix exec",
        }
    }

    pub const fn backend_kind(self) -> Option<ProviderBackendKind> {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => Some(ProviderBackendKind::GptOss),
            Self::AppleFoundationModelsInference => {
                Some(ProviderBackendKind::AppleFoundationModels)
            }
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => None,
        }
    }

    pub const fn backend_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => "gpt_oss",
            Self::AppleFoundationModelsInference => "apple_foundation_models",
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "sandbox",
        }
    }

    pub const fn compute_family_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::AppleFoundationModelsInference => "inference",
            Self::GptOssEmbeddings => "embeddings",
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "sandbox_execution",
        }
    }

    pub const fn sandbox_execution_class(self) -> Option<ProviderSandboxExecutionClass> {
        match self {
            Self::SandboxContainerExec => Some(ProviderSandboxExecutionClass::ContainerExec),
            Self::SandboxPythonExec => Some(ProviderSandboxExecutionClass::PythonExec),
            Self::SandboxNodeExec => Some(ProviderSandboxExecutionClass::NodeExec),
            Self::SandboxPosixExec => Some(ProviderSandboxExecutionClass::PosixExec),
            Self::GptOssInference
            | Self::GptOssEmbeddings
            | Self::AppleFoundationModelsInference => None,
        }
    }

    pub const fn capability_summary_base(self) -> &'static str {
        match self {
            Self::GptOssInference => "backend=gpt_oss execution=local_inference family=inference",
            Self::GptOssEmbeddings => {
                "backend=gpt_oss execution=local_inference family=embeddings status=unsupported"
            }
            Self::AppleFoundationModelsInference => {
                "backend=apple_foundation_models execution=local_inference family=inference apple_silicon=true apple_intelligence=true"
            }
            Self::SandboxContainerExec => {
                "backend=sandbox execution=sandbox.container.exec family=sandbox_execution"
            }
            Self::SandboxPythonExec => {
                "backend=sandbox execution=sandbox.python.exec family=sandbox_execution"
            }
            Self::SandboxNodeExec => {
                "backend=sandbox execution=sandbox.node.exec family=sandbox_execution"
            }
            Self::SandboxPosixExec => {
                "backend=sandbox execution=sandbox.posix.exec family=sandbox_execution"
            }
        }
    }

    pub fn capability_summary(self, availability: &ProviderAvailability) -> String {
        let base_summary = self.capability_summary_base();
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => {
                let health = &availability.gpt_oss;
                let ready_model = health.ready_model.as_deref().unwrap_or("none");
                let configured_model = health.configured_model.as_deref().unwrap_or("none");
                let latency_ms = health
                    .latency_ms_p50
                    .map(|value| value.to_string())
                    .unwrap_or_else(|| "n/a".to_string());
                format!(
                    "{base_summary} model={ready_model} configured_model={configured_model} latency_ms_p50={latency_ms}"
                )
            }
            Self::AppleFoundationModelsInference => {
                let health = &availability.apple_foundation_models;
                let ready_model = health.ready_model.as_deref().unwrap_or("none");
                let availability_message =
                    health.availability_message.as_deref().unwrap_or("ready");
                format!("{base_summary} model={ready_model} platform_gate={availability_message}")
            }
            Self::SandboxContainerExec => availability
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::ContainerExec)
                .unwrap_or_else(|| base_summary.to_string()),
            Self::SandboxPythonExec => availability
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::PythonExec)
                .unwrap_or_else(|| base_summary.to_string()),
            Self::SandboxNodeExec => availability
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::NodeExec)
                .unwrap_or_else(|| base_summary.to_string()),
            Self::SandboxPosixExec => availability
                .sandbox
                .capability_summary_for_class(ProviderSandboxExecutionClass::PosixExec)
                .unwrap_or_else(|| base_summary.to_string()),
        }
    }

    pub const fn terms_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => "spot session / local best effort",
            Self::AppleFoundationModelsInference => "spot session / Apple gated best effort",
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "spot session / declared sandbox profile",
        }
    }

    pub const fn forward_terms_label(self) -> &'static str {
        match self {
            Self::GptOssInference | Self::GptOssEmbeddings => {
                "forward physical / committed local window"
            }
            Self::AppleFoundationModelsInference => {
                "forward physical / Apple gated committed window"
            }
            Self::SandboxContainerExec
            | Self::SandboxPythonExec
            | Self::SandboxNodeExec
            | Self::SandboxPosixExec => "forward physical / declared sandbox profile window",
        }
    }

    pub const fn default_price_floor_sats(self) -> u64 {
        match self {
            Self::GptOssInference => 21,
            Self::GptOssEmbeddings => 8,
            Self::AppleFoundationModelsInference => 34,
            Self::SandboxContainerExec => 55,
            Self::SandboxPythonExec => 34,
            Self::SandboxNodeExec => 34,
            Self::SandboxPosixExec => 21,
        }
    }

    pub fn for_product_id(product_id: &str) -> Option<Self> {
        match product_id.trim() {
            "ollama.text_generation" | "gpt_oss.text_generation" => Some(Self::GptOssInference),
            "ollama.embeddings" | "gpt_oss.embeddings" => Some(Self::GptOssEmbeddings),
            "apple_foundation_models.text_generation" => Some(Self::AppleFoundationModelsInference),
            "sandbox.container.exec" => Some(Self::SandboxContainerExec),
            "sandbox.python.exec" => Some(Self::SandboxPythonExec),
            "sandbox.node.exec" => Some(Self::SandboxNodeExec),
            "sandbox.posix.exec" => Some(Self::SandboxPosixExec),
            _ => None,
        }
    }

    pub fn descriptor(self) -> ProviderProductDescriptor {
        ProviderProductDescriptor {
            product_id: self.product_id().to_string(),
            compute_family: self.compute_family_label().to_string(),
            backend_family: self.backend_label().to_string(),
            sandbox_execution_class: self
                .sandbox_execution_class()
                .map(|execution_class| execution_class.product_id().to_string()),
        }
    }
}

pub fn describe_provider_product_id(product_id: &str) -> Option<ProviderProductDescriptor> {
    ProviderComputeProduct::for_product_id(product_id).map(ProviderComputeProduct::descriptor)
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderInventoryRow {
    pub target: ProviderComputeProduct,
    pub enabled: bool,
    pub backend_ready: bool,
    pub eligible: bool,
    pub capability_summary: String,
    pub source_badge: String,
    pub capacity_lot_id: Option<String>,
    pub total_quantity: u64,
    pub reserved_quantity: u64,
    pub available_quantity: u64,
    pub delivery_state: String,
    pub price_floor_sats: u64,
    pub terms_label: String,
    pub forward_capacity_lot_id: Option<String>,
    pub forward_delivery_window_label: Option<String>,
    pub forward_total_quantity: u64,
    pub forward_reserved_quantity: u64,
    pub forward_available_quantity: u64,
    pub forward_terms_label: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderInventoryControls {
    #[serde(alias = "ollama_inference_enabled")]
    pub gpt_oss_inference_enabled: bool,
    #[serde(alias = "ollama_embeddings_enabled")]
    pub gpt_oss_embeddings_enabled: bool,
    pub apple_fm_inference_enabled: bool,
    pub sandbox_container_exec_enabled: bool,
    pub sandbox_python_exec_enabled: bool,
    pub sandbox_node_exec_enabled: bool,
    pub sandbox_posix_exec_enabled: bool,
}

impl Default for ProviderInventoryControls {
    fn default() -> Self {
        Self {
            gpt_oss_inference_enabled: true,
            gpt_oss_embeddings_enabled: false,
            apple_fm_inference_enabled: true,
            sandbox_container_exec_enabled: false,
            sandbox_python_exec_enabled: false,
            sandbox_node_exec_enabled: false,
            sandbox_posix_exec_enabled: false,
        }
    }
}

impl ProviderInventoryControls {
    pub const fn is_advertised(&self, target: ProviderComputeProduct) -> bool {
        match target {
            ProviderComputeProduct::GptOssInference => self.gpt_oss_inference_enabled,
            ProviderComputeProduct::GptOssEmbeddings => false,
            ProviderComputeProduct::AppleFoundationModelsInference => {
                self.apple_fm_inference_enabled
            }
            ProviderComputeProduct::SandboxContainerExec => self.sandbox_container_exec_enabled,
            ProviderComputeProduct::SandboxPythonExec => self.sandbox_python_exec_enabled,
            ProviderComputeProduct::SandboxNodeExec => self.sandbox_node_exec_enabled,
            ProviderComputeProduct::SandboxPosixExec => self.sandbox_posix_exec_enabled,
        }
    }

    pub fn is_product_advertised(&self, product_id: &str) -> bool {
        ProviderComputeProduct::for_product_id(product_id)
            .is_some_and(|target| self.is_advertised(target))
    }

    pub fn toggle(&mut self, target: ProviderComputeProduct) -> bool {
        let enabled = match target {
            ProviderComputeProduct::GptOssInference => &mut self.gpt_oss_inference_enabled,
            ProviderComputeProduct::AppleFoundationModelsInference => {
                &mut self.apple_fm_inference_enabled
            }
            ProviderComputeProduct::SandboxContainerExec => {
                &mut self.sandbox_container_exec_enabled
            }
            ProviderComputeProduct::SandboxPythonExec => &mut self.sandbox_python_exec_enabled,
            ProviderComputeProduct::SandboxNodeExec => &mut self.sandbox_node_exec_enabled,
            ProviderComputeProduct::SandboxPosixExec => &mut self.sandbox_posix_exec_enabled,
            ProviderComputeProduct::GptOssEmbeddings => {
                self.gpt_oss_embeddings_enabled = false;
                return false;
            }
        };
        *enabled = !*enabled;
        *enabled
    }
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ProviderAdvertisedProduct {
    pub product: ProviderComputeProduct,
    pub enabled: bool,
    pub backend_ready: bool,
    pub eligible: bool,
    pub capability_summary: String,
    pub price_floor_sats: u64,
    pub terms_label: String,
    pub forward_terms_label: String,
}

pub fn derive_provider_products(
    availability: &ProviderAvailability,
    controls: &ProviderInventoryControls,
) -> Vec<ProviderAdvertisedProduct> {
    ProviderComputeProduct::all()
        .into_iter()
        .filter(|product| availability.product_visible(*product))
        .map(|product| {
            let enabled = controls.is_advertised(product);
            let backend_ready = availability.product_backend_ready(product);
            ProviderAdvertisedProduct {
                product,
                enabled,
                backend_ready,
                eligible: enabled && backend_ready,
                capability_summary: availability.capability_summary_for_product(product),
                price_floor_sats: product.default_price_floor_sats(),
                terms_label: product.terms_label().to_string(),
                forward_terms_label: product.forward_terms_label().to_string(),
            }
        })
        .collect()
}

#[derive(Clone, Debug)]
pub struct ProviderLifecycleInput<'a> {
    pub current_mode: ProviderMode,
    pub ingress_mode: ProviderIngressMode,
    pub relay_error: Option<&'a str>,
    pub availability: &'a ProviderAvailability,
    pub backend_unavailable_detail: Option<&'a str>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ProviderLifecycleTransition {
    StayOffline,
    HoldCurrent,
    Degraded {
        reason_code: &'static str,
        error_detail: String,
        failure_class: ProviderFailureClass,
    },
    Online {
        active_backend: ProviderBackendKind,
    },
}

pub fn derive_provider_lifecycle(
    input: &ProviderLifecycleInput<'_>,
) -> ProviderLifecycleTransition {
    if input.current_mode == ProviderMode::Offline
        && input.ingress_mode != ProviderIngressMode::Online
    {
        return ProviderLifecycleTransition::StayOffline;
    }

    if let Some(relay_error) = input.relay_error {
        return ProviderLifecycleTransition::Degraded {
            reason_code: "NIP90_RELAY_INGRESS_ERROR",
            error_detail: relay_error.to_string(),
            failure_class: ProviderFailureClass::Relay,
        };
    }

    if input.ingress_mode != ProviderIngressMode::Online {
        return ProviderLifecycleTransition::HoldCurrent;
    }

    let Some(active_backend) = input.availability.active_inference_backend() else {
        return ProviderLifecycleTransition::Degraded {
            reason_code: "INFERENCE_BACKEND_UNAVAILABLE",
            error_detail: input
                .backend_unavailable_detail
                .unwrap_or("No local inference backend is ready")
                .to_string(),
            failure_class: ProviderFailureClass::Execution,
        };
    };

    if input.current_mode != ProviderMode::Offline {
        ProviderLifecycleTransition::Online { active_backend }
    } else {
        ProviderLifecycleTransition::HoldCurrent
    }
}

#[cfg(test)]
mod tests {
    use super::{
        ProviderAvailability, ProviderBackendHealth, ProviderBackendKind, ProviderComputeProduct,
        ProviderFailureClass, ProviderIngressMode, ProviderInventoryControls,
        ProviderLifecycleInput, ProviderLifecycleTransition, ProviderMode,
        ProviderSandboxAvailability, ProviderSandboxDetectionConfig, ProviderSandboxExecutionClass,
        ProviderSandboxProfileSpec, derive_provider_lifecycle, derive_provider_products,
        describe_provider_product_id, detect_sandbox_supply,
    };

    fn ensure(condition: bool, message: &str) -> Result<(), Box<dyn std::error::Error>> {
        if condition {
            Ok(())
        } else {
            Err(std::io::Error::other(message.to_string()).into())
        }
    }

    fn ready_health(
        configured_model: Option<&str>,
        ready_model: &str,
        latency_ms_p50: Option<u64>,
    ) -> ProviderBackendHealth {
        ProviderBackendHealth {
            reachable: true,
            ready: true,
            configured_model: configured_model.map(str::to_string),
            ready_model: Some(ready_model.to_string()),
            available_models: vec![ready_model.to_string()],
            last_error: None,
            last_action: Some("ready".to_string()),
            availability_message: None,
            latency_ms_p50,
        }
    }

    #[test]
    fn apple_backend_wins_when_both_backends_are_ready() {
        let availability = ProviderAvailability {
            gpt_oss: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ready_health(None, "apple-foundation-model", None),
            sandbox: ProviderSandboxAvailability::default(),
        };

        assert_eq!(
            availability.active_inference_backend(),
            Some(ProviderBackendKind::AppleFoundationModels)
        );
        assert_eq!(
            availability.execution_backend_label(),
            "Apple Foundation Models bridge"
        );
    }

    #[test]
    fn controls_gate_products_by_product_id() {
        let mut controls = ProviderInventoryControls::default();
        assert!(controls.is_product_advertised("gpt_oss.text_generation"));
        assert!(!controls.is_product_advertised("gpt_oss.embeddings"));
        assert!(controls.is_product_advertised("apple_foundation_models.text_generation"));
        assert!(!controls.is_product_advertised("sandbox.python.exec"));

        let enabled = controls.toggle(ProviderComputeProduct::GptOssEmbeddings);
        assert!(!enabled);
        assert!(!controls.is_product_advertised("gpt_oss.embeddings"));
    }

    #[test]
    fn derive_provider_products_reflects_backend_health_and_capability_summary() {
        let availability = ProviderAvailability {
            gpt_oss: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ProviderBackendHealth {
                reachable: true,
                ready: false,
                configured_model: None,
                ready_model: None,
                available_models: vec!["apple-foundation-model".to_string()],
                last_error: Some("apple fm unavailable".to_string()),
                last_action: Some("health check failed".to_string()),
                availability_message: Some("apple_intelligence_disabled".to_string()),
                latency_ms_p50: None,
            },
            sandbox: ProviderSandboxAvailability::default(),
        };
        let products =
            derive_provider_products(&availability, &ProviderInventoryControls::default());

        assert_eq!(products.len(), 2);
        assert!(products[0].eligible);
        assert!(!products[1].eligible);
        assert!(
            products[0]
                .capability_summary
                .contains("latency_ms_p50=140")
        );
        assert!(
            products[1]
                .capability_summary
                .contains("platform_gate=apple_intelligence_disabled")
        );
    }

    #[test]
    fn product_descriptor_preserves_launch_and_sandbox_taxonomy() {
        let embeddings = describe_provider_product_id("gpt_oss.embeddings");
        assert_eq!(
            embeddings
                .as_ref()
                .map(|descriptor| descriptor.compute_family.as_str()),
            Some("embeddings")
        );
        assert_eq!(
            embeddings
                .as_ref()
                .map(|descriptor| descriptor.backend_family.as_str()),
            Some("gpt_oss")
        );
        assert_eq!(
            embeddings
                .as_ref()
                .and_then(|descriptor| descriptor.sandbox_execution_class.as_deref()),
            None
        );

        let sandbox = describe_provider_product_id("sandbox.python.exec");
        assert_eq!(
            sandbox
                .as_ref()
                .map(|descriptor| descriptor.compute_family.as_str()),
            Some("sandbox_execution")
        );
        assert_eq!(
            sandbox
                .as_ref()
                .map(|descriptor| descriptor.backend_family.as_str()),
            Some("sandbox")
        );
        assert_eq!(
            sandbox
                .as_ref()
                .and_then(|descriptor| descriptor.sandbox_execution_class.as_deref()),
            Some("sandbox.python.exec")
        );
    }

    #[test]
    fn derive_provider_products_includes_declared_sandbox_profiles_when_enabled()
    -> Result<(), Box<dyn std::error::Error>> {
        let availability = ProviderAvailability {
            gpt_oss: ProviderBackendHealth::default(),
            apple_foundation_models: ProviderBackendHealth::default(),
            sandbox: detect_sandbox_supply(&ProviderSandboxDetectionConfig {
                path_entries: Vec::new(),
                declared_profiles: vec![ProviderSandboxProfileSpec {
                    profile_id: "python-batch".to_string(),
                    execution_class: ProviderSandboxExecutionClass::PythonExec,
                    runtime_family: "python3".to_string(),
                    runtime_version: Some("3.11".to_string()),
                    sandbox_engine: "local_subprocess".to_string(),
                    os_family: std::env::consts::OS.to_string(),
                    arch: std::env::consts::ARCH.to_string(),
                    cpu_limit: 2,
                    memory_limit_mb: 2048,
                    disk_limit_mb: 4096,
                    timeout_limit_s: 120,
                    network_mode: "none".to_string(),
                    filesystem_mode: "workspace_only".to_string(),
                    workspace_mode: "ephemeral".to_string(),
                    artifact_output_mode: "declared_paths_only".to_string(),
                    secrets_mode: "none".to_string(),
                    allowed_binaries: vec!["python3".to_string()],
                    toolchain_inventory: vec!["python3".to_string()],
                    container_image: None,
                    runtime_image_digest: None,
                    accelerator_policy: None,
                }],
            }),
        };
        let mut controls = ProviderInventoryControls::default();
        controls.sandbox_python_exec_enabled = true;

        let products = derive_provider_products(&availability, &controls);
        let sandbox_product = products
            .iter()
            .find(|product| product.product == ProviderComputeProduct::SandboxPythonExec)
            .ok_or_else(|| std::io::Error::other("missing sandbox python product"))?;
        ensure(
            !sandbox_product.backend_ready,
            "sandbox product should stay unready without a detected runtime",
        )?;
        ensure(
            sandbox_product
                .capability_summary
                .contains("sandbox.python.exec"),
            "sandbox capability summary should include python execution class",
        )?;
        ensure(
            sandbox_product
                .capability_summary
                .contains("profile_ids=python-batch"),
            "sandbox capability summary should include declared profile id",
        )?;
        Ok(())
    }

    #[test]
    fn lifecycle_degrades_on_relay_error() {
        let availability = ProviderAvailability::default();
        let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
            current_mode: ProviderMode::Connecting,
            ingress_mode: ProviderIngressMode::Online,
            relay_error: Some("relay write failed"),
            availability: &availability,
            backend_unavailable_detail: None,
        });

        assert_eq!(
            transition,
            ProviderLifecycleTransition::Degraded {
                reason_code: "NIP90_RELAY_INGRESS_ERROR",
                error_detail: "relay write failed".to_string(),
                failure_class: ProviderFailureClass::Relay,
            }
        );
    }

    #[test]
    fn lifecycle_degrades_when_no_backend_is_ready() {
        let availability = ProviderAvailability::default();
        let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
            current_mode: ProviderMode::Connecting,
            ingress_mode: ProviderIngressMode::Online,
            relay_error: None,
            availability: &availability,
            backend_unavailable_detail: Some("No local inference backend is ready"),
        });

        assert_eq!(
            transition,
            ProviderLifecycleTransition::Degraded {
                reason_code: "INFERENCE_BACKEND_UNAVAILABLE",
                error_detail: "No local inference backend is ready".to_string(),
                failure_class: ProviderFailureClass::Execution,
            }
        );
    }

    #[test]
    fn lifecycle_holds_while_offline_even_if_backends_are_ready() {
        let availability = ProviderAvailability {
            gpt_oss: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ProviderBackendHealth::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
            current_mode: ProviderMode::Offline,
            ingress_mode: ProviderIngressMode::Online,
            relay_error: None,
            availability: &availability,
            backend_unavailable_detail: None,
        });

        assert_eq!(transition, ProviderLifecycleTransition::HoldCurrent);
    }

    #[test]
    fn lifecycle_promotes_non_offline_runtime_to_online_when_backend_is_ready() {
        let availability = ProviderAvailability {
            gpt_oss: ready_health(Some("llama3.2:latest"), "llama3.2:latest", Some(140)),
            apple_foundation_models: ProviderBackendHealth::default(),
            sandbox: ProviderSandboxAvailability::default(),
        };
        let transition = derive_provider_lifecycle(&ProviderLifecycleInput {
            current_mode: ProviderMode::Connecting,
            ingress_mode: ProviderIngressMode::Online,
            relay_error: None,
            availability: &availability,
            backend_unavailable_detail: None,
        });

        assert_eq!(
            transition,
            ProviderLifecycleTransition::Online {
                active_backend: ProviderBackendKind::GptOss,
            }
        );
    }
}
