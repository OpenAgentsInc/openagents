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
pub fn quote(word: &str) -> String {
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

/// One command's run on a host: what it wrote, how it ended, and the route.
pub struct Run {
    pub output: String,
    pub exit: i32,
    pub timed_out: bool,
    pub phase: Phase,
    pub route: Option<String>,
    pub seconds: f64,
}

impl Run {
    pub fn json(&self, host: &str, words: &[String]) -> serde_json::Value {
        json!({
            "host": host, "command": words, "output": self.output, "exit": self.exit,
            "timed_out": self.timed_out, "shell": phase_json(&self.phase),
            "route": self.route, "seconds": self.seconds,
        })
    }
}

/// Run `words` on `host`, handing each output chunk to `sink` as it
/// arrives, and return the whole run once the command ends.
///
/// The command replaces the remote shell, so the shell's exit is the
/// command's exit. `--timeout` closes the shell and reports exit 124.
pub fn run(
    live: &Live,
    runtime: &tokio::runtime::Handle,
    host: &str,
    words: &[String],
    args: &Args,
    sink: &mut dyn FnMut(&[u8]),
) -> Result<Run, String> {
    let wait: u64 = args.number("wait", 15)?;
    let timeout: u64 = args.number("timeout", 600)?;
    let rows: u16 = args.number("rows", 50)?;
    let cols: u16 = args.number("cols", 200)?;
    let began = Instant::now();
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
                sink(&chunk);
                bytes.extend(chunk);
            }
            Err(mpsc::RecvTimeoutError::Timeout) => {}
            Err(mpsc::RecvTimeoutError::Disconnected) => break,
        }
        let phase = session.model().phase.clone();
        if phase.ended() {
            // Frames in flight arrive before the exit frame is applied.
            while let Ok(chunk) = frames.try_recv() {
                sink(&chunk);
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
    let exit = match &phase {
        Phase::Exited { code, signal, .. } => code.or_else(|| signal.map(|s| 128 + s)).unwrap_or(1),
        _ => crate::EXIT_FAILURE.into(),
    };
    let route = session.model().route.clone();
    Ok(Run {
        output: String::from_utf8_lossy(&bytes).replace("\r\n", "\n"),
        exit: if timed_out { 124 } else { exit },
        timed_out,
        phase,
        route,
        seconds: began.elapsed().as_secs_f64(),
    })
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
    let mut stdout = std::io::stdout();
    let mut sink = |chunk: &[u8]| {
        if !output.json() {
            let _ = stdout.write_all(chunk);
            let _ = stdout.flush();
        }
    };
    let run = run(live, runtime, host, words, args, &mut sink)?;
    crate::hosts::journal(args, "exec", host, words, &run);
    if output.json() {
        output.emit(&run.json(host, words), |_| String::new());
    } else if run.timed_out {
        eprintln!(
            "openagents computer exec: timed out after {}s",
            args.number("timeout", 600).unwrap_or(600)
        );
    } else if !matches!(run.phase, Phase::Exited { .. }) {
        eprintln!("openagents computer exec: {}", run.phase.describe());
    }
    Ok(u8::try_from(run.exit).unwrap_or(255))
}

/// Rerun `words` on `host` every `--every` seconds, printing each result as
/// it lands, until `--until TEXT` appears in the output, `--for` seconds
/// pass, or (with `--until-exit`) the command exits 0.
///
/// One long-lived command whose output is a record of what was seen, in
/// place of a sleep-and-look loop. `--json` prints one object per line.
pub fn watch(
    output: Output,
    live: &Live,
    runtime: &tokio::runtime::Handle,
    host: &str,
    words: &[String],
    args: &Args,
) -> Result<u8, String> {
    let every: u64 = args.number("every", 30)?;
    let limit: u64 = args.number("for", 3600)?;
    let until = args.option("until");
    let until_exit = args.switch("until-exit");
    let deadline = Instant::now() + Duration::from_secs(limit);
    let mut iteration = 0u64;
    loop {
        iteration += 1;
        let run = run(live, runtime, host, words, args, &mut |_| {})?;
        crate::hosts::journal(args, "watch", host, words, &run);
        let matched =
            until.is_some_and(|text| run.output.contains(text)) || (until_exit && run.exit == 0);
        let when = crate::hosts::now();
        if output.json() {
            let mut value = run.json(host, words);
            value["when"] = json!(when);
            value["iteration"] = json!(iteration);
            value["matched"] = json!(matched);
            println!("{value}");
        } else {
            println!(
                "--- {} #{iteration} exit {}{}",
                crate::hosts::clock(when),
                run.exit,
                if matched { " (matched)" } else { "" }
            );
            print!("{}", run.output);
            if !run.output.ends_with('\n') {
                println!();
            }
        }
        if matched {
            return Ok(0);
        }
        if Instant::now() + Duration::from_secs(every) >= deadline {
            if output.json() {
                println!(
                    "{}",
                    json!({ "host": host, "watch": "ended", "iterations": iteration, "matched": false })
                );
            } else {
                eprintln!("openagents computer watch: {limit}s passed without a match");
            }
            return Ok(crate::EXIT_FAILURE);
        }
        std::thread::sleep(Duration::from_secs(every));
    }
}

/// Print the last `--lines` lines of `path` on `host`, and with `--follow`
/// keep printing as the file grows (`tail -F`), one line per record.
pub fn tail(
    output: Output,
    live: &Live,
    runtime: &tokio::runtime::Handle,
    host: &str,
    path: &str,
    args: &Args,
) -> Result<u8, String> {
    let lines: u64 = args.number("lines", 20)?;
    let mut words = vec!["tail".to_owned(), "-n".to_owned(), lines.to_string()];
    if args.switch("follow") {
        words.push("-F".to_owned());
    }
    words.push("--".to_owned());
    words.push(path.to_owned());
    let mut stdout = std::io::stdout();
    let mut pending: Vec<u8> = Vec::new();
    let mut sink = |chunk: &[u8]| {
        if !output.json() {
            let _ = stdout.write_all(chunk);
            let _ = stdout.flush();
            return;
        }
        pending.extend_from_slice(chunk);
        while let Some(at) = pending.iter().position(|b| *b == b'\n') {
            let line: Vec<u8> = pending.drain(..=at).collect();
            let text = String::from_utf8_lossy(&line).trim_end().to_owned();
            println!(
                "{}",
                json!({ "host": host, "path": path, "when": crate::hosts::now(), "line": text })
            );
        }
    };
    let run = run(live, runtime, host, &words, args, &mut sink)?;
    crate::hosts::journal(args, "tail", host, &words, &run);
    if output.json() {
        println!(
            "{}",
            json!({ "host": host, "path": path, "exit": run.exit, "timed_out": run.timed_out, "shell": phase_json(&run.phase) })
        );
    }
    Ok(u8::try_from(run.exit).unwrap_or(255))
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
