//! Archil sandboxes for `openagents chat work --on archil` (#12136): the
//! second remote backend next to the GCE pool ([`crate::pool`]).
//!
//! One sandbox per issue. A template sandbox is prepared once per image
//! (the repository cloned at `origin/main`, the briefed agent built on the
//! warm target the checks use) and stopped; each issue forks it, so a run
//! starts on a warm target in seconds, then the fork is stopped and
//! deleted. The image carries the toolchains the repository pins
//! (`crates/coder-cloud/archil/Dockerfile`), because Archil's managed
//! network allowlist blocks rustup's downloads.
//!
//! The sandbox reaches github.com, the package registries and the model
//! APIs, but not GitHub's API, openagents.com or the landing queue's
//! bucket, so the orchestrator reads the issue, submits the pushed branch
//! to the landing queue, and comments; see `chat_archil.rs`.
//!
//! The account's key and region come from `ARCHIL_API_KEY` and
//! `ARCHIL_REGION`, else from an env file (`OA_ARCHIL_ENV`, else
//! `~/.openagents/cloud/archil.env`, else `~/work/.secrets/archil.env`).
//! Nothing here prints them.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::time::Duration;

use archil::{
    BuildImageRequest, ClientOptions, CreateSandboxRequest, FileSource, ImageStatus,
    ProcessStartOptions, ProcessStream, SandboxFileUploadOptions, SandboxForkOptions,
    SandboxStatus,
};
use serde::{Deserialize, Serialize};

pub use archil::{Archil, Sandbox};

/// The placement name route records carry.
pub const COMPUTER: &str = "archil";
/// The region Archil runs sandboxes in (its GCP regions refuse them).
pub const DEFAULT_REGION: &str = "aws-us-east-1";
/// Each sandbox's shape.
pub const VCPUS: u32 = 8;
pub const MEM_MIB: u32 = 16384;
/// The image every sandbox starts from, built from
/// `crates/coder-cloud/archil/Dockerfile`; `OA_ARCHIL_IMAGE` picks another.
pub const DEFAULT_IMAGE: &str =
    "us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/archil-worker:v3";
/// Published list prices (archil.com/pricing, 2026-10-18).
const USD_PER_VCPU_HOUR: f64 = 0.055;
const USD_PER_GIB_HOUR: f64 = 0.020;
/// A template older than this is prepared again from `origin/main`.
const TEMPLATE_HOURS: u64 = 24;
/// The run user's home in the image.
pub const HOME: &str = "/home/coder";
/// The run's private directory in the sandbox.
pub const RUN_DIR: &str = "/home/coder/.oa-run";

/// The machine name the spend ledger prices ([`hourly_usd`]).
pub fn machine() -> String {
    format!("archil-{VCPUS}x{MEM_MIB}")
}

/// The hourly list price of an `archil-{vcpus}x{mib}` machine, or `None`
/// for any other name.
pub fn hourly_usd(machine: &str) -> Option<f64> {
    let (vcpus, mib) = machine.strip_prefix("archil-")?.split_once('x')?;
    let (vcpus, mib) = (vcpus.parse::<f64>().ok()?, mib.parse::<f64>().ok()?);
    Some(vcpus * USD_PER_VCPU_HOUR + mib / 1024.0 * USD_PER_GIB_HOUR)
}

/// `NAME=value` lines (optionally `export`ed and quoted).
pub fn parse_env(text: &str) -> BTreeMap<String, String> {
    text.lines()
        .map(str::trim)
        .filter(|line| !line.is_empty() && !line.starts_with('#'))
        .filter_map(|line| line.trim_start_matches("export ").split_once('='))
        .map(|(name, value)| {
            let value = value.trim();
            let unquoted = value
                .strip_prefix('"')
                .and_then(|v| v.strip_suffix('"'))
                .or_else(|| value.strip_prefix('\'').and_then(|v| v.strip_suffix('\'')))
                .unwrap_or(value);
            (name.trim().to_owned(), unquoted.to_owned())
        })
        .collect()
}

