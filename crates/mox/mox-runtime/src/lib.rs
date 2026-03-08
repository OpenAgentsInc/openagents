//! Runtime traits and execution surfaces for Mox.

mod parity;

use std::collections::BTreeMap;

use mox_core::{DType, Device, QuantizationMode, QuantizedBlockLayout, TensorId, TensorSpec};
use mox_ir::ExecutionPlan;
pub use parity::*;
use rand::{Rng, SeedableRng, rngs::StdRng};
use serde::{Deserialize, Serialize};
use thiserror::Error;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "runtime traits for devices and execution";

/// Stable runtime backend name.
pub type BackendName = &'static str;

/// Runtime failure.
#[derive(Debug, Error, PartialEq, Eq)]
pub enum RuntimeError {
    /// The requested tensor input was not supplied.
    #[error("missing input tensor {0}")]
    MissingInput(TensorId),
    /// A buffer shape or dtype was not what execution expected.
    #[error("invalid buffer for tensor {tensor}: expected {expected:?}, actual {actual:?}")]
    InvalidBuffer {
        /// Tensor ID that failed validation.
        tensor: TensorId,
        /// Expected tensor specification.
        expected: TensorSpec,
        /// Actual tensor specification.
        actual: TensorSpec,
    },
    /// The execution plan referenced a node that the backend cannot execute.
    #[error("unsupported execution step `{0}`")]
    UnsupportedStep(String),
    /// Generic backend failure.
    #[error("{0}")]
    Backend(String),
}

/// Runtime-visible device description.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DeviceDescriptor {
    /// Backend family name.
    pub backend: String,
    /// Logical device.
    pub device: Device,
    /// Human-readable device name when the backend can supply one.
    pub device_name: Option<String>,
    /// Supported dtypes for the device.
    pub supported_dtypes: Vec<DType>,
    /// Supported quantization modes for model-backed execution.
    pub supported_quantization: Vec<QuantizationSupport>,
    /// Optional memory capacity in bytes.
    pub memory_capacity_bytes: Option<u64>,
    /// Whether the device shares memory with the host, when known.
    pub unified_memory: Option<bool>,
    /// Stable feature flags relevant to runtime/backend selection.
    pub feature_flags: Vec<String>,
    /// AMD-specific topology/risk metadata when the device belongs to an AMD backend.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amd_metadata: Option<AmdDeviceMetadata>,
}

/// Distinct AMD runtime mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRuntimeMode {
    /// Kernel-mediated AMD KFD posture using the standard `amdgpu` driver stack.
    Kfd,
    /// Explicitly opted-in userspace/AM-driver posture.
    Userspace,
}

/// Whether an AMD mode requires or has satisfied explicit opt-in.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdOptInStatus {
    /// The backend does not require an explicit opt-in gate.
    NotRequired,
    /// The backend is present but currently disabled until the operator opts in.
    Disabled,
    /// The operator has explicitly enabled the backend.
    Enabled,
}

/// Risk posture for an AMD backend mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRiskLevel {
    /// Lower-risk operational posture.
    Standard,
    /// Higher-risk posture that needs stronger operator intent.
    Elevated,
}

/// Driver ownership/binding state relevant to AMD recovery posture.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdDriverBinding {
    /// The kernel `amdgpu` driver still owns the device.
    KernelAmdgpu,
    /// A userspace stack has taken ownership of the device.
    UserspaceClaimed,
    /// Mox could not determine the binding state.
    Unknown,
}

/// Expected operator-level recovery step for an AMD backend mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AmdRecoveryAction {
    /// Restart the affected process/runtime first.
    ProcessRestart,
    /// Attempt a kernel-driver reset or recovery path.
    KernelDriverReset,
    /// Rebind or restore the kernel driver after userspace mode.
    RebindKernelDriver,
    /// Reboot the host when the runtime cannot recover in-place.
    RebootHost,
}

/// Stable AMD topology fields relevant to backend discovery and later capability reporting.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdTopologyInfo {
    /// Stable architecture label such as `gfx1100`, when known.
    pub architecture: Option<String>,
    /// PCI bus/device/function address, when known.
    pub pci_bdf: Option<String>,
    /// Number of XCC partitions, when known.
    pub xcc_count: Option<u16>,
    /// Number of shader engines, when known.
    pub shader_engine_count: Option<u16>,
    /// Number of compute units, when known.
    pub compute_unit_count: Option<u16>,
    /// Total VRAM bytes, when known.
    pub vram_bytes: Option<u64>,
    /// Host-visible VRAM bytes, when known.
    pub visible_vram_bytes: Option<u64>,
}

/// Stable AMD risk posture derived from the backend/runtime mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdRiskProfile {
    /// High-level risk classification.
    pub level: AmdRiskLevel,
    /// Whether the mode requires explicit operator intent before activation.
    pub requires_explicit_opt_in: bool,
    /// Whether the mode may unbind or otherwise displace the kernel driver.
    pub may_unbind_kernel_driver: bool,
    /// Plain-text warnings the operator should see or preserve in logs.
    pub warnings: Vec<String>,
}

/// Stable AMD recovery posture derived from the backend/runtime mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdRecoveryProfile {
    /// Current or expected driver binding state.
    pub driver_binding: AmdDriverBinding,
    /// Ordered recovery actions Mox expects the operator/runtime to consider.
    pub expected_actions: Vec<AmdRecoveryAction>,
}

/// AMD-specific device metadata carried through runtime and provider truth surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdDeviceMetadata {
    /// Runtime mode that discovered the device.
    pub mode: AmdRuntimeMode,
    /// Stable topology snapshot.
    pub topology: AmdTopologyInfo,
    /// Risk posture for the selected AMD mode.
    pub risk: AmdRiskProfile,
    /// Recovery posture for the selected AMD mode.
    pub recovery: AmdRecoveryProfile,
}

/// Backend-local AMD discovery report that preserves mode and opt-in truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct AmdBackendReport {
    /// AMD backend mode represented by the report.
    pub mode: AmdRuntimeMode,
    /// Opt-in state for the backend mode.
    pub opt_in: AmdOptInStatus,
    /// Discovered devices for the mode.
    pub devices: Vec<DeviceDescriptor>,
    /// Honest readiness/health for the mode.
    pub health: RuntimeHealth,
}

/// How a backend handles a quantization mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationExecution {
    /// Execute the quantized representation directly.
    Native,
    /// Dequantize weights to `f32` before execution.
    DequantizeToF32,
}

/// Explicit load/storage posture for a quantized mode.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum QuantizationLoadPath {
    /// Weights arrive as ordinary dense `f32` tensors.
    DenseF32,
    /// The runtime loads quantized weights and immediately dequantizes them to `f32`.
    DequantizedF32,
    /// The runtime preserves quantized blocks in backend-owned storage.
    BackendQuantized,
}

/// Runtime support declaration for a quantization mode.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct QuantizationSupport {
    /// Supported quantization mode.
    pub mode: QuantizationMode,
    /// Explicit load/storage path for the quantized weights.
    pub load_path: QuantizationLoadPath,
    /// How the runtime executes that mode.
    pub execution: QuantizationExecution,
}

