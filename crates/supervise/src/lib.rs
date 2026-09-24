//! One subprocess supervisor: a job owns its process tree until cleanup
//! finishes, and its output is bounded while it is read.
//!
//! Every path in this workspace that runs a program — a shell command from
//! a plan, a delegation handed to an executor, a benchmark run of the agent
//! — wants the same two properties, and both of them are easy to get wrong
//! in the same way.
//!
//! # A deadline is an ownership contract, not a timer
//!
//! `timeout(wall, command.output())` cancels the *wait*. The command keeps
//! running, its descendants keep running, and nobody reaps any of them. The
//! audited workspace recorded `timed out` for a shell that wrote a file a
//! second later, and a delegate that was killed on its bound left a
//! background child that wrote one too. A timed-out job nobody killed then
//! made the next job time out as well, which is how one loose process
//! becomes a stalled fan-out.
//!
//! So a `Job` ends when its cleanup ends. On Unix the direct child is
//! spawned into a process group of its own, the deadline signals the
//! *group*, and the supervisor reaps the direct child before it returns:
//!
//! 1. `SIGTERM` to the group, so a program that cleans up gets to.
//! 2. Up to [`GRACE`] for the direct child to exit.
//! 3. `SIGKILL` to the group, and reap the direct child.
//!
//! A job that exits on its own is the same contract read the other way:
//! whatever is still in the group is a descendant that outlived the job it
//! belongs to, so the supervisor kills the group there too, without a grace
//! period it has nothing to negotiate about. That is also what ends a drain
//! on a pipe a grandchild is still holding open.
//!
//! **Cancellation is the same mechanism as a deadline.** The work runs in a
//! task of its own, and the future `Job::run` hands back holds the only
//! live end of a channel. Dropping that future — an aborted turn, a
//! `select!` branch that lost — closes the channel, and the supervisor
//! terminates the tree and reaps it exactly as a deadline does. A caller
//! that walks away does not leave a process behind.
//!
//! # Output is bounded while it is captured
//!
//! Truncating the string an `output()` call returns bounds what a trace
//! records and bounds nothing the machine has to hold: a producer that
//! writes a gigabyte in its allotted seconds is a gigabyte in memory first.
//! So stdout and stderr are drained concurrently into capped buffers, and
//! bytes past [`Limits::stream_max`] are counted and dropped rather than
//! kept. Draining continues after the cap, because a full pipe stops the
//! writer, and a stopped writer cannot answer its own deadline.
//!
//! Peak capture memory for one job is `2 × stream_max` plus two 8 KiB read
//! buffers, and one bounded copy per stream when the bytes become text at
//! the end. A [`Captured`] stream reports how many bytes it saw, whether it
//! was cut, and — this matters for a job that failed — what it managed to
//! print before it did.
//!
//! # Memory is bounded per job
//!
//! A job has a memory cap, [`MEMORY_MAX`] unless the caller or the
//! [`MEMORY_ENV`] variable says otherwise. On a host with a systemd user
//! manager the job runs in a transient scope whose cgroup holds its whole
//! tree to the cap, and a job the kernel killed for passing it is reported
//! as such in [`Ended::memory`], apart from one that crashed. Elsewhere each
//! process gets `RLIMIT_DATA` instead. The [`memory`] module has the design
//! and the reasons for it.
//!
//! # Watching a job while it runs
//!
//! [`Job::start`] is the same contract for a caller that reads standard
//! output as it arrives, writes to standard input, and stops the job
//! itself: see [`live`]. An executor session's host is that caller.
//!
//! # Features
//!
//! The default `job` feature provides asynchronous `Job` execution and
//! capture. Disable default features for the blocking API alone. Tests
//! that exercise `Job` require that feature; blocking tests run in either
//! configuration.
//!
//! # Platform support
//!
//! Unix only, and stated rather than assumed. Process-tree ownership here
//! is `process_group(0)` and `killpg`; no equivalent is implemented for
//! another platform, so this crate does not build on one.

use std::time::Duration;

#[cfg(not(unix))]
compile_error!(
    "supervise owns a job through Unix process groups; no other platform is implemented"
);

mod group;

pub mod memory;

#[cfg(feature = "job")]
mod job;