fn home() -> PathBuf {
    user_home::home_os().map_or_else(|| PathBuf::from("."), PathBuf::from)
}

/// The operator's secrets directory, for files the env does not name.
fn secrets_dir() -> PathBuf {
    std::env::var_os("OA_SECRETS_DIR").map_or_else(|| home().join("work/.secrets"), PathBuf::from)
}

/// A variable from the environment, else from the Archil env file.
pub fn setting(name: &str) -> Option<String> {
    if let Some(value) = std::env::var(name).ok().filter(|v| !v.is_empty()) {
        return Some(value);
    }
    let files = [
        std::env::var_os("OA_ARCHIL_ENV").map(PathBuf::from),
        Some(home().join(".openagents/cloud/archil.env")),
        Some(secrets_dir().join("archil.env")),
    ];
    files
        .into_iter()
        .flatten()
        .filter_map(|path| std::fs::read_to_string(path).ok())
        .find_map(|text| parse_env(&text).remove(name).filter(|v| !v.is_empty()))
}

/// The Claude Code OAuth token the sandboxes' Claude fallback signs in
/// with: `CLAUDE_CODE_OAUTH_TOKEN`, else `OA_CLAUDE_TOKEN_FILE`, else the
/// operator's `claude-code-oauth-token` secret file.
pub fn claude_token() -> Option<String> {
    if let Some(token) = std::env::var("CLAUDE_CODE_OAUTH_TOKEN")
        .ok()
        .filter(|t| !t.is_empty())
    {
        return Some(token);
    }
    let path = std::env::var_os("OA_CLAUDE_TOKEN_FILE").map_or_else(
        || secrets_dir().join("claude-code-oauth-token"),
        PathBuf::from,
    );
    std::fs::read_to_string(path)
        .ok()
        .map(|t| t.trim().to_owned())
        .filter(|t| !t.is_empty())
}

/// The finder's embeddings key (`OPENROUTER_API_KEY`, else the operator's
/// `openrouter.env`); openrouter.ai is on Archil's allowlist.
pub fn openrouter_key() -> Option<String> {
    std::env::var("OPENROUTER_API_KEY")
        .ok()
        .filter(|k| !k.is_empty())
        .or_else(|| {
            let text = std::fs::read_to_string(secrets_dir().join("openrouter.env")).ok()?;
            parse_env(&text)
                .remove("OPENROUTER_API_KEY")
                .filter(|k| !k.is_empty())
        })
}

/// A client for the operator's Archil account.
pub fn client() -> Result<Archil, String> {
    let api_key = setting("ARCHIL_API_KEY").ok_or(
        "no Archil API key: set ARCHIL_API_KEY or put it in ~/.openagents/cloud/archil.env",
    )?;
    let region = setting("ARCHIL_REGION").unwrap_or_else(|| DEFAULT_REGION.to_owned());
    Archil::new(ClientOptions {
        api_key: Some(api_key),
        region: Some(region),
        ..ClientOptions::default()
    })
    .map_err(|e| format!("Archil: {e}"))
}

/// What this computer remembers of its Archil image and template.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct State {
    pub image_source: Option<String>,
    pub image_id: Option<String>,
    pub template_id: Option<String>,
    pub template_image_id: Option<String>,
    pub template_rev: Option<String>,
    /// Unix seconds the template was prepared.
    pub template_at: Option<u64>,
}

impl State {
    fn path() -> PathBuf {
        std::env::var_os("OPENAGENTS_CLOUD_HOME")
            .map_or_else(|| home().join(".openagents/cloud"), PathBuf::from)
            .join("archil.json")
    }

