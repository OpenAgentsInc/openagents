//! Turns: one command run on a computer for a chat, with its credentials
//! delivered in memory for that command only.
//!
//! `POST /v1/computers/{id}/turns` `{command, cwd?, timeout_seconds?,
//! env?}` records the turn (`queued`; the command and the env *names*
//! only), keeps `env`'s values in this process's memory, wants the
//! computer running, and answers `202` with the turn's id at once. The
//! reconciler, once the computer is awake, dispatches the turn through the
//! lifecycle core (a turn generation), takes the values out of memory, runs
//! the command with them as that one process's environment
//! (`computer_driver::Exec::secret_env`: on oa-boat, over the SSH channel's
//! standard input), records the exit code and output, finishes the turn
//! (its checkpoint stops the computer with its disk), and forgets the
//! values. They are never written to Postgres, a file, a log, or the
//! computer's disk. If this process restarts before a queued turn starts,
//! its values are gone and the turn fails asking to be sent again; nothing
//! runs without the credentials it was sent with.
//!
//! `GET /v1/computers/{id}/turns/{turn}` reads one turn.

use crate::api::{ACCOUNT_HEADER, Api};
use crate::store::{Desired, Entry, PgLease, Store};
use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use coder_working_computer::driver::{self, Held, Settled};
use coder_working_computer::on_driver::OnDriver;
use coder_working_computer::{Command, Computer, Phase, StopReason, Trigger};
use computer_driver::{Driver, Exec, Outcome, Secret, State as Machine, env_name_ok};
use serde::Deserialize;
use serde_json::{Value, json};
use std::collections::{BTreeMap, HashMap};
use std::sync::{Arc, Mutex};
use std::time::Duration;

/// Output kept per stream.
pub const MAX_OUTPUT: usize = 256 * 1024;
pub const MAX_COMMAND: usize = 64 * 1024;
pub const MAX_ENV: usize = 16;
pub const MAX_ENV_VALUE: usize = 64 * 1024;
/// A request key already names a different turn body.
pub const CONFLICT_ERROR: &str = "idempotency_conflict";
/// The longest turn.
pub const MAX_SECONDS: u64 = 3600;

/// Turn credentials waiting in memory for their one command, and the turns
/// asked to stop.
#[derive(Clone, Default)]
pub struct Secrets(
    Arc<Mutex<HashMap<String, BTreeMap<String, Secret>>>>,
    Arc<Mutex<std::collections::HashSet<String>>>,
    pub(crate) crate::command_credentials::Pending,
);
impl Secrets {
    /// Ask a running turn to stop.
    pub fn stop(&self, turn: &str) {
        self.1
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .insert(turn.to_owned());
    }
    fn stopping(&self, turn: &str) -> bool {
        self.1
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .contains(turn)
    }
    fn stopped(&self, turn: &str) {
        self.1
            .lock()
            .unwrap_or_else(|e| e.into_inner())
            .remove(turn);
    }
    fn put(&self, turn: &str, env: BTreeMap<String, Secret>) {
        if !env.is_empty() {
            self.lock().insert(turn.to_owned(), env);
        }
    }
    /// Take (and so forget) a turn's values.
    pub fn take(&self, turn: &str) -> BTreeMap<String, Secret> {
        self.lock().remove(turn).unwrap_or_default()
    }
    fn lock(&self) -> std::sync::MutexGuard<'_, HashMap<String, BTreeMap<String, Secret>>> {
        self.0.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// One turn row.
#[derive(Clone, Debug)]
pub struct Turn {
    pub id: String,
    pub computer: String,
    pub state: String,
    pub generation: Option<i64>,
    pub command: String,
    pub cwd: Option<String>,
    pub timeout_seconds: i32,
    pub env_names: Vec<String>,
    pub exit_code: Option<i64>,
    pub stdout: Option<String>,
    pub stderr: Option<String>,
    pub error: Option<String>,
    pub api_command: bool,
}

const TURN_COLUMNS: &str = "id, computer, state, generation, command, cwd, timeout_seconds, \
     env_names, exit_code, stdout, stderr, error, \
     EXISTS (SELECT 1 FROM computers.commands c WHERE c.turn = computers.turns.id)";

fn turn(row: &tokio_postgres::Row) -> Turn {
    Turn {
        id: row.get(0),
        computer: row.get(1),
        state: row.get(2),
        generation: row.get(3),
        command: row.get(4),
        cwd: row.get(5),
        timeout_seconds: row.get(6),
        env_names: row.get(7),
        exit_code: row.get(8),
        stdout: row.get(9),
        stderr: row.get(10),
        error: row.get(11),
        api_command: row.get(12),
    }
}

fn cap(s: &str) -> String {
    if s.len() <= MAX_OUTPUT {
        return s.to_owned();
    }
    let mut start = s.len() - MAX_OUTPUT;
    while !s.is_char_boundary(start) {
        start += 1;
    }
    s[start..].to_owned()
}

pub fn view(t: &Turn) -> Value {
    json!({
        "id": t.id,
        "computer": t.computer,
        "state": match t.state.as_str() {"command_queued"=>"queued","command_running"=>"running",other=>other},
        "generation": t.generation,
        "env_names": t.env_names,
        "exit_code": t.exit_code,
        "stdout": t.stdout,
        "stderr": t.stderr,
        "error": t.error,
    })
}

impl Store {
    pub async fn add_turn(
        &self,
        id: &str,
        computer: &str,
        owner: &str,
        command: &str,
        cwd: Option<&str>,
        timeout: i32,
        env_names: &[String],
    ) -> Result<(), crate::db::DbError> {
        self.add_turn_keyed(id, computer, owner, command, cwd, timeout, env_names, None)
            .await
            .map(|_| ())
    }

