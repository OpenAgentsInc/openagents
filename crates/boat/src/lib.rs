//! Async access to the Boat public API (formerly Ascii Box).
//!
//! You construct a [`Client`], fill an operation's parameter type, and await its
//! method. Reads, and creates or forks that carry an `Idempotency-Key`, are
//! retried on 429 and 5xx; every other request, including every command, runs
//! once.

#![doc = include_str!("../README.md")]

mod api;
pub mod auth;
mod client;
mod error;
pub mod exec;
pub mod follow;
mod helpers;
pub mod models;
mod nullable;
pub mod webhook;

pub use auth::ApiKey;
pub use client::{Client, ClientBuilder, Download, RetryPolicy};
pub use error::{ApiError, Error, Result};
pub use exec::{CommandFrame, CommandOutput, CommandStream};
pub use follow::{CommandFollower, OutputCursor, Signal, shell_quote};
pub use helpers::{Cancellation, EventStream, WaitOptions};
pub use nullable::Nullable;

/// The default API base: our own Boat-compatible service on Google Cloud
/// (`crates/oa-boat`, Cloud Run `oa-boat`, #11256). `BOAT_API_BASE`
/// overrides it.
pub const BASE_URL: &str = "https://oa-boat-157437760789.us-central1.run.app/api/v1";
/// Boat's own hosted API (boat.dev). Off unless `BOAT_HOSTED=1`
/// ([`auth::HOSTED_ENV`]); a client refuses a boat.dev base without it.
pub const HOSTED_BASE_URL: &str = "https://boat.dev/api/v1";
/// The legacy hosted Box base, which Boat still answers. The SDK never
/// targets it.
pub const LEGACY_BASE_URL: &str = "https://boat.dev/api/box/v1";
/// The pinned outbound operation inventory.
pub const OPERATIONS: &str = include_str!("../schema/operations.json");
/// SHA-256 of `schema/boat-v1.yaml`, fetched from
/// `https://docs.boat.dev/openapi/boat-v1.yaml` on 2026-10-02.
pub const SPEC_SHA256: &str = "84fe785eeb24d4bb0ff3b7cb2d3d36dd24c98483fcb55b38e880a1314b4b55bc";
