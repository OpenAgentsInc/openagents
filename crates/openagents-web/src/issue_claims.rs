//! Account-run issue claims. GitHub comments remain readable by the CLI.

mod github;

use crate::{
    App,
    chat_store::Store,
    coder_sync::{answer, refused},
};
use axum::{
    Router,
    body::Bytes,
    extract::{DefaultBodyLimit, Path, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, put},
};
use openagents_sdk::{
    issues::{BoardStatus, Claim, ClaimRequest},
    runs::RunHeartbeat,
};
use serde::{Deserialize, Serialize};
use serde_json::json;
use std::collections::{BTreeMap, BTreeSet};

const KEY: &str = "issue-claims/claims.json";
const MAX_CLAIMS: usize = 2048;
const PENDING_MS: i64 = 5 * 60_000;
const RETAIN_MS: i64 = 24 * 60 * 60_000;

#[derive(Clone, Debug)]
struct Failure {
    status: StatusCode,
    code: &'static str,
    message: String,
}
impl Failure {
    fn new(status: StatusCode, code: &'static str, message: impl Into<String>) -> Self {
        Self {
            status,
            code,
            message: message.into(),
        }
    }
    fn invalid() -> Self {
        Self::new(
            StatusCode::BAD_REQUEST,
            "invalid_claim",
            "Check the repository, issue number, holder, and batch.",
        )
    }
    fn unavailable() -> Self {
        Self::new(
            StatusCode::SERVICE_UNAVAILABLE,
            "claims_unavailable",
            "The claims could not be read or saved. Try again.",
        )
    }
    fn conflict(message: impl Into<String>) -> Self {
        Self::new(StatusCode::CONFLICT, "claim_conflict", message)
    }
    fn response(self) -> Response {
        refused(self.status, self.code, &self.message)
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
struct Held {
    claim: Claim,
    /// Native session identity, so aliases and replacement run IDs share a limit.
    session: String,
    batch: Vec<u64>,
    pending: bool,
    intent: String,
}

#[derive(Default, Serialize, Deserialize)]
struct Book {
    entries: BTreeMap<String, Held>,
}

fn key(repo: &str, issue: u64) -> String {
    format!("{}#{issue}", repo.to_ascii_lowercase())
}
fn session(beat: &RunHeartbeat) -> String {
    beat.harness_session.as_ref().map_or_else(
        || format!("run:{}", beat.run),
        |s| format!("{}:{s}", beat.harness),
    )
}
fn matches(holder: &str, beat: &RunHeartbeat) -> bool {
    holder == beat.run
        || holder == format!("run:{}", beat.run)
        || holder == format!("chat:{}", beat.run)
        || beat
            .harness_session
            .as_deref()
            .is_some_and(|s| holder == s || holder == format!("{}:{s}", beat.harness))
}
fn progress(claim: &mut Claim, beat: &RunHeartbeat) {
    claim.last_progress_at = Some(
        claim
            .claimed_at
            .max(claim.last_progress_at.unwrap_or(0))
            .max(beat.last_output_at)
            .max(beat.last_progress_at.unwrap_or(0)),
    );
}
fn lease(held: &Held) -> coder_lease::claims::IssueClaim {
    coder_lease::claims::IssueClaim {
        schema: coder_lease::claims::CLAIM_SCHEMA.into(),
        repository: held.claim.repo.clone(),
        issue: held.claim.issue,
        session: held.session.clone(),
        agent: "api".into(),
        pid: None,
        pid_started: None,
        claimed_at_ms: held.claim.claimed_at.max(0) as u64,
        batch: held.batch.clone(),
    }
}

impl Book {
    fn reserve(
        &mut self,
        repo: &str,
        issue: u64,
        request: &ClaimRequest,
        run: &RunHeartbeat,
        live: &BTreeSet<String>,
        legacy: &[Held],
        prior: Option<&Claim>,
        at: i64,
        intent: &str,
    ) -> Result<Held, Failure> {
        self.entries.retain(|_, h| {
            let live_run = h.claim.run.as_ref().is_some_and(|r| live.contains(r));
            (!h.pending || at.saturating_sub(h.claim.claimed_at) < PENDING_MS)
                && (live_run || at.saturating_sub(h.claim.claimed_at) <= RETAIN_MS)
        });
        let identity = session(run);
        let active = |h: &&Held| {
            h.session == identity && h.claim.run.as_ref().is_some_and(|r| live.contains(r))
        };
        let mut held: BTreeMap<_, _> = legacy
            .iter()
            .filter(active)
            .map(|h| (key(&h.claim.repo, h.claim.issue), lease(h)))
            .collect();
        for h in self.entries.values().filter(active) {
            held.insert(key(&h.claim.repo, h.claim.issue), lease(h));
        }
        let batch = coder_lease::claims::admit_batch(
            repo,
            issue,
            &request.batch,
            &held.into_values().collect::<Vec<_>>(),
        )
        .map_err(|e| Failure::conflict(e.to_string().replace("--batch", "batch")))?;
        let k = key(repo, issue);
        if let Some(old) = self.entries.get(&k) {
            if old.pending && old.intent != intent {
                return Err(Failure::conflict("This claim is being saved. Try again."));
            }
            if old.session != identity
                && old.claim.run.as_ref().is_some_and(|r| live.contains(r))
                && !request.force
            {
                return Err(Failure::conflict(format!(
                    "#{issue} is held by {}. Finish or release that work before taking it.",
                    old.claim.holder
                )));
            }
            if old.session == identity
                && old.claim.run.as_deref() == Some(run.run.as_str())
                && old.batch == batch
                && prior.is_some()
                && !old.pending
            {
                return Ok(old.clone());
            }
        } else if self.entries.len() >= MAX_CLAIMS {
            return Err(Failure::new(
                StatusCode::TOO_MANY_REQUESTS,
                "claims_full",
                "Release completed claims before taking more work.",
            ));
        }
        let mut claim = prior.cloned().unwrap_or_else(|| Claim {
            repo: repo.into(),
            issue,
            holder: request.holder.clone(),
            run: Some(run.run.clone()),
            claimed_at: at,
            last_progress_at: Some(at),
        });
        claim.run = Some(run.run.clone());
        progress(&mut claim, run);
        let held = Held {
            claim,
            session: identity,
            batch,
            pending: prior.is_none(),
            intent: intent.into(),
        };
        self.entries.insert(k, held.clone());
        Ok(held)
    }
}

async fn load(store: &Store, owner: &str) -> Result<(Book, Option<String>), Failure> {
    match store
        .read_key(&Store::owner_key(owner, KEY).map_err(|_| Failure::unavailable())?)
        .await
        .map_err(|_| Failure::unavailable())?
    {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes).map_err(|_| Failure::unavailable())?,
            Some(generation),
        )),
        None => Ok((Book::default(), None)),
    }
}
async fn change<T>(
    store: &Store,
    owner: &str,
    f: impl Fn(&mut Book) -> Result<T, Failure>,
) -> Result<T, Failure> {
    for _ in 0..8 {
        let (mut book, generation) = load(store, owner).await?;
        let result = f(&mut book)?;
        let bytes = serde_json::to_vec(&book).map_err(|_| Failure::unavailable())?;
        let path = Store::owner_key(owner, KEY).map_err(|_| Failure::unavailable())?;
        match store.write_key(&path, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(result),
            Err(crate::chat_store::Error::Conflict) => continue,
            Err(_) => return Err(Failure::unavailable()),
        }
    }
    Err(Failure::conflict("The claims changed. Try again."))
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis()
        .min(i64::MAX as u128) as i64
}
fn repo(owner: &str, name: &str, issue: u64) -> Result<String, Failure> {
    let repo = format!("{owner}/{name}").to_ascii_lowercase();
    if issue == 0 || !oa_auth::repos::full_name(&repo) {
        return Err(Failure::invalid());
    }
    Ok(repo)
}
pub(crate) fn owns(path: &str) -> bool {
    path.starts_with("/v1/repos/")
        && ["/claims", "/claim", "/board"]
            .iter()
            .any(|end| path.ends_with(end))
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/repos/{owner}/{name}/claims", get(list))
        .route(
            "/v1/repos/{owner}/{name}/issues/{n}/claim",
            put(take).delete(release),
        )
        .route("/v1/repos/{owner}/{name}/issues/{n}/board", put(board))
        .layer(DefaultBodyLimit::max(8192))
}
async fn context(app: &App, headers: &HeaderMap) -> Result<(String, github::Github), Response> {
    let owner = crate::coder_sync::owner(app, headers).await?;
    let Some(service) = app.config.cloud.as_deref() else {
        return Err(Failure::unavailable().response());
    };
    let bearer = headers
        .get(axum::http::header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    let token = service.github_token_as(bearer).await.map_err(|_| {
        Failure::new(
            StatusCode::FORBIDDEN,
            "github_connection_required",
            "Connect GitHub for this account before changing issue claims.",
        )
        .response()
    })?;
    Ok((owner, github::Github::new(app, token)))
}
async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    Path((owner, name)): Path<(String, String)>,
) -> Response {
    let (account, gh) = match context(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let result = async {
        let repo = repo(&owner, &name, 1)?;
        let runs = crate::runs::account_runs(&app.config.chat_store, &account)
            .await
            .map_err(|_| Failure::unavailable())?;
        let claims: Vec<_> = gh
            .claims(&repo)
            .await?
            .into_iter()
            .map(|o| join(o.claim, &runs))
            .collect();
        Ok(json!({"claims":claims}))
    }
    .await;
    reply(result)
}

type Body = Result<Bytes, axum::extract::rejection::BytesRejection>;
type IssuePath = Result<Path<(String, String, u64)>, axum::extract::rejection::PathRejection>;
fn issue_path(path: IssuePath) -> Result<(String, String, u64), Failure> {
    path.map(|Path(values)| values)
        .map_err(|_| Failure::invalid())
}
fn parse<T: serde::de::DeserializeOwned>(body: Body) -> Result<T, Failure> {
    let body =
        body.map_err(|e| Failure::new(e.status(), "too_large", "Send a smaller issue request."))?;
    serde_json::from_slice(if body.is_empty() { b"{}" } else { &body })
        .map_err(|_| Failure::invalid())
}
fn reply(result: Result<serde_json::Value, Failure>) -> Response {
    match result {
        Ok(v) => answer(StatusCode::OK, v),
        Err(e) => e.response(),
    }
}
fn join(mut claim: Claim, runs: &[crate::runs::RunPresence]) -> Claim {
    let found = claim
        .run
        .as_ref()
        .and_then(|id| {
            runs.iter()
                .find(|r| &r.beat.run == id && matches(&claim.holder, &r.beat))
        })
        .or_else(|| {
            runs.iter()
                .filter(|r| matches(&claim.holder, &r.beat))
                .max_by_key(|r| r.received_at)
        });
    claim.run = found.map(|r| r.beat.run.clone());
    if let Some(run) = found {
        progress(&mut claim, &run.beat);
    }
    claim
}
fn choose<'a>(
    request: &ClaimRequest,
    runs: &'a [crate::runs::RunPresence],
    at: i64,
) -> Result<&'a crate::runs::RunPresence, Failure> {
    let mut found = runs.iter().filter(|r| {
        r.is_live(at)
            && matches(&request.holder, &r.beat)
            && request.run.as_deref().is_none_or(|id| id == r.beat.run)
    });
    let run = found.next().ok_or_else(|| {
        Failure::conflict("Send a heartbeat for this holder's run before claiming an issue.")
    })?;
    if found.next().is_some() {
        return Err(Failure::conflict(
            "More than one live run matches this holder. Name the run in the claim request.",
        ));
    }
    Ok(run)
}
async fn take(State(app): State<App>, headers: HeaderMap, path: IssuePath, body: Body) -> Response {
    let (account, gh) = match context(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let result = async {
        let (owner, name, n) = issue_path(path)?;
        let repo = repo(&owner, &name, n)?;
        let mut request: ClaimRequest = parse(body)?;
        if !request.valid_for(n) {
            return Err(Failure::invalid());
        }
        let at = now();
        let runs = crate::runs::account_runs(&app.config.chat_store, &account)
            .await
            .map_err(|_| Failure::unavailable())?;
        let run = choose(&request, &runs, at)?;
        request.run = Some(run.beat.run.clone());
        let live: BTreeSet<_> = runs
            .iter()
            .filter(|r| r.is_live(at))
            .map(|r| r.beat.run.clone())
            .collect();
        let current = gh.issue(&repo, n).await?;
        if !current.open {
            return Err(Failure::conflict("This issue is closed."));
        }
        let observed = gh.claims(&repo).await?;
        let (before, _) = load(&app.config.chat_store, &account).await?;
        let latest = github::observation(&repo, n, &current.comments);
        let latest_joined = latest.as_ref().map(|o| join(o.claim.clone(), &runs));
        let ours = latest_joined
            .as_ref()
            .and_then(|c| c.run.as_ref())
            .and_then(|id| runs.iter().find(|r| &r.beat.run == id))
            .is_some_and(|r| session(&r.beat) == session(&run.beat));
        let ended = before
            .entries
            .get(&key(&repo, n))
            .filter(|h| h.claim.run.as_ref().is_none_or(|id| !live.contains(id)))
            .map(|h| h.claim.holder.as_str());
        if !ours && !request.force {
            if let Some(why) = coder::claim::foreign_marker(
                n,
                &current.comments,
                (at / 1000).max(0) as u64,
                6,
                &request.holder,
                ended,
            ) {
                return Err(Failure::conflict(why));
            }
        }
        let same_batch = latest.as_ref().is_some_and(|o| {
            request.batch.is_empty()
                || request.batch.iter().copied().collect::<BTreeSet<_>>()
                    == o.batch.iter().copied().collect()
        });
        let prior = latest
            .as_ref()
            .filter(|o| ours && same_batch && o.claim.run.as_deref() == Some(run.beat.run.as_str()))
            .map(|o| &o.claim);
        // Confirm old claims were closed, released, or taken over before freeing their slots.
        // An unavailable read keeps the existing slot.
        let mut gone = Vec::new();
        for (k, h) in &before.entries {
            if h.session != session(&run.beat) || h.pending {
                continue;
            }
            let still_ours = if h.claim.repo.eq_ignore_ascii_case(&repo) {
                Some(observed.iter().any(|o| {
                    o.claim.issue == h.claim.issue
                        && coder::claim::session_word(&o.claim.holder)
                            == coder::claim::session_word(&h.claim.holder)
                }))
            } else {
                gh.issue(&h.claim.repo, h.claim.issue)
                    .await
                    .ok()
                    .map(|issue| {
                        issue.open
                            && github::observation(&h.claim.repo, h.claim.issue, &issue.comments)
                                .is_some_and(|o| {
                                    coder::claim::session_word(&o.claim.holder)
                                        == coder::claim::session_word(&h.claim.holder)
                                })
                    })
            };
            if still_ours == Some(false) {
                gone.push((k.clone(), h.clone()));
            }
        }
        // GitHub reads may take longer than a presence window. Recheck the same
        // run and the account's current capacity before reserving any work.
        let at = now();
        let runs = crate::runs::account_runs(&app.config.chat_store, &account)
            .await
            .map_err(|_| Failure::unavailable())?;
        let run = choose(&request, &runs, at)?;
        let live: BTreeSet<_> = runs
            .iter()
            .filter(|r| r.is_live(at))
            .map(|r| r.beat.run.clone())
            .collect();
        let mut legacy = Vec::new();
        for o in &observed {
            let claim = join(o.claim.clone(), &runs);
            if at.saturating_sub(claim.claimed_at) >= 6 * 60 * 60_000 {
                continue;
            }
            if let Some(row) = claim
                .run
                .as_ref()
                .and_then(|id| runs.iter().find(|r| &r.beat.run == id))
            {
                legacy.push(Held {
                    claim,
                    session: session(&row.beat),
                    batch: o.batch.clone(),
                    pending: false,
                    intent: String::new(),
                });
            }
        }
        let intent = openagents_sdk::agents::request_id().map_err(|_| Failure::unavailable())?;
        let held = change(&app.config.chat_store, &account, |book| {
            for (k, old) in &gone {
                if book.entries.get(k) == Some(old) {
                    book.entries.remove(k);
                }
            }
            book.reserve(
                &repo, n, &request, &run.beat, &live, &legacy, prior, at, &intent,
            )
        })
        .await?;
        if held.pending {
            let numbers = held
                .batch
                .iter()
                .map(u64::to_string)
                .collect::<Vec<_>>()
                .join(",");
            let fields = format!(
                "sdk run={}{}",
                run.beat.run,
                if numbers.is_empty() {
                    String::new()
                } else {
                    format!(" batch={numbers}")
                }
            );
            let body = format!(
                "Claimed: an agent is working on this.\n\n{}",
                coder::claim::marker(&fields, &held.claim.holder)
            );
            if let Err(error) = gh.comment(&repo, n, &body).await {
                let _ = change(&app.config.chat_store, &account, |book| {
                    let k = key(&repo, n);
                    if book
                        .entries
                        .get(&k)
                        .is_some_and(|h| h.intent == held.intent && h.pending)
                    {
                        if let Some(old) = before.entries.get(&k) {
                            book.entries.insert(k, old.clone());
                        } else {
                            book.entries.remove(&k);
                        }
                    }
                    Ok(())
                })
                .await;
                return Err(error);
            }
            change(&app.config.chat_store, &account, |book| {
                if let Some(h) = book.entries.get_mut(&key(&repo, n)) {
                    if h.intent == held.intent {
                        h.pending = false;
                    }
                }
                Ok(())
            })
            .await?;
        }
        gh.assign(&repo, n, &current, true).await?;
        gh.board(&repo, n, BoardStatus::InProgress).await?;
        Ok(json!(held.claim))
    }
    .await;
    reply(result)
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Release {
    #[serde(default)]
    note: Option<String>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Board {
    status: BoardStatus,
}
async fn release(
    State(app): State<App>,
    headers: HeaderMap,
    path: IssuePath,
    body: Body,
) -> Response {
    let (account, gh) = match context(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let result = async {
        let (owner, name, n) = issue_path(path)?;
        let repo = repo(&owner, &name, n)?;
        let request: Release = parse(body)?;
        if request.note.as_ref().is_some_and(|n| n.len() > 2000) {
            return Err(Failure::invalid());
        }
        let current = gh.issue(&repo, n).await?;
        if coder::claim::active(&current.comments).is_some() {
            let note = request
                .note
                .as_deref()
                .unwrap_or_default()
                .replace("<!--", "&lt;!--");
            gh.comment(
                &repo,
                n,
                &format!(
                    "Released the claim. {note}\n\n{}",
                    coder::claim::RELEASE_MARK
                ),
            )
            .await?;
        }
        change(&app.config.chat_store, &account, |book| {
            book.entries.remove(&key(&repo, n));
            Ok(())
        })
        .await?;
        gh.assign(&repo, n, &current, false).await?;
        gh.board(&repo, n, BoardStatus::Ready).await?;
        Ok(json!({"ok":true}))
    }
    .await;
    reply(result)
}
async fn board(
    State(app): State<App>,
    headers: HeaderMap,
    path: IssuePath,
    body: Body,
) -> Response {
    let (_, gh) = match context(&app, &headers).await {
        Ok(c) => c,
        Err(r) => return r,
    };
    let result = async {
        let (owner, name, n) = issue_path(path)?;
        let repo = repo(&owner, &name, n)?;
        let request: Board = parse(body)?;
        gh.board(&repo, n, request.status).await?;
        Ok(json!({"ok":true}))
    }
    .await;
    reply(result)
}

#[cfg(test)]
mod tests;
