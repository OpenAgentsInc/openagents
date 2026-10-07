//! `openagents browser run`: run a command beside a Chrome of its own, with
//! a fresh profile and a debugging port Chrome picks, so agents on one
//! machine can verify in a browser at the same time. The profile lives in
//! the session's scratch directory and is removed when the command ends.
//! `docs/coder/guides/browser.md` is the guide.

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

#[cfg(test)]
use coder::cli_route::tree::{Declared, Effect};
use coder_lease::{Broker, Error, Holder, Lease, Request, Resource, Wait};
use serde_json::json;

use crate::Output;

pub(crate) const USAGE: &str = "usage: openagents browser COMMAND
  run [--headed] [--browser PATH] [--timeout SECONDS] -- CMD [ARGS...]
                 Start Chrome with a fresh profile and a debugging port of
                 its own, run CMD, then end Chrome and remove the profile,
                 also when CMD fails or this command is interrupted. Exits
                 with CMD's status.
CMD gets OPENAGENTS_CHROME_PORT, the DevTools port on 127.0.0.1, and
OPENAGENTS_CHROME_WS, the browser's DevTools WebSocket URL. The profile is
chrome-<random> in this session's scratch directory (openagents scratch).
Chrome runs headless unless --headed is given. A headed Chrome opens a
window on the real screen, so it takes the browser and screen leases; the
screen lease needs the owner's grant (openagents lease grant screen).
The browser is --browser PATH, else $OPENAGENTS_CHROME, else Google Chrome
or Chromium where they are usually installed. --timeout SECONDS (default
30) bounds how long Chrome may take to open its port. With --json, a
summary is printed after CMD's output.";

/// What the command does, for the chat router's command tree
/// (`coder::cli_route::tree`).
#[cfg(test)]
pub(crate) const EFFECTS: &[Declared] = &[Declared::computer("run", Effect::LongRunning)];

/// Names the browser when `--browser` doesn't.
const CHROME_VAR: &str = "OPENAGENTS_CHROME";
/// The port CMD gets.
const PORT_VAR: &str = "OPENAGENTS_CHROME_PORT";
/// The WebSocket URL CMD gets.
const WS_VAR: &str = "OPENAGENTS_CHROME_WS";
/// How long Chrome may take to write `DevToolsActivePort`, by default.
const DEFAULT_TIMEOUT: Duration = Duration::from_secs(30);

pub fn run(output: &Output, words: &[String]) -> u8 {
    match words.first().map(String::as_str) {
        None => output.usage("browser", "a command is required: run", USAGE),
        Some("help" | "-h" | "--help") => {
            println!("{USAGE}");
            0
        }
        Some("run") => match parse_run(&words[1..]) {
            Ok(run) => start(output, &run),
            Err(message) => output.usage("browser", &message, USAGE),
        },
        Some(other) => output.usage("browser", &format!("unknown command `{other}`"), USAGE),
    }
}

/// `openagents browser run [OPTIONS] -- CMD [ARGS...]`, parsed.
#[derive(Debug, PartialEq)]
struct Run {
    headed: bool,
    browser: Option<PathBuf>,
    timeout: Duration,
    command: Vec<String>,
}

fn parse_run(words: &[String]) -> Result<Run, String> {
    let split = words
        .iter()
        .position(|word| word == "--")
        .ok_or("put the command after `--`: openagents browser run -- CMD [ARGS...]")?;
    let (options, command) = (&words[..split], &words[split + 1..]);
    if command.is_empty() {
        return Err("a command is required after `--`".to_owned());
    }
    let args = crate::argv::parse_command(
        options,
        "browser run",
        &["browser", "timeout"],
        &["headed"],
        0,
        0,
    )?;
    let timeout = match args.option("timeout") {
        None => DEFAULT_TIMEOUT,
        Some(text) => text
            .parse::<u64>()
            .ok()
            .filter(|seconds| *seconds > 0)
            .map(Duration::from_secs)
            .ok_or_else(|| {
                format!("--timeout is `{text}`, not a whole number of seconds above 0")
            })?,
    };
    Ok(Run {
        headed: args.switch("headed"),
        browser: args.option("browser").map(PathBuf::from),
        timeout,
        command: command.to_vec(),
    })
}

