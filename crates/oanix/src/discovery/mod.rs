//! Environment discovery modules.

pub mod compute;
pub mod hardware;
pub mod identity;
pub mod network;

pub use compute::discover_compute;
pub use hardware::discover_hardware;
pub use identity::discover_identity;
pub use network::discover_network;
