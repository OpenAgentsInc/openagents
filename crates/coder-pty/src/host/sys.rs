//! The Unix half: a real PTY, a child that leads its own session and
//! process group, and the few system calls a terminal needs.
//!
//! # Why `libc` and not a PTY crate
//!
//! `openpty(3)`, `setsid(2)`, `TIOCSCTTY`, `TIOCSWINSZ`, `tcgetpgrp(3)`,
//! `poll(2)`, and `killpg(2)` are the whole requirement, and `libc` already
//! provides them to `crates/supervise`. A PTY crate would add a second
//! process-spawning path beside the supervisor's conventions, plus
//! Windows console code this crate refuses to use. `docs/dependencies.md`
//! records the choice.

use std::io;
use std::os::fd::{AsRawFd, FromRawFd, OwnedFd};
use std::os::unix::process::{CommandExt as _, ExitStatusExt as _};
use std::process::{Child, Command, Stdio};
use std::sync::Mutex;
use std::time::{Duration, Instant};

use crate::wire::{SignalKind, Size};

// glibc before 2.34 keeps `openpty` in libutil; later releases keep an
// empty libutil for compatibility, so linking it is correct on both.
#[cfg(all(target_os = "linux", target_env = "gnu"))]
#[link(name = "util")]
unsafe extern "C" {}

/// Serializes PTY allocation within this process.
static OPENING: Mutex<()> = Mutex::new(());

/// How long one input write may wait for the terminal to accept bytes.
const WRITE_WAIT: Duration = Duration::from_secs(1);

/// What one read of the PTY produced.
pub(super) enum Read {
    Data(usize),
    Timeout,
    /// Every holder of the terminal side closed it.
    Eof,
}

/// How the child ended: an exit code or a signal number.
#[derive(Clone, Copy, Debug)]
pub(super) struct Status {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

/// One terminal's process and the controlling side of its PTY.
pub(super) struct Process {
    master: OwnedFd,
    child: Mutex<Child>,
    group: i32,
}

/// Starts `command` on a new PTY of `size`.
///
/// The child calls `setsid`, so it leads a new session and a new process
/// group whose identifier is its process identifier, then takes the PTY as
/// its controlling terminal. The caller must not have set a process group
/// on the command: `setsid` refuses in a process that already leads one.
pub(super) fn spawn(mut command: Command, size: Size) -> io::Result<Process> {
    let window = winsize(size);
    let mut master = -1;
    let mut terminal = -1;
    // macOS implements `openpty` with `ptsname`, which returns a static
    // buffer, so two concurrent calls can race and fail. Opens in this
    // process go one at a time, through the spawn, which also keeps one
    // terminal's child from inheriting another's PTY before close-on-exec
    // is set.
    let _one_at_a_time = OPENING
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    // SAFETY: `openpty` writes two descriptors into the integers it is
    // handed; the name buffer and terminal settings are null, which it
    // permits, and the window size is a valid `winsize`.
    let opened = unsafe {
        libc::openpty(
            &mut master,
            &mut terminal,
            std::ptr::null_mut(),
            std::ptr::null_mut(),
            &window as *const libc::winsize as *mut libc::winsize,
        )
    };
    if opened != 0 {
        return Err(step("openpty", io::Error::last_os_error()));
    }
    // SAFETY: `openpty` succeeded, so both descriptors are open and owned
    // by nothing else in this process.
    let (master, terminal) =
        unsafe { (OwnedFd::from_raw_fd(master), OwnedFd::from_raw_fd(terminal)) };
    // A fork elsewhere in this process between `openpty` and these calls
    // could still inherit the PTY; closing on exec narrows that window.
    close_on_exec(&master)?;
    close_on_exec(&terminal)?;
    nonblocking(&master)?;

    command
        .stdin(Stdio::from(terminal.try_clone()?))
        .stdout(Stdio::from(terminal.try_clone()?))
        .stderr(Stdio::from(terminal));
    // SAFETY: the closure runs in the child between `fork` and `exec` and
    // calls only `setsid` and `ioctl`, which are async-signal-safe. Standard
    // input is already the PTY's terminal side when it runs.
    unsafe {
        command.pre_exec(|| {
            if libc::setsid() == -1 {
                return Err(io::Error::last_os_error());
            }
            if libc::ioctl(0, libc::TIOCSCTTY as _, 0) == -1 {
                return Err(io::Error::last_os_error());
            }
            Ok(())
        });
    }
    let child = command.spawn().map_err(|error| step("spawn", error))?;
    // The command holds the parent's copies of the terminal side; they must
    // close, or the PTY never reports end of file.
    drop(command);
    let group = i32::try_from(child.id())
        .map_err(|_| io::Error::other("process identifier out of range"))?;
    Ok(Process {
        master,
        child: Mutex::new(child),
        group,
    })
}

impl Process {
    /// The process group, which is also the session and the child's
    /// process identifier.
    pub(super) fn group(&self) -> i32 {
        self.group
    }

