//! Ordered-state, scheduling, and topology substrate over `psionic-net`.

mod benchmark_receipts;
mod layer_sharded;
mod ordered_state;
mod pipeline_sharded;
mod replicated_serving;
mod scheduler;
mod serving_policy;
mod tensor_sharded;

pub use benchmark_receipts::*;
pub use layer_sharded::*;
pub use ordered_state::*;
pub use pipeline_sharded::*;
pub use psionic_net::*;
pub use replicated_serving::*;
pub use scheduler::*;
pub use serving_policy::*;
pub use tensor_sharded::*;

/// Human-readable crate ownership summary.
pub const CRATE_ROLE: &str = "ordered-state and topology substrate over psionic-net";