    /// Adds a turn, or with a request key returns the turn that key already
    /// names. `Ok((turn, false))` is a repeat; a different body under the
    /// same key is `CONFLICT_ERROR`.
    pub async fn add_turn_keyed(
        &self,
        id: &str,
        computer: &str,
        owner: &str,
        command: &str,
        cwd: Option<&str>,
        timeout: i32,
        env_names: &[String],
        key: Option<(&str, &str)>,
    ) -> Result<(String, bool), crate::db::DbError> {
        let mut c = self.db.independent().await?;
        let tx = c.transaction().await?;
        crate::usage::lock(&tx).await?;
        if tx.query_opt("SELECT id FROM computers.computers WHERE id=$1 AND owner=$2 AND desired<>'deleted' FOR UPDATE", &[&computer,&owner]).await?.is_none() {
            return Err(crate::db::DbError("No such computer.".into()));
        }
        let (request_key, request_digest) = match key {
            Some((k, d)) => (Some(k), Some(d)),
            None => (None, None),
        };
        if let Some(k) = request_key {
            if let Some(r) = tx.query_opt("SELECT id, request_digest FROM computers.turns WHERE owner=$1 AND computer=$2 AND request_key=$3", &[&owner,&computer,&k]).await? {
                if r.get::<_, Option<String>>(1).as_deref() != request_digest {
                    return Err(crate::db::DbError(CONFLICT_ERROR.into()));
                }
                return Ok((r.get(0), false));
            }
        }
        crate::usage::check_turn(&tx, owner, timeout).await?;
        tx.execute(
            "INSERT INTO computers.turns (id, computer, owner, state, command, cwd, timeout_seconds, env_names, request_key, request_digest)
             VALUES ($1, $2, $3, 'queued', $4, $5, $6, $7, $8, $9)",
            &[&id, &computer, &owner, &command, &cwd, &timeout, &env_names, &request_key, &request_digest],
        ).await?;
        tx.execute(
            "UPDATE computers.computers SET desired = 'running', intent_generation = intent_generation + 1, usage_intent_generation = intent_generation + 1, next_attempt_at = now(), attempts = 0
             WHERE id = $1 AND owner=$2 AND desired <> 'deleted'",
            &[&computer,&owner],
        ).await?;
        crate::usage::record_intent(&tx, owner, computer).await?;
        tx.commit().await?;
        Ok((id.to_owned(), true))
    }

    pub async fn turn(
        &self,
        owner: &str,
        computer: &str,
        id: &str,
    ) -> Result<Option<Turn>, crate::db::DbError> {
        let c = self.db.client().await?;
        Ok(c
            .query_opt(
                &format!("SELECT {TURN_COLUMNS} FROM computers.turns WHERE id = $1 AND computer = $2 AND owner = $3"),
                &[&id, &computer, &owner],
            )
            .await?
            .map(|r| turn(&r)))
    }

    /// The oldest turn waiting on `computer`.
    pub async fn next_turn(&self, computer: &str) -> Result<Option<Turn>, crate::db::DbError> {
        let c = self.db.client().await?;
        Ok(c.query_opt(
            &format!(
                "SELECT {TURN_COLUMNS} FROM computers.turns
                     WHERE computer = $1 AND state IN ('queued', 'running', 'command_queued', 'command_running')
                     ORDER BY created_at,id LIMIT 1"
            ),
            &[&computer],
        )
        .await?
        .map(|r| turn(&r)))
    }

