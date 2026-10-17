//! Our cloud: the account's OpenAgents engine on a cloud computer (#11852).
//!
//! The phone has no desktop to run sessions on. These routes start the
//! headless OpenAgents engine on one of the account's cloud computers, so it
//! joins `coder-sync` as one more of the account's devices ("Our cloud") and
//! the phone drives it like a desktop.
//!
//! - `GET /v1/cloud-host`: the account's cloud host and whether its engine runs.
//! - `POST /v1/cloud-host`: start it. Creates the computer the first time,
//!   then runs the engine as one command of at most [`RUN_SECONDS`]. A
//!   command already running is returned unchanged.
//! - `POST /v1/cloud-host/stop`: end the engine and stop the computer.
//! - `DELETE /v1/cloud-host`: end the engine and delete the computer.
//!
//! Sessions run on OpenAgents inference: the run installs a pinned opencode
//! configured for the OpenAgents API, so no other model key is needed.
//! Claude Code also works when the account saved its own Claude key.
//!
//! Credentials go by reference ([`crate::computers_api::commands`]): the
//! engine's account credential is a sign-in key made for this run (opencode
//! uses the same key for inference), and Claude Code's credential is the
//! account's own (Settings) or the server's.
//! Both reach the engine's environment in memory and are never written to
//! the computer's disk or to the record kept here. A new run, stop, or
//! delete revokes the previous run's key. Usage is recorded by the
//! Computers service like any other computer.

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::Response;
use axum::routing::{get, post};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use crate::App;
use crate::coder_sync::{answer, refused};

const PATH: &str = "/v1/cloud-host";
const STOP: &str = "/v1/cloud-host/stop";
/// One engine run; the command limit is an hour.
pub(crate) const RUN_SECONDS: u64 = 3600;
/// The engine exits on its own a little before the command limit.
const ENGINE_SECONDS: u64 = 3480;
/// The device name the phone shows.
pub(crate) const DEVICE_NAME: &str = "Our cloud";
const KEY_NAME: &str = "OpenAgents cloud computer";
const KEY_DAYS: u64 = 30;
/// The opencode the run installs when the computer has none.
const OPENCODE_VERSION: &str = "1.19.3";
/// The OpenAgents API opencode calls, and the models it offers.
const INFERENCE_URL: &str = "https://api.openagents.com/v1";
const MODELS: [(&str, &str); 4] = [
    ("openagents/code", "OpenAgents Code"),
    ("openagents/fast", "OpenAgents Fast"),
    ("openagents/reason", "OpenAgents Reason"),
    ("google/gemini-2.5-flash-lite", "Gemini 2.5 Flash Lite"),
];

/// opencode's configuration: the OpenAgents provider, its key read from the
/// environment, never written here.
pub(crate) fn opencode_config() -> String {
    let models: serde_json::Map<String, Value> = MODELS
        .iter()
        .map(|(id, name)| ((*id).to_owned(), json!({ "name": name })))
        .collect();
    json!({
        "$schema": "https://opencode.ai/config.json",
        "provider": {"openagents": {
            "npm": "@ai-sdk/openai-compatible",
            "name": "OpenAgents",
            "options": {"baseURL": INFERENCE_URL, "apiKey": "{env:OPENAGENTS_API_KEY}"},
            "models": models,
        }},
        "model": "openagents/openagents/fast",
    })
    .to_string()
}

/// The Linux engine the computer runs, pinned by digest. Deployments can
/// pin another build with `OPENAGENTS_CLOUD_HOST_ENGINE_URL` and
/// `OPENAGENTS_CLOUD_HOST_ENGINE_SHA256` together.
const ENGINE_URL: &str = "https://storage.googleapis.com/openagentsgemini-cli-releases/openagents-host/3063ab036c/openagents-host-linux-x86_64";
const ENGINE_SHA256: &str = "c5e3ee693f8235039d5881c6d3721753cea94de19f26c6b1431f2e2add1138f6";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(status).post(start).delete(remove))
        .route(STOP, post(stop))
}

/// What is kept per account: never a credential.
#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub(crate) struct Record {
    pub computer: Option<String>,
    /// The sign-in key made for the current run, to revoke later.
    pub key_id: Option<String>,
    pub command: Option<String>,
    pub started_unix: Option<u64>,
    /// Counts computers made, so a deleted computer's request key is never reused.
    pub generation: u64,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Engine {
    pub url: String,
    pub sha256: String,
}

