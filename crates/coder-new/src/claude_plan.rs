//! One automated Claude Code run at a time on a Claude plan login (#11237).
//!
//! Anthropic's plan limits "assume ordinary, individual usage of Claude Code
//! and the Agent SDK", and Cloud already admits one automated turn at a time
//! on a plan (`coder_cloud::claude::admit_turns`). Here, on the person's own
//! computer, background agents and runs the website hands this computer take
//! a plan slot before they start `claude`; more wait, with "Waiting for your
//! Claude plan" on their card. The person sets the number on the `/agents`
//! panel (`+` and `-`); it is theirs, saved, and never overridden. A run on
//! an API key or a cloud credential, and an interactive chat, never waits.
//! A `rate_limit_event` whose status is `allowed_warning` holds new automated
//! starts until the window resets or a later event says it is under the
//! warning again.

use std::cell::RefCell;
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::time::Duration;

/// Automated runs on a plan login at once, unless the person sets more.
pub const DEFAULT_SLOTS: usize = 1;
/// The most the person can set.
pub const MAX_SLOTS: usize = 8;
/// What a waiting run's card says.
pub const WAITING: &str = "Waiting for your Claude plan";
/// How long a warning holds new starts when Claude Code named no reset.
const WARNING_HOLD_SECS: u64 = 15 * 60;
const POLL: Duration = Duration::from_millis(250);
const FILE: &str = "claude-plan.json";

#[derive(Default)]
struct Book {
    active: usize,
    /// New automated starts wait until this Unix second (a warning).
    held_until: Option<u64>,
}

static BOOK: Mutex<Book> = Mutex::new(Book {
    active: 0,
    held_until: None,
});
/// The plan's windows as Claude Code last reported them in this process,
/// for the status line (#11233), and whether the last event warned.
static WINDOWS: Mutex<(Vec<microcoder_loop::usage::Window>, bool)> =
    Mutex::new((Vec::new(), false));
/// The person's number; 0 until read from disk.
static LIMIT: AtomicUsize = AtomicUsize::new(0);
/// Claude Code reported this computer's login is an API key or a cloud
/// credential, not a plan.
static NOT_A_PLAN: AtomicBool = AtomicBool::new(false);

thread_local! {
    /// Set on a thread that drives an automated run: what to call when the
    /// run starts and stops waiting for a slot.
    static AUTOMATED: RefCell<Option<Box<dyn Fn(Option<&str>)>>> = const { RefCell::new(None) };
}

/// Marks this thread's runs as automated; `waiting` hears the card's line
/// while one waits for a slot (`None` once it has one).
pub fn automated(waiting: Box<dyn Fn(Option<&str>)>) {
    AUTOMATED.with(|slot| *slot.borrow_mut() = Some(waiting));
}

fn is_automated() -> bool {
    AUTOMATED.with(|slot| slot.borrow().is_some())
}

fn say_waiting(line: Option<&str>) {
    AUTOMATED.with(|slot| {
        if let Some(waiting) = slot.borrow().as_ref() {
            waiting(line);
        }
    });
}

fn file() -> Option<PathBuf> {
    model_access::store::openagents_dir().map(|root| root.join("coder-new").join(FILE))
}

fn read_limit(path: Option<PathBuf>) -> usize {
    path.and_then(|path| std::fs::read(path).ok())
        .and_then(|bytes| serde_json::from_slice::<serde_json::Value>(&bytes).ok())
        .and_then(|value| value["automated_runs"].as_u64())
        .and_then(|n| usize::try_from(n).ok())
        .filter(|n| (1..=MAX_SLOTS).contains(n))
        .unwrap_or(DEFAULT_SLOTS)
}

/// How many automated runs share the plan at once: the person's number.
#[must_use]
pub fn limit() -> usize {
    match LIMIT.load(Ordering::Relaxed) {
        0 => {
            let read = read_limit(file());
            LIMIT.store(read, Ordering::Relaxed);
            read
        }
        n => n,
    }
}

/// Sets and saves the person's number, kept within 1 to [`MAX_SLOTS`].
///
/// # Errors
///
/// The number could not be saved.
pub fn set_limit(n: usize) -> Result<usize, String> {
    let n = n.clamp(1, MAX_SLOTS);
    LIMIT.store(n, Ordering::Relaxed);
    if let Some(path) = file() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        std::fs::write(&path, serde_json::json!({"automated_runs": n}).to_string())
            .map_err(|_| "Coder couldn't save the number.".to_owned())?;
    }

    Ok(n)
}

/// Whether `claude` here runs on a plan login: no API key or cloud
/// credential is admitted to it, and Claude Code has not reported one.
#[must_use]
pub fn plan_login(env: &dyn Fn(&str) -> Option<String>) -> bool {
    if NOT_A_PLAN.load(Ordering::Relaxed) {
        return false;
    }
    let admitted = env("OA_CODER_CLOUD_CREDENTIAL_NAMES").unwrap_or_default();
    let api = admitted
        .split(',')
        .any(|name| matches!(name.trim(), "ANTHROPIC_API_KEY" | "ANTHROPIC_AUTH_TOKEN"));
    let cloud = [
        "CLAUDE_CODE_USE_VERTEX",
        "CLAUDE_CODE_USE_BEDROCK",
        "CLAUDE_CODE_USE_FOUNDRY",
    ]
    .iter()
    .any(|name| env(name).is_some_and(|v| !v.is_empty() && v != "0"));
    !(api || cloud)
}

