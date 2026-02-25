//! Services for the compute provider

pub mod dvm_service;
pub mod relay_service;
// Wallet service remains split until Spark integration lands in this crate.
// pub mod wallet_service;

pub use dvm_service::{DvmConfig, DvmError, DvmService};
pub use relay_service::{RelayService, RelayServiceApi};
