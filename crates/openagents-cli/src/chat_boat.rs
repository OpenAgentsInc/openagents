//! `openagents chat work --on boat`: each issue's flow runs on a Boat
//! sandbox of its own instead of this computer (issue #10220, Boat B6;
//! plan `docs/cloud/2026-10-02-boat-sdk-plan.md` §5.4).
//!
//! This computer only orchestrates. For each issue:
//!
//! 1. a start is taken from the dispatcher, which keeps under Boat's start
//!    limits (12 a minute on the $20 plan; [`Starts`]) and refuses when the
//!    day's starts are gone (`GET /limits`);
//! 2. a `large` sandbox starts from the newest ready `oa-coder-main-<date>`
//!    template (#10219), or, when there is none, is forked from a seed this
//!    command builds once with the same host setup the template uses
//!    (`scripts/cloud/coder-host-setup.sh --warm`) and deletes at the end;
//! 3. the run's credentials go to `/tmp/oa-run.env` through the files API,
//!    and the sandbox's command reads and deletes that file first: `/tmp` is
//!    outside every Boat snapshot, the sandbox starts with `noEnv`, and no
//!    credential is ever part of a command line, a template, or this
//!    command's output ([`Credentials`]);
//! 4. the sandbox builds `origin/main`'s `openagents` and `microcoder` on
//!    its warm target and runs `openagents chat work --issues N --json`
//!    there: the same issue flow as on this computer (claim, worktree,
//!    engine, checks, the multi-machine landing of #10226, comment, close);
//! 5. the flow's events stream back here as they arrive, marked with the
//!    issue, exactly as a local `chat work` shows them;
//! 6. the sandbox's machine time and list-price cost (`GET
//!    /sandboxes/{id}/usage`) go into a comment on the issue and into a
//!    route record (`~/.openagents/boat/runs.jsonl`): placement computer
//!    `boat`, granted by the operator who typed `--on boat`;
//! 7. the sandbox stops (a stopped sandbox is free), and is deleted when the
//!    issue landed. A failed run's sandbox stays stopped for inspection;
//!    `openagents boat delete ID` removes it.
//!
//! Engine logins (`--engine-logins`, or `OA_BOAT_ENGINE_LOGINS`):
//! `api-keys` (the default) passes provider API keys the issue flow can
//! use: today Grok Build's `XAI_API_KEY`, from the environment or Secret
//! Manager `openagents-xai-api-key`. Codex and Claude Code in the flow need
//! subscription logins, not API keys; `boat` lets Boat write the
//! subscriptions the owner connected on its dashboard into each sandbox
//! (Boat then holds those tokens: the owner's choice, B7). See
//! `docs/cloud/boat-chat-work.md`.

use std::collections::{BTreeMap, VecDeque};
use std::io::Write;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use boat::{CommandFrame, Nullable, WaitOptions, models::*, shell_quote};
use coder::task::issue_run::Land;
use openagents_chat::coder_events::Line;
use openagents_chat::tool_groups::Stream;
use serde_json::{Value, json};
use tokio::sync::{Semaphore, watch};

use super::{Failure, event, failed};
use crate::out::Output;

/// The most sandboxes one queue runs at once.
pub(super) const MAX_PARALLEL: u64 = 16;
/// The template names Boat B5 saves (#10219).
const TEMPLATE_PREFIX: &str = "oa-coder-main-";
/// The sandbox size a run gets: 8 vCPU, 16 GB.
const SIZE: &str = "large";
/// A run's sandbox lifetime: a backstop for a lost orchestrator, never a
/// limit on the run, which this command stops itself when it ends.
const RUN_TTL_SECONDS: i64 = 12 * 3600;
/// The seed's lifetime: its setup builds the warm target from nothing.
const SEED_TTL_SECONDS: i64 = 4 * 3600;
/// Where a run's credentials wait for the command that reads them.
const ENV_FILE: &str = "/tmp/oa-run.env";
/// The placement name the route record carries.
const COMPUTER: &str = "boat";
/// Boat's start limit per rolling minute on the $20 plan.
const STARTS_PER_MINUTE: usize = 12;
/// The Secret Manager project every credential below is read from.
const SECRET_PROJECT: &str = "openagentsgemini";
/// The GitHub token the run pushes and comments with, as the September
/// pool did: an OAuth token held in Secret Manager.
const GITHUB_SECRET: &str = "coder-pool-git-token";
/// Grok Build's key.
const XAI_SECRET: &str = "openagents-xai-api-key";
/// The OpenAI API key Codex logs in with on a cloud run (#10275): optional.
/// With it Coder prefers Codex (`gpt-6.1-sol`, medium, as one lean
/// `codex exec` session); without it the run is on Grok Build.
const OPENAI_SECRET: &str = "coder-openai-api-key";
/// The least time a ChatGPT access token must have left to go to a cloud
/// run: a run can take an hour, and the copy it gets cannot be refreshed.
const CODEX_MIN_LEFT_SECS: u64 = 2 * 3600;

/// How a run's coding agents log in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EngineLogins {
    /// Provider API keys this command passes per run (`noEnv` sandboxes).
    ApiKeys,
    /// The subscriptions the owner connected on Boat's dashboard, which Boat
    /// writes into each sandbox (`noEnv: false`).
    Boat,
}

impl EngineLogins {
    pub(super) fn parse(word: &str) -> Result<Self, String> {
        match word.trim() {
            "api-keys" | "api_keys" => Ok(Self::ApiKeys),
            "boat" => Ok(Self::Boat),
            other => Err(format!(
                "--engine-logins is `api-keys` or `boat`, not `{other}`"
            )),
        }
    }

    fn as_str(self) -> &'static str {
        match self {
            Self::ApiKeys => "api-keys",
            Self::Boat => "boat",
        }
    }
}

/// What `chat work --on boat` was asked to do.
pub(super) struct Request {
    pub repository: String,
    pub numbers: Vec<u64>,
    pub parallel: u64,
    pub land: Option<Land>,
    pub logins: EngineLogins,
    pub engine_fallback: bool,
    /// A named snapshot to start from instead of the newest template.
    pub template: Option<String>,
    /// Build `origin/main`'s `openagents` and `microcoder` in each sandbox
    /// even when the template has them (`OA_BOAT_BUILD=1`).
    pub build: bool,
}

// ---------------------------------------------------------------------------
// The dispatcher: Boat's start limits.

/// How long a start must wait so that no more than `per_window` starts fall
/// in any `window`, given the starts already made (oldest first).
fn start_wait(
    recent: &VecDeque<Instant>,
    now: Instant,
    window: Duration,
    per_window: usize,
) -> Option<Duration> {
    let live: Vec<&Instant> = recent
        .iter()
        .filter(|at| now.duration_since(**at) < window)
        .collect();
    if live.len() < per_window {
        return None;
    }
    let oldest = live[live.len() - per_window];
    Some(window.saturating_sub(now.duration_since(*oldest)))
}

/// Starts are queued so that no rolling minute holds more than Boat allows.
struct Starts {
    window: Duration,
    per_window: usize,
    recent: Mutex<VecDeque<Instant>>,
}

impl Starts {
    fn new(per_window: usize, window: Duration) -> Self {
        Self {
            window,
            per_window,
            recent: Mutex::new(VecDeque::new()),
        }
    }

    /// Waits for a start and records it.
    async fn take(&self) {
        loop {
            let wait = {
                let mut recent = self.recent.lock().unwrap_or_else(|e| e.into_inner());
                let now = Instant::now();
                while recent
                    .front()
                    .is_some_and(|at| now.duration_since(*at) >= self.window)
                {
                    recent.pop_front();
                }
                match start_wait(&recent, now, self.window, self.per_window) {
                    None => {
                        recent.push_back(now);
                        return;
                    }
                    Some(wait) => wait,
                }
            };
            tokio::time::sleep(wait.max(Duration::from_millis(100))).await;
        }
    }
}

/// The day's remaining starts, when Boat says.
async fn starts_left_today(client: &boat::Client) -> Option<i64> {
    let limits = client.limits(&LimitsParams::default()).await.ok()?;
    match &limits.starts.as_ref()?.day {
        Nullable::Value(day) => day.remaining,
        _ => None,
    }
}

// ---------------------------------------------------------------------------
// Credentials.

/// One run's credentials: written to the sandbox's `/tmp` and deleted by the
/// command that reads them. `Debug` names the variables, never the values.
#[derive(Clone, Default)]
pub(super) struct Credentials {
    pub(super) variables: BTreeMap<String, String>,
}

impl std::fmt::Debug for Credentials {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("Credentials")
            .field("names", &self.variables.keys().collect::<Vec<_>>())
            .finish()
    }
}

impl Credentials {
    /// The env file: `NAME='value'` lines the command sources with `set -a`.
    pub(super) fn file(&self) -> String {
        self.variables
            .iter()
            .map(|(name, value)| format!("{name}={}\n", shell_quote(value)))
            .collect()
    }
}