/// The browser to start: `--browser`, else `$OPENAGENTS_CHROME`, else the
/// first Chrome or Chromium found where it is usually installed.
fn find_browser(given: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(path) = given {
        return Ok(path.to_owned());
    }
    if let Some(path) = std::env::var_os(CHROME_VAR).filter(|path| !path.is_empty()) {
        return Ok(PathBuf::from(path));
    }
    let mut candidates: Vec<PathBuf> = Vec::new();
    if cfg!(target_os = "macos") {
        let apps = [
            "Google Chrome.app/Contents/MacOS/Google Chrome",
            "Chromium.app/Contents/MacOS/Chromium",
        ];
        for app in apps {
            candidates.push(Path::new("/Applications").join(app));
            if let Some(home) = std::env::var_os("HOME") {
                candidates.push(Path::new(&home).join("Applications").join(app));
            }
        }
    }
    let names = [
        "google-chrome",
        "google-chrome-stable",
        "chromium",
        "chromium-browser",
    ];
    if let Some(path) = std::env::var_os("PATH") {
        for dir in std::env::split_paths(&path) {
            candidates.extend(names.iter().map(|name| dir.join(name)));
        }
    }
    candidates.into_iter().find(|path| path.is_file()).ok_or_else(|| {
        format!(
            "no Chrome or Chromium was found; install one, or name it with --browser PATH or {CHROME_VAR}"
        )
    })
}

/// The leases a headed run holds: the screen and the browser.
fn headed_leases(command: &str) -> Result<Vec<Lease>, String> {
    let broker = Broker::from_env().map_err(|error| error.to_string())?;
    let mut leases = Vec::new();
    for resource in [Resource::Screen, Resource::Browser] {
        let request = Request::new(resource.clone(), Holder::detect(command))
            .wait(Wait::Forever)
            .inherit_env();
        let lease = broker
            .acquire_notify(request, &mut |blocked| {
                eprintln!(
                    "openagents browser: waiting for the {resource} lease: {}",
                    blocked.reason
                );
            })
            .map_err(|error| match error {
                Error::NoGrant(reason) => format!(
                    "a headed browser opens a window on the real screen, which needs the owner's grant: {reason}. \
                     The owner grants it with `openagents lease grant screen`; or run without --headed."
                ),
                error => format!("the {resource} lease was not taken: {error}"),
            })?;
        leases.push(lease);
    }
    Ok(leases)
}

/// The variables a headed run's CMD gets from its leases: the browser
/// lease's, with `OPENAGENTS_LEASES` naming the screen too, so a lease CMD
/// takes inside doesn't wait on this one.
fn lease_env(leases: &[Lease]) -> Vec<(String, String)> {
    let Some(last) = leases.last() else {
        return Vec::new();
    };
    let mut env = last.env();
    for (name, value) in &mut env {
        if name == "OPENAGENTS_LEASES" {
            let mut names: Vec<String> = value
                .split(',')
                .filter(|name| !name.is_empty())
                .map(str::to_owned)
                .collect();
            for lease in leases {
                if !names.contains(&lease.entry().resource) {
                    names.push(lease.entry().resource.clone());
                }
            }
            *value = names.join(",");
        }
    }
    env
}

/// Set when this process is asked to stop while Chrome starts.
static STOP: AtomicBool = AtomicBool::new(false);

#[cfg(unix)]
extern "C" fn stop(_signal: libc::c_int) {
    STOP.store(true, Ordering::Relaxed);
}

/// Makes an interrupt while Chrome starts end the wait, so Chrome and its
/// profile are still cleaned up. Running CMD replaces these handlers with
/// ones that pass the signal to CMD.
fn catch_stop() {
    #[cfg(unix)]
    // SAFETY: installing a handler that only stores to an atomic.
    unsafe {
        for signal in [libc::SIGINT, libc::SIGTERM, libc::SIGHUP, libc::SIGQUIT] {
            libc::signal(signal, stop as *const () as libc::sighandler_t);
        }
    }
}

