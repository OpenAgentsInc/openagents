//! Plays the studio's signals and raises its desktop notices, each on a
//! thread of its own so nothing holds the frame.
//!
//! The sound is synthesized by `openagents_chat_app::cues::tones`, the
//! voice the desktop app's cues use, and played by the system's own player
//! as the desktop app's `sound` module does: `afplay` on macOS, and on Linux
//! the first of `paplay`, `pw-play`, and `aplay` that plays it. A notice
//! goes through `osascript` on macOS and `notify-send` on Linux, its text
//! passed as arguments, never inside a script. Windows plays and shows
//! nothing yet. A missing player or notifier does nothing and says
//! nothing: a signal never holds or interrupts the world.

use super::{Event, Signal};
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::sync::atomic::{AtomicU64, Ordering};
use std::time::{Duration, Instant};

/// The longest a player or notifier may run before it is stopped.
const DEADLINE: Duration = Duration::from_secs(10);

/// Plays `signal`; nothing waits for it.
pub fn play(signal: Signal) {
    let _ = std::thread::Builder::new()
        .name("studio-signal".into())
        .spawn(move || {
            let bytes = openagents_chat_app::cues::tones(signal.notes());
            if let Ok(file) = Temporary::create(&bytes) {
                let _ = player(&file.0);
            }
        });
}

/// Raises `event`'s desktop notice; nothing waits for it.
pub fn notify(event: &Event) {
    let (title, body) = event.notice();
    let _ = std::thread::Builder::new()
        .name("studio-notice".into())
        .spawn(move || {
            let _ = notifier(title, &body);
        });
}

/// A signal's sound file, removed when dropped.
struct Temporary(PathBuf);

impl Temporary {
    /// Writes `bytes` to a new file in the temporary directory, created
    /// exclusively so a file or link already at the name is never written
    /// through.
    fn create(bytes: &[u8]) -> std::io::Result<Temporary> {
        static NEXT: AtomicU64 = AtomicU64::new(0);
        for _ in 0..64 {
            let path = std::env::temp_dir().join(format!(
                "verse-signal-{}-{}.wav",
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
            "no free name for a signal's file",
        ))
    }
}

impl Drop for Temporary {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

#[cfg(target_os = "macos")]
fn player(path: &std::path::Path) -> Result<(), String> {
    run(Command::new("afplay").arg(path))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn player(path: &std::path::Path) -> Result<(), String> {
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

#[cfg(not(unix))]
fn player(_path: &std::path::Path) -> Result<(), String> {
    Err("no system player on this platform".into())
}

#[cfg(target_os = "macos")]
fn notifier(title: &str, body: &str) -> Result<(), String> {
    // The script reads its text from its arguments, so a quote in the text
    // cannot change what runs.
    run(Command::new("osascript").args([
        "-e",
        "on run argv",
        "-e",
        "display notification (item 2 of argv) with title (item 1 of argv)",
        "-e",
        "end run",
        title,
        body,
    ]))
}

#[cfg(all(unix, not(target_os = "macos")))]
fn notifier(title: &str, body: &str) -> Result<(), String> {
    run(Command::new("notify-send").args(["--app-name=Verse", "--", title, body]))
}

#[cfg(not(unix))]
fn notifier(_title: &str, _body: &str) -> Result<(), String> {
    Err("no notifier on this platform".into())
}

/// Runs `command` with no input or output, stopping it at [`DEADLINE`].
#[cfg_attr(not(unix), allow(dead_code))]
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
            Ok(None) | Err(_) => {
                let _ = child.kill();
                let _ = child.wait();
                return Err(format!("{program} did not finish"));
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_signal_is_a_distinct_wav_and_its_file_is_removed_after() {
        let sounds: Vec<Vec<u8>> = [Signal::Bell, Signal::Chime, Signal::Fanfare]
            .into_iter()
            .map(|s| openagents_chat_app::cues::tones(s.notes()))
            .collect();
        for bytes in &sounds {
            assert_eq!(&bytes[..4], b"RIFF");
        }
        assert_ne!(sounds[0], sounds[1]);
        // The fanfare rings longer than a pair.
        assert!(sounds[2].len() > sounds[1].len());
        let file = Temporary::create(&sounds[0]).unwrap();
        let path = file.0.clone();
        assert_eq!(std::fs::read(&path).unwrap(), sounds[0]);
        drop(file);
        assert!(!path.exists());
    }
}
