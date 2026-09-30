//! Windows: one job's tree is a job object.
//!
//! Windows has no process groups to signal, so the supervisor owns a job's
//! tree through a job object instead:
//!
//! - The direct child is created suspended, joins a job object of its
//!   own, and only then runs. Everything it starts afterwards is in the
//!   job, whatever process group or console it asks for, unless the job
//!   allows breaking away, which this one does not.
//! - The job kills every process in it when its last handle closes
//!   (`JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE`), so a supervisor that drops a
//!   job, or exits, leaves nothing behind.
//! - Ending the tree is `TerminateJobObject`. Windows has no `SIGTERM` for
//!   a program with no window: a console control event reaches only a
//!   process that shares the sender's console, and a supervised job runs
//!   with a console of its own and no window. So asking the tree to stop
//!   ends it at once, and a deadline has no grace period to offer.
//! - A job's memory cap is the job object's `JobMemoryLimit`, over the
//!   whole tree's committed memory; see [`crate::memory`].
//!
//! The child runs with `CREATE_NO_WINDOW`, so a console program started
//! by a host that has no console never opens a window on the desktop.

use std::io;
use std::os::windows::io::RawHandle;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};

use windows_sys::Win32::Foundation::{
    CloseHandle, ERROR_ACCESS_DENIED, HANDLE, INVALID_HANDLE_VALUE, WAIT_TIMEOUT,
};
use windows_sys::Win32::System::Diagnostics::ToolHelp::{
    CreateToolhelp32Snapshot, TH32CS_SNAPTHREAD, THREADENTRY32, Thread32First, Thread32Next,
};
use windows_sys::Win32::System::IO::{
    CreateIoCompletionPort, GetQueuedCompletionStatus, OVERLAPPED,
};
use windows_sys::Win32::System::JobObjects::{
    AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_JOB_MEMORY,
    JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE, JOBOBJECT_ASSOCIATE_COMPLETION_PORT,
    JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
    JobObjectAssociateCompletionPortInformation, JobObjectBasicAccountingInformation,
    JobObjectExtendedLimitInformation, QueryInformationJobObject, SetInformationJobObject,
    TerminateJobObject,
};
use windows_sys::Win32::System::SystemServices::JOB_OBJECT_MSG_JOB_MEMORY_LIMIT;
use windows_sys::Win32::System::Threading::{
    CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW, CREATE_SUSPENDED, OpenProcess, OpenThread,
    PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE, ResumeThread, THREAD_SUSPEND_RESUME,
    WaitForSingleObject,
};

/// The creation flags of a supervised job's direct child: suspended until
/// it is in its job object, in a process group of its own so a console's
/// Ctrl+C never reaches it, and with no console window.
pub(crate) const SUSPENDED: u32 = CREATE_SUSPENDED | CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;

/// The creation flags [`crate::blocking::own_group`] sets: the same, but
/// running, since a blocking caller's spawn returns before the job object
/// exists.
pub(crate) const RUNNING: u32 = CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;

/// The exit code a process in an ended job reports.
const ENDED: u32 = 1;

/// A handle this module owns and closes.
#[derive(Debug)]
struct Owned(HANDLE);

// SAFETY: a kernel handle is a process-wide value that any thread may use
// and close; nothing here gives out a borrowed pointer into it.
unsafe impl Send for Owned {}
// SAFETY: as above; every call made through it is thread-safe.
unsafe impl Sync for Owned {}

impl Owned {
    /// Takes `handle`, or the last error when the call that made it failed.
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
        // SAFETY: the handle is open and owned by this value alone.
        unsafe { CloseHandle(self.0) };
    }
}

/// One job object, before a process is in it.
#[derive(Debug)]
pub(crate) struct JobObject {
    job: Owned,
    /// The completion port the job reports a process at its memory cap on;
    /// only a job with a cap has one.
    port: Option<Owned>,
    /// Set once the port has reported the cap.
    exceeded: AtomicBool,
}

