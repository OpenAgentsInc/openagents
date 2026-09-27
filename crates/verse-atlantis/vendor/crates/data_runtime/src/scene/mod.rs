//! Scene/zone data loaders.
//!
//! v0 focuses on destructible instances declared in scene data. Higher-level
//! registries and replication use these CPU records to compute world-space
//! bounds and assemble authoritative entities on the server.

pub mod destructibles;
