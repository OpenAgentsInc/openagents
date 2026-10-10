//! A local ACP agent as a process of its own.
//!
//! The agent is the leader of a new process group, so every tool it starts
//! is in the group, and stopping the agent stops the group: `SIGTERM`, a
//! grace, then `SIGKILL`, and the stop reports whether the group is empty.
//!
//! On Windows the agent joins a job object of its own as soon as it is
//! spawned, with no console window, and stopping it ends the job: Windows
//! has no signal that asks a windowless program to stop, so there is no
//! grace. A process the agent started in the moment before it joined the
//! job is not in it; an agent reads its first request before it starts
//! anything.
//! The caller passes the agent's whole environment; nothing is inherited.
//! Standard error is not protocol: it is drained, and its last lines are
//! kept for a failure to quote.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::process::Stdio;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use tokio::io::{AsyncBufReadExt, BufReader};
use tokio::process::{Child, ChildStdin, ChildStdout, Command};

use crate::client::Client;

/// How many of the agent's last standard error lines are kept.
const STDERR_TAIL: usize = 8;
/// The longest kept standard error line.
const STDERR_LINE: usize = 400;

/// How to start one agent.
#[derive(Clone, Debug)]
pub struct Spec {
    pub program: PathBuf,
    pub arguments: Vec<String>,
    pub cwd: PathBuf,
    /// The agent's whole environment.
    pub environment: Vec<(String, String)>,
}

/// A running agent and the client over its streams.
pub struct Agent {
    child: Child,
    group: i32,
    #[cfg(windows)]
    job: windows::Job,
    pub client: Client<ChildStdout, ChildStdin>,
    stderr: Arc<Mutex<VecDeque<String>>>,
}

/// The agent could not start.
#[derive(Debug)]
pub struct Unstartable(pub String);

impl std::fmt::Display for Unstartable {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Unstartable {}

#[cfg(unix)]
fn signal_group(group: i32, signal: i32) -> bool {
    if group <= 0 {
        return false;
    }
    // SAFETY: `killpg` reads two integers and returns one. The group is the
    // one this module made for the agent, so no process outside it is in it.
    unsafe { libc::killpg(group, signal) == 0 }
}

impl Agent {
    /// Start the agent described by `spec`.
    ///
    /// # Errors
    /// The program is missing or the operating system refused to start it.
    pub fn start(spec: &Spec) -> Result<Self, Unstartable> {
        let mut command = Command::new(&spec.program);
        command
            .args(&spec.arguments)
            .current_dir(&spec.cwd)
            .env_clear()
            .envs(spec.environment.iter().map(|(key, value)| (key, value)))
            // The agent's heavy `cargo` commands take a build lease through
            // the shim first on its `PATH`, once this process turned the
            // shims on (`coder_lease::shim`).
            .envs(
                spec.environment
                    .iter()
                    .find(|(key, _)| key == "PATH")
                    .map(|(_, path)| coder_lease::shim::delegate_vars(Some(path.as_ref())))
                    .unwrap_or_default(),
            )
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .stderr(Stdio::piped())
            .kill_on_drop(true);
        #[cfg(unix)]
        command.process_group(0);
        #[cfg(windows)]
        command.creation_flags(windows::CREATION_FLAGS);
        let mut child = command.spawn().map_err(|error| {
            Unstartable(format!("cannot start {}: {error}", spec.program.display()))
        })?;
        let pid = child.id().unwrap_or_default();
        let group = i32::try_from(pid).unwrap_or_default();
        #[cfg(windows)]
        let job = match windows::Job::holding(&child) {
            Ok(job) => job,
            Err(error) => {
                let _ = child.start_kill();
                return Err(Unstartable(format!(
                    "cannot put {} in a job object: {error}",
                    spec.program.display()
                )));
            }
        };
        let (Some(stdin), Some(stdout), Some(stderr)) =
            (child.stdin.take(), child.stdout.take(), child.stderr.take())
        else {
            #[cfg(unix)]
            signal_group(group, libc::SIGKILL);
            #[cfg(windows)]
            job.end();
            return Err(Unstartable("the agent's streams are unavailable".into()));
        };
        let tail = Arc::new(Mutex::new(VecDeque::new()));
        let kept = Arc::clone(&tail);
        tokio::spawn(async move {
            let mut lines = BufReader::new(stderr).lines();
            while let Ok(Some(mut line)) = lines.next_line().await {
                if line.len() > STDERR_LINE {
                    let mut end = STDERR_LINE;
                    while !line.is_char_boundary(end) {
                        end -= 1;
                    }
                    line.truncate(end);
                }
                if let Ok(mut kept) = kept.lock() {
                    kept.push_back(line);
                    while kept.len() > STDERR_TAIL {
                        kept.pop_front();
                    }
                }
            }
        });
        Ok(Agent {
            child,
            group,
            #[cfg(windows)]
            job,
            client: Client::new(stdout, stdin),
            stderr: tail,
        })
    }

    /// The agent's process identifier, which is also its group's.
    #[must_use]
    pub fn pid(&self) -> u32 {
        u32::try_from(self.group).unwrap_or_default()
    }

