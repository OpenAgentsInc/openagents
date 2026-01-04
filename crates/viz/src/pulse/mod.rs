//! Pulse primitives: discrete events

mod event;
mod heartbeat;
mod flash;

pub use event::EventMarker;
pub use heartbeat::Heartbeat;
pub use flash::Flash;