    pub fn load() -> Self {
        std::fs::read_to_string(Self::path())
            .ok()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    pub fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(dir) = path.parent() {
            std::fs::create_dir_all(dir).map_err(|e| format!("{}: {e}", dir.display()))?;
        }
        let text = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, text + "\n").map_err(|e| format!("{}: {e}", path.display()))
    }

    /// Whether the remembered template can be forked for `image_id` at `now`.
    pub fn template_fresh(&self, image_id: &str, now: u64) -> bool {
        self.template_id.is_some()
            && self.template_image_id.as_deref() == Some(image_id)
            && self
                .template_at
                .is_some_and(|at| now.saturating_sub(at) < TEMPLATE_HOURS * 3600)
    }
}

/// The image every sandbox starts from, imported into Archil once per
/// source. A private Artifact Registry source is pulled with a short-lived
/// token of the pool's gcloud account; Archil never stores it.
pub async fn image(client: &Archil) -> Result<String, String> {
    let source = std::env::var("OA_ARCHIL_IMAGE")
        .ok()
        .filter(|s| !s.is_empty())
        .unwrap_or_else(|| DEFAULT_IMAGE.to_owned());
    let mut state = State::load();
    if state.image_source.as_deref() == Some(source.as_str())
        && let Some(id) = state.image_id.clone()
        && client
            .images()
            .get(&id)
            .await
            .is_ok_and(|image| image.status == ImageStatus::Ready)
    {
        return Ok(id);
    }
    let mut request = BuildImageRequest::new(source.clone());
    if source.contains("-docker.pkg.dev/") {
        let token = tokio::task::spawn_blocking(|| {
            crate::pool::gcloud_in(
                &crate::pool::project(),
                &["auth", "print-access-token"],
                Duration::from_secs(60),
            )
        })
        .await
        .map_err(|_| "gcloud did not run".to_owned())??;
        request = request.registry_auth("oauth2accesstoken", token.trim());
    }
    let image = client
        .images()
        .build(request)
        .await
        .map_err(|e| format!("Archil could not import {source}: {e}"))?;
    state.image_source = Some(source);
    state.image_id = Some(image.image_id.clone());
    state.save()?;
    Ok(image.image_id)
}

/// Runs `script` in `sandbox` as the run user and returns its exit code,
/// passing each output line to `line` (stdout `true`, stderr `false`).
pub async fn run_lines(
    sandbox: &Sandbox,
    script_path: &str,
    timeout: Duration,
    mut line: impl FnMut(bool, &str),
) -> Result<i64, String> {
    let command =
        format!("cd {HOME} && exec runuser -u coder -- env HOME={HOME} bash {script_path}");
    let mut process = sandbox
        .run(
            &command,
            ProcessStartOptions::new()
                .stream_output(true)
                .collect_output(false)
                .timeout_seconds(timeout.as_secs().max(60)),
        )
        .await
        .map_err(|e| format!("the sandbox did not start the run: {e}"))?;
    let (mut out, mut err) = (Vec::new(), Vec::new());
    while let Some(chunk) = process.next_output().await {
        let (buffer, stdout) = match chunk.stream {
            ProcessStream::Stdout => (&mut out, true),
            ProcessStream::Stderr => (&mut err, false),
        };
        buffer.extend_from_slice(&chunk.data);
        while let Some(end) = buffer.iter().position(|b| *b == b'\n') {
            let text: Vec<u8> = buffer.drain(..=end).collect();
            line(stdout, String::from_utf8_lossy(&text).trim_end());
        }
    }
    for (buffer, stdout) in [(out, true), (err, false)] {
        if !buffer.is_empty() {
            line(stdout, String::from_utf8_lossy(&buffer).trim_end());
        }
    }
    let result = process
        .wait()
        .await
        .map_err(|e| format!("the sandbox lost the run: {e}"))?;
    Ok(result.exit_code.unwrap_or(-1))
}

/// Writes `bytes` to `path` in the sandbox, readable only by the run user.
pub async fn put(sandbox: &Sandbox, path: &str, bytes: Vec<u8>, mode: u32) -> Result<(), String> {
    sandbox
        .files()
        .upload_file(
            FileSource::Bytes(bytes.into()),
            path,
            SandboxFileUploadOptions { mode },
        )
        .await
        .map_err(|e| format!("could not write {path} in the sandbox: {e}"))?;
    sandbox
        .exec(
            &format!("chown -R coder:coder {RUN_DIR} 2>/dev/null; true"),
            ProcessStartOptions::new().timeout_seconds(60),
        )
        .await
        .map_err(|e| format!("could not hand {path} to the run user: {e}"))?;
    Ok(())
}

