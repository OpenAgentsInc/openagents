//! Chats on their own computers, through `oa-computers` (#11624,
//! `docs/cloud/computers.md`).
//!
//! When this server is given `--computers-url URL` (or
//! `OPENAGENTS_WEB_COMPUTERS_URL`) and the service token (`--computers-token
//! FILE`, or `OPENAGENTS_WEB_COMPUTERS_TOKEN`, a Secret Manager secret on
//! Cloud Run),
//! a Claude Code message in a chat about a saved environment runs on the
//! chat's own cloud computer instead of a fresh one: [`ensure`] asks
//! oa-computers for the chat's computer (made from the environment's saved
//! image the first time, woken with its disk on later turns), and [`turn`]
//! queues the turn there with its credentials in memory for that one
//! command ([`crate::cloud::byo::release_turn`]). The chat follows the turn
//! through [`read_turn`] like any run, and `GET /v1/computers` lists the
//! account's cloud computers beside its own ([`list`]).
//!
//! Cloud computers stay behind the agent-work gate
//! ([`crate::agent_work::permitted`]): admins and named agent accounts.
//! Billing and opening them to everyone come later.
//!
//! The service token is read from its file on each call and never logged;
//! a turn's secret values are written into exactly one request body and
//! dropped with it.

use serde_json::{Value, json};
use std::path::PathBuf;
use std::sync::OnceLock;
use std::time::Duration;

/// The oa-computers service this server uses.
pub struct Client {
    base: String,
    token: Token,
    http: reqwest::Client,
}

/// Where the service token comes from.
pub enum Token {
    /// A private file, read on each call (it may be rotated in place).
    File(PathBuf),
    /// The value itself (from the process environment).
    Value(String),
}

static CLIENT: OnceLock<Client> = OnceLock::new();

/// Turn chats-on-computers on (`--computers-url`, `--computers-token`).
pub fn configure(base: &str, token: Token) {
    let http = reqwest::Client::builder()
        .timeout(Duration::from_secs(20))
        .build()
        .unwrap_or_default();
    let _ = CLIENT.set(Client {
        base: base.trim_end_matches('/').to_owned(),
        token,
        http,
    });
}

pub(crate) fn client() -> Option<&'static Client> {
    CLIENT.get()
}

/// One computer as oa-computers reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Computer {
    pub id: String,
    pub chat: Option<String>,
    pub state: String,
    pub desired: String,
    pub size: String,
}

fn computer(v: &Value) -> Option<Computer> {
    Some(Computer {
        id: v["id"].as_str()?.to_owned(),
        chat: v["chat"].as_str().map(str::to_owned),
        state: v["state"].as_str().unwrap_or_default().to_owned(),
        desired: v["desired"].as_str().unwrap_or_default().to_owned(),
        size: v["size"].as_str().unwrap_or_default().to_owned(),
    })
}

/// One turn as oa-computers reports it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Turn {
    pub state: String,
    pub exit_code: Option<i64>,
    pub stdout: String,
    pub stderr: String,
    pub error: Option<String>,
}

impl Client {
    fn token(&self) -> Result<String, String> {
        match &self.token {
            Token::File(path) => std::fs::read_to_string(path)
                .map(|t| t.trim().to_owned())
                .ok(),
            Token::Value(value) => Some(value.trim().to_owned()),
        }
        .filter(|t| !t.is_empty())
        .ok_or_else(|| "Cloud computers aren't set up on this server.".to_owned())
    }

    async fn call(
        &self,
        method: reqwest::Method,
        path: &str,
        account: &str,
        body: Option<Value>,
    ) -> Result<(u16, Value), String> {
        let mut request = self
            .http
            .request(method, format!("{}{path}", self.base))
            .bearer_auth(self.token()?)
            .header("X-OpenAgents-Account", account);
        if let Some(body) = body {
            request = request.json(&body);
        }
        let response = request
            .send()
            .await
            .map_err(|_| "Cloud computers can't be reached right now. Try again.".to_owned())?;
        let status = response.status().as_u16();
        let value = response.json().await.unwrap_or(Value::Null);
        Ok((status, value))
    }
}

