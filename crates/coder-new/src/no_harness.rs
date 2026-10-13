//! Coder's side of the Coder SDK's no-harness policy (#11548,
//! [`coder_sdk::auto_policy`]): when no harness can run here, Auto runs
//! Coder's own loop and tools with every call on one model (Gemini 4 Argon
//! by default).
//!
//! `CODER_NO_HARNESS=1`, or `coder --no-harness`, forces it for a run on a
//! computer that has harnesses: Claude Code, Codex, OpenCode, and the ACP
//! agents count as off for the run (nothing is saved, so /plugins keeps
//! the person's switches), and Shift+Tab offers only Auto.

use std::sync::atomic::{AtomicBool, Ordering};

use coder_sdk::auto_policy::{self, AutoPolicy};

/// `--no-harness` on the command line.
static FORCED: AtomicBool = AtomicBool::new(false);

/// Forces no-harness mode for this process (`--no-harness`).
pub fn force() {
    FORCED.store(true, Ordering::Relaxed);
}

/// The policy in force, read from the environment.
#[cfg(not(test))]
#[must_use]
pub fn policy() -> Option<AutoPolicy> {
    let mut policy = AutoPolicy::from_env();
    policy.force_no_harness |= FORCED.load(Ordering::Relaxed);
    Some(policy)
}

/// Whether this run treats every harness as off.
#[must_use]
pub fn forced() -> bool {
    policy().is_some_and(|policy| policy.force_no_harness)
}

/// The person's own Gemini API key, when one is set here.
#[cfg(not(test))]
#[must_use]
pub fn gemini_key() -> Option<model_access::ApiKey> {
    auto_policy::gemini_key(|name| std::env::var(name).ok())
        .map(|key| model_access::ApiKey::new(&key))
}

/// The Gemini API's endpoint: `CODER_GEMINI_BASE_URL`, else Google's.
#[must_use]
pub fn gemini_base() -> String {
    std::env::var("CODER_GEMINI_BASE_URL")
        .ok()
        .filter(|base| !base.trim().is_empty())
        .unwrap_or_else(|| auto_policy::GEMINI_OPENAI_BASE.to_owned())
}

#[cfg(test)]
thread_local! {
    static TEST: std::cell::RefCell<Option<(AutoPolicy, Option<String>)>> =
        const { std::cell::RefCell::new(None) };
}

/// A unit test's policy: none unless the test set one, so the other
/// tests' Coder loop turns keep their doors.
#[cfg(test)]
#[must_use]
pub fn policy() -> Option<AutoPolicy> {
    let _ = &FORCED;
    TEST.with(|cell| cell.borrow().as_ref().map(|(policy, _)| policy.clone()))
}

#[cfg(test)]
#[must_use]
pub fn gemini_key() -> Option<model_access::ApiKey> {
    let _ = auto_policy::GEMINI_KEY_VARS;
    TEST.with(|cell| {
        cell.borrow()
            .as_ref()
            .and_then(|(_, key)| key.as_deref().map(model_access::ApiKey::new))
    })
}

/// Sets this test thread's policy and own Gemini key.
#[cfg(test)]
pub(crate) fn set_test(policy: Option<AutoPolicy>, gemini_key: Option<&str>) {
    TEST.with(|cell| {
        *cell.borrow_mut() = policy.map(|policy| (policy, gemini_key.map(str::to_owned)));
    });
}