/// The template's preparation, as the run user: the repository at
/// `origin/main` and the briefed agent built on the checks' warm target.
pub fn prepare_script(repository: &str) -> String {
    format!(
        r#"set -uo pipefail
{ENV}
cd "$HOME"
# A full clone: Archil's egress proxy refuses a partial clone's large
# lazy-fetch requests.
[ -d openagents/.git ] || git clone -q https://github.com/{repository}.git openagents || exit 2
cd openagents && git fetch -q origin main && git checkout -q --detach origin/main || exit 2
oa_agent || exit 3
# The workspace's shared dependencies, so a fork's first check is warm.
echo "archil: warming the target with openagents-cli" >&2
cargo build --locked -q -p openagents-cli 2>&1 | tail -n 20 >&2
git rev-parse HEAD
"#
    )
}

/// The environment and briefed-agent build every sandbox script shares.
const ENV: &str = r#"export PATH=/usr/local/cargo/bin:$HOME/.local/bin:/usr/local/bin:/usr/bin:/bin \
  RUSTUP_HOME=/usr/local/rustup CARGO_HOME=/usr/local/cargo CARGO_INCREMENTAL=0 \
  CARGO_PROFILE_DEV_DEBUG=0 CARGO_PROFILE_TEST_DEBUG=0 \
  CARGO_TARGET_DIR=$HOME/.openagents/work/target IS_SANDBOX=1
d=$HOME/.oa-run; mkdir -p "$d/bin"
oa_agent() {
  local rev; rev=$(git rev-parse HEAD)
  [ "$(cat "$d/agent-rev" 2>/dev/null)" = "$rev" ] && [ -x "$d/bin/briefed-agent" ] && return 0
  echo "archil: building the briefed agent at ${rev:0:10}" >&2
  cargo build --locked -q -p briefed-agent >"$d/build.log" 2>&1 || { tail -n 40 "$d/build.log" >&2; return 3; }
  cp -f "$CARGO_TARGET_DIR/debug/briefed-agent" "$d/bin/briefed-agent" && echo "$rev" >"$d/agent-rev"
}"#;

/// One issue's run, as the run user: read and delete the credentials,
/// sign the engines in, move the checkout to `origin/main`, rebuild the
/// briefed agent when main moved, and run the driver with the issue the
/// orchestrator read. With `direct` (#12165) the driver rebases, checks
/// again and pushes to main itself, and otherwise (or on its fallback)
/// pushes the change to `branch` for the orchestrator to land. The
/// orchestrator posts every issue comment.
pub fn run_script(
    repository: &str,
    issue: u64,
    engine: &str,
    direct: bool,
    branch: &str,
    run_id: &str,
) -> String {
    let quote = boat::shell_quote;
    format!(
        r#"set -uo pipefail
{ENV}
if [ -f "$d/env" ]; then set -a; . "$d/env"; set +a; rm -f "$d/env"; fi
[ -n "${{OA_GIT_NAME:-}}" ] && git config --global user.name "$OA_GIT_NAME"
[ -n "${{OA_GIT_EMAIL:-}}" ] && git config --global user.email "$OA_GIT_EMAIL"
unset OA_GIT_NAME OA_GIT_EMAIL
# git reaches github.com with the run's token; the engine's own
# environment drops it (work_issue.py's agent_env).
git config --global credential.helper '!f() {{ test "$1" = get && echo username=x-access-token && echo "password=$GH_TOKEN"; }}; f'
if [ -n "${{OA_CODEX_API_KEY:-}}" ]; then
  printenv OA_CODEX_API_KEY | codex login --with-api-key >/dev/null 2>&1 \
    || echo "archil: codex login with the API key failed" >&2
fi
unset OA_CODEX_API_KEY
if [ -n "${{OA_CODEX_AUTH:-}}" ]; then
  mkdir -p "$HOME/.codex" && (umask 077; printf '%s' "$OA_CODEX_AUTH" | base64 -d >"$HOME/.codex/auth.json") \
    || echo "archil: the Codex login could not be written" >&2
fi
unset OA_CODEX_AUTH
cd "$HOME/openagents" && git fetch -q origin main && git checkout -q --detach origin/main \
  || {{ echo "archil: git fetch failed" >&2; exit 2; }}
oa_agent || exit 3
export OA_BRIEFED_AGENT=$d/bin/briefed-agent OA_WORK_PUSH_BRANCH={branch} \
  OA_WORK_FALLBACK=push OA_WORK_HOST_COMMENTS=1
exec python3 "$d/engine/scripts/work/work_issue.py" --repo {repository} --issue {issue} \
  --engine {engine} --land {land} --checkout "$HOME/openagents" --issue-json "$d/issue.json" \
  --out "$d/out" --run-id {run_id}
"#,
        branch = quote(branch),
        land = if direct { "direct" } else { "push" },
        repository = quote(repository),
        engine = quote(engine),
        run_id = quote(run_id),
    )
}