/// Remembers the credential type a run's `init` named
/// ([`crate::bundled_runtime::claude_print::credential_type`]).
pub fn observe_credential(kind: &str) {
    match kind {
        "anthropic_api_key" | "anthropic_auth_token" | "api_key_helper" => {
            NOT_A_PLAN.store(true, Ordering::Relaxed);
        }
        "claude_ai_login" | "claude_subscription_token" => {
            NOT_A_PLAN.store(false, Ordering::Relaxed);
        }
        _ => {}
    }
}

/// Reads a `rate_limit_event`'s `rate_limit_info`: `allowed_warning` holds
/// new automated starts until its reset; `allowed` lets them go again.
pub fn observe_rate_limit(info: &serde_json::Value, now: u64) {
    if let Some(window) = microcoder_loop::usage::rate_limit_window(info, now) {
        let mut seen = WINDOWS
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        seen.0.retain(|kept| kept.window != window.window);
        seen.0.push(window);
        seen.0.sort_by_key(|kept| kept.window);
        seen.1 = info["status"] == "allowed_warning";
    }
    let mut book = BOOK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    match info["status"].as_str() {
        Some("allowed_warning") => {
            book.held_until = Some(
                info["resetsAt"]
                    .as_u64()
                    .filter(|at| *at > now)
                    .unwrap_or(now + WARNING_HOLD_SECS),
            );
        }
        Some("allowed") => book.held_until = None,
        _ => {}
    }
}

/// A held plan slot; dropping it frees the slot.
pub struct Slot;

impl Drop for Slot {
    fn drop(&mut self) {
        let mut book = BOOK
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        book.active = book.active.saturating_sub(1);
    }
}

fn try_take(now: u64) -> Option<Slot> {
    let mut book = BOOK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    if book.held_until.is_some_and(|until| until > now) {
        return None;
    }
    book.held_until = None;
    if book.active >= limit() {
        return None;
    }
    book.active += 1;
    Some(Slot)
}

fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_secs())
}

/// Waits for a plan slot when this thread's run is automated and `claude`
/// runs on a plan login; otherwise `Ok(None)` at once. `Err` when the run
/// was stopped while it waited.
///
/// # Errors
///
/// The run was stopped before it got a slot.
pub async fn admit(
    cancel: &AtomicBool,
    env: &dyn Fn(&str) -> Option<String>,
) -> Result<Option<Slot>, String> {
    if !is_automated() || !plan_login(env) {
        return Ok(None);
    }
    let mut waiting = false;
    loop {
        if let Some(slot) = try_take(now()) {
            if waiting {
                say_waiting(None);
            }
            return Ok(Some(slot));
        }
        if !plan_login(env) {
            if waiting {
                say_waiting(None);
            }
            return Ok(None);
        }
        if !waiting {
            waiting = true;
            say_waiting(Some(WAITING));
        }
        if cancel.load(Ordering::Relaxed) {
            say_waiting(None);
            return Err("Stopped while waiting for your Claude plan.".into());
        }
        tokio::time::sleep(POLL).await;
    }
}

/// The status line's reading of the plan, from Claude Code's own reports:
/// "Claude plan: 5h 62% · week 31% · resets 14:20", with a quiet "near the
/// limit" after a warning. `None` before any report, or once every window
/// reported has reset. `clock` writes a Unix-milliseconds time.
#[must_use]
pub fn status_text(now: u64, clock: &dyn Fn(u64) -> String) -> Option<String> {
    use microcoder_loop::usage::WindowName;
    let seen = WINDOWS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    let current: Vec<_> = seen
        .0
        .iter()
        .filter(|window| window.resets_at.is_none_or(|at| at > now))
        .collect();
    if current.is_empty() {
        return None;
    }
    let mut parts: Vec<String> = current
        .iter()
        .map(|window| {
            let name = match window.window {
                WindowName::FiveHour => "5h",
                WindowName::SevenDay => "week",
                WindowName::SevenDayTopModel => "top model week",
                WindowName::Primary | WindowName::Secondary => "window",
            };
            format!("{name} {:.0}%", window.used_fraction * 100.0)
        })
        .collect();
    let fullest = current
        .iter()
        .max_by(|a, b| a.used_fraction.total_cmp(&b.used_fraction));
    if let Some(at) = fullest.and_then(|window| window.resets_at) {
        parts.push(format!("resets {}", clock(at * 1000)));
    }
    if seen.1 {
        parts.push("near the limit".into());
    }
    Some(format!("Claude plan: {}", parts.join(" · ")))
}

/// The `/agents` panel's line about the plan.
#[must_use]
pub fn panel_line() -> String {
    let n = limit();
    format!(
        "Claude plan: {n} automated run{} at a time · + / - to change",
        if n == 1 { "" } else { "s" }
    )
}