    /// Reads output, waiting at most `wait` for some.
    pub(super) fn read(&self, buffer: &mut [u8], wait: Duration) -> Read {
        let mut poll = libc::pollfd {
            fd: self.master.as_raw_fd(),
            events: libc::POLLIN,
            revents: 0,
        };
        let millis = libc::c_int::try_from(wait.as_millis()).unwrap_or(libc::c_int::MAX);
        // SAFETY: one valid `pollfd` for a descriptor this process owns.
        let ready = unsafe { libc::poll(&mut poll, 1, millis) };
        if ready == 0 {
            return Read::Timeout;
        }
        if ready < 0 {
            return match io::Error::last_os_error().kind() {
                io::ErrorKind::Interrupted => Read::Timeout,
                _ => Read::Eof,
            };
        }
        if poll.revents & libc::POLLIN == 0 {
            return Read::Eof;
        }
        // SAFETY: the buffer is valid for `buffer.len()` bytes.
        let read = unsafe {
            libc::read(
                self.master.as_raw_fd(),
                buffer.as_mut_ptr().cast(),
                buffer.len(),
            )
        };
        match read {
            n if n > 0 => Read::Data(n as usize),
            0 => Read::Eof,
            _ => match io::Error::last_os_error().kind() {
                io::ErrorKind::WouldBlock | io::ErrorKind::Interrupted => Read::Timeout,
                // Linux reports EIO once the terminal side has closed.
                _ => Read::Eof,
            },
        }
    }

    /// Writes input, waiting up to [`WRITE_WAIT`] for the terminal to take
    /// it. Returns how many bytes it took.
    pub(super) fn write(&self, data: &[u8]) -> io::Result<usize> {
        let deadline = Instant::now() + WRITE_WAIT;
        let mut written = 0;
        while written < data.len() {
            let rest = &data[written..];
            // SAFETY: `rest` is valid for `rest.len()` bytes.
            let n =
                unsafe { libc::write(self.master.as_raw_fd(), rest.as_ptr().cast(), rest.len()) };
            if n > 0 {
                written += n as usize;
                continue;
            }
            let error = io::Error::last_os_error();
            match error.kind() {
                io::ErrorKind::Interrupted => continue,
                io::ErrorKind::WouldBlock if Instant::now() < deadline => {
                    let mut poll = libc::pollfd {
                        fd: self.master.as_raw_fd(),
                        events: libc::POLLOUT,
                        revents: 0,
                    };
                    // SAFETY: one valid `pollfd`.
                    unsafe { libc::poll(&mut poll, 1, 50) };
                }
                io::ErrorKind::WouldBlock => break,
                _ if written > 0 => break,
                _ => return Err(error),
            }
        }
        Ok(written)
    }

    /// Changes the PTY's size. The kernel signals `SIGWINCH` to the
    /// terminal's foreground process group.
    pub(super) fn resize(&self, size: Size) -> io::Result<()> {
        let window = winsize(size);
        // SAFETY: a valid `winsize` for a descriptor this process owns.
        let result =
            unsafe { libc::ioctl(self.master.as_raw_fd(), libc::TIOCSWINSZ as _, &window) };
        if result == -1 {
            Err(io::Error::last_os_error())
        } else {
            Ok(())
        }
    }

