//! Own coding capacity on the inference API (#11080;
//! `docs/inference/gateway.md`, section 4, "Own coding capacity").
//!
//! A request for `openagents/code` (or `openagents/auto`, which the typed
//! judgment may class as code) with `openagents.pay: "mine"` is also
//! offered the key owner's own linked computers: each Codex or Claude
//! Code account that Coder on one of them reports with a free session is
//! one upstream ([`inference::upstream::coder::OwnCoder`]), added to the
//! caller's own adapters ([`inference::run::Caller::own`]) beside the
//! provider keys of [`crate::inference_byok::own`].
//!
//! The computers and their runs live with the web server, which Coder
//! talks to (`openagents-web` `own_runs`). This module reaches it on
//! loopback (`inference.own_coders.web`) with a token both share
//! (`inference.own_coders.token_file`, made here on first use, readable
//! only by this user): [`Web`] reads the owner's computers with free
//! sessions, starts a run on one ([`Runs`]), follows its progress, and
//! cancels it when the caller leaves.
//!
//! Own capacity only: the computers are those of the account that owns
//! the personal workspace the caller's key acts in. A key acting in an
//! organization workspace is offered none, so one member's request never
//! reaches another member's computer.

use std::collections::VecDeque;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use inference::openagents::Payer;
use inference::request::CreateResponse;
use inference::response::Usage;
use inference::run::Caller;
use inference::upstream::coder::{Agent, Linked, Report, Reports, Run, Runs, own_upstreams};
use inference::upstream::{AttemptError, BoxFuture, ErrorClass};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::serve::ServeState;

/// Where the web server is and the token both share.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Config {
    /// The web server on loopback, such as `http://127.0.0.1:8080`.
    pub web: String,
    /// The shared token's file. Made here on first use (32 random bytes,
    /// hex, mode 0600) when missing; the web server reads the same file
    /// (`--own-runs-token`).
    pub token_file: PathBuf,
}

/// How long a run may wait for Coder on its computer to take it before
/// the router moves on (Coder asks every 5 seconds).
const TAKE_WITHIN: Duration = Duration::from_secs(25);
/// How long one read of a run's progress waits for news.
const READ_WAIT: u64 = 20;
/// How many reads in a row may fail before the run counts as lost.
const READ_TRIES: u32 = 5;

fn client() -> &'static reqwest::Client {
    static CLIENT: OnceLock<reqwest::Client> = OnceLock::new();
    CLIENT.get_or_init(|| {
        reqwest::Client::builder()
            .no_proxy()
            .redirect(reqwest::redirect::Policy::none())
            .connect_timeout(Duration::from_secs(2))
            .timeout(Duration::from_secs(READ_WAIT + 10))
            .build()
            .unwrap_or_default()
    })
}

/// The shared token, made when missing.
fn token(path: &Path) -> Option<String> {
    if let Ok(text) = std::fs::read_to_string(path) {
        let text = text.trim();
        return (text.len() >= 32).then(|| text.to_owned());
    }
    let made = hex::encode(secp256k1::rand::random::<[u8; 32]>());
    let mut options = std::fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    match options.open(path) {
        Ok(mut file) => {
            use std::io::Write;
            file.write_all(made.as_bytes()).ok()?;
            Some(made)
        }
        // Another request made it first.
        Err(_) => std::fs::read_to_string(path)
            .ok()
            .map(|text| text.trim().to_owned())
            .filter(|text| text.len() >= 32),
    }
}

/// The account that owns `workspace`, when it is a personal workspace.
fn owner_account(state: &ServeState, workspace: &str) -> Option<String> {
    let accounts = crate::accounts::accounts_store(state).ok()?;
    let store = accounts.store().ok()?;
    let found = store.workspaces.get(workspace)?;
    if found.kind != tenancy::WorkspaceKind::Personal {
        return None;
    }
    found
        .members
        .values()
        .find(|member| {
            member.role == tenancy::Role::Owner && member.status == tenancy::MemberStatus::Active
        })
        .map(|member| member.account.clone())
}

