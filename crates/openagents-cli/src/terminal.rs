//! `openagents computer exec` and `computer shell`: a shell on a linked host
//! over NIP-TERM, driven by `coder_computers::terminal::Session`.
//!
//! Both commands open the host's shell with NIP-HOST `terminal.open` and
//! attach with NIP-TERM, so the host checks the `terminal` right on every
//! request and the route (loopback, tailnet, direct, or relay) is whatever
//! the Computers supervisor picked. `exec` replaces the shell with the
//! command, so the shell's exit is the command's exit; `shell` puts this
//! terminal in raw mode and forwards keystrokes until the remote shell ends.

use std::io::{Read, Write};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use coder_computers::live::Live;
use coder_computers::terminal::session::Session;
use coder_computers::terminal::{Model, Phase};
use serde_json::json;

use crate::{Args, Output};

/// Marks where the echoed command line ends and the command's output starts.
const START_MARK: u8 = 0x1e;

/// The remote shell's phase, in words and as JSON.
fn phase_json(phase: &Phase) -> serde_json::Value {
    match phase {
        Phase::Exited {
            code,
            signal,
            cause,
        } => json!({ "phase": "exited", "code": code, "signal": signal, "cause": cause }),
        Phase::Refused(reason) => json!({ "phase": "refused", "reason": reason }),
        other => json!({ "phase": format!("{other:?}").to_lowercase() }),
    }
}

/// Quote `word` for a POSIX shell.
fn quote(word: &str) -> String {
    if !word.is_empty()
        && word
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || b"-_./=:@%+,".contains(&b))
    {
        return word.to_owned();
    }
    format!("'{}'", word.replace('\'', "'\\''"))
}

/// The size of this terminal, or 24 by 80 when it is not one.
fn local_size() -> (u16, u16) {
    #[cfg(unix)]
    {
        let mut size = libc::winsize {
            ws_row: 0,
            ws_col: 0,
            ws_xpixel: 0,
            ws_ypixel: 0,
        };
        // SAFETY: ioctl with TIOCGWINSZ writes a winsize into the struct we
        // pass and reads nothing else.
        let ok = unsafe { libc::ioctl(libc::STDOUT_FILENO, libc::TIOCGWINSZ, &raw mut size) };
        if ok == 0 && size.ws_row > 0 && size.ws_col > 0 {
            return (size.ws_row, size.ws_col);
        }
    }
    (24, 80)
}

