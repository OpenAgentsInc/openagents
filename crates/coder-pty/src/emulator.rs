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

use crate::wire::Size;

/// One terminal's authoritative emulator.
pub trait Emulator: Send {
    /// Applies output the terminal wrote, in order, and answers what it
    /// caused. Parsing must stay bounded for any input.
    fn output(&mut self, bytes: &[u8]) -> Effects;
    /// The terminal changed size.
    fn resize(&mut self, size: Size);
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

/// Makes the emulator for a terminal that opens at a size.
pub type Factory = Arc<dyn Fn(Size) -> Box<dyn Emulator> + Send + Sync>;