    /// Recover the current generation before considering a later queued turn.
    /// Result and output expiry retain these identities. A missing generation
    /// covers a crash between StartTurn and recording the queue's generation.
    pub(crate) async fn recovering_turn(
        &self,
        computer: &str,
        generation: u64,
    ) -> Result<Option<Turn>, crate::db::DbError> {
        let generation = i64::try_from(generation)
            .map_err(|_| crate::db::DbError("The turn number is invalid.".into()))?;
        let c = self.db.client().await?;
        Ok(c.query_opt(
            &format!(
                "SELECT {TURN_COLUMNS} FROM computers.turns
                 WHERE computer=$1 AND (generation=$2 OR
                     (generation IS NULL AND state IN ('queued','command_queued')))
                 ORDER BY (generation=$2) DESC NULLS LAST,created_at,id LIMIT 1"
            ),
            &[&computer, &generation],
        )
        .await?
        .map(|r| turn(&r)))
    }

    pub(crate) async fn turn_running(
        &self,
        id: &str,
        generation: u64,
    ) -> Result<(), crate::db::DbError> {
        let c = self.db.client().await?;
        c.execute(
            "UPDATE computers.turns SET state = CASE WHEN state='command_queued' THEN 'command_running' ELSE 'running' END, generation = $2, started_at = now()
             WHERE id = $1 AND state IN ('queued','command_queued')",
            &[&id, &(generation as i64)],
        )
        .await?;
        Ok(())
    }

    async fn turn_partial(
        &self,
        id: &str,
        stdout: &str,
        stderr: &str,
    ) -> Result<(), crate::db::DbError> {
        let client = self.db.client().await?;
        client
            .execute(
                "UPDATE computers.turns SET stdout=$2,stderr=$3
             WHERE id=$1 AND state='running'
                 AND octet_length(COALESCE(stdout,'')) <= octet_length($2::text)
                 AND octet_length(COALESCE(stderr,'')) <= octet_length($3::text)",
                &[&id, &cap(stdout), &cap(stderr)],
            )
            .await?;
        Ok(())
    }

    pub(crate) async fn turn_ended(
        &self,
        id: &str,
        ok: bool,
        exit: Option<i64>,
        stdout: Option<&str>,
        stderr: Option<&str>,
        error: Option<&str>,
    ) -> Result<(), crate::db::DbError> {
        let c = self.db.client().await?;
        let state = if ok { "done" } else { "failed" };
        c.execute(
            "UPDATE computers.turns
             SET state = $2, exit_code = $3, stdout = COALESCE($4,stdout), stderr = COALESCE($5,stderr), error = $6, finished_at = now()
             WHERE id = $1 AND state IN ('queued', 'running', 'command_queued', 'command_running')",
            &[&id, &state, &exit, &stdout.map(cap), &stderr.map(cap), &error],
        )
        .await?;
        Ok(())
    }

