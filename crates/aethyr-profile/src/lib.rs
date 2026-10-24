//! The Aethyr device profile and detail controller (ADR 0007).
//!
//! A host probes a [`DeviceProfile`]: what the GPU, display, input, power,
//! network, and the person's preferences allow. It is probed, never guessed
//! from a user agent. [`Budget::for_profile`] turns it into what a session may
//! spend per frame. A [`Controller`] watches measured frame times (GPU
//! timestamps when the host has them, frame pacing otherwise) and moves one
//! [`Detail`] value between a floor and 1, with hysteresis so it does not
//! oscillate. A [`KnobSet`] maps `Detail` to each subsystem's setting through
//! a [`Curve`] and a hard floor. Preferences are limits: reduced motion and
//! reduced transparency hold their knobs at the floor whatever `Detail` is,
//! and never move `Detail` itself.
//!
//! The web engine mirrors the controller and the knob mapping in TypeScript.
//! `fixtures/vectors.json` holds cases both implementations run, so the two
//! can't drift; `docs/aethyr/device-profile.md` explains why it is a mirror
//! rather than a Wasm build.

mod controller;
mod knob;
mod profile;
mod report;

pub use controller::{Controller, ControllerConfig, ControllerState, Pressure, Sample, Source};
pub use knob::{Curve, Detail, Knob, KnobDecl, KnobError, KnobSet, KnobValue, css_variables};
pub use profile::{
    Budget, DeviceProfile, DisplayInfo, GpuApi, GpuClass, GpuInfo, InputKinds, MemoryInfo,
    NetworkInfo, Pointer, PowerState, Thermal, UserPrefs,
};
pub use report::DetailReport;

/// The schema name every device profile carries.
pub const PROFILE_SCHEMA: &str = "aethyr.profile.v1";
/// The schema name every detail report carries.
pub const REPORT_SCHEMA: &str = "aethyr.detail-report.v1";

/// Round to six decimal places, as the TypeScript mirror does, so repeated
/// steps land on the same value in both languages.
pub(crate) fn round6(value: f64) -> f64 {
    (value * 1e6).round() / 1e6
}

#[cfg(test)]
mod tests;
