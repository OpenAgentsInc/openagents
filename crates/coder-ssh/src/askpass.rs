//! Password and passphrase prompts through a one-shot askpass helper.
//!
//! `ssh` runs the program named by `SSH_ASKPASS` with the prompt as its
//! argument and reads the answer from that program's standard output. The
//! helper this module writes carries no secret. It writes the prompt to one
//! named pipe and copies the answer from a second named pipe, both in a
//! private temporary directory. A thread in this process reads each prompt,
//! asks the caller's [`Prompter`], and writes the answer into the pipe once.
//! The answer never appears in an environment variable, an argument list,
//! or a file, and the directory is removed when the `ssh` invocation ends.

use std::ffi::CString;
use std::fs::{File, OpenOptions};
use std::io::{Read as _, Write as _};
use std::os::unix::ffi::OsStrExt as _;
use std::os::unix::fs::{OpenOptionsExt as _, PermissionsExt as _};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::thread::JoinHandle;
use std::time::{Duration, Instant};

/// The largest prompt this module reads from `ssh`.
const PROMPT_MAX: u64 = 8 * 1024;

/// How long an answer waits for the helper to open its pipe.
const DELIVERY_WAIT: Duration = Duration::from_secs(10);

/// A password or passphrase. Its bytes are overwritten when it is dropped,
/// and its debug form never shows them.
pub struct Secret(Vec<u8>);

impl Secret {
    /// Wraps a password or passphrase.
    #[must_use]
    pub fn new(value: impl Into<Vec<u8>>) -> Self {
        Secret(value.into())
    }
}

impl std::fmt::Debug for Secret {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("Secret(redacted)")
    }
}

impl Drop for Secret {
    fn drop(&mut self) {
        wipe(&mut self.0);
    }
}

fn wipe(bytes: &mut [u8]) {
    for byte in bytes.iter_mut() {
        // A volatile write keeps the compiler from removing the wipe of a
        // buffer that is about to be freed.
        // SAFETY: `byte` is a valid, exclusive reference into the slice.
        unsafe { std::ptr::write_volatile(byte, 0) };
    }
}

/// Answers the prompts `ssh` shows: a password, a key passphrase, or a
/// question about an unknown host key.
///
/// The prompt text comes from `ssh` verbatim. Show it to the person and
/// return their answer, or `None` to refuse, which makes `ssh` fail
/// authentication rather than guess. Each prompt is asked once; a later
/// prompt in the same invocation is a new question.
pub trait Prompter: Send + Sync {
    /// Returns the answer to one prompt, or `None` to refuse it.
    fn answer(&self, prompt: &str) -> Option<Secret>;
}

impl<F> Prompter for F
where
    F: Fn(&str) -> Option<Secret> + Send + Sync,
{
    fn answer(&self, prompt: &str) -> Option<Secret> {
        self(prompt)
    }
}

/// One invocation's helper, its pipes, and the thread that answers them.
pub(crate) struct Askpass {
    directory: Option<tempfile::TempDir>,
    helper: PathBuf,
    prompt: PathBuf,
    stop: Arc<AtomicBool>,
    thread: Option<JoinHandle<()>>,
}

impl Askpass {
    /// Writes a fresh helper and starts answering its prompts.
    pub(crate) fn start(prompter: Arc<dyn Prompter>) -> std::io::Result<Self> {
        let directory = tempfile::Builder::new()
            .prefix("coder-ssh-askpass-")
            .tempdir()?;
        std::fs::set_permissions(directory.path(), std::fs::Permissions::from_mode(0o700))?;
        let prompt = directory.path().join("prompt");
        let answer = directory.path().join("answer");
        fifo(&prompt)?;
        fifo(&answer)?;
        let helper = directory.path().join("askpass");
        let quoted = crate::ssh::quote(&directory.path().to_string_lossy());
        let script = format!(
            "#!/bin/sh\n\
             # One-shot askpass helper. It holds no secret: the answer arrives on a pipe.\n\
             dir={quoted}\n\
             printf '%s' \"${{1:-}}\" > \"$dir/prompt\" || exit 1\n\
             {{ IFS= read -r status || exit 1; [ \"$status\" = y ] || exit 1; exec cat; }} < \"$dir/answer\"\n"
        );
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .mode(0o700)
            .open(&helper)?;
        file.write_all(script.as_bytes())?;
        file.sync_all()?;
        drop(file);

        let stop = Arc::new(AtomicBool::new(false));
        let thread = {
            let stop = Arc::clone(&stop);
            let prompt = prompt.clone();
            std::thread::Builder::new()
                .name("coder-ssh-askpass".to_string())
                .spawn(move || serve(&prompt, &answer, prompter.as_ref(), &stop))?
        };
        Ok(Askpass {
            directory: Some(directory),
            helper,
            prompt,
            stop,
            thread: Some(thread),
        })
    }