    /// Fail every unfinished turn of a deleted computer.
    pub async fn end_turns(&self, computer: &str, why: &str) -> Result<(), crate::db::DbError> {
        let c = self.db.client().await?;
        c.execute(
            "UPDATE computers.turns SET state = 'failed', error = $2, finished_at = now()
             WHERE computer = $1 AND state IN ('queued', 'running', 'command_queued', 'command_running')",
            &[&computer, &why],
        )
        .await?;
        Ok(())
    }
}

// ------------------------------------------------------------------ API

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct TurnBody {
    command: String,
    #[serde(default)]
    cwd: Option<String>,
    #[serde(default)]
    timeout_seconds: Option<u64>,
    #[serde(default)]
    env: BTreeMap<String, String>,
}

fn err(status: StatusCode, code: &str, message: &str) -> Response {
    (status, Json(json!({"error": code, "message": message}))).into_response()
}

fn owner(headers: &HeaderMap) -> Option<String> {
    headers
        .get(ACCOUNT_HEADER)
        .and_then(|v| v.to_str().ok())
        .map(str::trim)
        .filter(|a| !a.is_empty() && a.len() <= 256)
        .map(str::to_owned)
}

fn new_id() -> String {
    let mut b = [0u8; 10];
    let _ = getrandom::fill(&mut b);
    format!(
        "trn_{}",
        b.iter().map(|x| format!("{x:02x}")).collect::<String>()
    )
}

async fn add(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: axum::body::Bytes,
) -> Response {
    let Some(owner) = owner(&headers) else {
        return err(
            StatusCode::BAD_REQUEST,
            "no_account",
            "Name the account in X-OpenAgents-Account.",
        );
    };
    // Parsed into secrets at once; the raw body is dropped right after.
    let parsed: Result<TurnBody, _> = serde_json::from_slice(&body);
    drop(body);
    let body = match parsed {
        Ok(b) => b,
        Err(_) => {
            return err(
                StatusCode::BAD_REQUEST,
                "invalid_body",
                "A turn is {command, cwd?, timeout_seconds?, env?}.",
            );
        }
    };
    if body.command.trim().is_empty() || body.command.len() > MAX_COMMAND {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_command",
            "The command is empty or too long.",
        );
    }
    if body.env.len() > MAX_ENV
        || body
            .env
            .iter()
            .any(|(k, v)| !env_name_ok(k) || v.len() > MAX_ENV_VALUE)
    {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_env",
            "Turn env names are letters, digits and underscores; values are short.",
        );
    }
    if body
        .cwd
        .as_deref()
        .is_some_and(|c| c.is_empty() || c.len() > 1024 || c.chars().any(char::is_control))
    {
        return err(
            StatusCode::BAD_REQUEST,
            "invalid_cwd",
            "The working directory is invalid.",
        );
    }
    let timeout = body
        .timeout_seconds
        .unwrap_or(MAX_SECONDS)
        .clamp(1, MAX_SECONDS) as i32;
    match api.store.get(&owner, &id).await {
        Ok(Some(e)) if e.desired != Desired::Deleted => match api.capabilities_for(&e) {
            Ok(caps) if caps.workspace_attachment => {
                return err(
                    StatusCode::UNPROCESSABLE_ENTITY,
                    "unsupported",
                    "Use an identified command on this workspace connection. Chat turns require suspension support.",
                );
            }
            Err(error) => return error,
            _ => {}
        },
        Ok(_) => {
            return err(
                StatusCode::NOT_FOUND,
                "not_found",
                "No such computer, or it was deleted.",
            );
        }
        Err(e) => {
            eprintln!("oa-computers: turn: {e}");
            return err(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "Try again.");
        }
    }
    let request_key = match headers.get("idempotency-key").map(|v| v.to_str()) {
        None => None,
        Some(Ok(k))
            if !k.is_empty()
                && k.len() <= 128
                && k.bytes()
                    .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b)) =>
        {
            Some(k.to_owned())
        }
        Some(_) => {
            return err(
                StatusCode::BAD_REQUEST,
                "invalid_request_key",
                "An Idempotency-Key is letters, digits, dots, dashes and underscores.",
            );
        }
    };
    let digest = request_key.as_ref().map(|_| {
        use sha2::{Digest, Sha256};
        let mut env: Vec<(&String, &String)> = body.env.iter().collect();
        env.sort();
        format!(
            "{:x}",
            Sha256::digest(
                serde_json::to_vec(&json!([body.command, body.cwd, timeout, env])).unwrap()
            )
        )
    });
    let turn = new_id();
    let names: Vec<String> = body.env.keys().cloned().collect();
    let secrets: BTreeMap<String, Secret> = body
        .env
        .into_iter()
        .map(|(k, v)| (k, Secret::new(v)))
        .collect();
    api.turns.put(&turn, secrets);
    let added = api
        .store
        .add_turn_keyed(
            &turn,
            &id,
            &owner,
            &body.command,
            body.cwd.as_deref(),
            timeout,
            &names,
            request_key.as_deref().zip(digest.as_deref()),
        )
        .await;
    let turn = match added {
        Ok((original, true)) => original,
        Ok((original, false)) => {
            api.turns.take(&turn);
            return (
                StatusCode::ACCEPTED,
                Json(json!({"id": original, "computer": id, "state": "queued", "env_names": names})),
            )
                .into_response();
        }
        Err(e) => {
            api.turns.take(&turn);
            if e.0 == CONFLICT_ERROR {
                return err(
                    StatusCode::CONFLICT,
                    CONFLICT_ERROR,
                    "That request key already names a different command.",
                );
            }
            if e.0 == crate::usage::QUOTA_ERROR {
                return err(
                    StatusCode::TOO_MANY_REQUESTS,
                    "quota_exhausted",
                    crate::usage::QUOTA_ERROR,
                );
            }
            eprintln!("oa-computers: turn: {e}");
            return err(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "Try again.");
        }
    };
    (
        StatusCode::ACCEPTED,
        Json(json!({"id": turn, "computer": id, "state": "queued", "env_names": names})),
    )
        .into_response()
}

