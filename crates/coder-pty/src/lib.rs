//! Terminal sessions a host keeps for its enrolled devices, as NIP-TERM
//! (`nips/openagents/NIP-TERM.md`) specifies them.
//!
//! A device with the `terminal` right opens a terminal on a host, types
//! into it, resizes it, detaches, and reattaches from another device to see
//! the output it missed, bounded and in order. This crate has two halves:
//!
//! - `host` (the default `host` feature, Unix and Windows) owns real PTYs
//!   (a pseudoconsole on Windows, whose tree is a job object). Each
//!   terminal's process leads a session and process group of its own, as
//!   `crates/supervise` requires of every job; closing, idle expiry, and
//!   host shutdown end that group with `SIGHUP` and `SIGTERM`, then
//!   `SIGKILL` after `supervise::GRACE`, and reap the direct child. Output
//!   is cut into frames with a monotonic sequence number per terminal and
//!   held in a bounded [`ring::Ring`]; a reader that asks for frames the
//!   ring discarded is told so with a gap frame.
//! - [`client`] is portable: it applies frames in order, detects gaps and
//!   duplicates, and keeps a bounded plain-text screen buffer a renderer
//!   draws. It is not a terminal emulator.
//!
//! [`wire`] holds the NIP-TERM bodies both halves exchange. Authority is not
//! decided here: the host asks a `host::Rights` implementation, which the
//! resident host backs with NIP-HOST grants, and it delivers frames through
//! a `host::FrameSink`, which the resident host backs with a NIP-REACH
//! direct channel or private `3188` artifacts over an admitted relay.
//!
//! # Platform support
//!
//! The host needs a Unix PTY and Unix process groups, or on Windows a
//! pseudoconsole (ConPTY) and a job object. On another platform
//! `host::Host::open` refuses with `unavailable` rather than pretending;
//! the wire types and client state build everywhere.

pub mod client;
pub mod ext;
pub mod ring;
pub mod wire;

#[cfg(feature = "host")]
pub mod host;

pub use client::{Applied, Line, Screen, TerminalState};
pub use ring::Ring;
pub use wire::{
    Attach, Body, Cause, Close, Detach, Detached, EnvVar, Exit, Frame, Input, Launch, Mode, Open,
    Reason, Refusal, Resize, Signal, SignalKind, Size, Status, TerminalRef, TerminalResult, Value,
};