/// A Secret Manager secret's latest value, through `gcloud` (which honors
/// `CLOUDSDK_CONFIG`), or `None`. Nothing is printed.
fn secret(name: &str) -> Option<String> {
    let output = std::process::Command::new("gcloud")
        .args([
            "secrets",
            "versions",
            "access",
            "latest",
            "--secret",
            name,
            "--project",
            SECRET_PROJECT,
        ])
        .stderr(std::process::Stdio::null())
        .output()
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !value.is_empty()).then_some(value)
}

/// Where this computer's Codex login is: `OA_CODER_CODEX_AUTH`, else
/// `$CODEX_HOME/auth.json`, else `~/.codex/auth.json`.
fn codex_auth_path() -> Option<std::path::PathBuf> {
    if let Some(path) = variable("OA_CODER_CODEX_AUTH") {
        return Some(path.into());
    }
    if let Some(home) = variable("CODEX_HOME") {
        return Some(std::path::Path::new(&home).join("auth.json"));
    }
    variable("HOME").map(|home| std::path::Path::new(&home).join(".codex/auth.json"))
}

/// This computer's Codex ChatGPT login as a cloud run carries it, base64:
/// see [`access_only_login`]. Reads the file and never writes it.
pub(super) fn codex_chatgpt_login() -> Result<(String, u64), String> {
    let path = codex_auth_path().ok_or("HOME is not set")?;
    let text = std::fs::read_to_string(&path)
        .map_err(|_| "Codex isn't signed in on this computer (`codex login`)".to_owned())?;
    let now = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_secs());
    let (login, left) = access_only_login(&text, now)?;
    use base64::Engine as _;
    Ok((
        base64::engine::general_purpose::STANDARD.encode(login),
        left,
    ))
}

/// A ChatGPT `auth.json` reduced to what a run needs: the access token, ID
/// token, and account, with the refresh token blanked. ChatGPT refresh
/// tokens are single use (Codex's `refresh_token_reused`), so a run that
/// refreshed a shared one would sign this computer out; a blank one cannot
/// be used, and Codex keeps its current access token when a refresh fails.
/// The access token must have [`CODEX_MIN_LEFT_SECS`] left. Returns the
/// JSON and the seconds left; errors never contain a token.
pub(super) fn access_only_login(text: &str, now: u64) -> Result<(String, u64), String> {
    let auth: Value =
        serde_json::from_str(text).map_err(|_| "the Codex login file can't be read".to_owned())?;
    let tokens = &auth["tokens"];
    let access = tokens["access_token"].as_str().unwrap_or_default();
    if access.is_empty() {
        return Err("Codex is signed in with an API key, not ChatGPT".to_owned());
    }
    let account = tokens["account_id"].as_str().unwrap_or_default();
    let expires = jwt_expiry(access).ok_or("the Codex access token has no expiry")?;
    let left = expires.saturating_sub(now);
    if left < CODEX_MIN_LEFT_SECS {
        return Err(format!(
            "the Codex access token has {} min left (cloud runs require at least 2 h); open Codex on the Mac once to refresh it, then rerun",
            left / 60
        ));
    }
    let login = json!({
        "auth_mode": "chatgpt",
        "OPENAI_API_KEY": Value::Null,
        "tokens": {
            "id_token": tokens["id_token"].clone(),
            "access_token": access,
            "refresh_token": "",
            "account_id": account,
        },
        "last_refresh": auth["last_refresh"].clone(),
    });
    Ok((login.to_string(), left))
}

/// A JWT's `exp`, without checking its signature.
fn jwt_expiry(token: &str) -> Option<u64> {
    use base64::Engine as _;
    let payload = token.split('.').nth(1)?;
    let bytes = base64::engine::general_purpose::URL_SAFE_NO_PAD
        .decode(payload.trim_end_matches('='))
        .ok()?;
    serde_json::from_slice::<Value>(&bytes).ok()?["exp"].as_u64()
}

fn variable(name: &str) -> Option<String> {
    std::env::var(name)
        .ok()
        .filter(|value| !value.trim().is_empty())
}

/// The run's GitHub token: `OA_BOAT_GH_TOKEN`, else Secret Manager
/// [`GITHUB_SECRET`], else this computer's `gh auth token`.
fn github_token() -> Option<String> {
    variable("OA_BOAT_GH_TOKEN")
        .or_else(|| secret(GITHUB_SECRET))
        .or_else(|| {
            let output = std::process::Command::new("gh")
                .args(["auth", "token"])
                .stderr(std::process::Stdio::null())
                .output()
                .ok()?;
            let token = String::from_utf8(output.stdout).ok()?.trim().to_owned();
            (output.status.success() && !token.is_empty()).then_some(token)
        })
}

fn git_config(key: &str) -> Option<String> {
    let output = std::process::Command::new("git")
        .args(["config", "--get", key])
        .output()
        .ok()?;
    let value = String::from_utf8(output.stdout).ok()?.trim().to_owned();
    (output.status.success() && !value.is_empty()).then_some(value)
}

/// Select Codex authentication before a cloud run starts. The API-key lookup
/// is lazy: a refused fallback never reads another credential.
fn select_codex(
    variables: &mut BTreeMap<String, String>,
    login: Result<(String, u64), String>,
    engine_fallback: bool,
    api_key: impl FnOnce() -> Option<String>,
) -> Result<bool, String> {
    match login {
        Ok((login, left)) => {
            eprintln!(
                "Codex runs on your ChatGPT login (its access token has {} min left; cloud runs require at least 2 h).",
                left / 60
            );
            variables.insert("OA_CODEX_AUTH".into(), login);
            Ok(true)
        }
        Err(why) => {
            if !engine_fallback {
                return Err(format!(
                    "Cloud Codex cannot use your ChatGPT login: {why}. No engine was started. Use --engine-fallback to allow an API key or Grok Build."
                ));
            }
            if let Some(key) = api_key() {
                eprintln!(
                    "Codex runs on an OpenAI API key because the ChatGPT login is unavailable: {why}. --engine-fallback allows this fallback."
                );
                variables.insert("OA_CODEX_API_KEY".into(), key);
                Ok(true)
            } else {
                eprintln!(
                    "Grok Build runs because Codex cannot use the ChatGPT login: {why}, and no OpenAI API key is available. --engine-fallback allows this engine switch."
                );
                Ok(false)
            }
        }
    }
}

/// Every credential a run needs, read once for the whole queue.
pub(super) fn credentials(
    logins: EngineLogins,
    engine_fallback: bool,
) -> Result<Credentials, String> {
    let mut variables = BTreeMap::new();
    if !engine_fallback {
        variables.insert("OA_CLOUD_CODEX_ONLY".into(), "1".into());
    }
    if logins == EngineLogins::ApiKeys {
        // Codex through this computer's ChatGPT login first: the run gets a
        // copy that cannot refresh ([`codex_chatgpt_login`]), so this
        // computer's own login is never rotated out from under it.
        let codex = select_codex(
            &mut variables,
            codex_chatgpt_login(),
            engine_fallback,
            || variable("OA_CODER_OPENAI_API_KEY").or_else(|| secret(OPENAI_SECRET)),
        )?;
        match variable("XAI_API_KEY").or_else(|| secret(XAI_SECRET)) {
            Some(xai) => {
                variables.insert("XAI_API_KEY".to_owned(), xai);
            }
            None if codex => {}
            None => {
                return Err(
                    "no engine login for the sandboxes: sign Codex in on this computer \
                     (`codex login`), set XAI_API_KEY, or give gcloud access to Secret Manager \
                     openagents-xai-api-key"
                        .to_owned(),
                );
            }
        }
    }
    if logins == EngineLogins::Boat {
        if engine_fallback {
            eprintln!(
                "Cloud runs use Boat dashboard subscriptions (--engine-logins boat); --engine-fallback allows the remote host to select an available engine."
            );
        } else {
            eprintln!(
                "Codex runs on the Boat dashboard subscription (--engine-logins boat); other engines are disabled without --engine-fallback."
            );
        }
    }
    let token = github_token().ok_or(
        "no GitHub token for the sandboxes: set OA_BOAT_GH_TOKEN, give gcloud access to \
         Secret Manager coder-pool-git-token, or sign `gh` in",
    )?;
    variables.insert("GH_TOKEN".to_owned(), token);
    for (name, key) in [("OA_GIT_NAME", "user.name"), ("OA_GIT_EMAIL", "user.email")] {
        if let Some(value) = variable(name).or_else(|| git_config(key)) {
            variables.insert(name.to_owned(), value);
        }
    }
    Ok(Credentials { variables })
}

// ---------------------------------------------------------------------------
// What the sandbox runs.