/// Stops and deletes `sandbox`, best effort.
pub async fn discard(sandbox: &mut Sandbox) {
    if !matches!(
        sandbox.status,
        SandboxStatus::Stopped | SandboxStatus::Exited | SandboxStatus::Deleted
    ) {
        let _ = sandbox.stop().await;
    }
    let _ = sandbox.delete().await;
}

/// The template for `image_id`, prepared now unless a fresh one is
/// remembered. `say` hears each step.
pub async fn template(
    client: &Archil,
    image_id: &str,
    repository: &str,
    say: &(dyn Fn(&str) + Sync),
) -> Result<Sandbox, String> {
    let mut state = State::load();
    let now = crate::pool::now();
    if state.template_fresh(image_id, now)
        && let Some(id) = state.template_id.clone()
        && let Ok(sandbox) = client.sandboxes().get(&id).await
        && matches!(
            sandbox.status,
            SandboxStatus::Stopped | SandboxStatus::Paused | SandboxStatus::Exited
        )
    {
        return Ok(sandbox);
    }
    say("preparing the Archil template: origin/main and a warm target (once a day)");
    let mut sandbox = client
        .sandboxes()
        .create(
            CreateSandboxRequest::new()
                .name(format!("oa-archil-template-{now}"))
                .image_id(image_id)
                .vcpu_count(VCPUS)
                .mem_size_mib(MEM_MIB)
                .idle_ttl_seconds(0),
        )
        .await
        .map_err(|e| format!("Archil could not create the template: {e}"))?;
    let prepared = async {
        let path = format!("{RUN_DIR}/prepare.sh");
        put(
            &sandbox,
            &path,
            prepare_script(repository).into_bytes(),
            0o644,
        )
        .await?;
        let mut rev = String::new();
        let code = run_lines(
            &sandbox,
            &path,
            Duration::from_secs(3 * 3600),
            |stdout, text| {
                if stdout {
                    rev = text.trim().to_owned();
                } else {
                    say(text);
                }
            },
        )
        .await?;
        if code != 0 {
            return Err(format!("the template's preparation failed (exit {code})"));
        }
        Ok(rev)
    }
    .await;
    let rev = match prepared {
        Ok(rev) => rev,
        Err(error) => {
            discard(&mut sandbox).await;
            return Err(error);
        }
    };
    sandbox
        .stop()
        .await
        .map_err(|e| format!("Archil could not stop the template: {e}"))?;
    if let Some(old) = state.template_id.take()
        && let Ok(mut old) = client.sandboxes().get(&old).await
    {
        discard(&mut old).await;
    }
    state.template_id = Some(sandbox.id.clone());
    state.template_image_id = Some(image_id.to_owned());
    state.template_rev = Some(rev);
    state.template_at = Some(now);
    state.save()?;
    Ok(sandbox)
}