#[cfg(test)]
pub(crate) fn reset_for_test(limit: usize) {
    LIMIT.store(limit, Ordering::Relaxed);
    NOT_A_PLAN.store(false, Ordering::Relaxed);
    let mut book = BOOK
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner);
    *book = Book::default();
    *WINDOWS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = (Vec::new(), false);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    // The book is process-wide, so its tests run as one.
    static SERIAL: Mutex<()> = Mutex::new(());

    fn none(_: &str) -> Option<String> {
        None
    }

    fn automated_thread<T: Send + 'static>(
        lines: Arc<Mutex<Vec<Option<String>>>>,
        run: impl FnOnce() -> T + Send + 'static,
    ) -> std::thread::JoinHandle<T> {
        std::thread::spawn(move || {
            automated(Box::new(move |line| {
                lines.lock().unwrap().push(line.map(str::to_owned));
            }));
            run()
        })
    }

    #[test]
    fn two_background_starts_on_a_plan_run_one_after_the_other() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reset_for_test(DEFAULT_SLOTS);
        let runtime = || {
            tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .unwrap()
        };
        let lines = Arc::new(Mutex::new(Vec::new()));
        let first = automated_thread(lines.clone(), move || {
            runtime()
                .block_on(admit(&AtomicBool::new(false), &none))
                .unwrap()
        })
        .join()
        .unwrap();
        assert!(first.is_some(), "the first takes the plan's slot");
        let second_lines = Arc::new(Mutex::new(Vec::new()));
        let second = automated_thread(second_lines.clone(), move || {
            runtime()
                .block_on(admit(&AtomicBool::new(false), &none))
                .map(|s| s.is_some())
        });
        std::thread::sleep(Duration::from_millis(600));
        assert_eq!(
            second_lines.lock().unwrap().as_slice(),
            &[Some(WAITING.to_owned())],
            "the second waits, and its card says so"
        );
        drop(first);
        assert_eq!(second.join().unwrap(), Ok(true));
        assert_eq!(second_lines.lock().unwrap().last(), Some(&None));
        assert!(lines.lock().unwrap().is_empty(), "the first never waited");
    }

    #[test]
    fn an_api_key_an_interactive_chat_and_the_persons_number_never_wait() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reset_for_test(DEFAULT_SLOTS);
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        let held = try_take(now()).unwrap();
        // An interactive chat's thread is not automated.
        assert!(
            runtime
                .block_on(admit(&AtomicBool::new(false), &none))
                .unwrap()
                .is_none()
        );
        // An API key admitted to `claude` is not a plan.
        let key = |name: &str| {
            (name == "OA_CODER_CLOUD_CREDENTIAL_NAMES").then(|| "ANTHROPIC_API_KEY".to_owned())
        };
        assert!(!plan_login(&key));
        observe_credential("anthropic_api_key");
        assert!(!plan_login(&none));
        observe_credential("claude_ai_login");
        assert!(plan_login(&none));
        // The person's number: two at once.
        LIMIT.store(2, Ordering::Relaxed);
        let second = try_take(now());
        assert!(second.is_some());
        assert!(try_take(now()).is_none());
        drop((held, second));
        assert_eq!(read_limit(None), DEFAULT_SLOTS);
    }

    #[test]
    fn the_status_line_reads_claude_codes_own_reports() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reset_for_test(DEFAULT_SLOTS);
        let clock = |ms: u64| format!("t{}", ms / 1000);
        let now = 1_000;
        assert_eq!(status_text(now, &clock), None);
        observe_rate_limit(
            &serde_json::json!({"status": "allowed", "rateLimitType": "five_hour", "utilization": 0.62, "resetsAt": now + 60}),
            now,
        );
        observe_rate_limit(
            &serde_json::json!({"status": "allowed_warning", "rateLimitType": "seven_day", "utilization": 0.31, "resetsAt": now + 600}),
            now,
        );
        assert_eq!(
            status_text(now, &clock).as_deref(),
            Some("Claude plan: 5h 62% · week 31% · resets t1060 · near the limit")
        );
        assert_eq!(
            status_text(now + 61, &clock).as_deref(),
            Some("Claude plan: week 31% · resets t1600 · near the limit")
        );
        reset_for_test(DEFAULT_SLOTS);
    }

    #[test]
    fn a_warning_holds_new_starts_until_it_clears() {
        let _serial = SERIAL
            .lock()
            .unwrap_or_else(std::sync::PoisonError::into_inner);
        reset_for_test(DEFAULT_SLOTS);
        let now = 1_000;
        observe_rate_limit(
            &serde_json::json!({"status": "allowed_warning", "resetsAt": now + 60}),
            now,
        );
        assert!(try_take(now).is_none());
        assert!(try_take(now + 61).is_some());
        observe_rate_limit(&serde_json::json!({"status": "allowed_warning"}), now);
        assert!(try_take(now + 60).is_none());
        observe_rate_limit(&serde_json::json!({"status": "allowed"}), now);
        assert!(try_take(now).is_some());
        reset_for_test(DEFAULT_SLOTS);
    }
}