impl JobObject {
    /// A job object that kills what is left in it when it closes, and holds
    /// its processes' committed memory to `memory_max` bytes when that is
    /// set.
    pub(crate) fn new(memory_max: Option<u64>) -> io::Result<Self> {
        // SAFETY: no security attributes and no name make an unnamed job
        // object only this process holds.
        let job = Owned::new(unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) })?;
        let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
        limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
        if let Some(max) = memory_max {
            limits.BasicLimitInformation.LimitFlags |= JOB_OBJECT_LIMIT_JOB_MEMORY;
            limits.JobMemoryLimit = usize::try_from(max).unwrap_or(usize::MAX);
        }
        // SAFETY: the information is a valid extended limit structure of
        // the size passed.
        let set = unsafe {
            SetInformationJobObject(
                job.0,
                JobObjectExtendedLimitInformation,
                (&raw const limits).cast(),
                size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
            )
        };
        if set == 0 {
            return Err(io::Error::last_os_error());
        }
        let port = match memory_max {
            Some(_) => Some(report_to_port(&job)?),
            None => None,
        };
        Ok(JobObject {
            job,
            port,
            exceeded: AtomicBool::new(false),
        })
    }

    /// Puts the suspended process `process` (identifier `pid`) in this job
    /// and lets it run. The process must have been created with
    /// [`SUSPENDED`].
    pub(crate) fn adopt(self, process: RawHandle, pid: u32) -> io::Result<Tree> {
        let tree = self.contain(process, pid)?;
        resume(pid)?;
        Ok(tree)
    }

    /// Puts the running process `process` (identifier `pid`) in this job.
    /// What it started before this is not in the job.
    pub(crate) fn contain(self, process: RawHandle, pid: u32) -> io::Result<Tree> {
        // SAFETY: both handles are open: the job is this value's, and the
        // process handle belongs to the caller's `Child`, which outlives the
        // call.
        if unsafe { AssignProcessToJobObject(self.job.0, process) } == 0 {
            return Err(io::Error::last_os_error());
        }
        Ok(Tree {
            id: i32::try_from(pid).ok(),
            job: Arc::new(self),
        })
    }

    fn terminate(&self) -> bool {
        // SAFETY: the job handle is open for as long as `self` is.
        unsafe { TerminateJobObject(self.job.0, ENDED) != 0 }
    }

    fn active(&self) -> u32 {
        let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
        // SAFETY: the buffer is a basic accounting structure of the size
        // passed, and the job handle is open.
        let queried = unsafe {
            QueryInformationJobObject(
                self.job.0,
                JobObjectBasicAccountingInformation,
                (&raw mut accounting).cast(),
                size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                std::ptr::null_mut(),
            )
        };
        if queried == 0 {
            0
        } else {
            accounting.ActiveProcesses
        }
    }

    /// Whether the port has reported a process at the job's memory cap.
    fn exceeded(&self) -> bool {
        if let Some(port) = &self.port {
            loop {
                let mut message = 0u32;
                let mut key = 0usize;
                let mut overlapped: *mut OVERLAPPED = std::ptr::null_mut();
                // SAFETY: the port is open, and the three outputs are this
                // frame's; a zero wait returns at once when nothing is
                // queued.
                let got = unsafe {
                    GetQueuedCompletionStatus(port.0, &mut message, &mut key, &mut overlapped, 0)
                };
                if got == 0 {
                    break;
                }
                if message == JOB_OBJECT_MSG_JOB_MEMORY_LIMIT {
                    self.exceeded.store(true, Ordering::Relaxed);
                }
            }
        }
        self.exceeded.load(Ordering::Relaxed)
    }
}

/// Makes a completion port and has `job` report its messages on it.
fn report_to_port(job: &Owned) -> io::Result<Owned> {
    // SAFETY: no file handle and no existing port make a new, empty port.
    let port = Owned::new(unsafe {
        CreateIoCompletionPort(INVALID_HANDLE_VALUE, std::ptr::null_mut(), 0, 1)
    })?;
    let association = JOBOBJECT_ASSOCIATE_COMPLETION_PORT {
        CompletionKey: std::ptr::null_mut(),
        CompletionPort: port.0,
    };
    // SAFETY: the information is a valid association structure of the size
    // passed, naming a port that outlives the job's use of it (the job
    // object holds it until it closes).
    let set = unsafe {
        SetInformationJobObject(
            job.0,
            JobObjectAssociateCompletionPortInformation,
            (&raw const association).cast(),
            size_of::<JOBOBJECT_ASSOCIATE_COMPLETION_PORT>() as u32,
        )
    };
    if set == 0 {
        return Err(io::Error::last_os_error());
    }
    Ok(port)
}

/// Lets every thread of the suspended process `pid` run. A process created
/// suspended has one.
fn resume(pid: u32) -> io::Result<()> {
    // SAFETY: a thread snapshot of the whole system; the process argument is
    // ignored for threads.
    let snapshot = Owned::new(unsafe { CreateToolhelp32Snapshot(TH32CS_SNAPTHREAD, 0) })?;
    let mut entry = THREADENTRY32 {
        dwSize: size_of::<THREADENTRY32>() as u32,
        ..THREADENTRY32::default()
    };
    let mut resumed = 0;
    // SAFETY: the snapshot is open and the entry's size is set.
    let mut more = unsafe { Thread32First(snapshot.0, &mut entry) } != 0;
    while more {
        if entry.th32OwnerProcessID == pid {
            // SAFETY: opening a thread by identifier; a failure is a null
            // handle, which `Owned::new` refuses.
            if let Ok(thread) =
                Owned::new(unsafe { OpenThread(THREAD_SUSPEND_RESUME, 0, entry.th32ThreadID) })
            {
                // SAFETY: the thread handle is open with the right to resume.
                if unsafe { ResumeThread(thread.0) } != u32::MAX {
                    resumed += 1;
                }
            }
        }
        // SAFETY: as for `Thread32First`.
        more = unsafe { Thread32Next(snapshot.0, &mut entry) } != 0;
    }
    if resumed == 0 {
        return Err(io::Error::other(
            "no thread of the job's process could be resumed",
        ));
    }
    Ok(())
}