/// Whether `request` may run on the caller's own coding capacity: it pays
/// with the caller's own (`pay: "mine"`) and names the code class or the
/// routed id.
fn wants_own_capacity(request: &CreateResponse) -> bool {
    let mine = request
        .openagents
        .as_ref()
        .and_then(|options| options.pay.as_ref())
        .is_some_and(|payer| *payer == Payer::Mine);
    let model = request.model.as_deref().unwrap_or_default();
    mine && matches!(model, "openagents/code" | "openagents/auto")
}

/// Adds the key owner's linked computers with a free session to the
/// caller's own adapters, when the request may use them. Anything that
/// fails here leaves the caller as it was.
pub async fn attach(state: &Arc<ServeState>, caller: &mut Caller, request: &CreateResponse) {
    let Some(config) = state
        .config
        .inference
        .as_ref()
        .and_then(|inference| inference.own_coders.as_ref())
    else {
        return;
    };
    if !wants_own_capacity(request) {
        return;
    }
    let Some(workspace) = caller.owner.clone() else {
        return;
    };
    let Some(account) = owner_account(state, &workspace) else {
        return;
    };
    let Some(token) = token(&config.token_file) else {
        return;
    };
    let web = Web {
        base: config.web.trim_end_matches('/').to_owned(),
        token,
        account: account.clone(),
    };
    let Ok(linked) = web.capacity().await else {
        return;
    };
    let runs: Arc<dyn Runs> = Arc::new(web);
    caller.own.0.extend(own_upstreams(&account, linked, &runs));
}

/// The web server's own-run routes, for one account.
#[derive(Clone)]
pub struct Web {
    base: String,
    token: String,
    account: String,
}

#[derive(Deserialize)]
struct Offered {
    computer: String,
    #[serde(default)]
    computer_label: String,
    account: String,
    #[serde(default)]
    account_label: String,
    agent: Agent,
    free_sessions: u32,
}

/// One read of a run.
#[derive(Debug, Default, Deserialize)]
struct View {
    state: String,
    #[serde(default)]
    lines: Vec<String>,
    #[serde(default)]
    next: usize,
    #[serde(default)]
    answer: Option<String>,
    #[serde(default)]
    usage: Option<ViewUsage>,
    #[serde(default)]
    why: Option<String>,
    #[serde(default)]
    limited: bool,
}

#[derive(Debug, Default, Deserialize)]
struct ViewUsage {
    #[serde(default)]
    input_tokens: u64,
    #[serde(default)]
    output_tokens: u64,
}

fn lost(why: impl Into<String>) -> AttemptError {
    AttemptError::new(ErrorClass::Connection, why)
}

impl Web {
    fn url(&self, path: &str) -> String {
        format!("{}{path}", self.base)
    }

    /// The owner's accounts with a free session, as the router's records.
    async fn capacity(&self) -> Result<Vec<Linked>, AttemptError> {
        let response = client()
            .get(self.url("/v1/own-runs/capacity"))
            .bearer_auth(&self.token)
            .query(&[("account", self.account.as_str())])
            .timeout(Duration::from_secs(3))
            .send()
            .await
            .map_err(|_| lost("The web server can't be reached."))?;
        if !response.status().is_success() {
            return Err(lost("The web server refused the capacity read."));
        }
        let body: Value = response
            .json()
            .await
            .map_err(|_| lost("The capacity read was unreadable."))?;
        Ok(body["accounts"]
            .as_array()
            .into_iter()
            .flatten()
            .filter_map(|row| serde_json::from_value::<Offered>(row.clone()).ok())
            .map(|row| Linked {
                owner: self.account.clone(),
                computer: row.computer,
                computer_label: row.computer_label,
                account: row.account,
                account_label: row.account_label,
                agent: row.agent,
                free_sessions: row.free_sessions,
            })
            .collect())
    }

