//! NIP-SKL: Agent Skill Registry.
//!
//! SKL core defines canonical skill identity and trust gating.
//! - `kind:33400` Skill Manifest (addressable)
//! - `kind:33401` Skill Version Log (regular)
//!
//! The module also includes trust/revocation helpers built on:
//! - NIP-32 labels (`kind:1985`)
//! - NIP-09 deletion requests (`kind:5`) with same-pubkey authority semantics

pub mod manifest;
pub mod revocation;
pub mod trust;
pub mod version_log;
pub mod yaml_derivation;

pub use manifest::*;
pub use revocation::*;
pub use trust::*;
pub use version_log::*;
pub use yaml_derivation::*;
