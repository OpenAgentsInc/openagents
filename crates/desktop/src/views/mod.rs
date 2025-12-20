//! Maud views for the desktop app

mod autopilot;
mod home;
mod layout;
mod projects;
mod sessions;

pub use autopilot::autopilot_page;
pub use home::{counter_fragment, home_page};
pub use layout::layout;
