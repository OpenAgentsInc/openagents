//! Domain types for the compute provider

pub mod earnings;
pub mod events;
pub mod identity;
pub mod job;

pub use earnings::EarningsTracker;
pub use events::DomainEvent;
pub use identity::UnifiedIdentity;
pub use job::{Job, JobStatus};