fn refusal(value: &Value) -> String {
    value["message"]
        .as_str()
        .unwrap_or("Cloud computers can't do that right now.")
        .to_owned()
}

/// The chat's own computer, made from `image` the first time and wanted
/// running on every later call. Answers at once; the computer comes up in
/// the background.
pub(crate) async fn ensure(
    client: &Client,
    account: &str,
    chat: &str,
    image: &str,
    environment: &str,
    version: &str,
    size: &str,
) -> Result<Computer, String> {
    let (status, v) = client
        .call(
            reqwest::Method::POST,
            "/v1/computers",
            account,
            Some(json!({
                "chat": chat,
                "image": image,
                "environment": environment,
                "version": version,
                "size": size,
            })),
        )
        .await?;
    match status {
        200 | 202 => computer(&v).ok_or_else(|| "Cloud computers answered oddly.".into()),
        _ => Err(refusal(&v)),
    }
}

/// Queue a turn on `computer`: `command` holds no secret; `env` (names to
/// values) reaches that one command's process and nothing else. Answers
/// the turn's id.
pub(crate) async fn turn(
    client: &Client,
    account: &str,
    computer: &str,
    command: &str,
    env: &[(String, String)],
) -> Result<String, String> {
    let env: serde_json::Map<String, Value> = env
        .iter()
        .map(|(k, v)| (k.clone(), Value::String(v.clone())))
        .collect();
    let mut body = json!({"command": command, "env": env});
    let answer = client
        .call(
            reqwest::Method::POST,
            &format!("/v1/computers/{computer}/turns"),
            account,
            Some(body.take()),
        )
        .await;
    let (status, v) = answer?;
    match (status, v["id"].as_str()) {
        (202, Some(id)) => Ok(id.to_owned()),
        _ => Err(refusal(&v)),
    }
}

pub(crate) async fn read_turn(
    client: &Client,
    account: &str,
    computer: &str,
    turn: &str,
) -> Result<Option<Turn>, String> {
    let (status, v) = client
        .call(
            reqwest::Method::GET,
            &format!("/v1/computers/{computer}/turns/{turn}"),
            account,
            None,
        )
        .await?;
    match status {
        200 => Ok(Some(Turn {
            state: v["state"].as_str().unwrap_or_default().to_owned(),
            exit_code: v["exit_code"].as_i64(),
            stdout: v["stdout"].as_str().unwrap_or_default().to_owned(),
            stderr: v["stderr"].as_str().unwrap_or_default().to_owned(),
            error: v["error"].as_str().map(str::to_owned),
        })),
        404 => Ok(None),
        _ => Err(refusal(&v)),
    }
}

/// Stop a turn: a waiting one ends at once, a running one's command is
/// abandoned.
pub(crate) async fn stop_turn(
    client: &Client,
    account: &str,
    computer: &str,
    turn: &str,
) -> Result<(), String> {
    let (status, v) = client
        .call(
            reqwest::Method::POST,
            &format!("/v1/computers/{computer}/turns/{turn}/stop"),
            account,
            None,
        )
        .await?;
    match status {
        200 | 202 => Ok(()),
        _ => Err(refusal(&v)),
    }
}

/// The account's cloud computers.
pub(crate) async fn list(client: &Client, account: &str) -> Result<Vec<Computer>, String> {
    let (status, v) = client
        .call(reqwest::Method::GET, "/v1/computers", account, None)
        .await?;
    if status != 200 {
        return Err(refusal(&v));
    }
    Ok(v["computers"]
        .as_array()
        .map(|all| all.iter().filter_map(computer).collect())
        .unwrap_or_default())
}

/// A chat task's id for a turn on a computer, and back.
pub(crate) fn task_id(computer: &str, turn: &str) -> String {
    format!("{computer}.{turn}")
}
pub(crate) fn split_task_id(id: &str) -> Option<(&str, &str)> {
    let (computer, turn) = id.split_once('.')?;
    (computer.starts_with("cmp_") && turn.starts_with("trn_")).then_some((computer, turn))
}