/// A new sandbox for one run: a fork of the template that answers a
/// command. Some forks report `running` but refuse every process
/// (HTTP 400, about 5 in 11 on 2026-10-18, #12157); such a fork is deleted
/// and the template forked again, up to three times.
pub async fn fork(template: &mut Sandbox, name: &str) -> Result<Sandbox, String> {
    let mut last = String::new();
    for _ in 0..FORK_TRIES {
        let mut sandbox = template
            .fork_with(SandboxForkOptions {
                wait: true,
                name: Some(name.to_owned()),
            })
            .await
            .map_err(|e| format!("Archil could not fork the template: {e}"))?;
        match answers(&sandbox).await {
            Ok(()) => return Ok(sandbox),
            Err(why) => {
                last = why;
                discard(&mut sandbox).await;
            }
        }
    }
    Err(format!(
        "Archil forked the template {FORK_TRIES} times, but no fork ran a command: {last}"
    ))
}

const FORK_TRIES: usize = 3;

/// Whether `sandbox` runs a command within about half a minute.
async fn answers(sandbox: &Sandbox) -> Result<(), String> {
    let mut last = String::new();
    for _ in 0..3 {
        match sandbox
            .exec("true", ProcessStartOptions::new().timeout_seconds(30))
            .await
        {
            Ok(result) if result.exit_code == Some(0) => return Ok(()),
            Ok(result) => last = format!("`true` exited {:?}", result.exit_code),
            Err(e) => last = e.to_string(),
        }
        tokio::time::sleep(Duration::from_secs(5)).await;
    }
    Err(last)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn prices_follow_the_published_rates() {
        let price = hourly_usd(&machine()).unwrap();
        assert!((price - (8.0 * 0.055 + 16.0 * 0.020)).abs() < 1e-9);
        assert_eq!(hourly_usd("c3-standard-8"), None);
        assert!(crate::pool::hourly_usd(&machine(), false) - price < 1e-9);
    }

    #[test]
    fn env_files_parse_quoted_and_exported_lines() {
        let parsed = parse_env("# c\nexport A='x y'\nB=\"z\"\nC=plain\n\nbad\n");
        assert_eq!(parsed["A"], "x y");
        assert_eq!(parsed["B"], "z");
        assert_eq!(parsed["C"], "plain");
        assert_eq!(parsed.len(), 3);
    }

    #[test]
    fn a_template_is_reused_only_for_its_image_within_a_day() {
        let state = State {
            template_id: Some("t".into()),
            template_image_id: Some("img".into()),
            template_at: Some(1_000),
            ..State::default()
        };
        assert!(state.template_fresh("img", 1_000 + 3600));
        assert!(!state.template_fresh("other", 1_000));
        assert!(!state.template_fresh("img", 1_000 + TEMPLATE_HOURS * 3600));
    }

    #[test]
    fn the_run_script_reads_and_deletes_credentials_before_anything_else() {
        let script = run_script("o/r", 7, "briefed:codex", false, "land/archil-7-x", "run1");
        let deleted = script.find(r#"rm -f "$d/env""#).unwrap();
        assert!(deleted < script.find("credential.helper").unwrap());
        assert!(script.contains("--land push"));
        assert!(script.contains("OA_WORK_FALLBACK=push OA_WORK_HOST_COMMENTS=1"));
        let direct = run_script("o/r", 7, "briefed", true, "land/archil-7-x", "run1");
        assert!(direct.contains("--land direct"));
        assert!(script.contains("--issue-json \"$d/issue.json\""));
        assert!(script.contains("OA_WORK_PUSH_BRANCH='land/archil-7-x'"));
        assert!(script.contains("--engine 'briefed:codex'"));
        // No credential value is ever in a script.
        assert!(!script.contains("GH_TOKEN="));
    }
}
