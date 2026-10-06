//! The Windows half: a pseudoconsole (ConPTY), a child created inside a
//! job object of its own, and the few calls a terminal needs.
//!
//! - **Terminal.** `CreatePseudoConsole` over two anonymous pipes; the
//!   child is attached to it through `PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE`
//!   and reads and writes the console, which the host sees as VT text on
//!   the pipes.
//! - **Tree.** The child is created suspended, joins a job object that
//!   kills every process in it when the job closes, and only then runs, so
//!   everything it starts is the terminal's, as a Unix session's process
//!   group is. [`Process::kill`] ends the job.
//! - **Hang-up.** Closing the pseudoconsole sends every attached program
//!   `CTRL_CLOSE_EVENT`, which is what a closed console window sends and
//!   what an interactive shell exits on; it is the counterpart of
//!   `SIGHUP`. It also ends the output pipe, so the reader sees end of
//!   file.
//! - **Signals.** An interrupt is a Ctrl+C byte (`0x03`) typed into the
//!   console, which the console turns into `CTRL_C_EVENT` for the programs
//!   attached to it, and a quit is `0x1c` (Ctrl+\). A hang-up or terminate
//!   is the hang-up above, and a kill ends the job.
//!
//! The command's program, arguments, working directory, and environment
//! are read back from the prepared `Command` ([`super::cmdline`] builds
//! the strings). The environment is exactly the variables the command
//! sets: the host clears it and adds the allowlist, so there is nothing
//! inherited to merge.

use std::ffi::{OsStr, OsString};
use std::io;
use std::process::Command;
use std::sync::Mutex;
use std::time::{Duration, Instant};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_BROKEN_PIPE, ERROR_NO_DATA, HANDLE, INVALID_HANDLE_VALUE, WAIT_OBJECT_0,
};
use windows_sys::Win32::Security::Cryptography::{
    BCRYPT_USE_SYSTEM_PREFERRED_RNG, BCryptGenRandom,
};
use windows_sys::Win32::Storage::FileSystem::{ReadFile, WriteFile};
use windows_sys::Win32::System::Console::{
    COORD, ClosePseudoConsole, CreatePseudoConsole, HPCON, ResizePseudoConsole,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
    QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
};
use windows_sys::Win32::System::Pipes::{CreatePipe, PeekNamedPipe};
use windows_sys::Win32::System::Threading::{
    CREATE_SUSPENDED, CREATE_UNICODE_ENVIRONMENT, CreateProcessW, DeleteProcThreadAttributeList,
    EXTENDED_STARTUPINFO_PRESENT, GetExitCodeProcess, InitializeProcThreadAttributeList,
    LPPROC_THREAD_ATTRIBUTE_LIST, PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE, PROCESS_INFORMATION,
    ResumeThread, STARTF_USESTDHANDLES, STARTUPINFOEXW, TerminateProcess,
    UpdateProcThreadAttribute, WaitForSingleObject,
};

use super::cmdline;
use crate::wire::{SignalKind, Size};

/// How long one input write may wait; `WriteFile` on the console's input
/// pipe blocks only while the console is not reading, which it always is.
const PEEK_EVERY: Duration = Duration::from_millis(5);

/// The exit code a process in an ended terminal reports.
const ENDED: u32 = 1;

/// What one read of the console produced.
pub(super) enum Read {
    Data(usize),
    Timeout,
    /// The console closed its output: the pseudoconsole is gone.
    Eof,
}

/// How the child ended. Windows has exit codes and no signals.
#[derive(Clone, Copy, Debug)]
pub(super) struct Status {
    pub code: Option<i32>,
    pub signal: Option<i32>,
}

/// A handle this module owns and closes.
#[derive(Debug)]
struct Owned(HANDLE);

// SAFETY: a kernel handle is a process-wide value any thread may use; the
// calls made through these are thread-safe.
unsafe impl Send for Owned {}
// SAFETY: as above.
unsafe impl Sync for Owned {}