    async fn create(&self, run: &Run) -> Result<String, AttemptError> {
        let brief = json!({
            "instructions": run.brief.instructions,
            "history": run.brief.history.iter()
                .map(|turn| json!({"role": turn.role, "content": turn.content}))
                .collect::<Vec<_>>(),
            "task": run.brief.task,
        });
        let response = client()
            .post(self.url("/v1/own-runs"))
            .bearer_auth(&self.token)
            .json(&json!({
                "account": self.account,
                "computer": run.computer,
                "run_account": run.account,
                "agent": run.agent,
                "brief": brief,
            }))
            .send()
            .await
            .map_err(|_| lost("The web server can't be reached."))?;
        let status = response.status().as_u16();
        let body: Value = response.json().await.unwrap_or(Value::Null);
        match (status, body["error"]["code"].as_str()) {
            (201, _) => body["id"]
                .as_str()
                .map(str::to_owned)
                .ok_or_else(|| lost("The run's id was missing.")),
            (409, Some("busy")) => Err(AttemptError::new(
                ErrorClass::RateLimited,
                "Too many runs already wait for your computers.",
            )),
            (409, _) => Err(lost("Coder on that computer isn't reporting now.")),
            _ => Err(lost("The web server refused the run.")),
        }
    }

    /// The run's progress after line `after`, waiting up to `wait` seconds
    /// for news; with `taken`, Coder taking the run is news too.
    async fn read(
        &self,
        id: &str,
        after: usize,
        wait: u64,
        taken: bool,
    ) -> Result<View, AttemptError> {
        let response = client()
            .get(self.url(&format!("/v1/own-runs/{id}")))
            .bearer_auth(&self.token)
            .query(&[
                ("account", self.account.clone()),
                ("after", after.to_string()),
                ("wait", wait.to_string()),
                ("taken", u8::from(taken).to_string()),
            ])
            .send()
            .await
            .map_err(|_| lost("The web server can't be reached."))?;
        if !response.status().is_success() {
            return Err(lost("The run can't be read."));
        }
        response
            .json()
            .await
            .map_err(|_| lost("The run's progress was unreadable."))
    }

    async fn cancel(&self, id: &str) {
        let _ = client()
            .post(self.url(&format!("/v1/own-runs/{id}/cancel")))
            .bearer_auth(&self.token)
            .query(&[("account", self.account.as_str())])
            .timeout(Duration::from_secs(5))
            .send()
            .await;
    }
}

/// Cancels the run when the caller's stream goes before it finished.
struct Cancel {
    web: Web,
    id: String,
    armed: bool,
}

impl Drop for Cancel {
    fn drop(&mut self) {
        if !self.armed {
            return;
        }
        if let Ok(handle) = tokio::runtime::Handle::try_current() {
            let web = self.web.clone();
            let id = std::mem::take(&mut self.id);
            handle.spawn(async move { web.cancel(&id).await });
        }
    }
}

/// The reports one read brings, and whether the run ended.
fn reports_of(view: &View) -> (Vec<Report>, bool) {
    let mut reports: Vec<Report> = view.lines.iter().cloned().map(Report::Step).collect();
    let ended = match view.state.as_str() {
        "done" => {
            reports.push(Report::Done {
                text: view.answer.clone().unwrap_or_default(),
                usage: view
                    .usage
                    .as_ref()
                    .map(|usage| Usage::new(usage.input_tokens, 0, usage.output_tokens, 0)),
            });
            true
        }
        "failed" => {
            reports.push(Report::Failed(
                view.why
                    .clone()
                    .unwrap_or_else(|| "The run stopped.".into()),
            ));
            true
        }
        "cancelled" => {
            reports.push(Report::Failed("The run was cancelled.".into()));
            true
        }
        _ => false,
    };
    (reports, ended)
}

struct Follow {
    guard: Cancel,
    after: usize,
    queued: VecDeque<Report>,
    ended: bool,
}

