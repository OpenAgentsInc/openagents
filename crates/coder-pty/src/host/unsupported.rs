//! The platform half where no PTY is implemented. Every spawn refuses, so
//! the host reports `unavailable` instead of pretending to open a terminal.

use std::io;
use std::process::Command;
use std::time::Duration;

use crate::wire::{SignalKind, Size};

// The reader loop matches on these; no value is ever produced here.
#[allow(dead_code)]
pub(super) enum Read {
    Data(usize),
    Timeout,
    Eof,
}

#[derive(Clone, Copy, Debug)]
pub(super) struct Status {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

/// No value of this type exists: nothing can be spawned.
pub(super) enum Process {}

pub(super) fn spawn(_command: Command, _size: Size) -> io::Result<Process> {
    Err(io::Error::new(
        io::ErrorKind::Unsupported,
        "terminal sessions need a Unix PTY; this platform has no implementation",
    ))
}

impl Process {
    pub(super) fn group(&self) -> i32 {
        match *self {}
    }
    pub(super) fn read(&self, _buffer: &mut [u8], _wait: Duration) -> Read {
        match *self {}
    }
    pub(super) fn write(&self, _data: &[u8]) -> io::Result<usize> {
        match *self {}
    }
    pub(super) fn resize(&self, _size: Size) -> io::Result<()> {
        match *self {}
    }
    pub(super) fn signal_foreground(&self, _kind: SignalKind) -> io::Result<()> {
        match *self {}
    }
    pub(super) fn hang_up(&self) {
        match *self {}
    }
    pub(super) fn kill(&self) {
        match *self {}
    }
    pub(super) fn try_wait(&self) -> Option<Status> {
        match *self {}
    }
}

pub(super) fn random_id() -> String {
    use std::hash::{BuildHasher as _, Hasher as _};
    let seed = std::collections::hash_map::RandomState::new();
    (0..4u64)
        .map(|index| {
            let mut hasher = seed.build_hasher();
            hasher.write_u64(index);
            format!("{:016x}", hasher.finish())
        })
        .collect()
}