/// The tree one job owns: the job object its direct child started in.
/// Cloning shares the job object, which closes, and kills what is left in
/// it, when the last clone goes.
#[derive(Clone, Debug)]
pub(crate) struct Tree {
    id: Option<i32>,
    job: Arc<JobObject>,
}

impl Tree {
    /// The direct child's process identifier, which stands for the tree
    /// where Unix reports a process group.
    pub(crate) fn id(&self) -> Option<i32> {
        self.id
    }

    /// Asks the tree to stop. With no signal to ask with, this ends it.
    pub(crate) fn ask(&self) {
        self.job.terminate();
    }

    /// Ends every process in the job.
    pub(crate) fn end(&self) {
        self.job.terminate();
    }

    /// Whether any process is still in the job.
    pub(crate) fn running(&self) -> bool {
        self.job.active() > 0
    }

    /// Whether a process in the job ran into its memory cap.
    pub(crate) fn exceeded(&self) -> bool {
        self.job.exceeded()
    }
}

/// Whether `group` names a live process. On Windows a job's tree is a job
/// object that only the supervisor holding it can see, so this answers for
/// the process the identifier names: the job's direct child.
#[must_use]
pub fn running(group: i32) -> bool {
    u32::try_from(group).is_ok_and(process_running)
}

/// Whether `pid` names a live process — not a tree, one process.
///
/// A process this user may not open is counted as running, as `EPERM` is on
/// Unix. A recovered process identifier can name a different live process
/// than the one that wrote it — liveness is the question this answers, not
/// identity.
#[must_use]
pub fn process_running(pid: u32) -> bool {
    if pid == 0 {
        return false;
    }
    // SAFETY: opening a process by identifier for a wait; a failure is a
    // null handle.
    let handle = unsafe {
        OpenProcess(
            PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
            0,
            pid,
        )
    };
    let Ok(process) = Owned::new(handle) else {
        return io::Error::last_os_error().raw_os_error() == Some(ERROR_ACCESS_DENIED as i32);
    };
    // SAFETY: the handle is open with the right to wait on it; a zero wait
    // reports whether it has ended without blocking.
    unsafe { WaitForSingleObject(process.0, 0) == WAIT_TIMEOUT }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn creation_flags_suspend_isolate_and_hide_the_child() {
        assert_eq!(SUSPENDED & CREATE_SUSPENDED, CREATE_SUSPENDED);
        assert_eq!(RUNNING & CREATE_SUSPENDED, 0);
        for flags in [SUSPENDED, RUNNING] {
            assert_ne!(flags & CREATE_NO_WINDOW, 0);
            assert_ne!(flags & CREATE_NEW_PROCESS_GROUP, 0);
        }
    }

    #[test]
    fn a_live_process_and_a_dead_one_probe_differently() {
        assert!(process_running(std::process::id()));
        assert!(!process_running(0));
        let mut child = std::process::Command::new("cmd")
            .args(["/c", "exit 0"])
            .spawn()
            .unwrap();
        let pid = child.id();
        child.wait().unwrap();
        assert!(!process_running(pid));
    }

    #[test]
    fn a_job_object_counts_and_ends_its_process() {
        use std::os::windows::io::AsRawHandle as _;
        use std::os::windows::process::CommandExt as _;
        let mut child = std::process::Command::new("cmd")
            .args(["/c", "ping -n 30 127.0.0.1 > NUL"])
            .creation_flags(SUSPENDED)
            .spawn()
            .unwrap();
        let tree = JobObject::new(Some(1 << 30))
            .unwrap()
            .adopt(child.as_raw_handle(), child.id())
            .unwrap();
        assert_eq!(tree.id(), i32::try_from(child.id()).ok());
        assert!(tree.running());
        tree.end();
        let status = child.wait().unwrap();
        assert_eq!(status.code(), Some(ENDED as i32));
        let settled = std::time::Instant::now();
        while tree.running() && settled.elapsed() < std::time::Duration::from_secs(2) {
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(!tree.running());
        assert!(!tree.exceeded());
    }
}
