//! The player controller: WoW-style movement over a flat world. It lives in
//! `verse_world::social::controller`, so a social world's authority moves
//! hosted avatars under the same rules; this module re-exports it.

pub use verse_world::social::controller::*;