    /// The helper `ssh` runs as `SSH_ASKPASS`.
    pub(crate) fn helper(&self) -> &Path {
        &self.helper
    }
}

impl Drop for Askpass {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);
        // The answering thread may be blocked opening the prompt pipe.
        // Opening its other end wakes it; it then sees the stop flag.
        let deadline = Instant::now() + Duration::from_secs(1);
        while let Some(thread) = &self.thread {
            if thread.is_finished() {
                break;
            }
            if Instant::now() >= deadline {
                break;
            }
            if let Ok(wake) = OpenOptions::new()
                .write(true)
                .custom_flags(libc::O_NONBLOCK)
                .open(&self.prompt)
            {
                drop(wake);
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if let Some(thread) = self.thread.take()
            && thread.is_finished()
        {
            let _ = thread.join();
        }
        // A thread still waiting on a person's answer is left to finish on
        // its own; the pipes it would write to are gone with the directory.
        if let Some(directory) = self.directory.take() {
            let _ = directory.close();
        }
    }
}

fn fifo(path: &Path) -> std::io::Result<()> {
    let name = CString::new(path.as_os_str().as_bytes())
        .map_err(|_| std::io::Error::other("askpass path holds a NUL byte"))?;
    // SAFETY: `name` is a valid NUL-terminated path for the call's duration.
    let result = unsafe { libc::mkfifo(name.as_ptr(), 0o600) };
    if result == 0 {
        Ok(())
    } else {
        Err(std::io::Error::last_os_error())
    }
}

/// Answers prompts until the invocation ends.
fn serve(prompt: &Path, answer: &Path, prompter: &dyn Prompter, stop: &AtomicBool) {
    loop {
        if stop.load(Ordering::SeqCst) {
            return;
        }
        // Blocks until the helper, or the wake-up in `Drop`, opens the
        // other end.
        let Ok(file) = File::open(prompt) else {
            return;
        };
        let mut text = Vec::new();
        if file.take(PROMPT_MAX).read_to_end(&mut text).is_err() {
            return;
        }
        if stop.load(Ordering::SeqCst) {
            return;
        }
        let reply = prompter.answer(&String::from_utf8_lossy(&text));
        deliver(answer, reply, stop);
    }
}

/// Writes one answer, or a refusal, to the helper waiting on the pipe.
fn deliver(answer: &Path, reply: Option<Secret>, stop: &AtomicBool) {
    let mut message = match &reply {
        Some(secret) => {
            let mut message = Vec::with_capacity(secret.0.len() + 3);
            message.extend_from_slice(b"y\n");
            message.extend_from_slice(&secret.0);
            message.push(b'\n');
            message
        }
        None => b"n\n".to_vec(),
    };
    drop(reply);
    let deadline = Instant::now() + DELIVERY_WAIT;
    loop {
        match OpenOptions::new()
            .write(true)
            .custom_flags(libc::O_NONBLOCK)
            .open(answer)
        {
            Ok(mut pipe) => {
                let _ = pipe.write_all(&message);
                break;
            }
            Err(_) if Instant::now() < deadline && !stop.load(Ordering::SeqCst) => {
                std::thread::sleep(Duration::from_millis(10));
            }
            Err(_) => break,
        }
    }
    wipe(&mut message);
}