/// What Claude Code answered in a turn's output: the reply in the last
/// JSON object `coder --json delegate` printed, else the output's last
/// lines.
pub(crate) fn answer(turn: &Turn) -> Option<String> {
    let from_json = turn.stdout.lines().rev().find_map(|line| {
        let v: Value = serde_json::from_str(line.trim()).ok()?;
        v["reply"]
            .as_str()
            .or_else(|| v["result"]["reply"].as_str())
            .or_else(|| v["result"].as_str())
            .map(str::to_owned)
    });
    from_json.or_else(|| {
        let text = turn.stdout.trim();
        (!text.is_empty()).then(|| {
            let lines: Vec<&str> = text.lines().collect();
            lines[lines.len().saturating_sub(40)..].join("\n")
        })
    })
}

/// Why a turn failed, in plain words.
pub(crate) fn why(turn: &Turn) -> String {
    if let Some(error) = &turn.error {
        return error.clone();
    }
    let tail: String = turn
        .stderr
        .trim()
        .lines()
        .last()
        .unwrap_or_default()
        .chars()
        .take(300)
        .collect();
    match (turn.exit_code, tail.is_empty()) {
        (Some(code), true) => format!("Claude Code stopped with exit code {code}."),
        (Some(code), false) => format!("Claude Code stopped with exit code {code}: {tail}"),
        (None, _) => "Claude Code didn't finish.".into(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_ids_round_trip_and_reject_others() {
        let id = task_id("cmp_ab12", "trn_cd34");
        assert_eq!(split_task_id(&id), Some(("cmp_ab12", "trn_cd34")));
        assert_eq!(split_task_id("claude-env-1-3"), None);
        assert_eq!(split_task_id("x.y"), None);
    }

    #[test]
    fn the_answer_is_the_last_reply_or_the_output_tail() {
        let turn = |stdout: &str| Turn {
            state: "done".into(),
            exit_code: Some(0),
            stdout: stdout.into(),
            stderr: String::new(),
            error: None,
        };
        assert_eq!(
            answer(&turn("{\"event\":1}\n{\"reply\":\"Fixed it.\"}\n")).as_deref(),
            Some("Fixed it.")
        );
        assert_eq!(
            answer(&turn("{\"result\":{\"reply\":\"Done.\"}}")).as_deref(),
            Some("Done.")
        );
        assert_eq!(
            answer(&turn("plain words\n")).as_deref(),
            Some("plain words")
        );
        assert_eq!(answer(&turn("")), None);
    }
}

/// Whether an app's request (its own `Authorization: Bearer sess_…`) may
/// see cloud computers: the agent-work gate ([`crate::agent_work::permitted`]).
pub(crate) async fn app_allowed(app: &crate::App, headers: &axum::http::HeaderMap) -> bool {
    let Some(service) = app.config.cloud.as_deref() else {
        return false;
    };
    let token = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .unwrap_or_default();
    match service.app_viewer(token).await {
        Ok(viewer) => crate::agent_work::permitted(app, &viewer),
        Err(_) => false,
    }
}

/// The account's cloud computers as `GET /v1/computers` rows, beside its
/// own ones: `kind: "cloud"`, the computer's id and chat, and its state.
pub(crate) fn rows(all: &[Computer]) -> Vec<Value> {
    all.iter()
        .map(|c| {
            let short: String = c.id.trim_start_matches("cmp_").chars().take(6).collect();
            json!({
                "name": format!("Cloud computer {short}"),
                "kind": "cloud",
                "id": c.id,
                "chat": c.chat,
                "state": c.state,
                "size": c.size,
                "online": matches!(c.state.as_str(), "awake" | "turn" | "booting" | "creating"),
                "seen_unix": Value::Null,
                "sync": Value::Null,
            })
        })
        .collect()
}