impl Engine {
    pub(crate) fn configured() -> Option<Self> {
        let pair = match (
            std::env::var("OPENAGENTS_CLOUD_HOST_ENGINE_URL").ok(),
            std::env::var("OPENAGENTS_CLOUD_HOST_ENGINE_SHA256").ok(),
        ) {
            (Some(url), Some(sha256)) => (url, sha256),
            (None, None) => (ENGINE_URL.to_owned(), ENGINE_SHA256.to_owned()),
            _ => return None,
        };
        let engine = Self {
            url: pair.0,
            sha256: pair.1.to_ascii_lowercase(),
        };
        engine.valid().then_some(engine)
    }
    fn valid(&self) -> bool {
        self.url.starts_with("https://")
            && self
                .url
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"/:._-".contains(&b))
            && self.sha256.len() == 64
            && self.sha256.bytes().all(|b| b.is_ascii_hexdigit())
    }
}

/// The command that runs the engine. Fixed text: no account data, so its
/// digest binds only the engine build.
pub(crate) fn command(engine: &Engine) -> String {
    let dir = &engine.sha256[..16];
    format!(
        r#"set -eu
umask 077
d="$HOME/.openagents-host"
b="$d/bin/{dir}/openagents-host"
if [ ! -x "$b" ]; then
  mkdir -p "$d/bin/{dir}"
  t=$(mktemp "$d/bin/{dir}/download.XXXXXX")
  curl -fsSL --retry 3 -o "$t" '{url}' || {{ rm -f "$t"; echo 'The OpenAgents engine could not be downloaded.' >&2; exit 1; }}
  echo "{sha}  $t" | sha256sum -c --quiet - || {{ rm -f "$t"; echo 'The OpenAgents engine download failed its check.' >&2; exit 1; }}
  chmod 755 "$t"
  mv "$t" "$b"
fi
export PATH="$HOME/.local/bin:/usr/local/bin:$PATH"
if ! command -v opencode >/dev/null 2>&1; then
  npm install --silent --prefix "$HOME/.local" -g 'opencode-ai@{opencode}' >/dev/null 2>&1 || echo 'opencode could not be installed; Claude Code still works with your own key.' >&2
fi
mkdir -p "$HOME/.config/opencode"
cat > "$HOME/.config/opencode/opencode.json" <<'OPENCODE'
{config}
OPENCODE
mkdir -p "$HOME/workspace" "$d/data"
export OPENAGENTS_DATA_DIR="$d/data" ZERON_DATA_DIR="$d/data"
export OPENAGENTS_DEVICE_NAME='{name}' ZERON_DEVICE_NAME='{name}'
export BROWSER=/usr/bin/true NO_OPEN_BROWSER=1
cd "$HOME/workspace"
echo 'OpenAgents engine starting'
exec timeout -k 30 {seconds} "$b" headless
"#,
        url = engine.url,
        sha = engine.sha256,
        name = DEVICE_NAME,
        seconds = ENGINE_SECONDS,
        opencode = OPENCODE_VERSION,
        config = opencode_config(),
    )
}

/// The Claude Code credential names the engine's harness reads.
pub(crate) fn engine_credential_name(name: &str) -> bool {
    matches!(name, "ANTHROPIC_API_KEY" | "CLAUDE_CODE_OAUTH_TOKEN")
}

pub(crate) fn running(state: &str) -> bool {
    matches!(state, "queued" | "starting" | "running")
}

struct Caller {
    owner: String,
    token: String,
}

async fn caller(app: &App, headers: &HeaderMap) -> Result<Caller, Response> {
    let identity = crate::computers_api::identity(app, headers).await?;
    if identity.environment_attempt.is_some() {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "forbidden",
            "This key can't use Our cloud.",
        ));
    }
    if !identity.permitted {
        return Err(refused(
            StatusCode::FORBIDDEN,
            "invite_only",
            "Cloud computers aren't available for this account yet.",
        ));
    }
    let token = headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default()
        .to_owned();
    Ok(Caller {
        owner: identity.owner,
        token,
    })
}

fn unavailable() -> Response {
    refused(
        StatusCode::SERVICE_UNAVAILABLE,
        "unavailable",
        "Our cloud isn't available right now. Try again later.",
    )
}