impl Owned {
    fn new(handle: HANDLE) -> io::Result<Self> {
        if handle.is_null() || handle == INVALID_HANDLE_VALUE {
            Err(io::Error::last_os_error())
        } else {
            Ok(Owned(handle))
        }
    }
}

impl Drop for Owned {
    fn drop(&mut self) {
        // SAFETY: open, owned by this value, closed once.
        unsafe { CloseHandle(self.0) };
    }
}

/// A pseudoconsole, closed on a thread of its own: before Windows 11,
/// `ClosePseudoConsole` waits for the output pipe to be drained, and the
/// thread that drains it may be the one asking.
struct Console(HPCON);

// SAFETY: a pseudoconsole handle may be resized and closed from any thread.
unsafe impl Send for Console {}

impl Console {
    fn close(self) {
        let console = self.0;
        std::mem::forget(self);
        let closing = std::thread::Builder::new()
            .name("coder-pty-close".into())
            .spawn(move || {
                // SAFETY: the pseudoconsole is open and closed once, here.
                unsafe { ClosePseudoConsole(console) };
            });
        if closing.is_err() {
            // SAFETY: as above; no thread to spare, so it is closed here.
            unsafe { ClosePseudoConsole(console) };
        }
    }
}

impl Drop for Console {
    fn drop(&mut self) {
        Console(self.0).close();
    }
}

/// One terminal's process and the host's side of its pseudoconsole.
pub(super) struct Process {
    /// Writes into the console's input.
    input: Owned,
    /// Reads the console's output.
    output: Owned,
    console: Mutex<Option<Console>>,
    process: Owned,
    job: Owned,
    pid: u32,
}

/// Starts `command` on a new pseudoconsole of `size`.
pub(super) fn spawn(command: Command, size: Size) -> io::Result<Process> {
    let program = command.get_program().to_owned();
    let args: Vec<&OsStr> = command.get_args().collect();
    let line = cmdline::command_line(&program, &args).map_err(io::Error::other)?;
    let vars: Vec<(OsString, OsString)> = command
        .get_envs()
        .filter_map(|(name, value)| value.map(|value| (name.to_owned(), value.to_owned())))
        .collect();
    let environment = cmdline::environment_block(&vars).map_err(io::Error::other)?;
    let directory = command
        .get_current_dir()
        .map(|dir| wide(cmdline::plain_directory(&dir.to_string_lossy())));
    drop(command);

    let (input_read, input) = pipe()?;
    let (output, output_write) = pipe()?;
    let mut console: HPCON = 0;
    // SAFETY: two open pipe ends and a size; `console` receives the handle.
    let made =
        unsafe { CreatePseudoConsole(coord(size), input_read.0, output_write.0, 0, &mut console) };
    if made < 0 {
        return Err(step(
            "CreatePseudoConsole",
            io::Error::from_raw_os_error(made),
        ));
    }
    let console = Console(console);
    // The console holds its own duplicates; the host's copies of the ends
    // it was given must close, or the output never ends.
    drop((input_read, output_write));

    let job = job()?;
    let mut attributes = Attributes::new(console.0)?;
    let mut startup = STARTUPINFOEXW::default();
    startup.StartupInfo.cb = size_of::<STARTUPINFOEXW>() as u32;
    // No standard handles of the host's reach the child, even when the
    // host's own are redirected: the console provides them.
    startup.StartupInfo.dwFlags = STARTF_USESTDHANDLES;
    startup.StartupInfo.hStdInput = INVALID_HANDLE_VALUE;
    startup.StartupInfo.hStdOutput = INVALID_HANDLE_VALUE;
    startup.StartupInfo.hStdError = INVALID_HANDLE_VALUE;
    startup.lpAttributeList = attributes.list();
    let mut line = wide(&line);
    let mut info = PROCESS_INFORMATION::default();
    // SAFETY: a writable, NUL-terminated command line; a NUL-terminated
    // UTF-16 environment block; an optional NUL-terminated directory; and a
    // startup structure whose attribute list outlives the call.
    let created = unsafe {
        CreateProcessW(
            std::ptr::null(),
            line.as_mut_ptr(),
            std::ptr::null(),
            std::ptr::null(),
            0,
            CREATE_SUSPENDED | EXTENDED_STARTUPINFO_PRESENT | CREATE_UNICODE_ENVIRONMENT,
            environment.as_ptr().cast(),
            directory
                .as_ref()
                .map_or(std::ptr::null(), |dir| dir.as_ptr()),
            &startup.StartupInfo,
            &mut info,
        )
    };
    if created == 0 {
        return Err(step("CreateProcessW", io::Error::last_os_error()));
    }
    drop(attributes);
    let process = Owned(info.hProcess);
    let thread = Owned(info.hThread);
    // SAFETY: an open job and a suspended process that has run nothing.
    if unsafe { AssignProcessToJobObject(job.0, process.0) } == 0 {
        let error = io::Error::last_os_error();
        // SAFETY: the process handle is open; it never ran.
        unsafe { TerminateProcess(process.0, ENDED) };
        return Err(step("AssignProcessToJobObject", error));
    }
    // SAFETY: the process's one thread, created suspended.
    if unsafe { ResumeThread(thread.0) } == u32::MAX {
        let error = io::Error::last_os_error();
        // SAFETY: the job is open.
        unsafe { TerminateJobObject(job.0, ENDED) };
        return Err(step("ResumeThread", error));
    }
    Ok(Process {
        input,
        output,
        console: Mutex::new(Some(console)),
        process,
        job,
        pid: info.dwProcessId,
    })
}