    /// The agent's last standard error lines, oldest first.
    #[must_use]
    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr
            .lock()
            .map(|kept| kept.iter().cloned().collect())
            .unwrap_or_default()
    }

    /// Stop the agent and its group: `SIGTERM`, up to `grace` for the leader
    /// to exit, then `SIGKILL`. Returns whether the group is empty.
    #[cfg(unix)]
    pub async fn stop(mut self, grace: Duration) -> bool {
        signal_group(self.group, libc::SIGTERM);
        let exited = tokio::time::timeout(grace, self.child.wait()).await.is_ok();
        if !exited || signal_group(self.group, 0) {
            signal_group(self.group, libc::SIGKILL);
            let _ = tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await;
        }
        for _ in 0..20 {
            if !signal_group(self.group, 0) {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }

    /// Stop the agent and everything in its job object. Windows has no
    /// signal that asks a windowless program to stop, so `grace` is not
    /// used: the job ends at once. Returns whether the job is empty.
    #[cfg(windows)]
    pub async fn stop(mut self, grace: Duration) -> bool {
        let _ = grace;
        self.job.end();
        let _ = tokio::time::timeout(Duration::from_secs(2), self.child.wait()).await;
        for _ in 0..20 {
            if !self.job.running() {
                return true;
            }
            tokio::time::sleep(Duration::from_millis(50)).await;
        }
        false
    }
}

/// The first executable file among `candidates`.
#[cfg(unix)]
#[must_use]
pub fn first_executable(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    use std::os::unix::fs::PermissionsExt;
    candidates.into_iter().find(|path| {
        std::fs::metadata(path)
            .is_ok_and(|meta| meta.is_file() && meta.permissions().mode() & 0o111 != 0)
    })
}

/// The first executable file among `candidates`: on Windows, a file whose
/// extension is a program's (`.exe`, `.com`, `.cmd`, or `.bat`).
#[cfg(windows)]
#[must_use]
pub fn first_executable(candidates: impl IntoIterator<Item = PathBuf>) -> Option<PathBuf> {
    candidates.into_iter().find(|path| {
        windows::runnable(path) && std::fs::metadata(path).is_ok_and(|meta| meta.is_file())
    })
}

/// `name` in each directory of `PATH`; on Windows, also `name` with each
/// program extension.
#[must_use]
pub fn on_path(name: &str, path: Option<&std::ffi::OsStr>) -> Vec<PathBuf> {
    path.map(|paths| {
        std::env::split_paths(paths)
            .flat_map(|dir| candidates(&dir, name))
            .collect()
    })
    .unwrap_or_default()
}

#[cfg(unix)]
fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    vec![dir.join(name)]
}

#[cfg(windows)]
fn candidates(dir: &Path, name: &str) -> Vec<PathBuf> {
    std::iter::once(dir.join(name))
        .chain(
            windows::EXTENSIONS
                .iter()
                .map(|extension| dir.join(format!("{name}.{extension}"))),
        )
        .collect()
}

#[cfg(windows)]
mod windows {
    //! The job object one agent and its tools run in.

    use std::io;
    use std::path::Path;

    use tokio::process::Child;
    use windows_sys::Win32::Foundation::{CloseHandle, HANDLE};
    use windows_sys::Win32::System::JobObjects::{
        AssignProcessToJobObject, CreateJobObjectW, JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE,
        JOBOBJECT_BASIC_ACCOUNTING_INFORMATION, JOBOBJECT_EXTENDED_LIMIT_INFORMATION,
        JobObjectBasicAccountingInformation, JobObjectExtendedLimitInformation,
        QueryInformationJobObject, SetInformationJobObject, TerminateJobObject,
    };
    use windows_sys::Win32::System::Threading::{CREATE_NEW_PROCESS_GROUP, CREATE_NO_WINDOW};

    /// A console's Ctrl+C never reaches the agent, and it opens no window.
    pub(super) const CREATION_FLAGS: u32 = CREATE_NEW_PROCESS_GROUP | CREATE_NO_WINDOW;

    /// The extensions of a file Windows runs as a program.
    pub(super) const EXTENSIONS: [&str; 4] = ["exe", "com", "cmd", "bat"];

    /// Whether `path` names a program by its extension.
    pub(super) fn runnable(path: &Path) -> bool {
        path.extension()
            .and_then(|extension| extension.to_str())
            .is_some_and(|extension| {
                EXTENSIONS
                    .iter()
                    .any(|known| extension.eq_ignore_ascii_case(known))
            })
    }

    /// A job object that kills what is left in it when it closes.
    pub(super) struct Job(HANDLE);

    // SAFETY: a kernel handle any thread may use and close.
    unsafe impl Send for Job {}
    // SAFETY: as above; every call through it is thread-safe.
    unsafe impl Sync for Job {}

