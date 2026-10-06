//! Verse application composition over the shared admitted-frame renderer.
pub use verse_pbr::imported::*;
pub mod chamber;
#[cfg(all(feature = "remote-chamber", any(test, feature = "chamber-loopback")))]
pub mod chamber_loopback;
#[cfg(feature = "remote-chamber")]
pub mod chamber_session;
#[cfg(feature = "remote-chamber")]
pub mod character_panel;
pub mod characters;
pub mod combat;
pub mod controls;
pub mod crypt_fight;
#[cfg(all(feature = "remote-chamber", feature = "imported-desktop"))]
pub mod giver_panel;
pub mod icons;
pub mod inventory;
mod meteor_swarm;
pub mod original;
pub mod overlay;
pub mod play;
pub mod props;
#[cfg(feature = "remote-chamber")]
pub mod remote_content;
#[cfg(all(feature = "remote-chamber", feature = "imported-desktop"))]
pub mod remote_record;
#[cfg(all(feature = "remote-chamber", feature = "imported-desktop"))]
pub mod remote_window;

#[cfg(test)]
mod locomotion_gpu_tests;