/// The command a run's sandbox runs: read and delete the credentials, build
/// `origin/main`'s CLI and engine on the warm target, and run the issue flow
/// with NDJSON events. No credential appears in it.
fn run_script(issue: u64, land: Option<Land>, build: bool) -> String {
    let build = if build { "1" } else { "" };
    let land = match land {
        Some(Land::Main) => " --land main",
        Some(Land::PullRequest) => " --land pr",
        Some(Land::Queue) => " --land queue",
        None => "",
    };
    format!(
        r#"set -uo pipefail
# Wait for Boat's restore of HOME, then repair ownership on plain disk
# (#10251, #10274); `ready` uploaded the script.
bash {FORK_READY_PATH} >&2
if [ -f {ENV_FILE} ]; then set -a; . {ENV_FILE}; set +a; rm -f {ENV_FILE}; fi
export PATH="$HOME/.cargo/bin:/usr/local/cargo/bin:$HOME/.grok/bin:$HOME/.local/bin:/usr/local/bin:$PATH" CARGO_INCREMENTAL=0
[ -z "${{RUSTUP_HOME:-}}" ] && [ -d /usr/local/rustup ] && export RUSTUP_HOME=/usr/local/rustup
# Coder hands Grok Build the XAI_API_KEY of the login shell, which it starts
# with an empty environment: the profile reads the key from /tmp (outside
# every snapshot), and the file goes when this script ends.
trap 'rm -f /tmp/oa-engine.env' EXIT
# An OpenAI key logs Codex in (API-key login, run as a lean codex exec
# session); the login goes with the sandbox.
if [ -n "${{OA_CODEX_API_KEY:-}}" ]; then
  printenv OA_CODEX_API_KEY | codex login --with-api-key >/dev/null 2>&1 || echo "boat: codex login with the API key failed" >&2
fi
unset OA_CODEX_API_KEY
# A ChatGPT login that cannot refresh (no refresh token): Codex runs on it
# until its access token expires, and it goes when this script ends.
if [ -n "${{OA_CODEX_AUTH:-}}" ]; then
  mkdir -p "$HOME/.codex" && (umask 077; printf '%s' "$OA_CODEX_AUTH" | base64 -d >"$HOME/.codex/auth.json") \
    || echo "boat: the Codex login could not be written" >&2
  trap 'rm -f /tmp/oa-engine.env "$HOME/.codex/auth.json"' EXIT
fi
unset OA_CODEX_AUTH
if [ -n "${{XAI_API_KEY:-}}" ]; then
  (umask 077; printf 'export XAI_API_KEY=%q\n' "$XAI_API_KEY" > /tmp/oa-engine.env)
  for f in "$HOME/.profile" "$HOME/.bash_profile"; do
    [ "$f" = "$HOME/.bash_profile" ] && [ ! -f "$f" ] && continue
    grep -q oa-engine.env "$f" 2>/dev/null || echo '[ -r /tmp/oa-engine.env ] && . /tmp/oa-engine.env' >> "$f"
  done
fi
[ -n "${{OA_GIT_NAME:-}}" ] && git config --global user.name "$OA_GIT_NAME"
[ -n "${{OA_GIT_EMAIL:-}}" ] && git config --global user.email "$OA_GIT_EMAIL"
unset OA_GIT_NAME OA_GIT_EMAIL
gh auth setup-git >/dev/null 2>&1 || true
# protoc's well-known types (spark-primitives needs them); templates built
# before coder-host-setup.sh installed libprotobuf-dev lack them.
[ -f /usr/include/google/protobuf/descriptor.proto ] || sudo -n env DEBIAN_FRONTEND=noninteractive apt-get install -y -q libprotobuf-dev >/dev/null 2>&1 || true
cd ~/openagents || {{ echo "boat: no clone at ~/openagents" >&2; exit 2; }}
git fetch -q origin main && git checkout -q --detach origin/main || exit 2
slot=$(jq -r .warm_target.slot ~/.openagents/coder-host.json 2>/dev/null)
[ -n "$slot" ] && [ "$slot" != null ] || slot=$HOME/openagents/target
# Read the template's binaries through once first: they stream in from the
# template, and a read of a 2 GB debug binary failed mid-stream ("Software
# caused connection abort") on 2026-10-02. A failed read is retried, then
# the run builds its own.
ready=""
# The template's binaries are from the day it was built. Use them only when
# no Rust source changed between that revision and origin/main; otherwise a
# run would check out today's main but run yesterday's Coder, bringing back
# bugs main already fixed (a stale template ran Grok's non-reasoning model
# after #10275 pinned grok-4.7, and its run changed nothing).
rev=$(jq -r .rev ~/.openagents/coder-host.json 2>/dev/null)
current=""
if [ -n "$rev" ] && [ "$rev" != null ] && git cat-file -e "$rev^{{commit}}" 2>/dev/null \
  && git diff --quiet "$rev" HEAD -- Cargo.toml Cargo.lock crates; then
  current=1
else
  echo "boat: the template's binaries ($(printf %s "$rev" | cut -c1-10)) predate Rust changes on main; building" >&2
fi
if [ -z "{build}" ] && [ -n "$current" ] && [ -x "$slot/debug/openagents" ] && [ -x "$slot/debug/microcoder" ]; then
  for attempt in 1 2 3 4 5 6; do
    cat "$slot/debug/openagents" "$slot/debug/microcoder" >/dev/null 2>&1 && {{ ready=1; break; }}
    echo "boat: the template's binaries are still streaming in (attempt $attempt)" >&2
    sleep 10
  done
fi
if [ -n "$ready" ]; then
  echo "boat: using the template's openagents and microcoder ($(jq -r .rev ~/.openagents/coder-host.json | cut -c1-10))" >&2
else
  echo "boat: building origin/main $(git rev-parse --short HEAD) on the warm target" >&2
  # One package per invocation, as the template warmed them: a combined
  # build unifies features and misses the warm target.
  {{ CARGO_TARGET_DIR="$slot" cargo build -q -p openagents-cli --bin openagents \
    && CARGO_TARGET_DIR="$slot" cargo build -q -p microcoder --bin microcoder; }} >/tmp/oa-build.log 2>&1 \
    || {{ tail -n 40 /tmp/oa-build.log >&2; exit 3; }}
fi
# Run them where they are: a copy reads gigabytes the sandbox may still be
# streaming in (a 2 GB copy took over 13 minutes once), and a later build
# into the slot replaces the files, never the running inodes.
export OPENAGENTS_CODER_CONTROLLER="$slot/debug/microcoder"
export OPENAGENTS_CODER_PLACEMENT=boat
"$slot/debug/openagents" chat work --local --json --issues {issue} --parallel 1{land}
"#
    )
}

/// The seed's setup: the template's own host setup, from `origin/main`.
const SEED_SCRIPT: &str = r#"set -uo pipefail
cd ~
[ -d openagents/.git ] || git clone -q https://github.com/OpenAgentsInc/openagents.git openagents || exit 1
git -C openagents fetch -q origin main || exit 1
git -C openagents show origin/main:scripts/cloud/coder-host-setup.sh > /tmp/coder-host-setup.sh || exit 1
printf '.cache/sccache/\n' > ~/.boxignore
bash /tmp/coder-host-setup.sh --warm --no-release-binary 2>&1 | grep --line-buffered '^OA_CODER_HOST_SETUP'
exit "${PIPESTATUS[0]}"
"#;

// ---------------------------------------------------------------------------
// Where a run starts.

/// Where every run's sandbox comes from.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Source {
    /// A named snapshot (a B5 template).
    Template(String),
    /// A sandbox this command set up and forks for each run.
    Seed(String),
}

impl Source {
    fn name(&self) -> String {
        match self {
            Self::Template(name) => name.clone(),
            Self::Seed(id) => format!("seed {id}"),
        }
    }
}

/// The newest ready template among the named snapshots.
fn newest_template(snapshots: &[NamedSnapshot]) -> Option<String> {
    snapshots
        .iter()
        .filter(|s| s.name.starts_with(TEMPLATE_PREFIX) && s.status == "ready")
        .map(|s| s.name.clone())
        .max()
}

/// An SDK error with Boat's error code, when it sent one (never the body).
fn why(error: &boat::Error) -> String {
    match error {
        boat::Error::Api(api) => match api.code() {
            Some(code) => format!("{error} ({code})"),
            None => error.to_string(),
        },
        other => other.to_string(),
    }
}

fn nonce() -> u128 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_nanos())
}

fn wait(timeout: Duration, interval: Duration) -> WaitOptions {
    WaitOptions {
        timeout,
        interval,
        ..Default::default()
    }
}