#[cfg(feature = "job")]
pub mod live;

pub mod blocking;

pub use group::{process_running, running};
#[cfg(feature = "job")]
pub use job::Job;
#[cfg(feature = "job")]
pub use live::{Delivery, Gap, Input, Live, Stopped};
pub use memory::{Enforcement, MEMORY_ENV, MEMORY_MAX, Memory};

/// How long a terminated job has to exit on `SIGTERM` before the group is
/// killed, and how long a drain has to reach the end of a pipe afterwards.
pub const GRACE: Duration = Duration::from_millis(250);

/// The bytes one stream keeps, unless a caller says otherwise.
pub const STREAM_MAX: usize = 64 * 1024;

/// How much one job may take: wall time, bytes per captured stream, and
/// memory.
#[derive(Clone, Copy, Debug)]
pub struct Limits {
    /// How long the job may run before the supervisor terminates its tree.
    pub wall: Duration,
    /// The bytes each of stdout and stderr keeps. Bytes past this are
    /// counted and dropped as they arrive.
    pub stream_max: usize,
    /// The memory the job's tree may take, in bytes, or `None` for no cap.
    pub memory_max: Option<u64>,
}

impl Limits {
    /// A job bounded in time, keeping [`STREAM_MAX`] bytes of each stream,
    /// under the default memory cap ([`memory::default_max`]).
    #[must_use]
    pub fn within(wall: Duration) -> Self {
        Limits {
            wall,
            stream_max: STREAM_MAX,
            memory_max: memory::default_max(),
        }
    }

    /// The same bound, keeping a different number of bytes per stream.
    #[must_use]
    pub fn keeping(mut self, stream_max: usize) -> Self {
        self.stream_max = stream_max;
        self
    }

    /// The same bound under a different memory cap, in bytes; `None` runs
    /// the job with no cap.
    #[must_use]
    pub fn memory(mut self, memory_max: Option<u64>) -> Self {
        self.memory_max = memory_max;
        self
    }
}

/// How a job ended.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Ending {
    /// The job finished on its own. The code is the process's, and `None`
    /// means a signal ended it.
    Exited(Option<i32>),
    /// The deadline passed, or the caller dropped the future, and the
    /// supervisor terminated the tree.
    TimedOut,
    /// The job never became a result: it would not spawn, or the wait
    /// failed. Nothing about this is the program's answer.
    Failed(String),
}

impl Ending {
    /// The exit code, when the job got far enough to have one.
    #[must_use]
    pub fn code(&self) -> Option<i32> {
        match self {
            Ending::Exited(code) => *code,
            _ => None,
        }
    }

    /// Whether the job exited zero.
    #[must_use]
    pub fn success(&self) -> bool {
        self.code() == Some(0)
    }
}

impl std::fmt::Display for Ending {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Ending::Exited(Some(code)) => write!(f, "exit {code}"),
            Ending::Exited(None) => write!(f, "stopped by a signal"),
            Ending::TimedOut => write!(f, "timed out"),
            Ending::Failed(why) => write!(f, "failed: {why}"),
        }
    }
}

/// One captured stream: what was kept, how much there was, and whether the
/// cap cut it.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Captured {
    /// The bytes that were kept, as text. Invalid UTF-8 inside them is
    /// replaced; an incomplete character at the end is dropped, so a cap
    /// that lands mid-character does not add a stray replacement.
    pub text: String,
    /// How many bytes the stream produced, including the ones the cap
    /// dropped.
    pub bytes: u64,
    /// Whether anything was dropped.
    pub truncated: bool,
}

impl Captured {
    /// The text with a truncation marker when there was more, which is the
    /// form a transcript or a trace records.
    #[must_use]
    pub fn marked(&self) -> String {
        match self.truncated {
            false => self.text.clone(),
            true => format!("{}\n…truncated, {} bytes in all", self.text, self.bytes),
        }
    }

    /// Whether the stream printed nothing.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.bytes == 0
    }
}

/// One finished job.
#[derive(Clone, Debug)]
pub struct Ended {
    /// How it ended.
    pub ending: Ending,
    /// Standard output, bounded.
    pub stdout: Captured,
    /// Standard error, bounded.
    pub stderr: Captured,
    /// Wall time from spawn to the end of cleanup.
    pub elapsed: Duration,
    /// The job's memory cap and whether the job ran into it, or `None` when
    /// it ran without one.
    pub memory: Option<Memory>,
}

