//! `oa-boat`: our own Boat-compatible sandbox service on Google Compute
//! Engine (#11256).
//!
//! It answers the part of Boat's v1 API that our code calls (see
//! `docs/cloud/oa-boat.md`) so the `boat` SDK, `boat-template`, the web
//! Environments and `openagents chat work --on boat` run unchanged against
//! VMs in our own project instead of boat.dev.

pub mod api;
pub mod gce;
pub mod remote;
pub mod service;
pub mod sizes;
pub mod testing;
pub mod time;

pub use service::{Config, Service};

/// This service's driver declaration (`GET /api/v1/capabilities`, #11624):
/// every conformance profile, and its size classes with the default first.
pub fn capabilities() -> computer_driver::Capabilities {
    let mut names: Vec<&str> = sizes::SIZES.iter().map(|s| s.name).collect();
    names.sort_by_key(|n| *n != "default");
    let mut caps =
        computer_driver::Capabilities::new("oa-boat", computer_driver::Profile::ALL, &names);
    caps.command_sessions = true;
    caps.shapes = sizes::SIZES
        .iter()
        .map(|size| {
            (
                size.name.into(),
                computer_driver::Shape {
                    vcpus: size.vcpu as u32,
                    memory_mib: size.memory_gb as u64 * 1024,
                    min_disk_gib: Some(size.disk_gb as u64),
                },
            )
        })
        .collect();
    caps
}