/// Runtime health state.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum HealthStatus {
    /// Device/runtime is ready for work.
    Ready,
    /// Device/runtime can execute but with caveats.
    Degraded,
    /// Device/runtime cannot execute.
    Offline,
}

/// Health report for a runtime or backend.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct RuntimeHealth {
    /// Current health status.
    pub status: HealthStatus,
    /// Plain-text explanation.
    pub message: String,
}

/// Maximum token-history window used when applying repetition-style penalties.
pub const DEFAULT_PENALTY_LOOKBACK: usize = 64;

/// Runtime-owned token-selection strategy.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum SamplingStrategy {
    /// Always choose the highest adjusted logit.
    Greedy,
    /// Draw from the adjusted probability distribution.
    Sample,
}

/// Reusable runtime sampling policy for token selection.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct SamplingPolicy {
    /// Sampling strategy.
    pub strategy: SamplingStrategy,
    /// Temperature override for stochastic sampling.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub temperature: Option<f32>,
    /// Top-k sampling cap.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_k: Option<usize>,
    /// Top-p / nucleus sampling threshold.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub top_p: Option<f32>,
    /// Repeat penalty applied to previously seen tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub repeat_penalty: Option<f32>,
    /// Presence penalty applied once to previously seen tokens.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presence_penalty: Option<f32>,
    /// Frequency penalty scaled by prior token count.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub frequency_penalty: Option<f32>,
    /// Deterministic seed for stochastic decode.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seed: Option<u64>,
}

impl SamplingPolicy {
    /// Returns the effective temperature after applying runtime defaults.
    #[must_use]
    pub fn effective_temperature(&self) -> f32 {
        self.temperature.unwrap_or(0.8).max(0.0)
    }

    /// Returns the effective top-k cap after applying runtime defaults.
    #[must_use]
    pub fn effective_top_k(&self) -> Option<usize> {
        self.top_k.or(Some(40))
    }

    /// Returns the effective top-p threshold after applying runtime defaults.
    #[must_use]
    pub fn effective_top_p(&self) -> Option<f32> {
        self.top_p.or(Some(0.9))
    }

    /// Returns the effective repeat penalty after applying runtime defaults.
    #[must_use]
    pub fn effective_repeat_penalty(&self) -> f32 {
        self.repeat_penalty.unwrap_or(1.0)
    }

    /// Returns the effective presence penalty after applying runtime defaults.
    #[must_use]
    pub fn effective_presence_penalty(&self) -> f32 {
        self.presence_penalty.unwrap_or(0.0)
    }

    /// Returns the effective frequency penalty after applying runtime defaults.
    #[must_use]
    pub fn effective_frequency_penalty(&self) -> f32 {
        self.frequency_penalty.unwrap_or(0.0)
    }
}

/// Reusable runtime sampler with optional seeded replay.
#[derive(Clone, Debug)]
pub struct TokenSampler {
    policy: SamplingPolicy,
    rng: StdRng,
}

impl TokenSampler {
    /// Creates a token sampler for one runtime policy.
    #[must_use]
    pub fn new(policy: &SamplingPolicy) -> Self {
        let rng = policy
            .seed
            .map_or_else(StdRng::from_os_rng, StdRng::seed_from_u64);
        Self {
            policy: policy.clone(),
            rng,
        }
    }

    /// Returns the runtime sampling policy.
    #[must_use]
    pub fn policy(&self) -> &SamplingPolicy {
        &self.policy
    }

    /// Selects the next token from logits and prior token history.
    pub fn select_next_token(&mut self, logits: &[f32], history: &[u32]) -> Option<u32> {
        let mut adjusted_logits = logits.to_vec();
        apply_sampling_penalties(&mut adjusted_logits, history, &self.policy);
        if self.policy.strategy == SamplingStrategy::Greedy
            || self.policy.effective_temperature() <= 1e-6
        {
            return select_argmax_token(&adjusted_logits);
        }
        sample_token_index(&mut self.rng, &adjusted_logits, &self.policy)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct SampleToken {
    id: u32,
    value: f32,
}

/// Applies repeat, presence, and frequency penalties using the bounded runtime history window.
pub fn apply_sampling_penalties(logits: &mut [f32], history: &[u32], policy: &SamplingPolicy) {
    let repeat_penalty = policy.effective_repeat_penalty();
    let presence_penalty = policy.effective_presence_penalty();
    let frequency_penalty = policy.effective_frequency_penalty();
    if (repeat_penalty - 1.0).abs() <= f32::EPSILON
        && presence_penalty.abs() <= f32::EPSILON
        && frequency_penalty.abs() <= f32::EPSILON
    {
        return;
    }

    for (token, count) in token_counts(history, logits.len()) {
        let Some(logit) = logits.get_mut(token as usize) else {
            continue;
        };
        if (repeat_penalty - 1.0).abs() > f32::EPSILON {
            if *logit < 0.0 {
                *logit *= repeat_penalty;
            } else {
                *logit /= repeat_penalty;
            }
        }
        if frequency_penalty.abs() > f32::EPSILON {
            *logit -= frequency_penalty * (count as f32);
        }
        if presence_penalty.abs() > f32::EPSILON {
            *logit -= presence_penalty;
        }
    }
}

/// Selects the highest-logit token index.
#[must_use]
pub fn select_argmax_token(logits: &[f32]) -> Option<u32> {
    logits
        .iter()
        .enumerate()
        .max_by(|(_, left), (_, right)| left.total_cmp(right))
        .map(|(index, _)| index as u32)
}

fn token_counts(history: &[u32], vocab_size: usize) -> BTreeMap<u32, usize> {
    let start = history.len().saturating_sub(DEFAULT_PENALTY_LOOKBACK);
    let mut counts = BTreeMap::new();
    for &token in &history[start..] {
        if token as usize >= vocab_size {
            continue;
        }
        *counts.entry(token).or_insert(0) += 1;
    }
    counts
}

fn sample_token_index(rng: &mut StdRng, logits: &[f32], policy: &SamplingPolicy) -> Option<u32> {
    let temperature = policy.effective_temperature();
    if temperature <= 1e-6 {
        return select_argmax_token(logits);
    }

    let mut tokens = logits
        .iter()
        .enumerate()
        .map(|(index, value)| SampleToken {
            id: index as u32,
            value: *value,
        })
        .collect::<Vec<_>>();
    top_k(&mut tokens, policy.effective_top_k());
    temperature_scale(&mut tokens, temperature);
    let total = softmax(&mut tokens);
    if !total.is_finite() || total <= 0.0 {
        return None;
    }
    top_p(&mut tokens, policy.effective_top_p());

    let distribution_total = tokens.iter().map(|token| token.value).sum::<f32>();
    if !distribution_total.is_finite() || distribution_total <= 0.0 {
        return None;
    }

    let mut target = rng.random::<f32>() * distribution_total;
    for token in &tokens {
        target -= token.value;
        if target <= 0.0 {
            return Some(token.id);
        }
    }
    tokens.last().map(|token| token.id)
}

fn top_k(tokens: &mut Vec<SampleToken>, top_k: Option<usize>) {
    tokens.sort_by(|left, right| right.value.total_cmp(&left.value));
    let Some(top_k) = top_k else {
        return;
    };
    if top_k > 0 && top_k < tokens.len() {
        tokens.truncate(top_k);
    }
}

fn temperature_scale(tokens: &mut [SampleToken], temperature: f32) {
    let temperature = temperature.max(1e-7);
    for token in tokens {
        token.value /= temperature;
    }
}

fn softmax(tokens: &mut [SampleToken]) -> f32 {
    let Some(max_logit) = tokens
        .iter()
        .map(|token| token.value)
        .max_by(f32::total_cmp)
    else {
        return 0.0;
    };
    let mut sum = 0.0;
    for token in tokens.iter_mut() {
        token.value = (token.value - max_logit).exp();
        sum += token.value;
    }
    if !sum.is_finite() || sum <= 0.0 {
        return sum;
    }
    for token in tokens.iter_mut() {
        token.value /= sum;
    }
    sum
}

fn top_p(tokens: &mut Vec<SampleToken>, top_p: Option<f32>) {
    let Some(top_p) = top_p else {
        return;
    };
    if top_p <= 0.0 || top_p >= 1.0 {
        return;
    }

    let mut cumulative = 0.0;
    let mut keep = tokens.len();
    for (index, token) in tokens.iter().enumerate() {
        cumulative += token.value;
        if cumulative >= top_p {
            keep = index + 1;
            break;
        }
    }
    tokens.truncate(keep.max(1));
}

/// Lifecycle state for a model that is resident in a local runtime.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LoadedModelState {
    /// The model is still warming/loading.
    Loading,
    /// The model is loaded and available for requests.
    Ready,
}

/// Explicit keepalive and residency truth for one loaded model.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct LoadedModelResidency {
    /// Current lifecycle state.
    pub state: LoadedModelState,
    /// Number of active requests currently using the model.
    pub active_requests: usize,
    /// Configured keepalive duration in milliseconds.
    pub keep_alive_millis: u64,
    /// Time the current residency was established.
    pub loaded_at_millis: u64,
    /// Most recent time the model was touched by load/warm/request activity.
    pub last_used_at_millis: u64,
    /// Planned expiration time when the model is idle.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub expires_at_millis: Option<u64>,
}