impl Ended {
    /// Whether the kernel killed a process in the job for passing its
    /// memory cap. The ending then reads as a signal or an exit code, and
    /// this is what tells it apart from a crash.
    #[must_use]
    pub fn over_memory(&self) -> bool {
        self.memory.as_ref().is_some_and(|memory| memory.exceeded)
    }

    /// How many bytes the job wrote across both streams, before the caps.
    #[must_use]
    pub fn bytes(&self) -> u64 {
        self.stdout.bytes + self.stderr.bytes
    }

    /// Whether either stream was cut.
    #[must_use]
    pub fn truncated(&self) -> bool {
        self.stdout.truncated || self.stderr.truncated
    }
}

/// One stream being read, held to its cap as it arrives.
#[cfg(any(feature = "job", test))]
#[derive(Debug, Default)]
struct Sink {
    held: Vec<u8>,
    bytes: u64,
    max: usize,
}

#[cfg(any(feature = "job", test))]
impl Sink {
    fn new(max: usize) -> Self {
        Sink {
            held: Vec::new(),
            bytes: 0,
            max,
        }
    }

    /// Counts a chunk and keeps as much of it as the cap still allows.
    fn push(&mut self, chunk: &[u8]) {
        self.bytes += chunk.len() as u64;
        let room = self.max.saturating_sub(self.held.len());
        if room > 0 {
            self.held.extend_from_slice(&chunk[..room.min(chunk.len())]);
        }
    }

    /// The stream as a reader sees it.
    fn captured(&self) -> Captured {
        let kept = whole(&self.held);
        Captured {
            text: String::from_utf8_lossy(kept).into_owned(),
            bytes: self.bytes,
            truncated: self.bytes > kept.len() as u64,
        }
    }
}

/// The bytes up to the last complete character.
///
/// A cap lands where the producer happened to be, which is often the middle
/// of a multi-byte character. Dropping the incomplete tail loses at most
/// three bytes and keeps a replacement character out of text nobody
/// truncated on purpose.
#[cfg(any(feature = "job", test))]
fn whole(bytes: &[u8]) -> &[u8] {
    match std::str::from_utf8(bytes) {
        Ok(_) => bytes,
        Err(error) if error.error_len().is_none() => &bytes[..error.valid_up_to()],
        Err(_) => bytes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_sink_counts_everything_and_keeps_its_cap() {
        let mut sink = Sink::new(4);
        sink.push(b"abc");
        sink.push(b"defgh");
        let captured = sink.captured();
        assert_eq!(captured.text, "abcd");
        assert_eq!(captured.bytes, 8);
        assert!(captured.truncated);
        assert_eq!(captured.marked(), "abcd\n…truncated, 8 bytes in all");
    }

    #[test]
    fn a_stream_under_its_cap_is_whole() {
        let mut sink = Sink::new(16);
        sink.push("héllo".as_bytes());
        let captured = sink.captured();
        assert_eq!(captured.text, "héllo");
        assert!(!captured.truncated);
        assert_eq!(captured.marked(), "héllo");
    }

    #[test]
    fn a_cap_inside_a_character_drops_the_partial_one() {
        // "é" is two bytes, so a cap of four lands inside the second one.
        let mut sink = Sink::new(4);
        sink.push("aéé".as_bytes());
        let captured = sink.captured();
        assert_eq!(captured.text, "aé");
        assert!(!captured.text.contains('\u{fffd}'));
        assert!(captured.truncated);
    }

    #[test]
    fn invalid_bytes_inside_the_kept_text_are_replaced() {
        let mut sink = Sink::new(8);
        sink.push(&[b'a', 0xff, b'b']);
        assert_eq!(sink.captured().text, "a\u{fffd}b");
    }

    #[test]
    fn a_live_process_and_a_dead_one_probe_differently() {
        assert!(process_running(std::process::id()));
        assert!(!process_running(0));
        let mut child = std::process::Command::new("true").spawn().unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(!process_running(pid));
    }
}