impl Process {
    /// The child's process identifier, which stands for the terminal's
    /// tree where Unix reports a process group.
    pub(super) fn group(&self) -> i32 {
        i32::try_from(self.pid).unwrap_or(i32::MAX)
    }

    /// Reads output, waiting at most `wait` for some.
    pub(super) fn read(&self, buffer: &mut [u8], wait: Duration) -> Read {
        let deadline = Instant::now() + wait;
        loop {
            let mut available = 0u32;
            // SAFETY: a peek that copies nothing and reports the bytes ready.
            let peeked = unsafe {
                PeekNamedPipe(
                    self.output.0,
                    std::ptr::null_mut(),
                    0,
                    std::ptr::null_mut(),
                    &mut available,
                    std::ptr::null_mut(),
                )
            };
            if peeked == 0 {
                return Read::Eof;
            }
            if available > 0 {
                let want = buffer.len().min(available as usize) as u32;
                let mut read = 0u32;
                // SAFETY: the buffer is valid for `want` bytes, which are
                // ready, so the read does not block.
                let ok = unsafe {
                    ReadFile(
                        self.output.0,
                        buffer.as_mut_ptr(),
                        want,
                        &mut read,
                        std::ptr::null_mut(),
                    )
                };
                return match (ok, read) {
                    (0, _) | (_, 0) => Read::Eof,
                    (_, read) => Read::Data(read as usize),
                };
            }
            if Instant::now() >= deadline {
                return Read::Timeout;
            }
            std::thread::sleep(PEEK_EVERY);
        }
    }

