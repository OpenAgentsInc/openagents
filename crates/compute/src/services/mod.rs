//! Services for the compute provider

pub mod dvm_service;
pub mod relay_service;
// TODO: Enable when Spark SDK is integrated
// pub mod wallet_service;

pub use dvm_service::{DvmConfig, DvmError, DvmService};
pub use relay_service::{RelayService, RelayServiceApi};
