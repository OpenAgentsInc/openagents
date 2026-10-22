//! The Grid's build billboard (#12442): NIP-DEPLOY statuses and livehost
//! pointers folded into a staging lane and a production lane, and the
//! board's line art and live pieces for the Grid's renderer to draw.
//!
//! The crate is pure: a host hands it the events its relay connection
//! received ([`Ingest::event`]) and its clock, and asks for the pieces to
//! draw ([`layout::pieces`]). `crates/everglade-web/src/build_board.rs`
//! turns the art into models and the pieces into instances.

pub mod font;
pub mod layout;
pub mod pointer;
pub mod state;

use nostr::deploy;
use nostr::domain::Event;

pub use state::Board;

/// The newest phase events a board asks the relay for.
pub const HISTORY_EVENTS: usize = 80;

/// The NIP-01 filters a board subscribes with: every lane head, the newest
/// phase events, and the trusted livehost pointers.
#[must_use]
pub fn filters() -> Vec<serde_json::Value> {
    let mut filters = deploy::filters(deploy::ANNOUNCER, HISTORY_EVENTS);
    filters.push(pointer::filter());
    filters
}

/// A board and the checks between it and the relay.
#[derive(Debug)]
pub struct Ingest {
    pub board: Board,
    announcer: String,
    /// Events refused, for the page's diagnostics.
    pub refused: u64,
}

impl Ingest {
    /// A board that trusts `announcer` (normally [`deploy::ANNOUNCER`]);
    /// with `test`, it shows only test events.
    #[must_use]
    pub fn new(announcer: &str, test: bool) -> Self {
        Self {
            board: Board::new(test),
            announcer: announcer.to_owned(),
            refused: 0,
        }
    }

    /// Checks one event from the relay and folds it in. Returns whether the
    /// board changed.
    pub fn event(&mut self, event: &Event, wall: f64) -> bool {
        let now = wall.max(0.0) as u64;
        if event.kind == pointer::KIND {
            return match pointer::read(event, now) {
                Some(pointer) => self.board.apply_pointer(&pointer, wall),
                None => {
                    self.refused += 1;
                    false
                }
            };
        }
        match deploy::read(event, &self.announcer, now) {
            Ok(record) => self.board.apply(&record, wall),
            Err(_) => {
                self.refused += 1;
                false
            }
        }
    }
}

#[cfg(test)]
mod tests;