    /// Writes input into the console. Returns how many bytes it took.
    pub(super) fn write(&self, data: &[u8]) -> io::Result<usize> {
        let mut written = 0;
        while written < data.len() {
            let rest = &data[written..];
            let mut wrote = 0u32;
            let chunk = rest.len().min(u32::MAX as usize) as u32;
            // SAFETY: `rest` is valid for `chunk` bytes.
            let ok = unsafe {
                WriteFile(
                    self.input.0,
                    rest.as_ptr(),
                    chunk,
                    &mut wrote,
                    std::ptr::null_mut(),
                )
            };
            if ok == 0 {
                let error = io::Error::last_os_error();
                let closed = matches!(
                    error.raw_os_error().map(|code| code as u32),
                    Some(ERROR_BROKEN_PIPE | ERROR_NO_DATA)
                );
                if written > 0 {
                    break;
                }
                return Err(if closed {
                    io::Error::new(io::ErrorKind::BrokenPipe, "the console has closed")
                } else {
                    error
                });
            }
            written += wrote as usize;
        }
        Ok(written)
    }

    /// Changes the console's size; the programs attached to it see a
    /// window-size event.
    pub(super) fn resize(&self, size: Size) -> io::Result<()> {
        let console = self
            .console
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        let Some(console) = console.as_ref() else {
            return Err(io::Error::new(
                io::ErrorKind::NotConnected,
                "the console has closed",
            ));
        };
        // SAFETY: an open pseudoconsole.
        let resized = unsafe { ResizePseudoConsole(console.0, coord(size)) };
        if resized < 0 {
            Err(io::Error::from_raw_os_error(resized))
        } else {
            Ok(())
        }
    }

    /// Sends a client's signal: Ctrl+C or Ctrl+\ typed into the console, a
    /// hang-up, or the end of the job.
    pub(super) fn signal_foreground(&self, kind: SignalKind) -> io::Result<()> {
        match kind {
            SignalKind::Interrupt => self.write(&[0x03]).map(drop),
            SignalKind::Quit => self.write(&[0x1c]).map(drop),
            SignalKind::Terminate | SignalKind::Hangup => {
                self.hang_up();
                Ok(())
            }
            SignalKind::Kill => {
                self.kill();
                Ok(())
            }
        }
    }

    /// Asks the session to end: closes the pseudoconsole, which sends its
    /// programs `CTRL_CLOSE_EVENT` and ends the output.
    pub(super) fn hang_up(&self) {
        let console = self
            .console
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner)
            .take();
        if let Some(console) = console {
            console.close();
        }
    }

    /// Ends every process in the terminal's job, and closes the console so
    /// the output reaches its end.
    pub(super) fn kill(&self) {
        // SAFETY: the job handle is open for as long as `self` is.
        unsafe { TerminateJobObject(self.job.0, ENDED) };
        self.hang_up();
    }

    /// Whether any process is still in the terminal's job.
    pub(super) fn group_running(&self) -> bool {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: a basic accounting structure of the size passed, and an
        // open job.
        let queried = unsafe {
            QueryInformationJobObject(
                self.job.0,
                JobObjectBasicAccountingInformation,
                (&raw mut accounting).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        };
        queried != 0 && accounting.ActiveProcesses > 0
    }

    /// The child's status once it has exited.
    pub(super) fn try_wait(&self) -> Option<Status> {
        // SAFETY: an open process handle; a zero wait does not block.
        if unsafe { WaitForSingleObject(self.process.0, 0) } != WAIT_OBJECT_0 {
            return None;
        }
        let mut code = 0u32;
        // SAFETY: an open process handle that has ended.
        let read = unsafe { GetExitCodeProcess(self.process.0, &mut code) } != 0;
        Some(Status {
            code: read.then_some(code as i32),
            signal: None,
        })
    }
}

impl Drop for Process {
    fn drop(&mut self) {
        // Nothing of the terminal outlives the host's hold on it; the job's
        // handle closing kills what is left, and the console closes after.
        // SAFETY: the job is open.
        unsafe { TerminateJobObject(self.job.0, ENDED) };
    }
}

/// A process-thread attribute list naming one pseudoconsole.
struct Attributes {
    storage: Vec<u64>,
}

