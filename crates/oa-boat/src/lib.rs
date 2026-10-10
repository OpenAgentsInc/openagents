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
pub mod time;

pub use service::{Config, Service};