async fn load(app: &App, owner: &str) -> Result<(Record, Option<String>), Response> {
    let key =
        crate::chat_store::Store::owner_key(owner, "cloud-host.json").map_err(|_| unavailable())?;
    match app.config.chat_store.read_key(&key).await {
        Ok(Some((bytes, generation))) => Ok((
            serde_json::from_slice(&bytes).unwrap_or_default(),
            Some(generation),
        )),
        Ok(None) => Ok((Record::default(), None)),
        Err(_) => Err(unavailable()),
    }
}

async fn save(
    app: &App,
    owner: &str,
    record: &Record,
    generation: Option<&str>,
) -> Result<String, Response> {
    let key =
        crate::chat_store::Store::owner_key(owner, "cloud-host.json").map_err(|_| unavailable())?;
    app.config
        .chat_store
        .write_key(&key, serde_json::to_vec(record).unwrap(), generation)
        .await
        .map_err(|error| match error {
            crate::chat_store::Error::Conflict => refused(
                StatusCode::CONFLICT,
                "busy",
                "Our cloud is changing. Try again.",
            ),
            _ => unavailable(),
        })
}

async fn call(
    owner: &str,
    method: reqwest::Method,
    path: &str,
    body: Option<Value>,
) -> Result<(u16, Value), Response> {
    let client = crate::computers::client().ok_or_else(unavailable)?;
    client
        .call_scoped(method, path, owner, body, None)
        .await
        .map_err(|_| unavailable())
}

/// The computer, when it still exists and isn't being deleted.
async fn computer(owner: &str, record: &Record) -> Result<Option<Value>, Response> {
    let Some(id) = record.computer.as_deref() else {
        return Ok(None);
    };
    match call(
        owner,
        reqwest::Method::GET,
        &format!("/v1/computers/{id}"),
        None,
    )
    .await?
    {
        (200, v) if v["desired"] != "deleted" => Ok(Some(v)),
        (200 | 404, _) => Ok(None),
        _ => Err(unavailable()),
    }
}

async fn command_state(owner: &str, record: &Record) -> Result<Option<String>, Response> {
    let (Some(computer), Some(command)) = (record.computer.as_deref(), record.command.as_deref())
    else {
        return Ok(None);
    };
    match call(
        owner,
        reqwest::Method::GET,
        &format!("/v1/computers/{computer}/commands/{command}"),
        None,
    )
    .await?
    {
        (200, v) => Ok(v["state"].as_str().map(str::to_owned)),
        (404, _) => Ok(None),
        _ => Err(unavailable()),
    }
}

fn view(record: &Record, computer: Option<&Value>, command: Option<&str>) -> Value {
    let engine = command.is_some_and(running);
    json!({
        "computer": computer.map(crate::computers_api::view),
        "engine": if engine { "running" } else { "stopped" },
        "device_name": DEVICE_NAME,
        "started_unix": if engine { json!(record.started_unix) } else { Value::Null },
        "until_unix": if engine { json!(record.started_unix.map(|s| s + RUN_SECONDS)) } else { Value::Null },
    })
}