impl Runs for Web {
    fn start(&self, run: Run) -> BoxFuture<'_, Result<Reports, AttemptError>> {
        Box::pin(async move {
            let id = self.create(&run).await?;
            let mut guard = Cancel {
                web: self.clone(),
                id: id.clone(),
                armed: true,
            };
            // Wait for Coder to take it: until then nothing has been said,
            // so a computer that is away lets the router try the next one.
            let until = Instant::now() + TAKE_WITHIN;
            let first = loop {
                let left = until.saturating_duration_since(Instant::now()).as_secs();
                let view = self.read(&id, 0, left.clamp(1, 10), true).await?;
                if view.state != "waiting" {
                    break view;
                }
                if Instant::now() >= until {
                    return Err(lost(format!(
                        "Coder on {} didn't take the run in time.",
                        run.computer
                    )));
                }
            };
            // A run that failed before saying anything: say why, so the
            // router benches a spent account and tries the next.
            if first.state == "failed" && first.lines.is_empty() {
                guard.armed = false;
                let why = first
                    .why
                    .clone()
                    .unwrap_or_else(|| "The run stopped.".into());
                let class = if first.limited {
                    ErrorClass::Payment
                } else {
                    ErrorClass::Upstream
                };
                return Err(AttemptError::new(class, why));
            }
            let (reports, ended) = reports_of(&first);
            if ended {
                guard.armed = false;
            }
            let follow = Follow {
                guard,
                after: first.next,
                queued: reports.into(),
                ended,
            };
            let stream = futures_util::stream::unfold(follow, |mut follow| async move {
                loop {
                    if let Some(report) = follow.queued.pop_front() {
                        return Some((report, follow));
                    }
                    if follow.ended {
                        return None;
                    }
                    let mut tries = 0;
                    let view = loop {
                        match follow
                            .guard
                            .web
                            .read(&follow.guard.id, follow.after, READ_WAIT, false)
                            .await
                        {
                            Ok(view) => break Some(view),
                            Err(_) if tries + 1 < READ_TRIES => {
                                tries += 1;
                                tokio::time::sleep(Duration::from_secs(1)).await;
                            }
                            Err(_) => break None,
                        }
                    };
                    let Some(view) = view else {
                        follow.ended = true;
                        follow.queued.push_back(Report::Failed(
                            "Lost touch with the computer running this.".into(),
                        ));
                        continue;
                    };
                    follow.after = follow.after.max(view.next);
                    let (reports, ended) = reports_of(&view);
                    follow.queued.extend(reports);
                    if ended {
                        follow.ended = true;
                        follow.guard.armed = false;
                    }
                }
            });
            let reports: Reports = Box::pin(stream);
            Ok(reports)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_pay_mine_code_requests_use_own_capacity() {
        let request = |model: &str, pay: &str| -> CreateResponse {
            serde_json::from_value(json!({
                "model": model, "input": "x", "openagents": {"pay": pay}
            }))
            .unwrap()
        };
        assert!(wants_own_capacity(&request("openagents/code", "mine")));
        assert!(wants_own_capacity(&request("openagents/auto", "mine")));
        assert!(!wants_own_capacity(&request("openagents/code", "ours")));
        assert!(!wants_own_capacity(&request("openagents/chat", "mine")));
    }

    #[test]
    fn a_read_becomes_progress_then_the_answer() {
        let view = View {
            state: "done".into(),
            lines: vec!["Using shell.".into()],
            next: 1,
            answer: Some("Opened the pull request.".into()),
            usage: Some(ViewUsage {
                input_tokens: 10,
                output_tokens: 4,
            }),
            why: None,
            limited: false,
        };
        let (reports, ended) = reports_of(&view);
        assert!(ended);
        assert_eq!(reports[0], Report::Step("Using shell.".into()));
        assert!(
            matches!(&reports[1], Report::Done { text, .. } if text == "Opened the pull request.")
        );
        let running = View {
            state: "running".into(),
            ..View::default()
        };
        assert_eq!(reports_of(&running), (Vec::new(), false));
    }

    #[test]
    fn the_token_is_made_once_and_kept() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("own-runs.key");
        let made = token(&path).unwrap();
        assert_eq!(made.len(), 64);
        assert_eq!(token(&path).unwrap(), made);
    }
}
