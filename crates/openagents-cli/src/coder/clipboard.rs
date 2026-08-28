//! Clipboard writing for the coder TUI, with an honest answer about whether
//! the write landed.
//!
//! The design follows grok-build's stack (see `#300`): every enabled route
//! fires, each write is hardened against wedged helper processes, and the
//! toast reports what the environment actually supports rather than what the
//! call returned.
//!
//! Three routes exist:
//!
//! - **native** — the operating system clipboard, through whatever helper
//!   binary the platform ships (`pbcopy`, `wl-copy`, `xclip`, `xsel`, `clip`).
//! - **tmux buffer** — `tmux load-buffer -`, when tmux is the immediate
//!   terminal.
//! - **OSC 52** — `\x1b]52;c;<base64>\x07` on stderr, the one route that
//!   crosses tmux, SSH, and displayless containers to reach the terminal the
//!   person is actually looking at.
//!
//! A route is a promise, not a result. Delivering the payload somewhere the
//! user cannot paste it while saying "Copied!" is the failure mode this
//! module exists to avoid, so [`Delivery`] distinguishes a *confirmed* write
//! (a trusted backend accepted it locally, or the terminal brand is known to
//! honor OSC 52) from an *unverified* one (OSC 52 was emitted across an
//! unknown hop). Unverified copies still have the backup file, which the
//! toast names.

use std::sync::OnceLock;

use base64::Engine as _;

/// Env var that turns the OSC 52 route off. Terminals that do not implement
/// OSC 52 paint the base64 payload as visible garbage, so presence of this
/// variable — any value — is a hard kill switch.
const NO_OSC52_ENV: &str = "OPENAGENTS_CLIPBOARD_NO_OSC52";

/// Env var overriding where the copy backup file is written. A leading `~`
/// expands against `$HOME`.
pub const COPY_FILE_ENV: &str = "OPENAGENTS_CLIPBOARD_COPY_FILE";

/// How long a clipboard helper child may run before it is killed. `wl-copy`
/// daemonizes, `xclip` waits for selection ownership, and a tmux server can
/// wedge; an unbounded wait on any of them freezes the frame loop.
const HELPER_DEADLINE: std::time::Duration = std::time::Duration::from_secs(2);

/// Whether a clipboard write is believed to have reached a pasteable place.
///
/// The distinction is the point. `Confirmed` backs the plain success toast;
/// `Unverified` means the bytes left the process toward a terminal that may
/// or may not act on them, and the user is told where the backup file is
/// rather than being wished luck.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Delivery {
    /// A trusted backend accepted the write for the environment the user is
    /// in, or the terminal is known to honor OSC 52.
    Confirmed,
    /// OSC 52 was emitted across an unknown hop (SSH, container, or a
    /// terminal brand this process could not identify).
    Unverified,
    /// No route can reach a clipboard the user can paste from.
    Failed,
}

impl Delivery {
    /// Whether a success-shaped toast may claim the copy worked.
    pub fn reported_success(self) -> bool {
        matches!(self, Delivery::Confirmed | Delivery::Unverified)
    }
}

/// Which OSC 52 dialect the immediate terminal speaks, as far as this
/// process can know. Derived from `TERM_PROGRAM`, `TERM`, and the brand
/// variables the common terminals set; an unrecognized terminal is
/// [`TerminalBrand::Unknown`], which downgrades remote OSC 52 copies from
/// confirmed to unverified.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum TerminalBrand {
    /// Known-good OSC 52 implementations.
    SupportsOsc52,
    /// A brand with no OSC 52 support. Apple Terminal is the one that
    /// matters: it is common over SSH, and an OSC 52 payload renders there
    /// as text.
    NoOsc52,
    /// Not recognized. Under an unknown brand the OSC 52 write is a guess.
    Unknown,
}

