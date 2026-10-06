//! The emulator a host runs for each terminal, so a terminal's side
//! effects have one owner (the effects feature, [`crate::ext::EFFECTS`]).
//!
//! The host feeds every output byte to its terminal's emulator before any
//! attachment sees it, writes the replies the program asked for back as
//! input, and turns bells, title and directory changes, and clipboard
//! writes into effect frames. A client that names the effects feature
//! stops answering queries and acting on effects it parses itself, so two
//! devices on one terminal send one reply and a reattach repeats nothing.
//!
//! This crate defines the seam and no emulator: `coder_vt::Authority`
//! implements it, and the resident host installs it with
//! `host::Config::emulator`.

use std::sync::Arc;

use crate::ext::Record;
use crate::wire::{Exit, Refusal, Size, TerminalRef};

/// One terminal's authoritative emulator.
pub trait Emulator: Send {
    /// Applies output the terminal wrote, in order, and answers what it
    /// caused. Parsing must stay bounded for any input.
    fn output(&mut self, bytes: &[u8]) -> Effects;
    /// The terminal changed size.
    fn resize(&mut self, size: Size);

    /// A snapshot stream of the state, which reflects every output byte of
    /// sequenced frames through `through`: the prefix through `READY`,
    /// history newest first, and `FINISH`. `None` when this emulator writes
    /// no snapshots.
    fn snapshot(
        &mut self,
        terminal: &TerminalRef,
        through: u64,
        exit: Option<Exit>,
    ) -> Option<Result<Vec<Record>, Refusal>> {
        let _ = (terminal, through, exit);
        None
    }

    /// A history stream of `rows` rows before absolute line `before` in
    /// line epoch `epoch`. `None` when this emulator writes no snapshots.
    fn history(
        &self,
        terminal: &TerminalRef,
        read: &HistoryRead,
    ) -> Option<Result<Vec<Record>, Refusal>> {
        let _ = (terminal, read);
        None
    }
}

/// A history read as an emulator answers it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryRead {
    /// The last sequenced frame the state reflects.
    pub through: u64,
    pub exit: Option<Exit>,
    pub epoch: u64,
    pub before: u64,
    pub rows: u64,
}

/// What a piece of output caused.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Effects {
    /// Replies the program asked for, such as its cursor position or device
    /// attributes. The host writes them to the terminal's input.
    pub replies: Vec<u8>,
    /// How many times the bell rang.
    pub bells: u32,
    /// The window title, when it changed.
    pub title: Option<String>,
    /// The working directory the shell reported, when it changed.
    pub directory: Option<String>,
    /// The last clipboard write the program asked for. A program can never
    /// read the clipboard.
    pub clipboard: Option<String>,
}

/// Makes each terminal's emulator.
pub trait Emulators: Send + Sync {
    /// The emulator for a terminal that opens at `size`.
    fn make(&self, size: Size) -> Box<dyn Emulator>;
    /// Whether its emulators write snapshot and history streams, so the
    /// host serves the snapshot feature.
    fn snapshots(&self) -> bool {
        false
    }
}

/// The emulators a host makes, shared.
pub type Factory = Arc<dyn Emulators>;