/// Builds the seed: a `large` sandbox set up as the template is.
async fn build_seed(
    client: &boat::Client,
    output: Output,
    starts: &Starts,
) -> Result<String, String> {
    starts.take().await;
    let created = client
        .create(&CreateParams {
            idempotency_key: Some(format!("oa-chat-work-seed-{}", nonce())),
            body: Some(CreateSandboxRequest {
                type_: Some(SIZE.into()),
                ttl_seconds: Nullable::Value(SEED_TTL_SECONDS),
                no_env: Some(true),
                ..Default::default()
            }),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("could not create the seed sandbox: {}", why(&e)))?;
    let id = created.sandbox.id;
    say(
        output,
        json!({"event": "boat_seed", "sandbox": id, "state": "building"}),
        &format!(
            "boat: no {TEMPLATE_PREFIX}<date> template yet; building seed {id} once with the \
             template's host setup (tens of minutes), then forking it per issue"
        ),
    );
    let built = async {
        client
            .wait_until_ready(&id, &wait(Duration::from_secs(600), Duration::from_secs(3)))
            .await
            .map_err(|e| format!("seed {id} did not become ready: {}", why(&e)))?;
        let process = client
            .exec_detached(
                &id,
                CommandRequest {
                    command: SEED_SCRIPT.into(),
                    ..Default::default()
                },
            )
            .await
            .map_err(|e| format!("the seed setup did not start: {}", why(&e)))?;
        let mut follower = client
            .follow_command(
                &id,
                process.process_id,
                wait(Duration::from_secs(4 * 3600), Duration::from_secs(10)),
            )
            .map_err(|e| e.to_string())?;
        let mut last = None;
        while let Some(frame) = follower.next().await.map_err(|e| e.to_string())? {
            match frame {
                CommandFrame::Stdout(text) | CommandFrame::Stderr(text) => {
                    for line in text.lines().filter(|l| !l.trim().is_empty()) {
                        say(
                            output,
                            json!({"event": "boat_seed", "sandbox": id, "line": line}),
                            &format!("boat: seed {line}"),
                        );
                    }
                }
                CommandFrame::Started | CommandFrame::Unknown(_) => {}
                end => last = Some(end),
            }
        }
        match last {
            Some(CommandFrame::Exit {
                exit_code: Some(0), ..
            }) => Ok(()),
            other => Err(format!("the seed setup failed on {id}: {other:?}")),
        }?;
        // Stopping takes the last snapshot, so every fork starts warm.
        stop_and_wait(client, &id).await
    }
    .await;
    match built {
        Ok(()) => Ok(id),
        Err(message) => {
            teardown(client, &id, true).await;
            Err(message)
        }
    }
}

/// Starts one run's sandbox from `source`; returns its id.
async fn start(
    client: &boat::Client,
    source: &Source,
    issue: u64,
    logins: EngineLogins,
) -> Result<String, String> {
    let no_env = logins == EngineLogins::ApiKeys;
    let key = format!("oa-chat-work-{issue}-{}", nonce());
    match source {
        Source::Template(name) => client
            .create(&CreateParams {
                idempotency_key: Some(key),
                body: Some(CreateSandboxRequest {
                    type_: Some(SIZE.into()),
                    ttl_seconds: Nullable::Value(RUN_TTL_SECONDS),
                    no_env: Some(no_env),
                    from_: Some(name.clone()),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await
            .map(|created| created.sandbox.id)
            .map_err(|e| format!("could not start a sandbox from {name}: {}", why(&e))),
        Source::Seed(seed) => client
            .fork(&ForkParams {
                sandbox_id: seed.clone(),
                idempotency_key: Some(key),
                body: Some(ForkParamsBody {
                    type_: Some(SIZE.into()),
                    ttl_seconds: Nullable::Value(RUN_TTL_SECONDS),
                    no_env: Some(no_env),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await
            .map(|forked| forked.id)
            .map_err(|e| format!("could not fork seed {seed}: {}", why(&e))),
    }
}

async fn stop_and_wait(client: &boat::Client, id: &str) -> Result<(), String> {
    client
        .stop(&StopParams {
            sandbox_id: id.into(),
            ..Default::default()
        })
        .await
        .map_err(|e| format!("could not stop {id}: {}", why(&e)))?;
    let deadline = Instant::now() + Duration::from_secs(1_800);
    loop {
        // A read can fail while Boat stops the machine (a 502 was seen):
        // keep reading until the deadline.
        let state = match client
            .get(&GetParams {
                sandbox_id: id.into(),
                ..Default::default()
            })
            .await
        {
            Ok(info) => info.sandbox.state,
            Err(e) if Instant::now() > deadline => {
                return Err(format!("could not read {id}: {}", why(&e)));
            }
            Err(_) => {
                tokio::time::sleep(Duration::from_secs(5)).await;
                continue;
            }
        };
        if matches!(state.as_str(), "stopped" | "archived") {
            return Ok(());
        }
        if state == "error" || Instant::now() > deadline {
            return Err(format!("{id} did not stop (state {state})"));
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
}

/// Whether a refusal means the machine is not reachable yet: a sandbox
/// started from a template reports `ready` a few seconds before Boat can
/// run commands on it, and refuses them with `400 sandbox_direct_failed` or
/// `409 sandbox_starting` (seen 2026-10-02). Neither ran the command.
fn not_reachable_yet(error: &boat::Error) -> bool {
    matches!(error, boat::Error::Api(api)
        if matches!(api.code(), Some("sandbox_direct_failed" | "sandbox_starting")))
}

/// What a sandbox from a template needs before anything builds
/// (`scripts/cloud/boat-fork-ready.sh`, #10251, #10274): it comes back with
/// directories under `HOME` owned by root, and Boat restores `HOME` lazily
/// through a FUSE mount. A repair walked through that mount is slow (26 to
/// 100 s) and misses directories the restore creates later, so a build into
/// the warm slot failed with `Permission denied` (#10274). `ready` repairs
/// only Boat's own `~/.ascii` (detached commands need it); the run script
/// waits for the restore and then repairs the rest on plain disk.
const FORK_READY: &str = include_str!("../../../scripts/cloud/boat-fork-ready.sh");
const FORK_READY_PATH: &str = "/tmp/oa-boat-fork-ready.sh";

/// Waits until `id` is ready and runs commands, then repairs Boat's own
/// directory.
async fn ready(client: &boat::Client, id: &str) -> Result<(), String> {
    client
        .wait_until_ready(id, &wait(Duration::from_secs(600), Duration::from_secs(3)))
        .await
        .map_err(|e| format!("{id} did not become ready: {}", why(&e)))?;
    reachable(client, id).await?;
    client
        .write_text(id, FORK_READY_PATH, FORK_READY)
        .await
        .map_err(|e| format!("uploading the fork setup to {id}: {}", why(&e)))?;
    let command = format!("bash {FORK_READY_PATH} --bookkeeping-only");
    match sh(client, id, command).await {
        Ok(done) if done.exit_code == Some(0) => Ok(()),
        Ok(done) => Err(format!(
            "repairing ownership on {id} exited {:?}",
            done.exit_code
        )),
        Err(e) => Err(format!("repairing ownership on {id}: {}", why(&e))),
    }
}

/// Waits until `id` runs a command: `true`, once every two seconds, up to
/// ten minutes.
async fn reachable(client: &boat::Client, id: &str) -> Result<(), String> {
    let deadline = Instant::now() + Duration::from_secs(600);
    loop {
        let tried = sh(client, id, "true".into()).await;
        match tried {
            Ok(_) => return Ok(()),
            Err(e) if not_reachable_yet(&e) && Instant::now() < deadline => {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(format!("{id} does not run commands: {}", why(&e))),
        }
    }
}

/// Runs `command` on `id` and waits for it (Boat's limit is 600 s).
async fn sh(client: &boat::Client, id: &str, command: String) -> boat::Result<CommandResponse> {
    match client
        .command(&CommandParams {
            sandbox_id: id.into(),
            body: CommandRequest {
                command,
                timeout_seconds: Some(300),
                ..Default::default()
            },
            ..Default::default()
        })
        .await?
    {
        CommandResponseBody::Finished(done) => Ok(done),
        CommandResponseBody::Started(_) => Err(boat::Error::Decode),
    }
}

/// Where a run's process writes, on the sandbox.
const RUN_SCRIPT: &str = "/tmp/oa-run.sh";
const RUN_OUT: &str = "/tmp/oa-run.out";
const RUN_ERR: &str = "/tmp/oa-run.err";
const RUN_EXIT: &str = "/tmp/oa-run.exit";
/// The most bytes of each stream one poll reads.
const POLL_BYTES: u64 = 1 << 20;

/// The command that starts the run in its own session, so it outlives the
/// command that started it, and prints its process id. Boat's detached
/// commands were refused for minutes on fresh template sandboxes
/// (`400 sandbox_direct_failed`) while plain commands ran, so the run is
/// started and read with plain commands.
fn launch_command() -> String {
    format!(
        "setsid nohup bash -c 'bash {RUN_SCRIPT} >{RUN_OUT} 2>{RUN_ERR} </dev/null; echo $? >{RUN_EXIT}' \
         >/dev/null 2>&1 </dev/null & echo $!"
    )
}

/// The command that reads what the run wrote past `out` and `err` bytes:
/// `EXIT ALIVE OUT ERR`, the streams in base64 (`_` when empty), EXIT `_`
/// while the run has not ended.
fn poll_command(pid: i64, out: u64, err: u64) -> String {
    format!(
        "o=$(tail -c +{} {RUN_OUT} 2>/dev/null | head -c {POLL_BYTES} | base64 -w0); \
         e=$(tail -c +{} {RUN_ERR} 2>/dev/null | head -c {POLL_BYTES} | base64 -w0); \
         x=$(cat {RUN_EXIT} 2>/dev/null); kill -0 {pid} 2>/dev/null && a=1 || a=0; \
         echo \"${{x:-_}} $a ${{o:-_}} ${{e:-_}}\"",
        out + 1,
        err + 1
    )
}

/// One poll's answer.
#[derive(Debug, PartialEq)]
struct Polled {
    exit: Option<i64>,
    alive: bool,
    out: Vec<u8>,
    err: Vec<u8>,
}

fn parse_poll(text: &str) -> Option<Polled> {
    use base64::Engine as _;
    let words: Vec<&str> = text.split_whitespace().collect();
    let [exit, alive, out, err] = words.as_slice() else {
        return None;
    };
    let bytes = |word: &str| {
        if word == "_" {
            Some(Vec::new())
        } else {
            base64::engine::general_purpose::STANDARD.decode(word).ok()
        }
    };
    Some(Polled {
        exit: match *exit {
            "_" => None,
            code => Some(code.parse().ok()?),
        },
        alive: *alive == "1",
        out: bytes(out)?,
        err: bytes(err)?,
    })
}

/// Reads a run's output as frames, from plain commands every two seconds.
struct Tail<'a> {
    client: &'a boat::Client,
    id: String,
    pid: i64,
    out: u64,
    err: u64,
    frames: VecDeque<CommandFrame>,
    ended: bool,
    misses: u32,
}

impl Tail<'_> {
    async fn next(&mut self) -> Result<Option<CommandFrame>, String> {
        loop {
            if let Some(frame) = self.frames.pop_front() {
                return Ok(Some(frame));
            }
            if self.ended {
                return Ok(None);
            }
            let answer = match sh(
                self.client,
                &self.id,
                poll_command(self.pid, self.out, self.err),
            )
            .await
            {
                Ok(answer) => answer,
                // A read can fail for a moment; a run is never resent.
                Err(e) => {
                    self.misses += 1;
                    if self.misses > 60 {
                        return Err(format!("reading the run on {}: {}", self.id, why(&e)));
                    }
                    tokio::time::sleep(Duration::from_secs(5)).await;
                    continue;
                }
            };
            self.misses = 0;
            let Some(polled) = parse_poll(&answer.stdout) else {
                return Err(format!(
                    "reading the run on {}: an unexpected answer",
                    self.id
                ));
            };
            let more =
                polled.out.len() as u64 == POLL_BYTES || polled.err.len() as u64 == POLL_BYTES;
            self.out += polled.out.len() as u64;
            self.err += polled.err.len() as u64;
            if !polled.out.is_empty() {
                self.frames.push_back(CommandFrame::Stdout(
                    String::from_utf8_lossy(&polled.out).into_owned(),
                ));
            }
            if !polled.err.is_empty() {
                self.frames.push_back(CommandFrame::Stderr(
                    String::from_utf8_lossy(&polled.err).into_owned(),
                ));
            }
            if more {
                continue;
            }
            if polled.exit.is_some() || !polled.alive {
                self.ended = true;
                self.frames.push_back(CommandFrame::Exit {
                    exit_code: polled.exit,
                    success: polled.exit == Some(0),
                    timed_out: false,
                });
                continue;
            }
            if self.frames.is_empty() {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
        }
    }
}

/// Ends a run's sandbox. A landed run's sandbox is deleted at once (a delete
/// stops it). Any other is asked to stop and kept: stopped is free, and the
/// stop finishes on Boat's side without this command waiting for it.
async fn end_run(client: &boat::Client, id: &str, delete: bool) -> &'static str {
    if delete {
        let deleted = client
            .delete_sandbox(&DeleteSandboxParams {
                sandbox_id: id.into(),
                x_ascii_confirm_delete: id.into(),
                ..Default::default()
            })
            .await;
        match deleted {
            Ok(_) => return "deleted",
            Err(e) => eprintln!("boat: could not delete {id}: {}", why(&e)),
        }
    }
    match client
        .stop(&StopParams {
            sandbox_id: id.into(),
            ..Default::default()
        })
        .await
    {
        Ok(_) => "stopped",
        Err(e) => {
            eprintln!("boat: could not stop {id}: {}", why(&e));
            "stop_failed"
        }
    }
}

/// Stops `id`, and deletes it when `delete`; reports and never fails.
async fn teardown(client: &boat::Client, id: &str, delete: bool) -> &'static str {
    if let Err(message) = stop_and_wait(client, id).await {
        eprintln!("boat: {message}");
        return "stop_failed";
    }
    if !delete {
        return "stopped";
    }
    let deleted = client
        .delete_sandbox(&DeleteSandboxParams {
            sandbox_id: id.into(),
            x_ascii_confirm_delete: id.into(),
            ..Default::default()
        })
        .await;
    match deleted {
        Ok(op) => {
            let _ = client
                .wait_for_deletion(
                    &op.operation.id,
                    &wait(Duration::from_secs(900), Duration::from_secs(5)),
                )
                .await;
            "deleted"
        }
        Err(e) => {
            eprintln!("boat: could not delete {id}: {e}");
            "stopped"
        }
    }
}

// ---------------------------------------------------------------------------
// Following the flow.

/// One NDJSON line from the sandbox's `chat work --json`.
#[derive(Debug, PartialEq)]
pub(super) enum Inner {
    /// The flow's task started (`coder`).
    Started { task: String, thread: String },
    /// The flow ended (`issue`).
    Done {
        outcome: String,
        message: String,
        commits: Vec<String>,
    },
    /// One of the flow's events.
    Event(Box<Line>),
    /// `queue`, `queue_done`, or anything else.
    Other,
}

pub(super) fn inner(text: &str) -> Inner {
    let Ok(value) = serde_json::from_str::<Value>(text) else {
        return Inner::Other;
    };
    match value["event"].as_str() {
        Some("coder") => Inner::Started {
            task: value["task"]["task"]
                .as_str()
                .unwrap_or_default()
                .to_owned(),
            thread: value["thread"].as_str().unwrap_or_default().to_owned(),
        },
        Some("issue") => Inner::Done {
            outcome: value["outcome"].as_str().unwrap_or("failed").to_owned(),
            message: value["message"].as_str().unwrap_or_default().to_owned(),
            commits: value["commits"]
                .as_array()
                .map(|commits| {
                    commits
                        .iter()
                        .filter_map(|c| c.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default(),
        },
        Some("queue" | "queue_done") | None => Inner::Other,
        Some(_) => serde_json::from_value::<Line>(value)
            .map_or(Inner::Other, |line| Inner::Event(Box::new(line))),
    }
}

/// Prints a message: NDJSON `value` under `--json`, else `text` on stderr.
pub(super) fn say(output: Output, value: Value, text: &str) {
    if output.json() {
        event(&output, value);
    } else {
        eprintln!("{text}");
    }
}

/// Dollars at Boat's list price, to the tenth of a cent.
fn dollars(amount: f64) -> String {
    format!("${amount:.4}")
}

/// What a run cost and how long it took.
#[derive(Clone, Debug, Default, PartialEq)]
struct Cost {
    wall: Duration,
    machine_seconds: Option<i64>,
    dollars: Option<f64>,
}

/// The issue comment that records where the run ran and what it cost.
fn cost_comment(
    sandbox: &str,
    source: &str,
    outcome: &str,
    cost: &Cost,
    logins: EngineLogins,
) -> String {
    let machine = cost
        .machine_seconds
        .map_or_else(|| "unknown".to_owned(), |s| format!("{s} s"));
    let price = cost.dollars.map_or_else(|| "unknown".to_owned(), dollars);
    format!(
        "Ran on Boat (`openagents chat work --on boat`): sandbox `{sandbox}` ({SIZE}) from \
         `{source}`, engine logins `{}`.\n\n\
         - outcome: {outcome}\n\
         - wall time (start to end, from the orchestrator): {} s\n\
         - billed time (Boat counts `default`-size seconds; `large` bills two a second): {machine}\n\
         - cost at Boat list price (`GET /sandboxes/{{id}}/usage`): {price}\n",
        logins.as_str(),
        cost.wall.as_secs(),
    )
}

/// The route record for one run: placement on the `boat` computer the
/// operator granted, the run's projected outcome, its cost and wall time.
fn route_record(
    repository: &str,
    issue: u64,
    task: &str,
    sandbox: &str,
    source: &str,
    outcome: &str,
    cost: &Cost,
) -> Value {
    let placement = super::placement::granted(COMPUTER, format!("boat:{sandbox}"), 0, repository);
    let run = super::placement::outcome(
        task,
        outcome,
        cost.dollars.map(super::placement::microusd),
        u64::try_from(cost.wall.as_millis()).ok(),
    );
    json!({
        "schema": "openagents.boat.run.v1",
        "issue": issue,
        "sandbox": sandbox,
        "source": source,
        "outcome": outcome,
        "machine_seconds": cost.machine_seconds,
        "placement": placement,
        "run": run,
    })
}

fn append_record(record: &Value) {
    let dir = std::env::var_os("HOME")
        .map_or_else(|| std::path::PathBuf::from("."), std::path::PathBuf::from)
        .join(".openagents/boat");
    let _ = std::fs::create_dir_all(&dir);
    if let Ok(mut file) = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(dir.join("runs.jsonl"))
    {
        let _ = writeln!(file, "{record}");
    }
}

pub(super) fn comment(repository: &str, issue: u64, body: &str) {
    let posted = std::process::Command::new("gh")
        .args([
            "issue",
            "comment",
            &issue.to_string(),
            "-R",
            repository,
            "--body",
            body,
        ])
        .stdout(std::process::Stdio::null())
        .status();
    if !posted.is_ok_and(|status| status.success()) {
        eprintln!("boat: #{issue}: the cost comment could not be posted");
    }
}

/// One issue on one sandbox, start to teardown. Returns the `issue` record.
#[allow(clippy::too_many_arguments)]
async fn run_issue(
    client: Arc<boat::Client>,
    output: Output,
    request: Arc<Request>,
    source: Source,
    credentials: Arc<Credentials>,
    starts: Arc<Starts>,
    issue: u64,
    mut stopping: watch::Receiver<bool>,
) -> Value {
    let done = |outcome: &str, message: String, extra: Value| {
        let mut record = json!({"event": "issue", "issue": issue, "outcome": outcome,
            "message": message, "placement": COMPUTER});
        if let (Some(record), Some(extra)) = (record.as_object_mut(), extra.as_object()) {
            record.extend(extra.clone());
        }
        record
    };
    if *stopping.borrow() {
        return done(
            "not_started",
            "Stopped before it started.".into(),
            json!({}),
        );
    }
    if starts_left_today(&client).await == Some(0) {
        return done(
            "not_started",
            "Boat has no starts left today (200 a day on the $20 plan).".into(),
            json!({}),
        );
    }
    let began = Instant::now();
    // A sandbox that never runs commands is replaced once; what it cost is
    // added to the run's.
    let mut discarded = (0_i64, 0.0_f64);
    let mut attempt = 0;
    let id = loop {
        attempt += 1;
        starts.take().await;
        let id = match start(&client, &source, issue, request.logins).await {
            Ok(id) => id,
            Err(message) => return done("not_started", message, json!({})),
        };
        say(
            output,
            json!({"event": "boat_sandbox", "issue": issue, "sandbox": id, "source": source.name()}),
            &format!("#{issue}: Boat sandbox {id} from {}", source.name()),
        );
        match ready(&client, &id).await {
            Ok(()) if *stopping.borrow() => {
                let state = end_run(&client, &id, true).await;
                return done(
                    "not_started",
                    "Stopped before it started.".into(),
                    json!({"sandbox": id, "sandbox_state": state}),
                );
            }
            Ok(()) => break id,
            Err(message) => {
                if let Ok(usage) = client
                    .usage(&UsageParams {
                        sandbox_id: id.clone(),
                        ..Default::default()
                    })
                    .await
                {
                    discarded.0 += usage.seconds;
                    discarded.1 += usage.dollars;
                }
                let state = end_run(&client, &id, true).await;
                if attempt >= 2 || *stopping.borrow() {
                    return done(
                        "not_started",
                        message,
                        json!({"sandbox": id, "sandbox_state": state,
                            "cost_usd": discarded.1, "machine_seconds": discarded.0}),
                    );
                }
                say(
                    output,
                    json!({"event": "boat_sandbox", "issue": issue, "sandbox": id,
                        "replaced": true, "reason": message}),
                    &format!("#{issue}: {message}; sandbox {id} {state}, starting another"),
                );
            }
        }
    };

    let followed = follow(
        &client,
        output,
        &request,
        &credentials,
        &id,
        issue,
        &mut stopping,
    )
    .await;
    let wall = began.elapsed();
    let (outcome, message, commits, task, thread) = match followed {
        Ok(ended) => ended,
        Err(message) => (
            "failed".to_owned(),
            message,
            Vec::new(),
            String::new(),
            String::new(),
        ),
    };
    let landed = matches!(
        outcome.as_str(),
        "landed" | "pull_request" | "queued" | "skipped" | "closed" | "unchanged"
    );
    // Usage is read as the run ends, before a delete removes the sandbox.
    let usage = client
        .usage(&UsageParams {
            sandbox_id: id.clone(),
            ..Default::default()
        })
        .await
        .ok();
    let state = end_run(&client, &id, landed).await;
    let cost = Cost {
        wall,
        machine_seconds: usage.as_ref().map(|u| u.seconds + discarded.0),
        dollars: usage.as_ref().map(|u| u.dollars + discarded.1),
    };
    let record = route_record(
        &request.repository,
        issue,
        &task,
        &id,
        &source.name(),
        &outcome,
        &cost,
    );
    append_record(&record);
    event(&output, json!({"event": "route_record", "record": record}));
    if !matches!(outcome.as_str(), "skipped" | "closed" | "not_started") {
        let body = cost_comment(&id, &source.name(), &outcome, &cost, request.logins);
        let repository = request.repository.clone();
        let _ = tokio::task::spawn_blocking(move || comment(&repository, issue, &body)).await;
    }
    if !output.json() && state != "deleted" {
        eprintln!("#{issue}: sandbox {id} is {state}; `openagents boat delete {id}` removes it.");
    }
    done(
        &outcome,
        message,
        json!({"sandbox": id, "sandbox_state": state, "source": source.name(),
            "wall_seconds": wall.as_secs(), "machine_seconds": cost.machine_seconds,
            "cost_usd": cost.dollars, "commits": commits,
            "task": (!task.is_empty()).then_some(task),
            "thread": (!thread.is_empty()).then_some(thread)}),
    )
}

/// Runs the flow on sandbox `id` and follows it; returns (outcome, message,
/// commits, task, thread).
async fn follow(
    client: &boat::Client,
    output: Output,
    request: &Request,
    credentials: &Credentials,
    id: &str,
    issue: u64,
    stopping: &mut watch::Receiver<bool>,
) -> Result<(String, String, Vec<String>, String, String), String> {
    let written = client
        .write_text(id, ENV_FILE, &credentials.file())
        .await
        .map_err(|e| format!("the run's credentials could not be written: {}", why(&e)))?;
    if written.type_ != "file.written" {
        return Err("the run's credentials could not be written".into());
    }
    let written = client
        .write_text(
            id,
            RUN_SCRIPT,
            &run_script(issue, request.land, request.build),
        )
        .await
        .map_err(|e| format!("the run's script could not be written: {}", why(&e)))?;
    if written.type_ != "file.written" {
        return Err("the run's script could not be written".into());
    }
    // A refusal that means "not reachable yet" ran nothing, so it alone is
    // retried.
    let mut attempt = 0;
    let pid = loop {
        attempt += 1;
        match sh(client, id, launch_command()).await {
            Ok(started) => match started.stdout.trim().parse::<i64>() {
                Ok(pid) => break pid,
                Err(_) => return Err(format!("the flow did not start on {id}")),
            },
            Err(e) if not_reachable_yet(&e) && attempt < 30 => {
                tokio::time::sleep(Duration::from_secs(5)).await;
            }
            Err(e) => {
                return Err(format!("the flow did not start on {id}: {}", why(&e)));
            }
        }
    };
    let mut follower = Tail {
        client,
        id: id.to_owned(),
        pid,
        out: 0,
        err: 0,
        frames: VecDeque::new(),
        ended: false,
        misses: 0,
    };
    let mut pending = String::new();
    let mut tools = Stream::default();
    let mut errors: VecDeque<String> = VecDeque::new();
    let (mut task, mut thread) = (String::new(), String::new());
    let mut ended: Option<(String, String, Vec<String>)> = None;
    let mut last = None;
    loop {
        let frame = tokio::select! {
            frame = follower.next() => frame?,
            changed = stopping.changed() => {
                if changed.is_ok() && *stopping.borrow() {
                    let _ = sh(client, id, format!("kill -TERM -- -{pid} 2>/dev/null || kill -TERM {pid}")).await;
                    return Err("Stopped: the flow on the sandbox was killed.".into());
                }
                continue;
            }
        };
        let Some(frame) = frame else { break };
        match frame {
            CommandFrame::Stdout(text) => {
                pending.push_str(&text);
                while let Some(at) = pending.find('\n') {
                    let line: String = pending.drain(..=at).collect();
                    match inner(line.trim()) {
                        Inner::Started {
                            task: t,
                            thread: th,
                        } => {
                            say(
                                output,
                                json!({"event": "coder", "issue": issue, "thread": th,
                                    "accepted": true, "placement": COMPUTER, "sandbox": id,
                                    "task": {"host": COMPUTER, "task": t, "issue": issue}}),
                                &format!("#{issue}: Coder took the issue on {id} as task {t}."),
                            );
                            (task, thread) = (t, th);
                        }
                        Inner::Done {
                            outcome,
                            message,
                            commits,
                        } => ended = Some((outcome, message, commits)),
                        Inner::Event(line) => super::work::show(&output, issue, &mut tools, &line),
                        Inner::Other => {}
                    }
                }
            }
            CommandFrame::Stderr(text) => {
                for row in text.lines().filter(|row| !row.trim().is_empty()) {
                    if output.json() {
                        event(
                            &output,
                            json!({"event": "stderr", "issue": issue, "sandbox": id, "line": row}),
                        );
                    } else if row.starts_with("boat:") {
                        eprintln!("#{issue} {row}");
                    }
                    errors.push_back(row.to_owned());
                    if errors.len() > 20 {
                        errors.pop_front();
                    }
                }
            }
            CommandFrame::Started | CommandFrame::Unknown(_) => {}
            end => last = Some(end),
        }
    }
    match ended {
        Some((outcome, message, commits)) => Ok((outcome, message, commits, task, thread)),
        None => {
            let code = match last {
                Some(CommandFrame::Exit { exit_code, .. }) => format!("{exit_code:?}"),
                other => format!("{other:?}"),
            };
            let tail = errors.into_iter().collect::<Vec<_>>().join("\n");
            Err(format!(
                "The flow on {id} ended without an outcome (exit {code}).\n{tail}"
            ))
        }
    }
}

/// `chat work --on boat`.
pub(super) async fn work(output: &Output, request: Request) -> Result<u8, Failure> {
    let output = *output;
    let client = Arc::new(
        boat::Client::from_env()
            .await
            .map_err(|e| failed(format!("no Boat key: {e} (set BOAT_API_KEY)")))?,
    );
    let logins = request.logins;
    let engine_fallback = request.engine_fallback;
    let credentials = Arc::new(
        tokio::task::spawn_blocking(move || credentials(logins, engine_fallback))
            .await
            .map_err(|_| failed("the run credentials could not be read"))?
            .map_err(failed)?,
    );
    let starts = Arc::new(Starts::new(STARTS_PER_MINUTE, Duration::from_secs(60)));
    let named = match &request.template {
        Some(name) => Some(name.clone()),
        None => client
            .list_named_snapshots()
            .await
            .ok()
            .and_then(|list| newest_template(&list.snapshots)),
    };
    let (source, seed) = match named {
        Some(name) => (Source::Template(name), None),
        None => {
            let seed = build_seed(&client, output, &starts).await.map_err(failed)?;
            (Source::Seed(seed.clone()), Some(seed))
        }
    };
    event(
        &output,
        json!({"event": "queue", "repository": request.repository, "issues": request.numbers,
            "parallel": request.parallel, "placement": COMPUTER, "source": source.name(),
            "engine_logins": logins.as_str()}),
    );
    if !output.json() {
        eprintln!(
            "Coder works {} issue(s) of {} on Boat, {} at a time, from {}: {}",
            request.numbers.len(),
            request.repository,
            request.parallel,
            source.name(),
            request
                .numbers
                .iter()
                .map(|n| format!("#{n}"))
                .collect::<Vec<_>>()
                .join(", ")
        );
    }
    let (stop, stopping) = watch::channel(false);
    let slots = Arc::new(Semaphore::new(
        usize::try_from(request.parallel).unwrap_or(1),
    ));
    let request = Arc::new(request);
    let mut runs = tokio::task::JoinSet::new();
    for &issue in &request.numbers {
        let (client, request, source, credentials, starts, slots, stopping) = (
            Arc::clone(&client),
            Arc::clone(&request),
            source.clone(),
            Arc::clone(&credentials),
            Arc::clone(&starts),
            Arc::clone(&slots),
            stopping.clone(),
        );
        runs.spawn(async move {
            let _slot = slots.acquire_owned().await;
            run_issue(
                client,
                output,
                request,
                source,
                credentials,
                starts,
                issue,
                stopping,
            )
            .await
        });
    }
    let mut results = Vec::new();
    let interrupt = tokio::signal::ctrl_c();
    tokio::pin!(interrupt);
    let mut interrupted = false;
    loop {
        let joined = tokio::select! {
            joined = runs.join_next() => joined,
            _ = &mut interrupt, if !interrupted => {
                interrupted = true;
                let _ = stop.send(true);
                eprintln!("Stopping: no more sandboxes start, and each running flow is killed and its sandbox stopped.");
                continue;
            }
        };
        let Some(joined) = joined else { break };
        let Ok(record) = joined else { continue };
        event(&output, record.clone());
        if !output.json() {
            let cost = record["cost_usd"]
                .as_f64()
                .map_or_else(String::new, |d| format!(" Cost {}.", dollars(d)));
            println!(
                "#{}: {}. {}{cost} ({} s on {})",
                record["issue"],
                record["outcome"].as_str().unwrap_or("failed"),
                record["message"].as_str().unwrap_or_default(),
                record["wall_seconds"],
                record["sandbox"].as_str().unwrap_or("no sandbox"),
            );
            let _ = std::io::stdout().flush();
        }
        results.push(record);
    }
    if let Some(seed) = seed {
        let state = teardown(&client, &seed, true).await;
        say(
            output,
            json!({"event": "boat_seed", "sandbox": seed, "state": state}),
            &format!("boat: seed {seed} {state}"),
        );
    }
    let landed = results
        .iter()
        .filter(|r| {
            matches!(
                r["outcome"].as_str(),
                Some("landed" | "pull_request" | "queued")
            )
        })
        .count();
    let total: f64 = results.iter().filter_map(|r| r["cost_usd"].as_f64()).sum();
    event(
        &output,
        json!({"event": "queue_done", "issues": results.len(), "landed": landed,
            "placement": COMPUTER, "cost_usd": total}),
    );
    if !output.json() {
        eprintln!(
            "Coder landed {landed} of {} issue(s) on Boat; sandboxes cost {} in all.",
            results.len(),
            dollars(total)
        );
    }
    let good = results.iter().all(|r| {
        matches!(
            r["outcome"].as_str(),
            Some("landed" | "pull_request" | "queued" | "skipped" | "closed")
        )
    });
    Ok(if good { 0 } else { crate::EXIT_FAILURE })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn starts_wait_only_when_a_rolling_minute_is_full() {
        let now = Instant::now();
        let window = Duration::from_secs(60);
        let mut recent = VecDeque::new();
        assert_eq!(start_wait(&recent, now, window, 2), None);
        recent.push_back(now.checked_sub(Duration::from_secs(50)).unwrap());
        assert_eq!(start_wait(&recent, now, window, 2), None);
        recent.push_back(now.checked_sub(Duration::from_secs(10)).unwrap());
        // Full: the oldest of the last two leaves the window in 10 s.
        assert_eq!(
            start_wait(&recent, now, window, 2),
            Some(Duration::from_secs(10))
        );
        // A start older than the window no longer counts.
        let mut old = VecDeque::new();
        old.push_back(now.checked_sub(Duration::from_secs(61)).unwrap());
        old.push_back(now.checked_sub(Duration::from_secs(5)).unwrap());
        assert_eq!(start_wait(&old, now, window, 2), None);
    }

    #[test]
    fn the_run_script_holds_no_credential_and_deletes_the_env_file_first() {
        let script = run_script(10220, Some(Land::Main), false);
        let read = script.find(". /tmp/oa-run.env").unwrap();
        let removed = script.find("rm -f /tmp/oa-run.env").unwrap();
        let work = script.find("/debug/openagents\" chat work").unwrap();
        assert!(read < removed && removed < work);
        // Boat's restore and the ownership repair come before anything.
        let setup = script.find("bash /tmp/oa-boat-fork-ready.sh").unwrap();
        assert!(setup < read && FORK_READY.contains("ascii-lazyfs"));
        assert!(script.ends_with("--issues 10220 --parallel 1 --land main\n"));
        assert!(
            run_script(10220, Some(Land::Queue), false)
                .ends_with("--issues 10220 --parallel 1 --land queue\n")
        );
        assert!(script.contains("OPENAGENTS_CODER_CONTROLLER"));
        assert!(script.contains("OPENAGENTS_CODER_PLACEMENT=boat"));
        for word in ["GH_TOKEN=", "ghp_", "gho_", "xai-"] {
            assert!(!script.contains(word), "{word}");
        }
        // The engine key reaches the login shell through /tmp, by name only.
        assert!(script.contains(r#"printf 'export XAI_API_KEY=%q\n' "$XAI_API_KEY""#));
        assert!(script.contains("trap 'rm -f /tmp/oa-engine.env' EXIT"));
        assert!(run_script(1, None, true).ends_with("--parallel 1\n"));
    }

    #[test]
    fn the_template_binaries_are_used_only_when_no_rust_changed_since_its_revision() {
        let script = run_script(10342, Some(Land::Main), false);
        let gate = script
            .find(r#"git diff --quiet "$rev" HEAD -- Cargo.toml Cargo.lock crates"#)
            .unwrap();
        let reuse = script.find(r#"[ -n "$current" ]"#).unwrap();
        let used = script.find("using the template's openagents").unwrap();
        assert!(gate < reuse && reuse < used);
        assert!(script.contains(r#"git cat-file -e "$rev^{commit}""#));
        // The script parses.
        let checked = std::process::Command::new("bash")
            .args(["-n", "-c", &script])
            .status()
            .unwrap();
        assert!(checked.success());
    }

    #[test]
    fn credentials_quote_their_values_and_never_print_them() {
        let mut credentials = Credentials::default();
        credentials
            .variables
            .insert("GH_TOKEN".into(), "gho_secret'$(x)".into());
        assert_eq!(credentials.file(), "GH_TOKEN='gho_secret'\\''$(x)'\n");
        let shown = format!("{credentials:?}");
        assert!(shown.contains("GH_TOKEN") && !shown.contains("gho_secret"));
    }

    #[test]
    fn the_newest_ready_template_wins() {
        let snapshot = |name: &str, status: &str| NamedSnapshot {
            name: name.into(),
            status: status.into(),
            ..Default::default()
        };
        let list = [
            snapshot("oa-coder-main-20261001", "ready"),
            snapshot("oa-coder-main-20261003", "pending"),
            snapshot("oa-coder-main-20261002", "ready"),
            snapshot("gym-regex-log", "ready"),
        ];
        assert_eq!(
            newest_template(&list).as_deref(),
            Some("oa-coder-main-20261002")
        );
        assert_eq!(newest_template(&list[3..]), None);
    }

    #[test]
    fn inner_lines_name_the_task_the_outcome_and_the_events() {
        assert_eq!(
            inner(
                r#"{"event":"coder","issue":7,"thread":"t1","accepted":true,"task":{"host":"local","task":"k1"}}"#
            ),
            Inner::Started {
                task: "k1".into(),
                thread: "t1".into()
            }
        );
        assert_eq!(
            inner(
                r#"{"event":"issue","issue":7,"outcome":"landed","message":"Landed abc.","commits":["abc"]}"#
            ),
            Inner::Done {
                outcome: "landed".into(),
                message: "Landed abc.".into(),
                commits: vec!["abc".into()],
            }
        );
        assert_eq!(inner(r#"{"event":"queue","issues":[7]}"#), Inner::Other);
        assert_eq!(inner("not json"), Inner::Other);
    }

    #[test]
    fn the_cost_comment_and_route_record_carry_time_and_price() {
        let cost = Cost {
            wall: Duration::from_secs(754),
            machine_seconds: Some(760),
            dollars: Some(0.0152),
        };
        let body = cost_comment(
            "bx_1",
            "oa-coder-main-20261002",
            "landed",
            &cost,
            EngineLogins::ApiKeys,
        );
        assert!(body.contains("`bx_1`") && body.contains("754 s") && body.contains("760 s"));
        assert!(body.contains("billed time"));
        assert!(body.contains("$0.0152") && body.contains("`api-keys`"));
        let record = route_record(
            "o/r",
            7,
            "k1",
            "bx_1",
            "oa-coder-main-20261002",
            "landed",
            &cost,
        );
        assert_eq!(record["placement"]["computer"], "boat");
        assert_eq!(record["placement"]["grant"]["source"], "operator");
        assert_eq!(record["run"]["cost_microusd"], 15_200);
        assert_eq!(record["run"]["wall_ms"], 754_000);
        assert_eq!(record["run"]["projection"]["state"], "completed");
        let failed = route_record("o/r", 7, "k1", "bx_1", "s", "failed", &cost);
        assert_eq!(failed["run"]["projection"]["state"], "failed");
    }

    #[test]
    fn a_poll_reads_exit_liveness_and_both_streams() {
        assert_eq!(
            parse_poll("_ 1 eyJhIjoxfQo= _\n"),
            Some(Polled {
                exit: None,
                alive: true,
                out: b"{\"a\":1}\n".to_vec(),
                err: Vec::new(),
            })
        );
        assert_eq!(
            parse_poll("3 0 _ Ym9vbQ=="),
            Some(Polled {
                exit: Some(3),
                alive: false,
                out: Vec::new(),
                err: b"boom".to_vec(),
            })
        );
        assert_eq!(parse_poll("garbage"), None);
        let poll = poll_command(41, 10, 0);
        assert!(poll.contains("tail -c +11 /tmp/oa-run.out") && poll.contains("kill -0 41"));
        assert!(launch_command().starts_with("setsid nohup bash -c 'bash /tmp/oa-run.sh >"));
        assert!(launch_command().ends_with("& echo $!"));
    }

    fn fake_jwt(exp: u64) -> String {
        use base64::Engine as _;
        let part = |v: &Value| {
            base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(v.to_string().as_bytes())
        };
        format!(
            "{}.{}.sig",
            part(&json!({"alg": "none"})),
            part(&json!({"exp": exp, "iat": exp - 864_000}))
        )
    }

    fn chatgpt_auth(exp: u64) -> String {
        json!({
            "auth_mode": "chatgpt",
            "OPENAI_API_KEY": null,
            "tokens": {
                "id_token": "id-token",
                "access_token": fake_jwt(exp),
                "refresh_token": "rt-secret-single-use",
                "account_id": "acct-1",
            },
            "last_refresh": "2026-09-25T00:00:00Z",
        })
        .to_string()
    }

    #[test]
    fn a_cloud_run_gets_the_chatgpt_login_without_its_refresh_token() {
        let now = 1_000_000;
        let (login, left) = access_only_login(&chatgpt_auth(now + 50 * 3600), now).unwrap();
        assert_eq!(left, 50 * 3600);
        assert!(!login.contains("rt-secret-single-use"));
        let login: Value = serde_json::from_str(&login).unwrap();
        assert_eq!(login["auth_mode"], "chatgpt");
        assert_eq!(login["tokens"]["refresh_token"], "");
        assert_eq!(login["tokens"]["account_id"], "acct-1");
        assert_eq!(login["tokens"]["id_token"], "id-token");
        assert_eq!(
            login["tokens"]["access_token"].as_str(),
            Some(fake_jwt(now + 50 * 3600).as_str())
        );
    }

    #[test]
    fn cloud_fallback_is_opt_in_and_api_keys_are_only_read_when_allowed() {
        let mut variables = BTreeMap::new();
        let why =
            "access token has 60 min left; open Codex on the Mac once to refresh it, then rerun";
        let refused = select_codex(&mut variables, Err(why.into()), false, || {
            panic!("must not read API key")
        })
        .unwrap_err();
        assert!(refused.contains(why));
        assert!(refused.contains("--engine-fallback"));
        assert!(variables.is_empty());
        assert!(
            select_codex(&mut variables, Err(why.into()), true, || Some(
                "fixture-key".into()
            ))
            .unwrap()
        );
        assert!(variables.contains_key("OA_CODEX_API_KEY"));
        variables.clear();
        assert!(!select_codex(&mut variables, Err(why.into()), true, || None).unwrap());
        assert!(variables.is_empty());
        assert!(
            select_codex(
                &mut variables,
                Ok(("fixture-login".into(), 7200)),
                false,
                || panic!("must prefer ChatGPT")
            )
            .unwrap()
        );
        assert!(variables.contains_key("OA_CODEX_AUTH"));
    }

    #[test]
    fn cloud_login_requires_two_hours_including_the_boundary() {
        let now = 1_000_000;
        for left in [0, 3600, 7199] {
            let why = access_only_login(&chatgpt_auth(now + left), now).unwrap_err();
            assert!(why.contains("at least 2 h"), "{why}");
            assert!(
                why.contains("open Codex on the Mac once to refresh it, then rerun"),
                "{why}"
            );
            assert!(!why.contains("rt-secret"));
        }
        assert_eq!(
            access_only_login(&chatgpt_auth(now + 7200), now).unwrap().1,
            7200
        );
    }

    #[test]
    fn a_nearly_expired_or_api_key_codex_login_stays_here() {
        let now = 1_000_000;
        let soon = access_only_login(&chatgpt_auth(now + 3600), now).unwrap_err();
        assert!(soon.contains("60 min"), "{soon}");
        assert!(!soon.contains("rt-secret"));
        let api = json!({"auth_mode": "apikey", "OPENAI_API_KEY": "sk-x"}).to_string();
        assert!(
            access_only_login(&api, now)
                .unwrap_err()
                .contains("API key")
        );
        assert!(access_only_login("not json", now).is_err());
    }

    #[test]
    fn the_run_script_writes_the_codex_login_and_removes_it() {
        let script = run_script(7, None, false);
        assert!(script.contains("OA_CODEX_AUTH"));
        assert!(script.contains("base64 -d >\"$HOME/.codex/auth.json\""));
        assert!(script.contains("trap 'rm -f /tmp/oa-engine.env \"$HOME/.codex/auth.json\"' EXIT"));
        assert!(script.contains("unset OA_CODEX_AUTH"));
    }

    #[test]
    fn engine_logins_are_api_keys_or_boat() {
        assert_eq!(EngineLogins::parse("api-keys"), Ok(EngineLogins::ApiKeys));
        assert_eq!(EngineLogins::parse("boat"), Ok(EngineLogins::Boat));
        assert!(EngineLogins::parse("chatgpt").is_err());
    }
}