fn terminal_brand() -> TerminalBrand {
    // Terminals that document OSC 52 clipboard support, matched against
    // TERM_PROGRAM first and TERM second.
    for marker in [
        "iTerm.app",
        "ghostty",
        "xterm-ghostty",
        "kitty",
        "WezTerm",
        "Alacritty",
        "rio",
        "contour",
        "foot",
    ] {
        if env_is(marker) {
            return TerminalBrand::SupportsOsc52;
        }
    }
    // VS Code, Cursor, Windsurf, Zed, and JetBrains all embed xterm.js or a
    // derivative that honors OSC 52.
    for marker in ["vscode", "VSCodium", "windsurf", "Zed", "JetBrains"] {
        if env_is(marker) {
            return TerminalBrand::SupportsOsc52;
        }
    }
    // Windows Terminal accepts OSC 52; it identifies itself through
    // WT_SESSION rather than TERM_PROGRAM.
    if std::env::var_os("WT_SESSION").is_some() {
        return TerminalBrand::SupportsOsc52;
    }
    // tmux passes OSC 52 through to the outer terminal when it is the
    // immediate terminal, so treating it as capable is what keeps copies
    // working across `tmux + ssh` hops where the outer brand is invisible.
    if std::env::var_os("TMUX").is_some() {
        return TerminalBrand::SupportsOsc52;
    }
    if env_is("Apple_Terminal") {
        return TerminalBrand::NoOsc52;
    }
    TerminalBrand::Unknown
}

fn env_is(value: &str) -> bool {
    let program = std::env::var("TERM_PROGRAM").unwrap_or_default();
    if program.eq_ignore_ascii_case(value) {
        return true;
    }
    std::env::var("TERM").is_ok_and(|term| term.eq_ignore_ascii_case(value))
}

/// Whether this process is on the far side of an SSH hop, where the native
/// clipboard belongs to the remote host and not to the user's desk.
fn is_remote() -> bool {
    std::env::var_os("SSH_CONNECTION").is_some()
        || std::env::var_os("SSH_TTY").is_some()
        || std::env::var_os("SSH_CLIENT").is_some()
}

/// Whether this process runs in a container with no display server. There
/// the native clipboard cannot exist and OSC 52 is the only route out.
fn is_displayless_container() -> bool {
    if std::env::var_os("DISPLAY").is_some() || std::env::var_os("WAYLAND_DISPLAY").is_some() {
        return false;
    }
    std::path::Path::new("/.dockerenv").exists()
        || std::path::Path::new("/run/.containerenv").exists()
        || std::env::var_os("container").is_some()
}

/// The routes this process will fire on each copy, resolved once.
///
/// Resolution is cached because every input is environment state that cannot
/// change while the process runs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct ClipboardRoute {
    native: bool,
    tmux_buffer: bool,
    osc52: bool,
    /// Wrap the OSC 52 sequence in tmux's DCS passthrough envelope. Only
    /// correct when tmux is the immediate terminal; inside an editor's
    /// embedded terminal the envelope renders as garbage.
    osc52_tmux_passthrough: bool,
}

fn resolve_route() -> ClipboardRoute {
    let tmux = std::env::var_os("TMUX").is_some();
    let osc52 = std::env::var_os(NO_OSC52_ENV).is_none()
        && (cfg!(target_os = "linux") || tmux || is_remote() || is_displayless_container());
    ClipboardRoute {
        native: true,
        tmux_buffer: tmux,
        osc52,
        osc52_tmux_passthrough: osc52 && tmux,
    }
}

/// Cached environment facts the delivery decision needs, resolved once.
#[derive(Debug, Clone, Copy)]
struct Environment {
    route: ClipboardRoute,
    brand: TerminalBrand,
    remote: bool,
    container: bool,
}

fn environment() -> &'static Environment {
    static ENV: OnceLock<Environment> = OnceLock::new();
    ENV.get_or_init(|| Environment {
        route: resolve_route(),
        brand: terminal_brand(),
        remote: is_remote(),
        container: is_displayless_container(),
    })
}

/// The per-route outcomes of one copy, used by the delivery decision.
#[derive(Debug, Default)]
struct WriteLegs {
    native_ok: bool,
    tmux_ok: bool,
    osc52_ok: bool,
}

