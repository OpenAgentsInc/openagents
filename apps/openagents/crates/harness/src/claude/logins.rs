//! Which Claude login an app chat's Claude Code runs on (#12215).
//!
//! The host's capacity book can hold several Claude logins (#12211). The
//! harness doesn't read the book itself: the host hands it a
//! [`ClaudeLogins`] ([`super::ClaudeHarness::with_logins`]). Each run's
//! request names its login in [`LOGIN_OPTION`], set by
//! [`crate::Harness::prepare_launch`]: the live runtime's login while it has
//! capacity, else the first login that has. A runtime whose login changed
//! is replaced, resuming its session on the new login, because a running
//! process can't change its token.

use openagents_proto::RunRequest;
use serde_json::Value;

/// The `model_options` key that names the login a run launches on.
pub const LOGIN_OPTION: &str = "openagentsClaudeLogin";

/// The name the signed-in Claude Code login goes by.
pub const DEFAULT_LOGIN: &str = "default";

/// The variables a launch on a named login removes, so an API key never
/// takes precedence over the login's token.
pub const API_KEY_VARS: [&str; 2] = ["ANTHROPIC_API_KEY", "ANTHROPIC_AUTH_TOKEN"];

/// A usage or rate limit a run met.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct UsageLimit {
    /// When the limit resets, in Unix seconds, when Claude Code said.
    pub resets_at: Option<u64>,
    /// The plan window that ran out (`five_hour`, `seven_day`, ...), when
    /// Claude Code named one. Without one the limit is a rate limit.
    pub window: Option<String>,
}

/// The host's Claude logins: which one a launch runs on, its variables, and
/// where a limit is recorded.
pub trait ClaudeLogins: Send + Sync {
    /// The login a launch should run on, given the one the live runtime runs
    /// on (`None` when none runs): `live` while it has capacity, else the
    /// first login that has. `None` leaves the request as it is (no named
    /// logins, or every login is out).
    fn choose(&self, live: Option<&str>) -> Option<String>;
    /// The variables a launch on the named login `name` sets (its token and
    /// its name), or `None` for [`DEFAULT_LOGIN`] or a token that can't be
    /// read.
    fn launch_env(&self, name: &str) -> Option<Vec<(String, String)>>;
    /// Records that `name` met `limit`.
    fn record_limit(&self, name: &str, limit: &UsageLimit);
}

/// The login `request` launches on, if the host chose one.
#[must_use]
pub fn login_of(request: &RunRequest) -> Option<&str> {
    request
        .model_options
        .get(LOGIN_OPTION)
        .and_then(Value::as_str)
}

/// Sets the login `next` launches on: the one `logins` chooses given
/// `live`'s, else `live`'s own.
pub fn prepare(logins: &dyn ClaudeLogins, live: Option<&RunRequest>, next: &mut RunRequest) {
    let live_login = live.and_then(login_of);
    match logins
        .choose(live_login)
        .or_else(|| live_login.map(str::to_owned))
    {
        Some(name) => {
            next.model_options
                .insert(LOGIN_OPTION.into(), Value::String(name));
        }
        None => {
            next.model_options.remove(LOGIN_OPTION);
        }
    }
}
