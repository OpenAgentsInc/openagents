//! What a device can do, and what a session may spend on it.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::PROFILE_SCHEMA;

/// Probed once at startup and refreshed on platform events.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DeviceProfile {
    pub schema: String,
    pub gpu: GpuInfo,
    pub memory: MemoryInfo,
    pub display: DisplayInfo,
    pub input: InputKinds,
    pub power: PowerState,
    pub network: NetworkInfo,
    pub prefs: UserPrefs,
}

impl Default for DeviceProfile {
    fn default() -> Self {
        Self {
            schema: PROFILE_SCHEMA.to_owned(),
            gpu: GpuInfo::default(),
            memory: MemoryInfo::default(),
            display: DisplayInfo::default(),
            input: InputKinds::default(),
            power: PowerState::default(),
            network: NetworkInfo::default(),
            prefs: UserPrefs::default(),
        }
    }
}

/// The graphics interface the host reached.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuApi {
    #[default]
    None,
    Webgpu,
    Metal,
    Vulkan,
    D3d12,
}

/// A coarse class from the adapter's own limits. Provisional: ADR 0007 sets
/// the qualification classes later.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum GpuClass {
    /// No adapter.
    #[default]
    None,
    /// A fallback (CPU) adapter.
    Software,
    /// Meets the WebGPU default limits and little more.
    Baseline,
    /// Large textures and buffers: typically a recent laptop or desktop GPU.
    Extended,
}

/// Adapter identity, features, and limits as the graphics API reports them.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct GpuInfo {
    pub api: GpuApi,
    pub vendor: String,
    pub architecture: String,
    pub device: String,
    pub description: String,
    pub fallback: bool,
    pub class: GpuClass,
    /// Sorted feature names.
    pub features: Vec<String>,
    /// Limit names as the API spells them (`maxTextureDimension2D`).
    pub limits: BTreeMap<String, u64>,
}

impl GpuInfo {
    /// The class these limits put the adapter in.
    pub fn classify(&self) -> GpuClass {
        if self.api == GpuApi::None {
            return GpuClass::None;
        }
        if self.fallback {
            return GpuClass::Software;
        }
        let limit = |name: &str| self.limits.get(name).copied().unwrap_or(0);
        if limit("maxTextureDimension2D") >= 16_384 && limit("maxBufferSize") >= 1 << 30 {
            GpuClass::Extended
        } else {
            GpuClass::Baseline
        }
    }

    /// Whether the adapter can time GPU passes.
    pub fn timestamps(&self) -> bool {
        self.features.iter().any(|f| f == "timestamp-query")
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MemoryInfo {
    /// Approximate device memory in gigabytes, when the platform says.
    pub device_memory_gb: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisplayInfo {
    /// Viewport size in layout (CSS) pixels.
    pub width: u32,
    pub height: u32,
    /// Device pixels per layout pixel.
    pub scale: f64,
    /// Measured refresh rate, when the host measured one.
    pub refresh_hz: Option<f64>,
    pub hdr: Option<bool>,
}

impl Default for DisplayInfo {
    fn default() -> Self {
        Self {
            width: 0,
            height: 0,
            scale: 1.0,
            refresh_hz: None,
            hdr: None,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Pointer {
    #[default]
    None,
    Coarse,
    Fine,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct InputKinds {
    /// The primary pointer.
    pub pointer: Pointer,
    /// Whether the primary pointer hovers.
    pub hover: bool,
    pub any_fine: bool,
    pub any_coarse: bool,
    pub touch_points: u32,
    pub gamepad: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Thermal {
    #[default]
    Unknown,
    Nominal,
    Fair,
    Serious,
    Critical,
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PowerState {
    /// Battery charge from 0 to 1, when the platform says.
    pub battery_level: Option<f64>,
    pub charging: Option<bool>,
    /// The operating system's low-power mode, when the platform says.
    pub low_power: Option<bool>,
    pub thermal: Thermal,
}

impl PowerState {
    /// Running on a low battery, in low-power mode, or hot.
    pub fn constrained(&self) -> bool {
        let low_battery =
            self.charging == Some(false) && self.battery_level.is_some_and(|level| level < 0.2);
        low_battery || self.low_power == Some(true) || self.thermal >= Thermal::Serious
    }
}

#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct NetworkInfo {
    /// The Network Information API's `effectiveType` (`4g`, `3g`, ...).
    pub effective_type: Option<String>,
    pub downlink_mbps: Option<f64>,
    pub rtt_ms: Option<f64>,
    pub save_data: Option<bool>,
}

impl NetworkInfo {
    pub fn slow(&self) -> bool {
        self.save_data == Some(true)
            || matches!(
                self.effective_type.as_deref(),
                Some("slow-2g" | "2g" | "3g")
            )
    }
}

/// The person's settings. These are limits, never inputs to the controller.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct UserPrefs {
    pub reduced_motion: bool,
    pub reduced_transparency: bool,
    pub more_contrast: bool,
    /// Root text size over the platform default (1 is the default).
    pub text_scale: f64,
}

impl Default for UserPrefs {
    fn default() -> Self {
        Self {
            reduced_motion: false,
            reduced_transparency: false,
            more_contrast: false,
            text_scale: 1.0,
        }
    }
}

/// What the current session may spend.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Budget {
    /// Wall time per frame.
    pub frame_ms: f64,
    /// GPU time per frame, checked when the host has GPU timestamps.
    pub gpu_ms: f64,
    pub memory_mb: Option<u32>,
    pub download_mb: Option<u32>,
}

impl Budget {
    /// One frame at 60 Hz.
    pub const FULL_MS: f64 = 1000.0 / 60.0;
    /// One frame at 30 Hz, for constrained power.
    pub const CONSTRAINED_MS: f64 = 1000.0 / 30.0;
    /// The download allowance on a slow or data-saving network.
    pub const SLOW_DOWNLOAD_MB: u32 = 50;

    /// The starting budget for a profile. Provisional until the ADR 0007
    /// qualification sets one per device class: 60 Hz (or the display's
    /// rate when slower), 30 Hz under constrained power, GPU time at 80% of
    /// the frame, a quarter of device memory, and a download cap on slow
    /// networks.
    pub fn for_profile(profile: &DeviceProfile) -> Self {
        let refresh = profile
            .display
            .refresh_hz
            .filter(|hz| hz.is_finite() && *hz > 0.0)
            .unwrap_or(60.0)
            .clamp(24.0, 60.0);
        let frame_ms = if profile.power.constrained() {
            Self::CONSTRAINED_MS
        } else {
            (1000.0 / refresh).max(Self::FULL_MS)
        };
        let memory_mb = profile
            .memory
            .device_memory_gb
            .filter(|gb| gb.is_finite() && *gb > 0.0)
            .map(|gb| (gb * 1024.0 / 4.0).round() as u32);
        Self {
            frame_ms: crate::round6(frame_ms),
            gpu_ms: crate::round6(frame_ms * 0.8),
            memory_mb,
            download_mb: profile.network.slow().then_some(Self::SLOW_DOWNLOAD_MB),
        }
    }
}