async fn read(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((id, turn)): Path<(String, String)>,
) -> Response {
    let Some(owner) = owner(&headers) else {
        return err(
            StatusCode::BAD_REQUEST,
            "no_account",
            "Name the account in X-OpenAgents-Account.",
        );
    };
    match api.store.turn(&owner, &id, &turn).await {
        Ok(Some(mut t)) => {
            if t.api_command {
                match api.store.command(&owner, &id, &turn).await {
                    Ok(Some(r)) if !r.expired => {
                        t.stdout = Some(String::from_utf8_lossy(&r.stdout).into_owned());
                        t.stderr = Some(String::from_utf8_lossy(&r.stderr).into_owned());
                        t.error = match r.status.as_str() {
                            "unknown" => {
                                Some("We couldn't confirm whether this command finished.".into())
                            }
                            "cancelled" => Some(STOPPED.into()),
                            "timed_out" => Some("This command timed out.".into()),
                            _ => None,
                        };
                    }
                    Ok(Some(_)) => t.error = Some("This command's output has expired.".into()),
                    _ => return err(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "Try again."),
                }
            }
            Json(view(&t)).into_response()
        }
        Ok(None) => err(StatusCode::NOT_FOUND, "not_found", "No such turn."),
        Err(e) => {
            eprintln!("oa-computers: read turn: {e}");
            err(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "Try again.")
        }
    }
}

/// `POST /v1/computers/{id}/turns/{turn}/stop`: a queued turn ends at
/// once; a running one's command is abandoned (its connection to the
/// computer closed) and the turn ends as stopped. Identified command IDs use
/// their durable cancellation and expose the legacy turn result shape.
async fn stop(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((id, turn)): Path<(String, String)>,
) -> Response {
    let Some(owner) = owner(&headers) else {
        return err(
            StatusCode::BAD_REQUEST,
            "no_account",
            "Name the account in X-OpenAgents-Account.",
        );
    };
    match api.store.turn(&owner, &id, &turn).await {
        Ok(Some(t)) if t.api_command => {
            let cancelled = crate::commands::cancel(
                State(api.clone()),
                headers.clone(),
                Path((id.clone(), turn.clone())),
            )
            .await;
            if !cancelled.status().is_success() {
                return cancelled;
            }
            let mut response = read(State(api), headers, Path((id, turn))).await;
            if response.status() == StatusCode::OK {
                *response.status_mut() = StatusCode::ACCEPTED;
            }
            response
        }
        Ok(Some(t)) if t.state == "queued" => {
            api.turns.take(&turn);
            let _ = api
                .store
                .turn_ended(&turn, false, None, None, None, Some(STOPPED))
                .await;
            (
                StatusCode::ACCEPTED,
                Json(json!({"id": turn, "state": "failed"})),
            )
                .into_response()
        }
        Ok(Some(t)) if t.state == "running" => {
            api.turns.stop(&turn);
            (
                StatusCode::ACCEPTED,
                Json(json!({"id": turn, "state": "running"})),
            )
                .into_response()
        }
        Ok(Some(t)) => Json(view(&t)).into_response(),
        Ok(None) => err(StatusCode::NOT_FOUND, "not_found", "No such turn."),
        Err(e) => {
            eprintln!("oa-computers: stop turn: {e}");
            err(StatusCode::SERVICE_UNAVAILABLE, "unavailable", "Try again.")
        }
    }
}

/// What a stopped turn says.
pub const STOPPED: &str = "Stopped.";

pub(crate) const UNCERTAIN_MEMORY_TURN: &str =
    "We couldn't confirm whether the command stopped. Delete this computer to end it.";

/// Keep an uncertain live process out of a private memory checkpoint. Only
/// explicit deletion can discard it; a new turn cannot take over its resource.
pub(crate) async fn uncertain_memory_turn<D: Driver>(
    p: &OnDriver<D>,
    lease: &PgLease,
    e: &Entry,
    computer: Computer,
) -> coder_working_computer::store::Result<Settled> {
    let expired = computer
        .boot()
        .is_some_and(|boot| crate::reconcile::now_ms() >= boot.absolute_deadline_ms);
    let deleting = if e.desired == Desired::Deleted {
        true
    } else if expired {
        lease.read().await?;
        lease
            .store
            .settle_desired(&e.id, e.intent_generation, e.desired, Desired::Deleted)
            .await
            .map_err(|_| {
                coder_working_computer::store::StoreError::Io(
                    "The computer's time limit couldn't be applied.",
                )
            })?
    } else {
        false
    };
    if deleting {
        use coder_working_computer::provider::Provider;
        if let Some(resource) = computer.resource() {
            lease.read().await?;
            if let Outcome::Done { .. } = p.delete(&computer, resource).await {
                // Deletion was observed, so stopping cannot capture RAM.
                return driver::delete(p, lease, crate::reconcile::now_ms()).await;
            }
        }
    }
    let message = if deleting {
        "We couldn't confirm whether this computer was deleted. We will check again."
    } else {
        UNCERTAIN_MEMORY_TURN
    };
    Ok(Settled::Refused(message.into(), computer))
}