impl LoadedModelResidency {
    /// Creates a loading residency record.
    #[must_use]
    pub fn loading(now_millis: u64, keep_alive_millis: u64) -> Self {
        Self {
            state: LoadedModelState::Loading,
            active_requests: 0,
            keep_alive_millis,
            loaded_at_millis: now_millis,
            last_used_at_millis: now_millis,
            expires_at_millis: Self::idle_expiration(now_millis, keep_alive_millis, 0),
        }
    }

    /// Creates a ready residency record.
    #[must_use]
    pub fn ready(now_millis: u64, keep_alive_millis: u64) -> Self {
        Self {
            state: LoadedModelState::Ready,
            active_requests: 0,
            keep_alive_millis,
            loaded_at_millis: now_millis,
            last_used_at_millis: now_millis,
            expires_at_millis: Self::idle_expiration(now_millis, keep_alive_millis, 0),
        }
    }

    /// Marks the model ready without changing its residency anchor.
    pub fn mark_ready(&mut self, now_millis: u64) {
        self.state = LoadedModelState::Ready;
        self.last_used_at_millis = now_millis;
        self.expires_at_millis =
            Self::idle_expiration(now_millis, self.keep_alive_millis, self.active_requests);
    }

    /// Refreshes keepalive and idle-expiration posture.
    pub fn refresh_keep_alive(&mut self, keep_alive_millis: u64, now_millis: u64) {
        self.keep_alive_millis = keep_alive_millis;
        self.last_used_at_millis = now_millis;
        self.expires_at_millis =
            Self::idle_expiration(now_millis, keep_alive_millis, self.active_requests);
    }

    /// Marks the start of a request using the model.
    pub fn begin_request(&mut self, now_millis: u64) {
        self.active_requests += 1;
        self.last_used_at_millis = now_millis;
        self.expires_at_millis = None;
    }

    /// Marks the completion of a request using the model.
    pub fn finish_request(&mut self, now_millis: u64) {
        if self.active_requests > 0 {
            self.active_requests -= 1;
        }
        self.last_used_at_millis = now_millis;
        self.expires_at_millis =
            Self::idle_expiration(now_millis, self.keep_alive_millis, self.active_requests);
    }

    /// Forces the model to expire immediately once idle.
    pub fn expire_now(&mut self, now_millis: u64) {
        self.keep_alive_millis = 0;
        self.last_used_at_millis = now_millis;
        self.expires_at_millis = Some(now_millis);
    }

    /// Returns whether the model should be unloaded at the provided time.
    #[must_use]
    pub fn is_expired(&self, now_millis: u64) -> bool {
        self.active_requests == 0
            && self
                .expires_at_millis
                .is_some_and(|expires_at_millis| expires_at_millis <= now_millis)
    }

    fn idle_expiration(
        now_millis: u64,
        keep_alive_millis: u64,
        active_requests: usize,
    ) -> Option<u64> {
        if active_requests > 0 {
            None
        } else {
            now_millis.checked_add(keep_alive_millis)
        }
    }
}

/// Whether KV pages stay bound to one backend/device posture or can move.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheDeviceScope {
    /// KV state stays bound to the active backend/device and is not migrated.
    SameDeviceOnly,
    /// KV state may move across devices through an explicit transfer path.
    CrossDeviceExplicit,
}

/// Policy to apply when paged KV growth would exceed the admitted budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum KvCacheSpillPolicy {
    /// Refuse additional KV growth instead of evicting or spilling silently.
    RefuseNewPages,
    /// Evict older pages to admit new ones.
    EvictOldestPages,
    /// Spill pages to a slower/offloaded tier.
    SpillToHost,
}

/// Stable logical page layout for paged KV state.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCachePageLayout {
    /// Maximum supported context tokens for the cache.
    pub max_context_tokens: usize,
    /// Number of tokens stored in one logical page.
    pub tokens_per_page: usize,
    /// Number of bytes consumed per cached token.
    pub bytes_per_token: usize,
    /// Number of bytes consumed by one full logical page.
    pub page_bytes: usize,
    /// Maximum number of pages the cache may own.
    pub max_pages: usize,
}

impl KvCachePageLayout {
    /// Creates a logical page layout from token and byte geometry.
    #[must_use]
    pub fn new(max_context_tokens: usize, tokens_per_page: usize, bytes_per_token: usize) -> Self {
        let max_context_tokens = max_context_tokens.max(1);
        let tokens_per_page = tokens_per_page.max(1);
        let bytes_per_token = bytes_per_token.max(1);
        let max_pages = max_context_tokens.div_ceil(tokens_per_page);
        Self {
            max_context_tokens,
            tokens_per_page,
            bytes_per_token,
            page_bytes: tokens_per_page.saturating_mul(bytes_per_token),
            max_pages,
        }
    }