    /// Sends a client's signal to the terminal's foreground process group,
    /// or to the session's own group when the terminal reports none.
    pub(super) fn signal_foreground(&self, kind: SignalKind) -> io::Result<()> {
        // SAFETY: reads one descriptor this process owns.
        let foreground = unsafe { libc::tcgetpgrp(self.master.as_raw_fd()) };
        let group = if foreground > 0 {
            foreground
        } else {
            self.group
        };
        let signal = match kind {
            SignalKind::Interrupt => libc::SIGINT,
            SignalKind::Quit => libc::SIGQUIT,
            SignalKind::Terminate => libc::SIGTERM,
            SignalKind::Hangup => libc::SIGHUP,
            SignalKind::Kill => libc::SIGKILL,
        };
        if signal_group(group, signal) {
            Ok(())
        } else {
            Err(io::Error::last_os_error())
        }
    }

    /// Asks the session to end: `SIGHUP`, as a closed terminal window
    /// sends, then `SIGTERM`, and `SIGCONT` so a stopped job can act on
    /// them.
    pub(super) fn hang_up(&self) {
        signal_group(self.group, libc::SIGHUP);
        signal_group(self.group, libc::SIGTERM);
        signal_group(self.group, libc::SIGCONT);
    }

    /// Ends the session's process group, whatever it is doing.
    pub(super) fn kill(&self) {
        signal_group(self.group, libc::SIGKILL);
    }

    /// Whether any process, including an unreaped zombie, is still in the
    /// session's process group.
    pub(super) fn group_running(&self) -> bool {
        // Signal zero checks existence and sends nothing.
        signal_group(self.group, 0)
    }

    /// The child's status once it has exited; reaps it.
    pub(super) fn try_wait(&self) -> Option<Status> {
        let mut child = self
            .child
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        match child.try_wait() {
            Ok(Some(status)) => Some(Status {
                code: status.code(),
                signal: status.signal(),
            }),
            Ok(None) => None,
            Err(_) => Some(Status {
                code: None,
                signal: None,
            }),
        }
    }
}

/// Names the step an error came from.
fn step(step: &str, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{step}: {error}"))
}

/// Sends `signal` to every process in `group`.
fn signal_group(group: i32, signal: i32) -> bool {
    if group <= 0 {
        return false;
    }
    // SAFETY: `killpg` reads two integers. The group is one this host made
    // for a terminal's child, so no process outside the terminal is in it.
    unsafe { libc::killpg(group, signal) == 0 }
}

fn winsize(size: Size) -> libc::winsize {
    libc::winsize {
        ws_row: size.rows,
        ws_col: size.cols,
        ws_xpixel: 0,
        ws_ypixel: 0,
    }
}

fn close_on_exec(fd: &OwnedFd) -> io::Result<()> {
    // SAFETY: `fcntl` on a descriptor this process owns.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFD) };
    // SAFETY: as above.
    if flags == -1
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFD, flags | libc::FD_CLOEXEC) } == -1
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

fn nonblocking(fd: &OwnedFd) -> io::Result<()> {
    // SAFETY: `fcntl` on a descriptor this process owns.
    let flags = unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_GETFL) };
    // SAFETY: as above.
    if flags == -1
        || unsafe { libc::fcntl(fd.as_raw_fd(), libc::F_SETFL, flags | libc::O_NONBLOCK) } == -1
    {
        return Err(io::Error::last_os_error());
    }
    Ok(())
}

/// A common ID from the system's random source.
pub(super) fn random_id() -> String {
    use std::io::Read as _;
    let mut bytes = [0u8; 32];
    let filled =
        std::fs::File::open("/dev/urandom").and_then(|mut file| file.read_exact(&mut bytes));
    if filled.is_err() {
        // No random source: fall back to a process-local mix. The ID stays
        // unique within the host; authority never rests on it.
        let seed = std::collections::hash_map::RandomState::new();
        for (index, chunk) in bytes.chunks_mut(8).enumerate() {
            use std::hash::{BuildHasher as _, Hasher as _};
            let mut hasher = seed.build_hasher();
            hasher.write_usize(index);
            hasher.write_u128(Instant::now().elapsed().as_nanos());
            chunk.copy_from_slice(&hasher.finish().to_le_bytes());
        }
    }
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}