/// Copy text through every enabled route and classify what happened.
///
/// This is the entry point the TUI calls. It never blocks longer than
/// [`HELPER_DEADLINE`] per helper child and never panics on clipboard
/// failure: a clipboard that cannot be written is a fact to report, not an
/// error to raise.
pub fn copy_text(text: &str) -> Delivery {
    if text.is_empty() {
        return Delivery::Failed;
    }
    let env = environment();
    let mut legs = WriteLegs::default();
    if env.route.native {
        legs.native_ok = backend::set_text(text, HELPER_DEADLINE);
    }
    if env.route.tmux_buffer {
        legs.tmux_ok = write_tmux_buffer(text);
    }
    if env.route.osc52 {
        legs.osc52_ok = write_osc52(text, env.route.osc52_tmux_passthrough);
    }
    delivery_for_legs(&legs, env)
}

/// Classify the write from the route outcomes and the environment.
///
/// A local trusted write always confirms. Otherwise a successful OSC 52
/// confirms only when the terminal brand is known to honor it; over SSH or
/// from a container with an unknown brand the honest answer is unverified.
/// tmux's own paste buffer is a real destination but not the user's
/// clipboard, so it confirms only when it is the route that worked.
fn delivery_for_legs(legs: &WriteLegs, env: &Environment) -> Delivery {
    if legs.native_ok && !env.remote && !env.container {
        return Delivery::Confirmed;
    }
    if legs.osc52_ok {
        match env.brand {
            TerminalBrand::SupportsOsc52 => return Delivery::Confirmed,
            TerminalBrand::Unknown if env.remote || env.container => {
                return Delivery::Unverified;
            }
            TerminalBrand::Unknown | TerminalBrand::NoOsc52 => {}
        }
    }
    if legs.tmux_ok {
        return Delivery::Confirmed;
    }
    Delivery::Failed
}

/// Write text into the tmux paste buffer.
fn write_tmux_buffer(text: &str) -> bool {
    let spooled = match backend::spool_stdin(text.as_bytes()) {
        Ok(file) => file,
        Err(_) => return false,
    };
    let mut child = match std::process::Command::new("tmux")
        .args(["load-buffer", "-"])
        .stdin(std::process::Stdio::from(spooled))
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return false,
    };
    backend::wait_with_deadline(&mut child, HELPER_DEADLINE).is_ok_and(|status| status.success())
}

/// Build the OSC 52 clipboard-set sequence. Pure so tests can pin the bytes.
fn osc52_sequence(text: &str, tmux_passthrough: bool) -> String {
    let encoded = base64::engine::general_purpose::STANDARD.encode(text.as_bytes());
    if tmux_passthrough {
        // tmux's passthrough wraps the inner sequence in a DCS, and every
        // ESC inside the payload must be doubled so tmux does not end the
        // wrapper early. The base64 body contains none, so only the inner
        // introducer needs it.
        format!("\x1bPtmux;\x1b\x1b]52;c;{encoded}\x07\x1b\\")
    } else {
        format!("\x1b]52;c;{encoded}\x07")
    }
}

/// Emit the OSC 52 clipboard-set sequence on stderr.
fn write_osc52(text: &str, tmux_passthrough: bool) -> bool {
    use std::io::Write;
    let sequence = osc52_sequence(text, tmux_passthrough);
    let stderr = std::io::stderr();
    let mut handle = stderr.lock();
    handle.write_all(sequence.as_bytes()).is_ok() && handle.flush().is_ok()
}

/// The default path of the copy backup file: `~/.openagents/last-copy.txt`.
///
/// `None` when no home directory resolves and no override is set. A missing
/// home skips the file rather than writing a world-readable copy to a
/// predictable temp path.
pub fn default_copy_path() -> Option<std::path::PathBuf> {
    if let Ok(raw) = std::env::var(COPY_FILE_ENV) {
        let trimmed = raw.trim();
        if !trimmed.is_empty() {
            return Some(expand_tilde(trimmed));
        }
    }
    std::env::var_os("HOME")?;
    Some(
        crate::auth::home_directory()
            .join(".openagents")
            .join("last-copy.txt"),
    )
}

fn expand_tilde(raw: &str) -> std::path::PathBuf {
    match raw.strip_prefix("~/") {
        Some(rest) => crate::auth::home_directory().join(rest),
        None => std::path::PathBuf::from(raw),
    }
}

