//! Shared voice presentation and, on native clients, the call lifecycle.
mod view;
pub use view::{Caption, SpeakerActivity, VoiceView, orb_state};

#[cfg(feature = "runtime")]
mod runtime;
#[cfg(feature = "runtime")]
pub use runtime::*;