    /// Returns the number of pages required for the provided token count.
    #[must_use]
    pub fn page_count_for_tokens(&self, tokens: usize) -> usize {
        if tokens == 0 {
            0
        } else {
            tokens.div_ceil(self.tokens_per_page)
        }
    }

    /// Returns the number of bytes required for the provided token count.
    #[must_use]
    pub fn bytes_for_tokens(&self, tokens: usize) -> u64 {
        tokens
            .saturating_mul(self.bytes_per_token)
            .try_into()
            .unwrap_or(u64::MAX)
    }
}

/// Explicit paged-KV policy exposed through runtime and evidence surfaces.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCachePolicy {
    /// Whether KV state stays on one backend/device or can move explicitly.
    pub device_scope: KvCacheDeviceScope,
    /// What to do when the page budget would be exceeded.
    pub spill_policy: KvCacheSpillPolicy,
    /// Logical page layout for the cache.
    pub page_layout: KvCachePageLayout,
}

/// Snapshot of current paged-KV usage.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheState {
    /// Number of tokens currently cached.
    pub tokens: usize,
    /// Number of bytes currently owned by the cache.
    pub bytes: u64,
    /// Number of pages currently owned by the cache.
    pub pages: usize,
}

impl KvCacheState {
    /// Builds paged-KV state from a logical layout and token count.
    #[must_use]
    pub fn paged(layout: &KvCachePageLayout, tokens: usize) -> Self {
        Self {
            tokens,
            bytes: layout.bytes_for_tokens(tokens),
            pages: layout.page_count_for_tokens(tokens),
        }
    }
}

/// Growth delta between two paged-KV states.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheGrowth {
    /// Net token growth between the baseline and current state.
    pub tokens: usize,
    /// Net byte growth between the baseline and current state.
    pub bytes: u64,
    /// Net page growth between the baseline and current state.
    pub pages: usize,
}

/// Current paged-KV state plus request-local growth accounting.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct KvCacheAccounting {
    /// Current paged-KV state after the request.
    pub current: KvCacheState,
    /// Growth attributable to the request.
    pub growth: KvCacheGrowth,
}

impl KvCacheAccounting {
    /// Creates accounting from a before/after paged-KV snapshot.
    #[must_use]
    pub fn from_states(before: &KvCacheState, current: KvCacheState) -> Self {
        Self {
            growth: KvCacheGrowth {
                tokens: current.tokens.saturating_sub(before.tokens),
                bytes: current.bytes.saturating_sub(before.bytes),
                pages: current.pages.saturating_sub(before.pages),
            },
            current,
        }
    }
}

/// Observable state for shared prompt-prefix reuse.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PrefixCacheState {
    /// No compatible shared prefix cache existed for the request.
    None,
    /// A compatible shared prefix cache was found and reused.
    Hit,
    /// Compatible shared prefix caches existed but none matched the request prefix.
    Miss,
    /// Reuse was intentionally skipped under the current policy.
    Bypassed,
    /// A stale or invalid shared prefix entry was discarded and rebuilt.
    Rebuilt,
}

/// Explicit reuse boundaries for shared prompt-prefix caches.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixCacheReusePolicy {
    /// Whether prefixes may be reused across distinct sessions.
    pub shared_across_sessions: bool,
    /// Whether prefixes may be reused across distinct user/security domains.
    pub shared_across_users: bool,
    /// Whether prefixes may be reused across different models or revisions.
    pub shared_across_models: bool,
    /// Whether prefixes may be reused across different backend identities.
    pub shared_across_backends: bool,
}

/// Stable identity tuple for one reusable shared prompt prefix.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PrefixCacheIdentity {
    /// Stable model identifier.
    pub model_id: String,
    /// Stable model revision.
    pub model_revision: String,
    /// Stable weight-bundle digest.
    pub weight_bundle_digest: String,
    /// Tokenizer family label used to produce the prompt tokens.
    pub tokenizer_family: String,
    /// Stable tokenizer digest when known.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tokenizer_digest: Option<String>,
    /// Stable chat-template digest when prompt rendering supplied one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub chat_template_digest: Option<String>,
    /// Stable generation-defaults digest when prompt rendering depended on one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub generation_defaults_digest: Option<String>,
    /// Stable backend compatibility label required for reuse.
    pub backend_compatibility: String,
    /// Stable digest of the reusable prompt-prefix tokens.
    pub prefix_digest: String,
    /// Number of reusable prompt-prefix tokens represented by the digest.
    pub prefix_tokens: usize,
}

/// Explicit runtime backend selection and fallback truth.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct BackendSelection {
    /// Backend the caller or higher-level runtime requested.
    pub requested_backend: String,
    /// Backend that will actually execute the work.
    pub effective_backend: String,
    /// Selected device for the effective backend, when one exists.
    pub selected_device: Option<DeviceDescriptor>,
    /// Supported op labels for the advertised product path.
    pub supported_ops: Vec<String>,
    /// Explicit fallback reason when the effective backend differs from the requested backend.
    pub fallback_reason: Option<String>,
}

impl BackendSelection {
    /// Creates a direct backend selection with no fallback.
    #[must_use]
    pub fn direct(
        backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
    ) -> Self {
        let backend = backend.into();
        Self {
            requested_backend: backend.clone(),
            effective_backend: backend,
            selected_device,
            supported_ops,
            fallback_reason: None,
        }
    }

    /// Creates an explicit fallback selection.
    #[must_use]
    pub fn fallback(
        requested_backend: impl Into<String>,
        effective_backend: impl Into<String>,
        selected_device: Option<DeviceDescriptor>,
        supported_ops: Vec<String>,
        fallback_reason: impl Into<String>,
    ) -> Self {
        Self {
            requested_backend: requested_backend.into(),
            effective_backend: effective_backend.into(),
            selected_device,
            supported_ops,
            fallback_reason: Some(fallback_reason.into()),
        }
    }

    /// Creates a direct selection from a discovered backend.
    pub fn from_backend<B>(backend: &B, supported_ops: &[&str]) -> Result<Self, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        Ok(Self::direct(
            backend.backend_name(),
            backend.discover_devices()?.into_iter().next(),
            supported_ops
                .iter()
                .map(|label| String::from(*label))
                .collect(),
        ))
    }

    /// Creates a fallback selection to an effective backend discovered at runtime.
    pub fn fallback_to_backend<B>(
        requested_backend: impl Into<String>,
        effective_backend: &B,
        supported_ops: &[&str],
        fallback_reason: impl Into<String>,
    ) -> Result<Self, RuntimeError>
    where
        B: DeviceDiscovery + ?Sized,
    {
        Ok(Self::fallback(
            requested_backend,
            effective_backend.backend_name(),
            effective_backend.discover_devices()?.into_iter().next(),
            supported_ops
                .iter()
                .map(|label| String::from(*label))
                .collect(),
            fallback_reason,
        ))
    }
}