pub fn routes() -> Router<Arc<Api>> {
    Router::new()
        .route("/v1/computers/{id}/turns", post(add))
        .route("/v1/computers/{id}/turns/{turn}", get(read))
        .route("/v1/computers/{id}/turns/{turn}/stop", post(stop))
}

// ----------------------------------------------------------- reconciler

/// Run the computer's next waiting turn, if any, on an awake computer.
/// Returns the settle the visit ends with.
pub async fn run_next<D: Driver + Clone + 'static>(
    store: &Store,
    secrets: &Secrets,
    p: &OnDriver<D>,
    lease: &PgLease,
    e: &Entry,
    awake: Computer,
    heartbeat: Duration,
) -> coder_working_computer::store::Result<Settled> {
    let io = |_| coder_working_computer::store::StoreError::Io("The turn store is unavailable.");
    let Some(t) = store.next_turn(&e.id).await.map_err(io)? else {
        return Ok(Settled::Idle(awake));
    };
    if t.api_command {
        return crate::commands::run(store, secrets, p, lease, e, awake, t, heartbeat).await;
    }
    // Queueing can race the visit's initial claim. Bind this turn's automatic
    // stop to the intent seen at dispatch, while preserving later user requests.
    let Some(dispatch_entry) = store.get(&e.owner, &e.id).await.map_err(io)? else {
        return Ok(Settled::Idle(awake));
    };
    if dispatch_entry.desired != Desired::Running {
        return Ok(Settled::Idle(awake));
    }
    let dispatch_intent_generation = dispatch_entry.intent_generation;
    let resource = awake.resource().unwrap_or_default().to_owned();
    if t.state == "running" {
        // A turn that was running when a process stopped: its process and
        // credentials are gone. It ends, and its computer checkpoints.
        let _ = store
            .turn_ended(
                &t.id,
                false,
                None,
                None,
                None,
                Some("The turn was interrupted. Send it again."),
            )
            .await;
        return Ok(Settled::Idle(awake));
    }
    let env = secrets.take(&t.id);
    if env.len() != t.env_names.len() {
        let _ = store
            .turn_ended(
                &t.id,
                false,
                None,
                None,
                None,
                Some("This turn's credentials were not kept across a restart. Send it again."),
            )
            .await;
        return Ok(Settled::Idle(awake));
    }
    // A machine stopped underneath an awake record (the backend's own idle
    // stop) is stopped through the lifecycle and woken again first.
    if let Outcome::Done { value } = p.driver.observe(&resource).await
        && value.state != Machine::Running
    {
        lease
            .apply(
                &Command::RequestStop {
                    reason: StopReason::Idle,
                },
                crate::reconcile::now_ms(),
            )
            .await?;
        driver::settle(p, lease, Trigger::Tick, crate::reconcile::now_ms()).await?;
        let woke = driver::settle(p, lease, Trigger::Wake, crate::reconcile::now_ms()).await?;
        if woke.computer().phase != Phase::Awake {
            secrets.put(&t.id, env);
            return Ok(woke);
        }
    }
    let settled = driver::settle(p, lease, Trigger::Prompt, crate::reconcile::now_ms()).await?;
    let Settled::Dispatch {
        generation,
        computer,
    } = settled
    else {
        secrets.put(&t.id, env);
        return Ok(settled);
    };
    store.turn_running(&t.id, generation).await.map_err(io)?;
    let mut exec = Exec::new(t.command.clone());
    exec.cwd = t.cwd.clone();
    exec.timeout_seconds = t.timeout_seconds.max(1) as u64;
    exec.secret_env = env;
    let capture = exec.output.clone();
    let resource = computer.resource().unwrap_or_default().to_owned();
    // The turn's owner says it is alive while the command runs.
    let beat = {
        let lease = PgLease {
            store: lease.store.clone(),
            id: lease.id.clone(),
            holder: lease.holder.clone(),
            lease: lease.lease,
        };
        tokio::task::spawn_local(async move {
            loop {
                tokio::time::sleep(heartbeat).await;
                if lease
                    .apply(
                        &Command::Heartbeat { generation },
                        crate::reconcile::now_ms(),
                    )
                    .await
                    .is_err()
                {
                    break;
                }
            }
        })
    };
    // The command runs until it ends or the turn is asked to stop; a
    // stop abandons the call, which closes its connection to the computer.
    let outcome = {
        let run = p.driver.exec(&resource, &exec);
        tokio::pin!(run);
        loop {
            tokio::select! {
                o = &mut run => break o,
                () = tokio::time::sleep(Duration::from_millis(500)) => {
                    if p.driver.capabilities().memory_suspend {
                        let (stdout, stderr) = capture.read();
                        if !stdout.is_empty() || !stderr.is_empty() {
                            let _ = store.turn_partial(&t.id, &String::from_utf8_lossy(&stdout), &String::from_utf8_lossy(&stderr)).await;
                        }
                    }
                    let stop_requested = secrets.stopping(&t.id)
                        || store.get(&e.owner, &e.id).await
                            .map_or(true, |entry| entry.is_none_or(|current| current.desired != Desired::Running));
                    if stop_requested {
                        break if p.driver.capabilities().memory_suspend {
                            // Kill the command's session and wait for its
                            // exit; only an unconfirmed stop is uncertain.
                            match p.driver.cancel_exec(&resource).await {
                                Outcome::Done { .. } => Outcome::failed(STOPPED),
                                _ => Outcome::unknown(STOPPED),
                            }
                        } else {
                            Outcome::failed(STOPPED)
                        };
                    }
                }
            }
        }
    };
    secrets.stopped(&t.id);
    beat.abort();
    drop(exec);
    match &outcome {
        Outcome::Failed { reason } if reason == STOPPED => {
            store
                .turn_ended(&t.id, false, None, None, None, Some(STOPPED))
                .await
                .map_err(io)?;
        }
        Outcome::Done { value } => {
            store
                .turn_ended(
                    &t.id,
                    true,
                    Some(value.code),
                    Some(&value.stdout),
                    Some(&value.stderr),
                    None,
                )
                .await
                .map_err(io)?;
        }
        Outcome::Unknown { .. } if p.driver.capabilities().memory_suspend => {
            let (stdout, stderr) = capture.read();
            store
                .turn_ended(
                    &t.id,
                    false,
                    None,
                    Some(&String::from_utf8_lossy(&stdout)),
                    Some(&String::from_utf8_lossy(&stderr)),
                    Some(UNCERTAIN_MEMORY_TURN),
                )
                .await
                .map_err(io)?;
        }
        Outcome::Failed { reason } | Outcome::Unknown { reason } => {
            store
                .turn_ended(
                    &t.id,
                    false,
                    None,
                    None,
                    None,
                    Some(&format!("The command did not run to the end: {reason}")),
                )
                .await
                .map_err(io)?;
        }
    }
    if p.driver.capabilities().memory_suspend && matches!(outcome, Outcome::Unknown { .. }) {
        return uncertain_memory_turn(p, lease, e, computer).await;
    }
    let finished = driver::turn_finished(p, lease, generation, crate::reconcile::now_ms()).await?;
    // Nothing else waiting: it stays stopped (its disk kept) until the
    // next turn or wake.
    if store.next_turn(&e.id).await.map_err(io)?.is_none() {
        let _ = store
            .settle_desired(
                &e.id,
                dispatch_intent_generation,
                Desired::Running,
                Desired::Stopped,
            )
            .await;
    } else {
        let _ = store.poke(&e.id).await;
    }
    Ok(finished)
}