impl Attributes {
    fn new(console: HPCON) -> io::Result<Self> {
        let mut bytes = 0usize;
        // SAFETY: a size query with no list.
        unsafe { InitializeProcThreadAttributeList(std::ptr::null_mut(), 1, 0, &mut bytes) };
        if bytes == 0 {
            return Err(step(
                "InitializeProcThreadAttributeList",
                io::Error::last_os_error(),
            ));
        }
        let mut attributes = Attributes {
            storage: vec![0u64; bytes.div_ceil(8)],
        };
        // SAFETY: storage of at least `bytes` bytes, aligned for the list.
        if unsafe { InitializeProcThreadAttributeList(attributes.list(), 1, 0, &mut bytes) } == 0 {
            // Nothing to delete: the list never initialised.
            let error = io::Error::last_os_error();
            attributes.storage.clear();
            return Err(step("InitializeProcThreadAttributeList", error));
        }
        // SAFETY: an initialised list; the pseudoconsole attribute takes the
        // handle's value itself, as `HPCON`'s size says.
        let updated = unsafe {
            UpdateProcThreadAttribute(
                attributes.list(),
                0,
                PROC_THREAD_ATTRIBUTE_PSEUDOCONSOLE as usize,
                console as *const std::ffi::c_void,
                size_of::<HPCON>(),
                std::ptr::null_mut(),
                std::ptr::null(),
            )
        };
        if updated == 0 {
            return Err(step(
                "UpdateProcThreadAttribute",
                io::Error::last_os_error(),
            ));
        }
        Ok(attributes)
    }

    fn list(&mut self) -> LPPROC_THREAD_ATTRIBUTE_LIST {
        self.storage.as_mut_ptr().cast()
    }
}

impl Drop for Attributes {
    fn drop(&mut self) {
        if !self.storage.is_empty() {
            // SAFETY: an initialised list, deleted once.
            unsafe { DeleteProcThreadAttributeList(self.list()) };
        }
    }
}

/// A job object that kills what is left in it when it closes.
fn job() -> io::Result<Owned> {
    // SAFETY: an unnamed job with default security.
    let job = Owned::new(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) })
        .map_err(|error| step("CreateJobObjectW", error))?;
    let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
    limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
    // SAFETY: an extended limit structure of the size passed.
    let set = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectExtendedLimitInformation,
            (&raw const limits).cast(),
            size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
        )
    };
    if set == 0 {
        return Err(step("SetInformationJobObject", io::Error::last_os_error()));
    }
    Ok(job)
}

/// An anonymous pipe: its read end and write end, neither inheritable.
fn pipe() -> io::Result<(Owned, Owned)> {
    let (mut read, mut write): (HANDLE, HANDLE) = (std::ptr::null_mut(), std::ptr::null_mut());
    // SAFETY: two outputs and no security attributes (not inheritable).
    if unsafe { CreatePipe(&mut read, &mut write, std::ptr::null(), 0) } == 0 {
        return Err(step("CreatePipe", io::Error::last_os_error()));
    }
    Ok((Owned(read), Owned(write)))
}

fn coord(size: Size) -> COORD {
    let clamp = |value: u16| i16::try_from(value.max(1)).unwrap_or(i16::MAX);
    COORD {
        X: clamp(size.cols),
        Y: clamp(size.rows),
    }
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Names the step an error came from.
fn step(step: &str, error: io::Error) -> io::Error {
    io::Error::new(error.kind(), format!("{step}: {error}"))
}

/// A common ID from the system's random source.
pub(super) fn random_id() -> String {
    let mut bytes = [0u8; 32];
    // SAFETY: no algorithm handle with the system-preferred flag, and a
    // buffer of the length passed.
    let status = unsafe {
        BCryptGenRandom(
            std::ptr::null_mut(),
            bytes.as_mut_ptr(),
            bytes.len() as u32,
            BCRYPT_USE_SYSTEM_PREFERRED_RNG,
        )
    };
    if status < 0 {
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

pub(super) fn process_cwd(_: i32) -> Option<String> {
    None
}