/// Minimal execution metrics.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ExecutionMetrics {
    /// Number of plan steps executed.
    pub steps_executed: usize,
}

/// Trait for backend-owned buffers.
pub trait BufferHandle {
    /// Returns the buffer tensor spec.
    fn spec(&self) -> &TensorSpec;

    /// Returns the storage posture for the buffer.
    fn storage_kind(&self) -> BufferStorageKind {
        BufferStorageKind::DenseF32
    }
}

/// Physical residency of a backend-owned buffer.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum BufferResidency {
    /// Storage lives in host-managed memory.
    Host,
    /// Storage lives in backend-owned device memory.
    Backend,
}

/// Explicit buffer storage kind surfaced by runtime backends.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum BufferStorageKind {
    /// Ordinary dense `f32` tensor storage.
    DenseF32,
    /// Dense `f32` storage that came from a quantized source tensor.
    DequantizedF32 {
        /// Source quantization mode that was dequantized.
        source_quantization: QuantizationMode,
    },
    /// Quantized GGML/GGUF block storage that remains quantized.
    QuantizedBlocks {
        /// Quantized storage family.
        mode: QuantizationMode,
        /// Stable GGML block layout.
        layout: QuantizedBlockLayout,
        /// Whether the storage is host- or backend-resident.
        residency: BufferResidency,
    },
}

/// How a runtime load plan sources model artifact bytes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactStorageKind {
    /// The artifact was copied into an in-memory buffer before planning.
    InMemoryCopy,
    /// The artifact stays backed by a paged local blob.
    PagedLocalBlob,
}

/// Blob family used by a paged local model artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModelArtifactBlobKind {
    /// Standalone GGUF file discovered on disk.
    GgufFile,
    /// Ollama-managed blob resolved by digest.
    OllamaBlob,
}

/// Actual local read path used for a paged model artifact.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ArtifactReadPath {
    /// The artifact bytes are exposed through a memory map.
    MemoryMapped,
    /// The artifact bytes are exposed from a buffered host copy.
    Buffered,
}

/// Runtime-visible storage truth for a model artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ModelArtifactStorage {
    /// Stable artifact name.
    pub artifact_name: String,
    /// Stable SHA-256 digest of the artifact bytes.
    pub artifact_sha256: String,
    /// High-level storage posture used by the runtime.
    pub storage_kind: ModelArtifactStorageKind,
    /// Blob family when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub blob_kind: Option<ModelArtifactBlobKind>,
    /// Actual local read path when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub read_path: Option<ArtifactReadPath>,
    /// Logical page size when the runtime kept paged local blob storage.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_size: Option<usize>,
    /// Explicit fallback reason when mmap was preferred but not used.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<String>,
}

impl ModelArtifactStorage {
    /// Creates storage truth for an eager in-memory artifact copy.
    #[must_use]
    pub fn in_memory_copy(
        artifact_name: impl Into<String>,
        artifact_sha256: impl Into<String>,
    ) -> Self {
        Self {
            artifact_name: artifact_name.into(),
            artifact_sha256: artifact_sha256.into(),
            storage_kind: ModelArtifactStorageKind::InMemoryCopy,
            blob_kind: None,
            read_path: None,
            page_size: None,
            fallback_reason: None,
        }
    }

    /// Creates storage truth for a paged local blob artifact.
    #[must_use]
    pub fn paged_local_blob(
        artifact_name: impl Into<String>,
        artifact_sha256: impl Into<String>,
        blob_kind: ModelArtifactBlobKind,
        read_path: ArtifactReadPath,
        page_size: usize,
        fallback_reason: Option<String>,
    ) -> Self {
        Self {
            artifact_name: artifact_name.into(),
            artifact_sha256: artifact_sha256.into(),
            storage_kind: ModelArtifactStorageKind::PagedLocalBlob,
            blob_kind: Some(blob_kind),
            read_path: Some(read_path),
            page_size: Some(page_size),
            fallback_reason,
        }
    }
}

/// Runtime-visible paged tensor byte plan derived from a blob-backed artifact.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PagedTensorStoragePlan {
    /// Stable tensor name.
    pub tensor_name: String,
    /// Backing artifact name.
    pub artifact_name: String,
    /// Byte offset inside the artifact.
    pub byte_offset: u64,
    /// Tensor byte length inside the artifact.
    pub byte_length: u64,
    /// Logical page size for reads over the tensor bytes.
    pub page_size: usize,
    /// Total page count for the tensor byte range.
    pub page_count: usize,
}

/// Trait for device discovery.
pub trait DeviceDiscovery {
    /// Returns the backend name.
    fn backend_name(&self) -> BackendName;

    /// Returns discovered devices.
    fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError>;

    /// Returns current runtime health.
    fn health(&self) -> RuntimeHealth;
}

/// Trait for backend allocators.
pub trait Allocator {
    /// Concrete buffer type.
    type Buffer: BufferHandle;

    /// Allocates a buffer for a tensor spec.
    fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError>;
}

/// Trait for graph execution.
pub trait ExecutionBackend {
    /// Concrete buffer type.
    type Buffer: BufferHandle;

    /// Executes a compiled plan with host-supplied inputs.
    fn execute(
        &mut self,
        plan: &ExecutionPlan,
        inputs: &BTreeMap<TensorId, Self::Buffer>,
    ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError>;
}

/// Execution result containing output buffers and basic metrics.
#[derive(Clone, Debug, PartialEq)]
pub struct ExecutionResult<B> {
    /// Materialized outputs by tensor ID.
    pub outputs: BTreeMap<TensorId, B>,
    /// Runtime metrics for the execution.
    pub metrics: ExecutionMetrics,
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use mox_core::{DType, Device, Shape, TensorSpec};
    use mox_ir::{ExecutionOp, ExecutionPlan, ExecutionStep};
    use serde_json::json;

    use super::{
        Allocator, AmdBackendReport, AmdDeviceMetadata, AmdDriverBinding, AmdOptInStatus,
        AmdRecoveryAction, AmdRecoveryProfile, AmdRiskLevel, AmdRiskProfile, AmdRuntimeMode,
        AmdTopologyInfo, ArtifactReadPath, BackendSelection, BufferHandle, BufferResidency,
        BufferStorageKind, DEFAULT_PENALTY_LOOKBACK, DeviceDescriptor, DeviceDiscovery,
        ExecutionBackend, ExecutionMetrics, ExecutionResult, HealthStatus, KvCacheAccounting,
        KvCacheDeviceScope, KvCachePageLayout, KvCachePolicy, KvCacheSpillPolicy, KvCacheState,
        LoadedModelResidency, LoadedModelState, ModelArtifactBlobKind, ModelArtifactStorage,
        ModelArtifactStorageKind, PagedTensorStoragePlan, PrefixCacheIdentity,
        PrefixCacheReusePolicy, PrefixCacheState, QuantizationExecution, QuantizationLoadPath,
        QuantizationSupport, RuntimeError, RuntimeHealth, SamplingPolicy, SamplingStrategy,
        TokenSampler, apply_sampling_penalties,
    };