async fn status(State(app): State<App>, headers: HeaderMap) -> Response {
    let caller = match caller(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (record, _) = match load(&app, &caller.owner).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let computer = match computer(&caller.owner, &record).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let command = match computer {
        Some(_) => match command_state(&caller.owner, &record).await {
            Ok(c) => c,
            Err(r) => return r,
        },
        None => None,
    };
    answer(
        StatusCode::OK,
        view(&record, computer.as_ref(), command.as_deref()),
    )
}

async fn revoke_key(app: &App, token: &str, record: &mut Record) {
    if let (Some(id), Some(service)) = (record.key_id.take(), app.config.cloud.as_deref()) {
        let _ = service.revoke_sign_in_key(token, &id).await;
    }
}

async fn start(State(app): State<App>, headers: HeaderMap) -> Response {
    let caller = match caller(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let Some(engine) = Engine::configured() else {
        return unavailable();
    };
    let Some(service) = app.config.cloud.as_deref() else {
        return unavailable();
    };
    let (mut record, mut generation) = match load(&app, &caller.owner).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let mut computer = match computer(&caller.owner, &record).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    if computer.is_some() {
        match command_state(&caller.owner, &record).await {
            Ok(Some(state)) if running(&state) => {
                return answer(
                    StatusCode::OK,
                    view(&record, computer.as_ref(), Some(&state)),
                );
            }
            Ok(_) => {}
            Err(r) => return r,
        }
    }
    if computer.is_none() {
        record.generation += 1;
        let account = crate::chat_store::digest(caller.owner.as_bytes());
        let key = format!("cloud-host-{}-{}", &account[..16], record.generation);
        match call(
            &caller.owner,
            reqwest::Method::POST,
            "/v1/computers",
            Some(json!({"idempotency_key": key})),
        )
        .await
        {
            Ok((200 | 202, v))
                if crate::computers_api::cloud_id(v["id"].as_str().unwrap_or_default()) =>
            {
                record.computer = v["id"].as_str().map(str::to_owned);
                record.command = None;
                computer = Some(v);
            }
            Ok((429, _)) => {
                return refused(
                    StatusCode::TOO_MANY_REQUESTS,
                    "quota_exhausted",
                    "Delete an unused computer before starting Our cloud.",
                );
            }
            Ok((403, _)) => {
                return refused(
                    StatusCode::FORBIDDEN,
                    "invite_only",
                    "Cloud computers aren't available for this account yet.",
                );
            }
            _ => return unavailable(),
        }
        generation = match save(&app, &caller.owner, &record, generation.as_deref()).await {
            Ok(g) => Some(g),
            Err(r) => return r,
        };
    }
    let Some(computer_id) = record.computer.clone() else {
        return unavailable();
    };
    // Claude Code's credential: the account's own, else the server's.
    let claude = match crate::cloud::byo::release_turn(&app, &headers).await {
        Some(own) => Some(own),
        None => app
            .config
            .environments
            .studio()
            .and_then(|studio| studio.claude_key()),
    };
    let claude = claude
        .filter(|key| engine_credential_name(key.name()))
        .map(|key| key.into_env());
    revoke_key(&app, &caller.token, &mut record).await;
    let made = match service
        .create_sign_in_key(&caller.token, KEY_NAME, Some(KEY_DAYS))
        .await
    {
        Ok(Ok(made)) => made,
        Ok(Err(message)) => return refused(StatusCode::CONFLICT, "key_refused", &message),
        Err(crate::cloud::session::SessionError::Unauthenticated) => {
            return refused(
                StatusCode::FORBIDDEN,
                "forbidden",
                "Sign in on this device to start Our cloud.",
            );
        }
        Err(_) => return unavailable(),
    };
    record.key_id = Some(made.key.id.clone());
    generation = match save(&app, &caller.owner, &record, generation.as_deref()).await {
        Ok(g) => Some(g),
        Err(r) => {
            let _ = service
                .revoke_sign_in_key(&caller.token, &made.key.id)
                .await;
            return r;
        }
    };
    let text = command(&engine);
    let request = match openagents_sdk::computers::RequestKey::generate() {
        Ok(k) => format!("cloud-host-{}", k.as_str()),
        Err(_) => return unavailable(),
    };
    let reference = format!("cred_{}", &request["cloud-host-".len()..]);
    let mut values = vec![
        ("CODER_DESKTOP_CREDENTIAL".to_owned(), made.token.clone()),
        ("OPENAGENTS_API_KEY".to_owned(), made.token.clone()),
    ];
    values.extend(claude);
    if let Err(message) = crate::computers_api::commands::prepare_web_credentials(
        &caller.owner,
        &computer_id,
        &reference,
        &request,
        &text,
        &values,
    )
    .await
    {
        return refused(StatusCode::SERVICE_UNAVAILABLE, "unavailable", &message);
    }
    let sent = call(
        &caller.owner,
        reqwest::Method::POST,
        &format!("/v1/computers/{computer_id}/commands"),
        Some(json!({
            "command": text,
            "timeout_seconds": RUN_SECONDS,
            "idempotency_key": request,
            "credential_ref": reference,
        })),
    )
    .await;
    let command = match sent {
        Ok((200 | 202, v)) => v,
        _ => return unavailable(),
    };
    record.command = command["id"].as_str().map(str::to_owned);
    record.started_unix = Some(crate::chat_store::now_unix());
    if let Err(r) = save(&app, &caller.owner, &record, generation.as_deref()).await {
        return r;
    }
    answer(
        StatusCode::ACCEPTED,
        view(&record, computer.as_ref(), command["state"].as_str()),
    )
}

/// End the engine (and its key), then stop or delete the computer.
async fn end(app: &App, headers: &HeaderMap, delete: bool) -> Response {
    let caller = match caller(app, headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let (mut record, generation) = match load(app, &caller.owner).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let (Some(computer), Some(command)) = (record.computer.as_deref(), record.command.as_deref())
    {
        let _ = call(
            &caller.owner,
            reqwest::Method::POST,
            &format!("/v1/computers/{computer}/commands/{command}/cancel"),
            None,
        )
        .await;
    }
    revoke_key(app, &caller.token, &mut record).await;
    record.command = None;
    record.started_unix = None;
    let mut computer = None;
    if let Some(id) = record.computer.clone() {
        let (method, path) = if delete {
            (reqwest::Method::DELETE, format!("/v1/computers/{id}"))
        } else {
            (reqwest::Method::POST, format!("/v1/computers/{id}/stop"))
        };
        match call(&caller.owner, method, &path, None).await {
            Ok((200 | 202, v)) => computer = Some(v),
            Ok((404, _)) => {}
            _ => return unavailable(),
        }
        if delete {
            record.computer = None;
        }
    }
    if let Err(r) = save(app, &caller.owner, &record, generation.as_deref()).await {
        return r;
    }
    answer(StatusCode::ACCEPTED, view(&record, computer.as_ref(), None))
}

async fn stop(State(app): State<App>, headers: HeaderMap) -> Response {
    end(&app, &headers, false).await
}

async fn remove(State(app): State<App>, headers: HeaderMap) -> Response {
    end(&app, &headers, true).await
}

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> Engine {
        Engine {
            url: "https://storage.googleapis.com/b/openagents-host".into(),
            sha256: "ab".repeat(32),
        }
    }

    #[test]
    fn command_holds_no_credential_and_checks_the_download() {
        let text = command(&engine());
        assert!(text.contains("sha256sum -c"));
        assert!(text.contains(&"ab".repeat(32)));
        assert!(text.contains("headless"));
        assert!(text.contains("Our cloud"));
        assert!(text.contains("opencode-ai@1.19.3"));
        assert!(text.contains("{env:OPENAGENTS_API_KEY}"));
        for secret in [
            "CODER_DESKTOP_CREDENTIAL",
            "ANTHROPIC_API_KEY",
            "oa_sk_",
            "sess_",
        ] {
            assert!(!text.contains(secret), "{secret} in the command text");
        }
        // The engine ends before the command limit, so its key is not left running.
        assert!(ENGINE_SECONDS < RUN_SECONDS);
        assert_eq!(text, command(&engine()), "the digest must be stable");
    }

    #[test]
    fn the_built_in_engine_is_pinned() {
        let pinned = Engine {
            url: ENGINE_URL.into(),
            sha256: ENGINE_SHA256.into(),
        };
        assert!(pinned.valid());
    }

    #[test]
    fn engine_pins_need_both_parts() {
        assert!(engine().valid());
        let mut bad = engine();
        bad.sha256 = "zz".repeat(32);
        assert!(!bad.valid());
        let mut quoted = engine();
        quoted.url = "https://x/'; rm -rf ~".into();
        assert!(!quoted.valid());
        let mut plain = engine();
        plain.url = "http://x/engine".into();
        assert!(!plain.valid());
    }

    #[test]
    fn only_claude_code_credentials_reach_the_engine() {
        assert!(engine_credential_name("ANTHROPIC_API_KEY"));
        assert!(engine_credential_name("CLAUDE_CODE_OAUTH_TOKEN"));
        assert!(!engine_credential_name("OA_CLAUDE_VERTEX"));
        assert!(!engine_credential_name("CODER_DESKTOP_CREDENTIAL"));
    }

    #[test]
    fn a_running_command_is_reused() {
        for state in ["queued", "starting", "running"] {
            assert!(running(state));
        }
        for state in ["succeeded", "failed", "cancelled", "timed_out", "unknown"] {
            assert!(!running(state));
        }
    }

    #[test]
    fn record_never_serializes_a_token() {
        let record = Record {
            computer: Some("cmp_0123456789abcdef0123".into()),
            key_id: Some("ab".repeat(32)),
            command: Some("trn_0123456789abcdef0123".into()),
            started_unix: Some(1),
            generation: 1,
        };
        let text = serde_json::to_string(&record).unwrap();
        assert!(!text.contains("oa_sk_"));
        let view = view(&record, None, Some("running"));
        assert_eq!(view["engine"], "running");
        assert_eq!(view["until_unix"], json!(1 + RUN_SECONDS));
    }
}