/// Write the copy backup file, owner-readable only.
///
/// Copied text can be secret and the default path is predictable, so the
/// file is `0600` on unix and its parent is created `0700`. A pre-existing
/// world-readable file is tightened, because the create-time mode only
/// applies to new files.
pub fn write_copy_file(text: &str) -> std::io::Result<std::path::PathBuf> {
    let path = default_copy_path().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "no home directory resolves; set OPENAGENTS_CLIPBOARD_COPY_FILE to enable the copy backup file",
        )
    })?;
    if let Some(parent) = path.parent() {
        #[cfg(unix)]
        {
            use std::os::unix::fs::DirBuilderExt;
            std::fs::DirBuilder::new()
                .recursive(true)
                .mode(0o700)
                .create(parent)?;
        }
        #[cfg(not(unix))]
        std::fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    {
        use std::io::Write;
        use std::os::unix::fs::{OpenOptionsExt, PermissionsExt};
        let mut file = std::fs::OpenOptions::new()
            .write(true)
            .create(true)
            .truncate(true)
            .mode(0o600)
            .open(&path)?;
        file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
        file.write_all(text.as_bytes())?;
    }
    #[cfg(not(unix))]
    std::fs::write(&path, text)?;
    Ok(path)
}

/// Where the copy landed, and what the toast should say about it.
///
/// The backup file is written on every copy, not only the failed ones: the
/// write is cheap, and "the last thing you copied" being a fact about the
/// filesystem rather than the terminal makes every copy recoverable.
pub struct CopyOutcome {
    pub delivery: Delivery,
    /// The backup file path, when the file write succeeded.
    pub backup: Option<std::path::PathBuf>,
}

impl CopyOutcome {
    /// The user-facing one-line result. A confirmed copy names nothing; an
    /// unverified or failed copy names the backup file, because that file is
    /// the recovery path this toast exists to point at.
    pub fn toast_message(&self) -> String {
        match (self.delivery, &self.backup) {
            (Delivery::Confirmed, _) => "Copied!".to_string(),
            (Delivery::Unverified, Some(path)) => {
                format!("Copy sent; if paste fails, read {}", path.display())
            }
            (Delivery::Unverified, None) => {
                "Copy sent; delivery could not be verified.".to_string()
            }
            (Delivery::Failed, Some(path)) => {
                format!("Copy failed; text saved to {}", path.display())
            }
            (Delivery::Failed, None) => {
                "Copy failed, and the backup file could not be written.".to_string()
            }
        }
    }
}

/// Copy text through every route and always attempt the backup file.
pub fn copy_text_with_backup(text: &str) -> CopyOutcome {
    let delivery = copy_text(text);
    let backup = write_copy_file(text).ok();
    CopyOutcome { delivery, backup }
}

/// Platform helper processes for the native clipboard leg.
///
/// The TUI already depends on a terminal, a shell environment, and nothing
/// fancier, so the native leg shells out to whatever helper the platform
/// ships instead of pulling in an AppKit/Wayland client stack. Each helper
/// gets the payload through a spooled file rather than a pipe — a helper
/// that stops draining stdin would otherwise block the frame loop once the
/// payload passed the pipe buffer.
mod backend {
    use std::io::Write as _;
    use std::process::Child;
    use std::process::ExitStatus;
    use std::time::Duration;
    use std::time::Instant;