#[cfg(test)]
mod output_tests {
    use super::*;

    #[tokio::test]
    async fn a_repeated_request_key_returns_the_first_turn_and_a_changed_body_conflicts() {
        let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
            return;
        };
        let (db, _) = crate::Db::scratch(&admin).await.unwrap();
        let store = Store::new(db);
        let caps =
            computer_driver::Capabilities::new("fake", computer_driver::Profile::ALL, &["small"]);
        let record = crate::api::admit_create(
            &caps,
            "key-test",
            &crate::registry::CreateIntent::default(),
        )
        .unwrap();
        let (entry, _) = store
            .create(&crate::store::NewComputer {
                owner: "key-test",
                chat: None,
                idempotency_key: Some("turn-keys"),
                driver: "fake",
                image: None,
                record: &record,
            })
            .await
            .unwrap();
        let add = |id: &'static str, cmd: &'static str, digest: &'static str| {
            let store = store.clone();
            let computer = entry.id.clone();
            async move {
                store
                    .add_turn_keyed(id, &computer, "key-test", cmd, None, 30, &[], Some(("k1", digest)))
                    .await
            }
        };
        assert_eq!(add("first", "true", "d1").await.unwrap(), ("first".into(), true));
        assert_eq!(add("second", "true", "d1").await.unwrap(), ("first".into(), false));
        let e = add("third", "false", "d2").await.unwrap_err();
        assert_eq!(e.0, CONFLICT_ERROR);
    }

    #[tokio::test]
    async fn uncertain_completion_keeps_partial_output_and_late_reads_cannot_rewind_it() {
        let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
            return;
        };
        let (db, _) = crate::Db::scratch(&admin).await.unwrap();
        let store = Store::new(db);
        let caps =
            computer_driver::Capabilities::new("fake", computer_driver::Profile::ALL, &["small"]);
        let record = crate::api::admit_create(
            &caps,
            "output-test",
            &crate::registry::CreateIntent::default(),
        )
        .unwrap();
        let (entry, _) = store
            .create(&crate::store::NewComputer {
                owner: "output-test",
                chat: None,
                idempotency_key: Some("partial-output"),
                driver: "fake",
                image: None,
                record: &record,
            })
            .await
            .unwrap();
        store
            .add_turn(
                "partial-turn",
                &entry.id,
                "output-test",
                "printf early",
                None,
                30,
                &[],
            )
            .await
            .unwrap();
        store.turn_running("partial-turn", 1).await.unwrap();
        store
            .turn_partial("partial-turn", "early output", "early error")
            .await
            .unwrap();
        store
            .turn_partial("partial-turn", "early", "early")
            .await
            .unwrap();
        store
            .turn_ended(
                "partial-turn",
                false,
                None,
                None,
                None,
                Some(UNCERTAIN_MEMORY_TURN),
            )
            .await
            .unwrap();
        store
            .turn_partial("partial-turn", "late replacement", "late replacement")
            .await
            .unwrap();
        let saved = store
            .turn("output-test", &entry.id, "partial-turn")
            .await
            .unwrap()
            .unwrap();
        assert_eq!(saved.stdout.as_deref(), Some("early output"));
        assert_eq!(saved.stderr.as_deref(), Some("early error"));
        assert_eq!(saved.exit_code, None);
        assert_eq!(saved.state, "failed");
    }
}