    #[derive(Clone, Debug, PartialEq, Eq)]
    struct MockBuffer {
        spec: TensorSpec,
    }

    impl BufferHandle for MockBuffer {
        fn spec(&self) -> &TensorSpec {
            &self.spec
        }
    }

    struct MockRuntime;

    impl DeviceDiscovery for MockRuntime {
        fn backend_name(&self) -> super::BackendName {
            "mock"
        }

        fn discover_devices(&self) -> Result<Vec<DeviceDescriptor>, RuntimeError> {
            Ok(vec![DeviceDescriptor {
                backend: String::from("mock"),
                device: Device::cpu(),
                device_name: Some(String::from("mock cpu")),
                supported_dtypes: vec![DType::F32],
                supported_quantization: vec![QuantizationSupport {
                    mode: mox_core::QuantizationMode::None,
                    load_path: QuantizationLoadPath::DenseF32,
                    execution: QuantizationExecution::Native,
                }],
                memory_capacity_bytes: None,
                unified_memory: Some(true),
                feature_flags: vec![String::from("mock_execution")],
                amd_metadata: None,
            }])
        }

        fn health(&self) -> RuntimeHealth {
            RuntimeHealth {
                status: HealthStatus::Ready,
                message: String::from("ready"),
            }
        }
    }

    impl Allocator for MockRuntime {
        type Buffer = MockBuffer;

        fn allocate(&mut self, spec: &TensorSpec) -> Result<Self::Buffer, RuntimeError> {
            Ok(MockBuffer { spec: spec.clone() })
        }
    }

    impl ExecutionBackend for MockRuntime {
        type Buffer = MockBuffer;

        fn execute(
            &mut self,
            plan: &ExecutionPlan,
            _inputs: &BTreeMap<mox_core::TensorId, Self::Buffer>,
        ) -> Result<ExecutionResult<Self::Buffer>, RuntimeError> {
            Ok(ExecutionResult {
                outputs: BTreeMap::new(),
                metrics: ExecutionMetrics {
                    steps_executed: plan.steps.len(),
                },
            })
        }
    }

    #[test]
    fn mock_runtime_reports_device_and_executes_plan() -> Result<(), RuntimeError> {
        let mut runtime = MockRuntime;
        let devices = runtime.discover_devices()?;
        if devices.len() != 1 {
            return Err(RuntimeError::Backend(format!(
                "expected 1 discovered device, found {}",
                devices.len()
            )));
        }
        if runtime.health().status != HealthStatus::Ready {
            return Err(RuntimeError::Backend(String::from(
                "expected mock runtime health to be ready",
            )));
        }

        let spec = TensorSpec::new(Shape::new(vec![1, 2]), DType::F32, Device::cpu());
        let buffer = runtime.allocate(&spec)?;
        let mut inputs = BTreeMap::new();
        inputs.insert(mox_core::TensorId(0), buffer);

        let plan = ExecutionPlan {
            graph_digest: String::from("digest"),
            steps: vec![ExecutionStep {
                output: mox_core::TensorId(1),
                op: ExecutionOp::Add,
                spec: TensorSpec::new(Shape::new(vec![1, 2]), DType::F32, Device::cpu()),
                inputs: vec![mox_core::TensorId(0)],
            }],
            outputs: vec![mox_core::TensorId(1)],
        };

        let result = runtime.execute(&plan, &inputs)?;
        if result.metrics.steps_executed != 1 {
            return Err(RuntimeError::Backend(format!(
                "expected 1 executed step, found {}",
                result.metrics.steps_executed
            )));
        }
        Ok(())
    }

    #[test]
    fn backend_selection_helpers_capture_direct_and_fallback_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let direct = BackendSelection::from_backend(&MockRuntime, &["input", "matmul"])?;
        assert_eq!(direct.requested_backend, "mock");
        assert_eq!(direct.effective_backend, "mock");
        assert_eq!(
            direct.supported_ops,
            vec![String::from("input"), String::from("matmul")]
        );
        assert!(direct.fallback_reason.is_none());
        assert_eq!(
            serde_json::to_value(&direct)?,
            json!({
                "requested_backend": "mock",
                "effective_backend": "mock",
                "selected_device": {
                    "backend": "mock",
                    "device": {
                        "kind": "Cpu",
                        "ordinal": 0,
                        "label": "cpu:0"
                    },
                    "device_name": "mock cpu",
                    "supported_dtypes": ["F32"],
                    "supported_quantization": [{
                        "mode": "none",
                        "load_path": "dense_f32",
                        "execution": "native"
                    }],
                    "memory_capacity_bytes": null,
                    "unified_memory": true,
                    "feature_flags": ["mock_execution"]
                },
                "supported_ops": ["input", "matmul"],
                "fallback_reason": null
            })
        );

        let fallback = BackendSelection::fallback_to_backend(
            "metal",
            &MockRuntime,
            &["input", "matmul"],
            "metal backend unavailable: offline",
        )?;
        assert_eq!(fallback.requested_backend, "metal");
        assert_eq!(fallback.effective_backend, "mock");
        assert_eq!(
            fallback.fallback_reason.as_deref(),
            Some("metal backend unavailable: offline")
        );
        Ok(())
    }

    #[test]
    fn quantization_support_surfaces_storage_path_and_pending_execution_truth()
    -> Result<(), Box<dyn std::error::Error>> {
        let support = QuantizationSupport {
            mode: mox_core::QuantizationMode::GgmlQ4_0,
            load_path: QuantizationLoadPath::BackendQuantized,
            execution: QuantizationExecution::DequantizeToF32,
        };

        assert_eq!(
            serde_json::to_value(&support)?,
            json!({
                "mode": "ggml_q4_0",
                "load_path": "backend_quantized",
                "execution": "dequantize_to_f32"
            })
        );
        Ok(())
    }

    #[test]
    fn buffer_handles_can_distinguish_quantized_storage_from_dequantized_fallback() {
        #[derive(Clone, Debug, PartialEq, Eq)]
        struct QuantizedMockBuffer {
            spec: TensorSpec,
        }

        impl BufferHandle for QuantizedMockBuffer {
            fn spec(&self) -> &TensorSpec {
                &self.spec
            }

            fn storage_kind(&self) -> BufferStorageKind {
                BufferStorageKind::QuantizedBlocks {
                    mode: mox_core::QuantizationMode::GgmlQ8_0,
                    layout: mox_core::QuantizedBlockLayout::new(32, 34, 2),
                    residency: BufferResidency::Backend,
                }
            }
        }

        #[derive(Clone, Debug, PartialEq, Eq)]
        struct DequantizedMockBuffer {
            spec: TensorSpec,
        }

        impl BufferHandle for DequantizedMockBuffer {
            fn spec(&self) -> &TensorSpec {
                &self.spec
            }

            fn storage_kind(&self) -> BufferStorageKind {
                BufferStorageKind::DequantizedF32 {
                    source_quantization: mox_core::QuantizationMode::GgmlQ8_0,
                }
            }
        }

        let spec = TensorSpec::new(Shape::new(vec![64]), DType::F32, Device::cpu());
        let quantized = QuantizedMockBuffer { spec: spec.clone() };
        let dequantized = DequantizedMockBuffer { spec };

        assert_eq!(
            quantized.storage_kind(),
            BufferStorageKind::QuantizedBlocks {
                mode: mox_core::QuantizationMode::GgmlQ8_0,
                layout: mox_core::QuantizedBlockLayout::new(32, 34, 2),
                residency: BufferResidency::Backend,
            }
        );
        assert_eq!(
            dequantized.storage_kind(),
            BufferStorageKind::DequantizedF32 {
                source_quantization: mox_core::QuantizationMode::GgmlQ8_0,
            }
        );
    }