    /// The native helpers to try, in order, with their arguments.
    ///
    /// First exit-0 wins. `xclip` and `xsel` hold selection ownership until
    /// killed, so they are spawned with the payload on stdin and reaped by
    /// the deadline wait even when they would rather linger.
    fn helpers() -> &'static [(&'static str, &'static [&'static str])] {
        if cfg!(target_os = "macos") {
            &[("pbcopy", &[])]
        } else if cfg!(target_os = "windows") {
            &[("clip", &[])]
        } else {
            &[
                ("wl-copy", &[]),
                ("xclip", &["-selection", "clipboard"]),
                ("xsel", &["--clipboard", "--input"]),
            ]
        }
    }

    /// Write text to the system clipboard through the first helper that
    /// exits successfully within `deadline`. Returns `false` when no helper
    /// exists, fails, or hangs.
    pub fn set_text(text: &str, deadline: Duration) -> bool {
        for (program, args) in helpers() {
            let spooled = match spool_stdin(text.as_bytes()) {
                Ok(file) => file,
                Err(_) => continue,
            };
            let mut child = match std::process::Command::new(program)
                .args(*args)
                .stdin(std::process::Stdio::from(spooled))
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn()
            {
                Ok(child) => child,
                Err(_) => continue,
            };
            if wait_with_deadline(&mut child, deadline).is_ok_and(|status| status.success()) {
                return true;
            }
        }
        false
    }

    /// Wait for a child, bounded by `deadline`.
    ///
    /// Polls `try_wait`; on expiry kills and reaps the child. Clipboard
    /// helpers can hang on a stuck compositor or tmux server, and an
    /// unbounded `wait()` would freeze the UI thread. The caller must close
    /// or hand off stdin first: unlike `wait()`, the poll loop never drops
    /// it, so a child still reading a held pipe would burn the deadline.
    pub fn wait_with_deadline(
        child: &mut Child,
        deadline: Duration,
    ) -> std::io::Result<ExitStatus> {
        let started = Instant::now();
        loop {
            if let Some(status) = child.try_wait()? {
                return Ok(status);
            }
            if started.elapsed() >= deadline {
                let _ = child.kill();
                let _ = child.wait();
                return Err(std::io::Error::new(
                    std::io::ErrorKind::TimedOut,
                    "clipboard helper did not exit in time",
                ));
            }
            std::thread::sleep(Duration::from_millis(15));
        }
    }

    /// Spool `data` to a `0600` temp file and return a read handle to feed a
    /// helper's stdin.
    ///
    /// A regular file is fully written before spawn and needs no writer
    /// afterwards, so a helper that never drains its stdin cannot block the
    /// spooler. On unix the file is unlinked before returning: the open read
    /// handle (and the child's dup of it) stays valid, and nothing readable
    /// is left on disk. On Windows the handle to delete with an open file
    /// does not exist, so the file is removed best-effort after the fact.
    pub fn spool_stdin(data: &[u8]) -> std::io::Result<std::fs::File> {
        let mut path = std::env::temp_dir();
        path.push(format!(
            "oa-clipboard-{}-{}",
            std::process::id(),
            Instant::now().elapsed().subsec_nanos()
        ));
        #[cfg(unix)]
        let file = {
            use std::os::unix::fs::OpenOptionsExt;
            std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .mode(0o600)
                .open(&path)?
        };
        #[cfg(not(unix))]
        let file = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let mut file = file;
        file.write_all(data)?;
        file.sync_all().ok();
        drop(file);
        let read = std::fs::File::open(&path)?;
        // Failing the unlink is fine: the file is 0600 (on unix) in the
        // system temp dir and will be cleaned by the OS.
        std::fs::remove_file(&path).ok();
        Ok(read)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // The route and delivery decisions depend on process environment state
    // that caches once per run, so the hermetic tests pin the pure pieces:
    // the sequence bytes, the delivery matrix, and the toast copy. The
    // full route is exercised by hand against tmux, ssh, and plain
    // terminals.

    #[test]
    fn osc52_sequence_is_the_documented_bytes() {
        // Base64 of "hi" is "aGk=".
        assert_eq!(osc52_sequence("hi", false), "\x1b]52;c;aGk=\x07");
        assert_eq!(
            osc52_sequence("hi", true),
            "\x1bPtmux;\x1b\x1b]52;c;aGk=\x07\x1b\\"
        );
    }

    #[test]
    fn toast_messages_name_the_backup_only_when_needed() {
        let confirmed = CopyOutcome {
            delivery: Delivery::Confirmed,
            backup: Some(std::path::PathBuf::from("/tmp/x")),
        };
        assert_eq!(confirmed.toast_message(), "Copied!");
        let unverified = CopyOutcome {
            delivery: Delivery::Unverified,
            backup: Some(std::path::PathBuf::from("/tmp/x")),
        };
        assert!(unverified.toast_message().contains("/tmp/x"));
        let failed = CopyOutcome {
            delivery: Delivery::Failed,
            backup: None,
        };
        assert!(failed.toast_message().contains("could not be written"));
    }

    #[test]
    fn delivery_states_report_success_exactly_as_documented() {
        assert!(Delivery::Confirmed.reported_success());
        assert!(Delivery::Unverified.reported_success());
        assert!(!Delivery::Failed.reported_success());
    }

    fn environment(brand: TerminalBrand, remote: bool, container: bool) -> Environment {
        Environment {
            route: ClipboardRoute {
                native: true,
                tmux_buffer: false,
                osc52: true,
                osc52_tmux_passthrough: false,
            },
            brand,
            remote,
            container,
        }
    }

    fn legs(native_ok: bool, tmux_ok: bool, osc52_ok: bool) -> WriteLegs {
        WriteLegs {
            native_ok,
            tmux_ok,
            osc52_ok,
        }
    }

    #[test]
    fn local_native_write_confirms() {
        let env = environment(TerminalBrand::Unknown, false, false);
        assert_eq!(
            delivery_for_legs(&legs(true, false, false), &env),
            Delivery::Confirmed
        );
    }

    #[test]
    fn remote_native_write_does_not_confirm_locally() {
        // The native leg on the far side of SSH wrote the remote host's
        // clipboard, which the user cannot paste from.
        let env = environment(TerminalBrand::Unknown, true, false);
        assert_eq!(
            delivery_for_legs(&legs(true, false, false), &env),
            Delivery::Failed
        );
    }

    #[test]
    fn known_osc52_terminal_confirms_over_ssh() {
        let env = environment(TerminalBrand::SupportsOsc52, true, false);
        assert_eq!(
            delivery_for_legs(&legs(false, false, true), &env),
            Delivery::Confirmed
        );
    }

    #[test]
    fn unknown_brand_over_ssh_is_unverified_not_failed() {
        let env = environment(TerminalBrand::Unknown, true, false);
        assert_eq!(
            delivery_for_legs(&legs(false, false, true), &env),
            Delivery::Unverified
        );
    }

    #[test]
    fn unknown_brand_locally_cannot_confirm_osc52() {
        // Locally, an unknown terminal means the payload may render as
        // garbage; there is no unknown-hop story to fall back on.
        let env = environment(TerminalBrand::Unknown, false, false);
        assert_eq!(
            delivery_for_legs(&legs(false, false, true), &env),
            Delivery::Failed
        );
    }

    #[test]
    fn apple_terminal_never_confirms_osc52() {
        let env = environment(TerminalBrand::NoOsc52, true, false);
        assert_eq!(
            delivery_for_legs(&legs(false, false, true), &env),
            Delivery::Failed
        );
    }

    #[test]
    fn tmux_buffer_confirms_when_it_is_what_worked() {
        let env = environment(TerminalBrand::NoOsc52, true, false);
        assert_eq!(
            delivery_for_legs(&legs(false, true, false), &env),
            Delivery::Confirmed
        );
    }

    #[test]
    fn container_without_display_is_unverified_on_unknown_brand() {
        let env = environment(TerminalBrand::Unknown, false, true);
        assert_eq!(
            delivery_for_legs(&legs(false, false, true), &env),
            Delivery::Unverified
        );
    }

    #[test]
    fn empty_text_fails_without_firing_legs() {
        assert_eq!(copy_text(""), Delivery::Failed);
    }

    #[test]
    fn spooled_stdin_round_trips_the_payload() {
        let data = b"payload".repeat(1000);
        let mut file = backend::spool_stdin(&data).expect("spool");
        use std::io::Read;
        let mut read_back = Vec::new();
        file.read_to_end(&mut read_back).expect("read spool");
        assert_eq!(read_back, data);
    }

    #[test]
    fn deadline_wait_kills_a_hung_child() {
        use std::time::{Duration, Instant};
        let sleep = if cfg!(target_os = "windows") {
            "timeout"
        } else {
            "sleep"
        };
        let mut child = std::process::Command::new(sleep)
            .arg(if cfg!(target_os = "windows") {
                "5"
            } else {
                "30"
            })
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .expect("spawn hung helper");
        let started = Instant::now();
        let result = backend::wait_with_deadline(&mut child, Duration::from_millis(150));
        assert!(
            result.is_err(),
            "a 30s child must not survive a 150ms deadline"
        );
        assert!(started.elapsed() < Duration::from_secs(5));
        // The child was killed and reaped, so try_wait reports its exit.
        assert!(child.try_wait().expect("reaped").is_some());
    }
}