    impl Job {
        /// A new job object holding the running `child`.
        pub(super) fn holding(child: &Child) -> io::Result<Self> {
            let process = child
                .raw_handle()
                .ok_or_else(|| io::Error::other("the agent exited at once"))?;
            // SAFETY: an unnamed job object with default security.
            let handle = unsafe { CreateJobObjectW(std::ptr::null(), std::ptr::null()) };
            if handle.is_null() {
                return Err(io::Error::last_os_error());
            }
            let job = Job(handle);
            let mut limits = JOBOBJECT_EXTENDED_LIMIT_INFORMATION::default();
            limits.BasicLimitInformation.LimitFlags = JOB_OBJECT_LIMIT_KILL_ON_JOB_CLOSE;
            // SAFETY: a valid limit structure of the size passed.
            let set = unsafe {
                SetInformationJobObject(
                    job.0,
                    JobObjectExtendedLimitInformation,
                    (&raw const limits).cast(),
                    size_of::<JOBOBJECT_EXTENDED_LIMIT_INFORMATION>() as u32,
                )
            };
            // SAFETY: both handles are open.
            if set == 0 || unsafe { AssignProcessToJobObject(job.0, process) } == 0 {
                return Err(io::Error::last_os_error());
            }
            Ok(job)
        }

        /// Ends every process in the job.
        pub(super) fn end(&self) {
            // SAFETY: the handle is open for as long as `self` is.
            unsafe { TerminateJobObject(self.0, 1) };
        }

        /// Whether any process is still in the job.
        pub(super) fn running(&self) -> bool {
            let mut accounting = JOBOBJECT_BASIC_ACCOUNTING_INFORMATION::default();
            // SAFETY: a buffer of the size passed, and an open handle.
            let queried = unsafe {
                QueryInformationJobObject(
                    self.0,
                    JobObjectBasicAccountingInformation,
                    (&raw mut accounting).cast(),
                    size_of::<JOBOBJECT_BASIC_ACCOUNTING_INFORMATION>() as u32,
                    std::ptr::null_mut(),
                )
            };
            queried != 0 && accounting.ActiveProcesses > 0
        }
    }

    impl Drop for Job {
        fn drop(&mut self) {
            // SAFETY: the handle is open and closed once; closing it kills
            // what is left in the job.
            unsafe { CloseHandle(self.0) };
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        #[test]
        fn programs_are_known_by_their_extension() {
            assert!(runnable(Path::new(r"C:\bin\devin.exe")));
            assert!(runnable(Path::new(r"C:\bin\opencode.CMD")));
            assert!(!runnable(Path::new(r"C:\bin\devin")));
            assert!(!runnable(Path::new(r"C:\bin\notes.txt")));
        }
    }
}

/// Whether a variable name names a credential the agent must not inherit:
/// the workspace's one policy, [`secret_screen::is_credential_name`].
pub use secret_screen::is_credential_name;

/// `path` relative to `home`, for a message that names a location without
/// the account's home directory.
#[must_use]
pub fn tilde(path: &Path, home: Option<&Path>) -> String {
    match home.and_then(|home| path.strip_prefix(home).ok()) {
        Some(rest) => format!("~/{}", rest.display()),
        None => path.display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn credential_names_are_recognized_by_suffix() {
        assert!(is_credential_name("OPENAI_API_KEY"));
        assert!(is_credential_name("github_token"));
        assert!(is_credential_name("AWS_SECRET"));
        assert!(is_credential_name("AWS_SECRET_ACCESS_KEY"));
        assert!(is_credential_name("DB_PASSWORD"));
        assert!(is_credential_name("GOOGLE_APPLICATION_CREDENTIALS"));
        assert!(!is_credential_name("HOME"));
        assert!(!is_credential_name("TOKENIZER"));
    }

    #[tokio::test]
    #[cfg(unix)]
    async fn stopping_an_agent_empties_its_group() {
        let spec = Spec {
            program: PathBuf::from("/bin/sh"),
            arguments: vec!["-c".into(), "sleep 30 & sleep 30".into()],
            cwd: std::env::temp_dir(),
            environment: vec![("PATH".into(), "/bin:/usr/bin".into())],
        };
        let agent = Agent::start(&spec).unwrap();
        assert!(agent.pid() > 0);
        assert!(agent.stop(Duration::from_millis(500)).await);
    }

    #[tokio::test]
    #[cfg(windows)]
    async fn stopping_an_agent_empties_its_job() {
        let spec = Spec {
            program: PathBuf::from("cmd.exe"),
            arguments: vec!["/c".into(), "ping -n 30 127.0.0.1 > NUL".into()],
            cwd: std::env::temp_dir(),
            environment: std::env::vars().collect(),
        };
        let agent = Agent::start(&spec).unwrap();
        assert!(agent.pid() > 0);
        assert!(agent.stop(Duration::from_millis(500)).await);
    }

    #[tokio::test]
    async fn a_missing_program_is_unstartable() {
        let spec = Spec {
            program: PathBuf::from("/nonexistent/agent"),
            arguments: Vec::new(),
            cwd: std::env::temp_dir(),
            environment: Vec::new(),
        };
        assert!(Agent::start(&spec).is_err());
    }
}