    #[test]
    fn runtime_model_storage_truth_distinguishes_paged_blobs_from_copies()
    -> Result<(), Box<dyn std::error::Error>> {
        let copy = ModelArtifactStorage::in_memory_copy("weights.gguf", "abcd");
        let paged = ModelArtifactStorage::paged_local_blob(
            "weights.gguf",
            "abcd",
            ModelArtifactBlobKind::OllamaBlob,
            ArtifactReadPath::MemoryMapped,
            4096,
            Some(String::from("mmap preferred and available")),
        );

        assert_eq!(copy.storage_kind, ModelArtifactStorageKind::InMemoryCopy);
        assert_eq!(
            serde_json::to_value(&copy)?,
            json!({
                "artifact_name": "weights.gguf",
                "artifact_sha256": "abcd",
                "storage_kind": "in_memory_copy"
            })
        );
        assert_eq!(paged.storage_kind, ModelArtifactStorageKind::PagedLocalBlob);
        assert_eq!(
            serde_json::to_value(&paged)?,
            json!({
                "artifact_name": "weights.gguf",
                "artifact_sha256": "abcd",
                "storage_kind": "paged_local_blob",
                "blob_kind": "ollama_blob",
                "read_path": "memory_mapped",
                "page_size": 4096,
                "fallback_reason": "mmap preferred and available"
            })
        );
        Ok(())
    }

    #[test]
    fn paged_tensor_storage_plan_serializes_byte_window_and_page_counts()
    -> Result<(), Box<dyn std::error::Error>> {
        let plan = PagedTensorStoragePlan {
            tensor_name: String::from("blk.0.attn_q.weight"),
            artifact_name: String::from("weights.gguf"),
            byte_offset: 8192,
            byte_length: 16384,
            page_size: 4096,
            page_count: 4,
        };

        assert_eq!(
            serde_json::to_value(&plan)?,
            json!({
                "tensor_name": "blk.0.attn_q.weight",
                "artifact_name": "weights.gguf",
                "byte_offset": 8192,
                "byte_length": 16384,
                "page_size": 4096,
                "page_count": 4
            })
        );
        Ok(())
    }

    #[test]
    fn loaded_model_residency_tracks_keepalive_and_request_activity() {
        let mut residency = LoadedModelResidency::loading(1_000, 5_000);
        assert_eq!(residency.state, LoadedModelState::Loading);
        assert_eq!(residency.expires_at_millis, Some(6_000));

        residency.mark_ready(1_500);
        assert_eq!(residency.state, LoadedModelState::Ready);
        assert_eq!(residency.expires_at_millis, Some(6_500));

        residency.begin_request(2_000);
        assert_eq!(residency.active_requests, 1);
        assert_eq!(residency.expires_at_millis, None);

        residency.finish_request(3_000);
        assert_eq!(residency.active_requests, 0);
        assert_eq!(residency.expires_at_millis, Some(8_000));
        assert!(!residency.is_expired(7_999));
        assert!(residency.is_expired(8_000));

        residency.refresh_keep_alive(0, 8_500);
        assert_eq!(residency.expires_at_millis, Some(8_500));
        assert!(residency.is_expired(8_500));
    }

    #[test]
    fn kv_page_layout_reports_page_and_byte_geometry() {
        let layout = KvCachePageLayout::new(9, 4, 32);
        assert_eq!(layout.page_bytes, 128);
        assert_eq!(layout.max_pages, 3);
        assert_eq!(layout.page_count_for_tokens(0), 0);
        assert_eq!(layout.page_count_for_tokens(1), 1);
        assert_eq!(layout.page_count_for_tokens(4), 1);
        assert_eq!(layout.page_count_for_tokens(5), 2);
        assert_eq!(layout.bytes_for_tokens(3), 96);
    }

    #[test]
    fn kv_cache_state_and_growth_serialize_stably() -> Result<(), Box<dyn std::error::Error>> {
        let policy = KvCachePolicy {
            device_scope: KvCacheDeviceScope::SameDeviceOnly,
            spill_policy: KvCacheSpillPolicy::RefuseNewPages,
            page_layout: KvCachePageLayout::new(8, 4, 64),
        };
        let before = KvCacheState::paged(&policy.page_layout, 3);
        let current = KvCacheState::paged(&policy.page_layout, 6);
        let accounting = KvCacheAccounting::from_states(&before, current.clone());

        assert_eq!(
            serde_json::to_value(&policy)?,
            json!({
                "device_scope": "same_device_only",
                "spill_policy": "refuse_new_pages",
                "page_layout": {
                    "max_context_tokens": 8,
                    "tokens_per_page": 4,
                    "bytes_per_token": 64,
                    "page_bytes": 256,
                    "max_pages": 2
                }
            })
        );
        assert_eq!(
            serde_json::to_value(&accounting)?,
            json!({
                "current": {
                    "tokens": 6,
                    "bytes": 384,
                    "pages": 2
                },
                "growth": {
                    "tokens": 3,
                    "bytes": 192,
                    "pages": 1
                }
            })
        );
        Ok(())
    }

    #[test]
    fn prefix_cache_identity_and_policy_serialize_stably() -> Result<(), Box<dyn std::error::Error>>
    {
        let policy = PrefixCacheReusePolicy {
            shared_across_sessions: true,
            shared_across_users: false,
            shared_across_models: false,
            shared_across_backends: false,
        };
        let identity = PrefixCacheIdentity {
            model_id: String::from("fixture-word-decoder-v0"),
            model_revision: String::from("v0"),
            weight_bundle_digest: String::from("bundle-digest"),
            tokenizer_family: String::from("fixture_wordpiece"),
            tokenizer_digest: Some(String::from("tokenizer-digest")),
            chat_template_digest: None,
            generation_defaults_digest: None,
            backend_compatibility: String::from("cpu"),
            prefix_digest: String::from("prefix-digest"),
            prefix_tokens: 3,
        };

        assert_eq!(
            serde_json::to_value(&policy)?,
            json!({
                "shared_across_sessions": true,
                "shared_across_users": false,
                "shared_across_models": false,
                "shared_across_backends": false
            })
        );
        assert_eq!(
            serde_json::to_value(&(PrefixCacheState::Hit, identity))?,
            json!([
                "hit",
                {
                    "model_id": "fixture-word-decoder-v0",
                    "model_revision": "v0",
                    "weight_bundle_digest": "bundle-digest",
                    "tokenizer_family": "fixture_wordpiece",
                    "tokenizer_digest": "tokenizer-digest",
                    "backend_compatibility": "cpu",
                    "prefix_digest": "prefix-digest",
                    "prefix_tokens": 3
                }
            ])
        );
        Ok(())
    }

