//! Plays Coder's cues ([`openagents_chat_app::cues`]) through the system's
//! own player, on a thread of its own: `afplay` on macOS, PowerShell's
//! `Media.SoundPlayer` on Windows, and on Linux the first of `paplay`,
//! `pw-play`, and `aplay` that plays it. The players want a file, so each
//! cue is written to a fresh temporary file that is removed afterwards.
//!
//! A missing player or a failed write plays nothing and says nothing: a
//! sound never holds or interrupts the window. Reimplemented from Zeron's
//! `crates/ui/src/sound.rs` (public MIT zeronsh/zeron at `9e1a1115`).

use openagents_chat_app::cues::{Cue, wav};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The longest a player may run before it is stopped, so a stuck audio
/// service never gathers waiting threads.
const DEADLINE: Duration = Duration::from_secs(10);

/// Plays `cue`; nothing waits for it.
pub fn play(cue: Cue) {
    let _ = std::thread::Builder::new()
        .name("cue".into())
        .spawn(move || {
            let _ = play_now(cue);
        });
}

/// Plays `cue` and waits for the player to finish.
fn play_now(cue: Cue) -> Result<(), String> {
    let file = Temporary::create(&wav(cue)).map_err(|error| error.to_string())?;
    run_player(&file.0)
}

/// A cue's file, removed when dropped.
struct Temporary(PathBuf);

impl Temporary {
    /// Writes `bytes` to a new file in the temporary directory. The file
    /// is created exclusively, so a file already at the name (or a link
    /// planted there) is never written through.
    fn create(bytes: &[u8]) -> std::io::Result<Temporary> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..64 {
            let path = std::env::temp_dir().join(format!(
                "openagents-cue-{}-{}.wav",
                std::process::id(),
                NEXT.fetch_add(1, Ordering::Relaxed)
            ));
            match std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(&path)
            {
                Ok(mut file) => {
                    let temporary = Temporary(path);
                    file.write_all(bytes)?;
                    return Ok(temporary);
                }
                Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => {}
                Err(error) => return Err(error),
            }
        }
        Err(std::io::Error::new(
            std::io::ErrorKind::AlreadyExists,
            "no free name for a cue's file",
        ))
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(target_os = "macos")]
fn run_player(path: &Path) -> Result<(), String> {
    run(Command::new("afplay").arg(path))
}

#[cfg(windows)]
fn run_player(path: &Path) -> Result<(), String> {
    use std::os::windows::process::CommandExt;
    /// Starts the console program without a window of its own.
    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    // The path goes through the environment, never into the script, so a
    // quote in it cannot change what runs.
    run(Command::new("powershell.exe")
        .args([
            "-NoLogo",
            "-NoProfile",
            "-NonInteractive",
            "-Command",
            "(New-Object Media.SoundPlayer $env:OPENAGENTS_CUE).PlaySync()",
        ])
        .env("OPENAGENTS_CUE", path)
        .creation_flags(CREATE_NO_WINDOW))
}

#[cfg(not(any(target_os = "macos", windows)))]
fn run_player(path: &Path) -> Result<(), String> {
    let players: [(&str, &[&str]); 3] = [("paplay", &[]), ("pw-play", &[]), ("aplay", &["-q"])];
    let mut errors = vec![];
    for (program, args) in players {
        match run(Command::new(program).args(args).arg(path)) {
            Ok(()) => return Ok(()),
            Err(error) => errors.push(error),
        }
    }
    Err(errors.join("; "))
}

/// Runs `command` with no input or output, stopping it at [`DEADLINE`].
fn run(command: &mut Command) -> Result<(), String> {
    let program = command.get_program().to_string_lossy().into_owned();
    let mut child = command
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| format!("{program}: {error}"))?;
    let until = Instant::now() + DEADLINE;
    loop {
        match child.try_wait() {
            Ok(Some(status)) if status.success() => return Ok(()),
            Ok(Some(status)) => return Err(format!("{program} exited with {status}")),
            Ok(None) if Instant::now() < until => std::thread::sleep(Duration::from_millis(25)),
            Ok(None) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} took longer than {DEADLINE:?}"));
            }
            Err(error) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program}: {error}"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_cues_file_holds_its_bytes_and_is_removed_after() {
        let bytes = wav(Cue::Request);
        let file = Temporary::create(&bytes).unwrap();
        let path = file.0.clone();
        assert_eq!(std::fs::read(&path).unwrap(), bytes);
        let other = Temporary::create(&bytes).unwrap();
        assert_ne!(other.0, path, "each cue has a file of its own");
        drop(file);
        assert!(!path.exists());
    }
}