/// Wait until the session is attached, or return the phase it ended in.
fn attached(session: &Session, deadline: Instant) -> Result<(), Phase> {
    loop {
        let phase = session.model().phase.clone();
        match phase {
            Phase::Attached => return Ok(()),
            phase if phase.ended() => return Err(phase),
            _ => {}
        }
        if Instant::now() >= deadline {
            return Err(Phase::Refused(
                "the host did not open a terminal in time; pass --wait SECONDS to wait longer"
                    .into(),
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

fn start(
    live: &Live,
    runtime: &tokio::runtime::Handle,
    host: &str,
    rows: u16,
    cols: u16,
) -> (Session, mpsc::Receiver<Vec<u8>>) {
    let (tap, output) = mpsc::channel();
    let mut model = Model::new(host, host, rows, cols);
    model.tap = Some(tap);
    let links = live.terminals().links(host);
    (Session::start(runtime, links, model), output)
}

/// Run `words` on `host` and report its output and exit code.
pub fn exec(
    output: Output,
    live: &Live,
    runtime: &tokio::runtime::Handle,
    host: &str,
    words: &[String],
    args: &Args,
) -> Result<u8, String> {
    let wait: u64 = args.number("wait", 15)?;
    let timeout: u64 = args.number("timeout", 600)?;
    let rows: u16 = args.number("rows", 50)?;
    let cols: u16 = args.number("cols", 200)?;
    let (session, frames) = start(live, runtime, host, rows, cols);
    if let Err(phase) = attached(&session, Instant::now() + Duration::from_secs(wait)) {
        return Err(phase.describe());
    }
    let command = words.iter().map(|w| quote(w)).collect::<Vec<_>>().join(" ");
    // Echo is off before the marker prints, so the marker separates what
    // the shell echoed from what the command wrote. `exec` makes the
    // shell's exit the command's exit.
    let line = format!("stty -echo 2>/dev/null; printf '\\036'; exec {command}\n");
    session.send(line.into_bytes());

    let deadline = Instant::now() + Duration::from_secs(timeout);
    let mut bytes = Vec::new();
    let mut started = false;
    let mut timed_out = false;
    let mut stdout = std::io::stdout();
    loop {
        match frames.recv_timeout(Duration::from_millis(100)) {
            Ok(chunk) => {
                let chunk = if started {
                    chunk
                } else if let Some(at) = chunk.iter().position(|b| *b == START_MARK) {
                    started = true;
                    chunk[at + 1..].to_vec()
                } else {
                    continue;
                };
                if !output.json() {
                    let _ = stdout.write_all(&chunk);
                    let _ = stdout.flush();
                }
                bytes.extend(chunk);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let phase = session.model().phase.clone();
        if phase.ended() {
            // Frames in flight arrive before the exit frame is applied.
            while let Ok(chunk) = frames.try_recv() {
                if !output.json() {
                    let _ = stdout.write_all(&chunk);
                }
                bytes.extend(chunk);
            }
            break;
        }
        if Instant::now() >= deadline {
            timed_out = true;
            session.close();
            break;
        }
    }
    let phase = session.model().phase.clone();
    let (code, ended) = match &phase {
        Phase::Exited { code, signal, .. } => (
            code.or_else(|| signal.map(|s| 128 + s)).unwrap_or(1),
            phase_json(&phase),
        ),
        other => (crate::EXIT_FAILURE.into(), phase_json(other)),
    };
    let code = if timed_out { 124 } else { code };
    let text = String::from_utf8_lossy(&bytes).replace("\r\n", "\n");
    if output.json() {
        output.emit(
            &json!({
                "host": host, "command": words, "output": text, "exit": code,
                "timed_out": timed_out, "shell": ended,
                "route": session.model().route,
            }),
            |_| String::new(),
        );
    } else if timed_out {
        eprintln!("openagents computer exec: timed out after {timeout}s");
    } else if !matches!(phase, Phase::Exited { .. }) {
        eprintln!("openagents computer exec: {}", phase.describe());
    }
    Ok(u8::try_from(code).unwrap_or(255))
}

/// An interactive shell on `host`, in raw mode on this terminal.
pub fn shell(
    output: Output,
    live: &Live,
    runtime: &tokio::runtime::Handle,
    host: &str,
    args: &Args,
) -> Result<u8, String> {
    let wait: u64 = args.number("wait", 15)?;
    let (rows, cols) = local_size();
    let (session, frames) = start(live, runtime, host, rows, cols);
    eprintln!("connecting to {host}… (Ctrl-] detaches; the shell keeps running)");
    if let Err(phase) = attached(&session, Instant::now() + Duration::from_secs(wait)) {
        return Err(phase.describe());
    }
    let _raw = RawMode::enter();
    let (keys, typed) = mpsc::channel::<Vec<u8>>();
    std::thread::spawn(move || {
        let mut stdin = std::io::stdin();
        let mut buffer = [0u8; 1024];
        loop {
            match stdin.read(&mut buffer) {
                Ok(0) | Err(_) => break,
                Ok(n) => {
                    if keys.send(buffer[..n].to_vec()).is_err() {
                        break;
                    }
                }
            }
        }
    });
    let mut stdout = std::io::stdout();
    let mut size = (rows, cols);
    let mut checked = Instant::now();
    loop {
        if let Ok(chunk) = frames.recv_timeout(Duration::from_millis(20)) {
            let _ = stdout.write_all(&chunk);
            let _ = stdout.flush();
        }
        while let Ok(bytes) = typed.try_recv() {
            if bytes.contains(&0x1d) {
                session.leave();
                drop(_raw);
                eprintln!("\r\ndetached; the shell keeps running on {host}");
                return Ok(0);
            }
            session.send(bytes);
        }
        if checked.elapsed() > Duration::from_millis(500) {
            checked = Instant::now();
            let now = local_size();
            if now != size {
                size = now;
                session.resize(now.0, now.1);
            }
        }
        let phase = session.model().phase.clone();
        if phase.ended() {
            while let Ok(chunk) = frames.try_recv() {
                let _ = stdout.write_all(&chunk);
            }
            let _ = stdout.flush();
            drop(_raw);
            let code = match &phase {
                Phase::Exited { code, signal, .. } => {
                    code.or_else(|| signal.map(|s| 128 + s)).unwrap_or(0)
                }
                _ => crate::EXIT_FAILURE.into(),
            };
            if output.json() {
                output.emit(
                    &json!({ "host": host, "exit": code, "shell": phase_json(&phase) }),
                    |_| String::new(),
                );
            } else {
                eprintln!("\r\n{}", phase.describe());
            }
            return Ok(u8::try_from(code).unwrap_or(255));
        }
    }
}

/// This terminal in raw mode until dropped.
struct RawMode {
    #[cfg(unix)]
    saved: Option<libc::termios>,
}

impl RawMode {
    fn enter() -> Self {
        #[cfg(unix)]
        {
            // SAFETY: termios is plain data; tcgetattr fills it and
            // tcsetattr reads it, both on stdin.
            let saved = unsafe {
                let mut term: libc::termios = std::mem::zeroed();
                if libc::tcgetattr(libc::STDIN_FILENO, &raw mut term) != 0 {
                    None
                } else {
                    let saved = term;
                    libc::cfmakeraw(&raw mut term);
                    libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const term);
                    Some(saved)
                }
            };
            RawMode { saved }
        }
        #[cfg(not(unix))]
        {
            RawMode {}
        }
    }
}

impl Drop for RawMode {
    fn drop(&mut self) {
        #[cfg(unix)]
        if let Some(saved) = self.saved {
            // SAFETY: restores the termios tcgetattr returned.
            unsafe {
                libc::tcsetattr(libc::STDIN_FILENO, libc::TCSANOW, &raw const saved);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::quote;

    #[test]
    fn quotes_for_a_posix_shell() {
        assert_eq!(quote("ls"), "ls");
        assert_eq!(quote("--model=gpt"), "--model=gpt");
        assert_eq!(quote("hello world"), "'hello world'");
        assert_eq!(quote("it's"), "'it'\\''s'");
        assert_eq!(quote(""), "''");
    }
}
