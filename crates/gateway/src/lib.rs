//! The keyed HTTP gateway: the one admission path in front of decision
//! doors.
//!
//! Every `POST /v1/systemone` through this service runs the same
//! sequence — authenticate the bearer key to its tenant, authorize the
//! named door against the registry, bound the request's shape and the
//! door's capacity, reserve the tenant's quota durably, verify the
//! backend publishes the artifact the binding pins, forward the request,
//! settle the reservation, and leave a receipt. Nothing reaches a door
//! any other way: a directly reachable backend is a deployment
//! misconfiguration, not a second path.
//!
//! # What the response promises
//!
//! A forwarded answer arrives with `x-request-id`, `x-attempt`,
//! `x-outcome`, and `x-receipt` headers — the receipt's digest, which
//! resolves to a sealed record in `receipts.jsonl` beside the registry.
//! The gateway's own refusals are typed JSON: a stable `code` plus the
//! request and attempt they name. The backend's answer and typed
//! refusals pass through untouched — the gateway authenticates,
//! authorizes, meters, and witnesses; it does not edit the door's words.
//!
//! # What it does not do
//!
//! Discovery lists the doors a caller's tenant may name; it is the
//! registry's word, not proof of remote weights. Receipts are the
//! serving process's attributable claim — not attestation. And
//! monetary admission — `money` in the config — is an explicit opt-in:
//! absent the field, no workspace is charged and no balance route
//! exists. The same rule governs `accounts`: absent the block, no
//! account, session, or workspace-management route mounts and a `sess_`
//! token is not a credential the service knows.

// The module's helpers answer `Result<_, Response>`: the refusal is the
// response itself, built once and returned to the caller — boxing it on
// the heap would rename the same shape without changing what it is.
#[allow(clippy::result_large_err)]
pub mod accounts;
pub mod advertise;
#[allow(clippy::result_large_err)]
pub mod billing;
pub mod classify;
pub mod config;
#[allow(clippy::result_large_err)]
pub mod dashboard;
pub mod discovery;
pub mod jobs;
pub mod money;
pub mod relay_worker;
pub mod serve;
#[allow(clippy::result_large_err)]
pub mod usage;