#[cfg(test)]
mod deadline_tests {
    use super::*;
    use computer_driver::fake::FakeDriver;

    #[tokio::test]
    async fn an_uncertain_command_cannot_run_past_the_absolute_limit_or_take_a_checkpoint() {
        let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
            return;
        };
        let (db, _) = crate::Db::scratch(&admin).await.unwrap();
        let store = Store::new(db);
        let root = tempfile::tempdir().unwrap();
        let fake = FakeDriver::new(root.path()).boot_observations(0);
        let mut record = crate::api::admit_create(
            &fake.capabilities(),
            "deadline-test",
            &crate::registry::CreateIntent::default(),
        )
        .unwrap();
        record.bounds = coder_working_computer::Bounds {
            idle_ms: 1,
            observed_extension_ms: 1,
            absolute_ms: 1,
        };
        let (entry, _) = store
            .create(&crate::store::NewComputer {
                owner: "deadline-test",
                chat: None,
                idempotency_key: Some("deadline"),
                driver: "fake",
                image: None,
                record: &record,
            })
            .await
            .unwrap();
        let duration = Duration::from_secs(30);
        store
            .claim("deadline-holder", duration)
            .await
            .unwrap()
            .unwrap();
        let lease = PgLease {
            store: store.clone(),
            id: entry.id.clone(),
            holder: "deadline-holder".into(),
            lease: duration,
        };
        let provider = OnDriver::new(fake.clone(), None);
        let started = driver::settle(
            &provider,
            &lease,
            Trigger::Prompt,
            crate::reconcile::now_ms(),
        )
        .await
        .unwrap();
        assert!(matches!(started, Settled::Dispatch { generation: 1, .. }));
        tokio::time::sleep(Duration::from_millis(5)).await;
        let finished = uncertain_memory_turn(&provider, &lease, &entry, started.computer().clone())
            .await
            .unwrap();
        assert_eq!(finished.computer().phase, Phase::Deleted);
        assert_eq!(
            store
                .get("deadline-test", &entry.id)
                .await
                .unwrap()
                .unwrap()
                .desired,
            Desired::Deleted
        );
        assert!(fake.live().is_empty());
        assert!(!fake.calls().iter().any(|call| call.starts_with("suspend:")));
    }
}