/// A started Chrome: its process group and the profile it owns.
struct Chrome {
    child: Option<Child>,
    profile: PathBuf,
}

impl Chrome {
    /// Ends Chrome's process group, then removes the profile. Returns
    /// whether the profile is gone.
    fn end(&mut self) -> bool {
        if let Some(mut child) = self.child.take() {
            // A zero deadline asks the group to stop, waits out the grace
            // period, and kills what is left.
            let _ = supervise::blocking::wait(&mut child, Duration::ZERO);
        }
        // A helper that outlived the group can still be writing; try again
        // briefly.
        for _ in 0..20 {
            match std::fs::remove_dir_all(&self.profile) {
                Err(error) if error.kind() != std::io::ErrorKind::NotFound => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                _ => break,
            }
        }
        !self.profile.exists()
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        self.end();
    }
}

/// Starts Chrome on a fresh profile under `scratch` and waits for its
/// port: the started browser, the port, and the WebSocket URL.
fn launch(
    browser: &Path,
    scratch: &Path,
    headed: bool,
    timeout: Duration,
) -> Result<(Chrome, u16, String), String> {
    let id = uuid::Uuid::new_v4().simple().to_string();
    let profile = scratch.join(format!("chrome-{}", &id[..12]));
    std::fs::create_dir_all(&profile).map_err(|error| {
        format!(
            "the profile {} could not be made: {error}",
            profile.display()
        )
    })?;
    let mut chrome = Chrome {
        child: None,
        profile,
    };
    let log_path = chrome.profile.join("browser.log");
    let log = std::fs::File::create(&log_path)
        .map_err(|error| format!("{} could not be written: {error}", log_path.display()))?;
    let mut command = Command::new(browser);
    command
        .arg(format!("--user-data-dir={}", chrome.profile.display()))
        .arg("--remote-debugging-port=0")
        .args([
            "--no-first-run",
            "--no-default-browser-check",
            "--disable-background-networking",
            "--disable-sync",
            // Keep the owner's keychain and password store out of it.
            "--use-mock-keychain",
            "--password-store=basic",
        ]);
    if !headed {
        command.arg("--headless=new");
    }
    command
        .arg("about:blank")
        .stdin(Stdio::null())
        .stdout(log.try_clone().map_err(|error| error.to_string())?)
        .stderr(log);
    supervise::blocking::own_group(&mut command);
    let child = command
        .spawn()
        .map_err(|error| format!("`{}` did not start: {error}", browser.display()))?;
    chrome.child = Some(child);
    let active = chrome.profile.join("DevToolsActivePort");
    let deadline = Instant::now() + timeout;
    loop {
        if let Some((port, path)) = read_active_port(&active) {
            let ws = format!("ws://127.0.0.1:{port}{path}");
            return Ok((chrome, port, ws));
        }
        if STOP.load(Ordering::Relaxed) {
            return Err("interrupted while the browser started".to_owned());
        }
        if let Some(child) = chrome.child.as_mut()
            && let Ok(Some(status)) = child.try_wait()
        {
            return Err(format!(
                "`{}` exited ({status}) before it opened a debugging port: {}",
                browser.display(),
                log_tail(&log_path)
            ));
        }
        if Instant::now() >= deadline {
            return Err(format!(
                "`{}` did not open a debugging port within {}s: {}",
                browser.display(),
                timeout.as_secs(),
                log_tail(&log_path)
            ));
        }
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The port and browser path in a complete `DevToolsActivePort` file:
/// the port on the first line, `/devtools/browser/ID` on the second.
fn read_active_port(path: &Path) -> Option<(u16, String)> {
    let text = std::fs::read_to_string(path).ok()?;
    let mut lines = text.lines();
    let port = lines
        .next()?
        .trim()
        .parse::<u16>()
        .ok()
        .filter(|port| *port > 0)?;
    let path = lines.next()?.trim();
    path.starts_with('/').then(|| (port, path.to_owned()))
}

/// The last few lines Chrome wrote, for a startup failure.
fn log_tail(path: &Path) -> String {
    let text = std::fs::read_to_string(path).unwrap_or_default();
    let lines: Vec<&str> = text
        .lines()
        .filter(|line| !line.trim().is_empty())
        .collect();
    let tail = lines[lines.len().saturating_sub(3)..].join(" | ");
    if tail.is_empty() {
        "it wrote nothing".to_owned()
    } else {
        tail
    }
}

fn start(output: &Output, run: &Run) -> u8 {
    let browser = match find_browser(run.browser.as_deref()) {
        Ok(browser) => browser,
        Err(message) => return output.fail("browser", &message),
    };
    let scratch = match crate::scratch::locate(None) {
        Ok(value) => PathBuf::from(value["path"].as_str().unwrap_or_default()),
        Err(message) => return output.fail("browser", &message),
    };
    let leases = if run.headed {
        match headed_leases(&run.command[0]) {
            Ok(leases) => leases,
            Err(message) => return output.fail("browser", &message),
        }
    } else {
        Vec::new()
    };
    catch_stop();
    let (mut chrome, port, ws) = match launch(&browser, &scratch, run.headed, run.timeout) {
        Ok(started) => started,
        Err(message) => return output.fail("browser", &message),
    };
    let profile = chrome.profile.clone();
    let mut env = lease_env(&leases);
    env.push((PORT_VAR.to_owned(), port.to_string()));
    env.push((WS_VAR.to_owned(), ws.clone()));
    let (exit, failure) = crate::lease::run_command(&run.command, &env);
    let removed = chrome.end();
    for lease in leases {
        let _ = lease.release(exit);
    }
    if let Some(message) = &failure {
        eprintln!("openagents browser: {message}");
    }
    if !removed {
        eprintln!(
            "openagents browser: the profile {} could not be removed",
            profile.display()
        );
    }
    if output.json() {
        println!(
            "{}",
            json!({
                "browser": browser.display().to_string(),
                "headed": run.headed,
                "port": port,
                "ws": ws,
                "profile": profile.display().to_string(),
                "profile_removed": removed,
                "exit": exit,
            })
        );
    }
    match exit {
        Some(code) => u8::try_from(code & 0xff).unwrap_or(crate::EXIT_FAILURE),
        None => crate::EXIT_FAILURE,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn words(text: &[&str]) -> Vec<String> {
        text.iter().map(|word| (*word).to_owned()).collect()
    }

    #[test]
    fn run_parses_its_options_and_the_command() {
        let run = parse_run(&words(&[
            "--headed",
            "--browser",
            "/opt/chrome",
            "--timeout",
            "5",
            "--",
            "python3",
            "smoke.py",
            "--headed",
        ]))
        .unwrap();
        assert_eq!(
            run,
            Run {
                headed: true,
                browser: Some(PathBuf::from("/opt/chrome")),
                timeout: Duration::from_secs(5),
                command: words(&["python3", "smoke.py", "--headed"]),
            }
        );
        assert!(parse_run(&words(&["python3"])).is_err());
        assert!(parse_run(&words(&["--"])).is_err());
        assert!(parse_run(&words(&["--timeout", "0", "--", "true"])).is_err());
        assert!(parse_run(&words(&["extra", "--", "true"])).is_err());
    }

    #[test]
    fn the_active_port_file_counts_only_when_complete() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("DevToolsActivePort");
        assert_eq!(read_active_port(&path), None);
        std::fs::write(&path, "41234\n").unwrap();
        assert_eq!(read_active_port(&path), None);
        std::fs::write(&path, "41234\n/devtools/browser/abc\n").unwrap();
        assert_eq!(
            read_active_port(&path),
            Some((41234, "/devtools/browser/abc".to_owned()))
        );
    }
}