    #[test]
    fn sampling_policy_serializes_supported_generation_controls()
    -> Result<(), Box<dyn std::error::Error>> {
        let policy = SamplingPolicy {
            strategy: SamplingStrategy::Sample,
            temperature: Some(0.7),
            top_k: Some(32),
            top_p: Some(0.85),
            repeat_penalty: Some(1.2),
            presence_penalty: Some(0.4),
            frequency_penalty: Some(0.3),
            seed: Some(17),
        };
        let encoded = serde_json::to_value(&policy)?;

        assert_eq!(encoded["strategy"], "sample");
        assert!((encoded["temperature"].as_f64().expect("temperature") - 0.7).abs() < 1e-6);
        assert_eq!(encoded["top_k"], 32);
        assert!((encoded["top_p"].as_f64().expect("top_p") - 0.85).abs() < 1e-6);
        assert!((encoded["repeat_penalty"].as_f64().expect("repeat_penalty") - 1.2).abs() < 1e-6);
        assert!(
            (encoded["presence_penalty"]
                .as_f64()
                .expect("presence_penalty")
                - 0.4)
                .abs()
                < 1e-6
        );
        assert!(
            (encoded["frequency_penalty"]
                .as_f64()
                .expect("frequency_penalty")
                - 0.3)
                .abs()
                < 1e-6
        );
        assert_eq!(encoded["seed"], 17);
        Ok(())
    }

    #[test]
    fn seeded_token_sampler_replays_draws() {
        let policy = SamplingPolicy {
            strategy: SamplingStrategy::Sample,
            temperature: Some(0.9),
            top_k: Some(3),
            top_p: Some(0.95),
            repeat_penalty: None,
            presence_penalty: None,
            frequency_penalty: None,
            seed: Some(42),
        };
        let logits = vec![3.0, 2.9, 2.8];
        let history = Vec::new();
        let mut left = TokenSampler::new(&policy);
        let mut right = TokenSampler::new(&policy);

        let left_draws = (0..4)
            .map(|_| left.select_next_token(&logits, &history).expect("sample"))
            .collect::<Vec<_>>();
        let right_draws = (0..4)
            .map(|_| right.select_next_token(&logits, &history).expect("sample"))
            .collect::<Vec<_>>();

        assert_eq!(left_draws, right_draws);
    }

    #[test]
    fn sampling_penalties_honor_the_bounded_lookback_window() {
        let policy = SamplingPolicy {
            strategy: SamplingStrategy::Greedy,
            temperature: None,
            top_k: None,
            top_p: None,
            repeat_penalty: Some(1.0),
            presence_penalty: Some(0.0),
            frequency_penalty: Some(1.0),
            seed: None,
        };
        let mut logits = vec![0.0, 10.0];
        let mut history = vec![1u32; DEFAULT_PENALTY_LOOKBACK];
        history.insert(0, 0);

        apply_sampling_penalties(&mut logits, &history, &policy);

        assert_eq!(logits[0], 0.0);
        assert_eq!(logits[1], 10.0 - (DEFAULT_PENALTY_LOOKBACK as f32));
    }

    #[test]
    fn amd_backend_model_serializes_mode_topology_risk_and_recovery()
    -> Result<(), Box<dyn std::error::Error>> {
        let device = DeviceDescriptor {
            backend: String::from("amd_userspace"),
            device: Device::new(
                mox_core::DeviceKind::AmdUserspace,
                0,
                Some(String::from("amd_userspace:0")),
            ),
            device_name: Some(String::from("AMD Radeon Test")),
            supported_dtypes: vec![DType::F32],
            supported_quantization: Vec::new(),
            memory_capacity_bytes: Some(24 * 1024 * 1024 * 1024),
            unified_memory: Some(false),
            feature_flags: vec![String::from("userspace_opt_in")],
            amd_metadata: Some(AmdDeviceMetadata {
                mode: AmdRuntimeMode::Userspace,
                topology: AmdTopologyInfo {
                    architecture: Some(String::from("gfx1100")),
                    pci_bdf: Some(String::from("0000:03:00.0")),
                    xcc_count: Some(1),
                    shader_engine_count: Some(4),
                    compute_unit_count: Some(60),
                    vram_bytes: Some(24 * 1024 * 1024 * 1024),
                    visible_vram_bytes: Some(16 * 1024 * 1024 * 1024),
                },
                risk: AmdRiskProfile {
                    level: AmdRiskLevel::Elevated,
                    requires_explicit_opt_in: true,
                    may_unbind_kernel_driver: true,
                    warnings: vec![String::from(
                        "userspace mode may require unloading or rebinding amdgpu",
                    )],
                },
                recovery: AmdRecoveryProfile {
                    driver_binding: AmdDriverBinding::UserspaceClaimed,
                    expected_actions: vec![
                        AmdRecoveryAction::ProcessRestart,
                        AmdRecoveryAction::RebindKernelDriver,
                    ],
                },
            }),
        };
        let report = AmdBackendReport {
            mode: AmdRuntimeMode::Userspace,
            opt_in: AmdOptInStatus::Enabled,
            devices: vec![device],
            health: RuntimeHealth {
                status: HealthStatus::Degraded,
                message: String::from("amdgpu is still loaded; userspace mode not yet ready"),
            },
        };

        assert_eq!(
            serde_json::to_value(&report)?,
            json!({
                "mode": "userspace",
                "opt_in": "enabled",
                "devices": [{
                    "backend": "amd_userspace",
                    "device": {
                        "kind": "AmdUserspace",
                        "ordinal": 0,
                        "label": "amd_userspace:0"
                    },
                    "device_name": "AMD Radeon Test",
                    "supported_dtypes": ["F32"],
                    "supported_quantization": [],
                    "memory_capacity_bytes": 25769803776u64,
                    "unified_memory": false,
                    "feature_flags": ["userspace_opt_in"],
                    "amd_metadata": {
                        "mode": "userspace",
                        "topology": {
                            "architecture": "gfx1100",
                            "pci_bdf": "0000:03:00.0",
                            "xcc_count": 1,
                            "shader_engine_count": 4,
                            "compute_unit_count": 60,
                            "vram_bytes": 25769803776u64,
                            "visible_vram_bytes": 17179869184u64
                        },
                        "risk": {
                            "level": "elevated",
                            "requires_explicit_opt_in": true,
                            "may_unbind_kernel_driver": true,
                            "warnings": [
                                "userspace mode may require unloading or rebinding amdgpu"
                            ]
                        },
                        "recovery": {
                            "driver_binding": "userspace_claimed",
                            "expected_actions": ["process_restart", "rebind_kernel_driver"]
                        }
                    }
                }],
                "health": {
                    "status": "Degraded",
                    "message": "amdgpu is still loaded; userspace mode not yet ready"
                }
            })
        );
        Ok(())
    }
}
