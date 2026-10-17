//! Public commands share the turn queue, lease, and checkpoint owner.
//! Dispatch is recorded before transport. Recovery only observes that identity.
mod redaction;
use crate::{
    Store,
    api::Api,
    db::DbError,
    store::{Desired, Entry, PgLease},
    turns::Turn,
};
use axum::{
    Json, Router,
    extract::{Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use base64::Engine;
use coder_working_computer::{
    Command, Computer, Phase, Trigger,
    driver::{self, Held, Settled},
    on_driver::OnDriver,
};
use computer_driver::{CommandCursor, CommandProgress, CommandSpec, Driver, Outcome};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeMap, BTreeSet},
    sync::Arc,
    time::Duration,
};

const LIMIT: usize = 256 * 1024;

#[derive(Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Request {
    pub command: String,
    #[serde(default = "default_cwd")]
    pub cwd: String,
    #[serde(default = "default_timeout")]
    pub timeout_seconds: u64,
    #[serde(default)]
    pub env: BTreeMap<String, String>,
    pub idempotency_key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
}
fn default_cwd() -> String {
    ".".into()
}
fn default_timeout() -> u64 {
    600
}

impl Request {
    pub fn valid(&self) -> bool {
        self.credential_ref
            .as_deref()
            .is_none_or(crate::command_credentials::valid_reference)
            && (self.credential_ref.is_none() || (self.env.is_empty() && self.cwd == "."))
            && !self.command.trim().is_empty()
            && self.command.len() <= 65536
            && !self.command.contains('\0')
            && !self.cwd.is_empty()
            && self.cwd.len() <= 1024
            && !self.cwd.starts_with('/')
            && !self.cwd.chars().any(char::is_control)
            && self.cwd.split('/').all(|p| !p.is_empty() && p != "..")
            && (1..=3600).contains(&self.timeout_seconds)
            && self.env.len() <= 16
            && self.env.iter().all(|(k, v)| {
                computer_driver::env_name_ok(k)
                    && (matches!(k.as_str(), "LANG" | "LC_ALL" | "TZ" | "CI" | "TERM")
                        || k.starts_with("OA_PARAM_"))
                    && v.len() <= 4096
                    && !v.contains('\0')
            })
            && !self.idempotency_key.is_empty()
            && self.idempotency_key.len() <= 128
            && !self.idempotency_key.starts_with('.')
            && self
                .idempotency_key
                .bytes()
                .all(|b| b.is_ascii_alphanumeric() || b"_.-".contains(&b))
    }
    fn digest(&self) -> String {
        let mut request = self.clone();
        request.idempotency_key.clear();
        format!(
            "{:x}",
            Sha256::digest(serde_json::to_vec(&request).unwrap())
        )
    }
}

#[derive(Clone)]
pub struct Record {
    pub id: String,
    pub status: String,
    pub digest: String,
    pub options: Value,
    pub cancel: bool,
    pub stdout: Vec<u8>,
    pub stderr: Vec<u8>,
    pub out_gap: bool,
    pub err_gap: bool,
    pub expired: bool,
    pub exit_code: Option<i64>,
    pub created: i64,
    pub started: Option<i64>,
    pub finished: Option<i64>,
    pub expires: i64,
    pub resource: Option<String>,
}
const COLUMNS: &str = "c.turn,c.status,c.digest,c.options,c.cancel_requested,c.stdout,c.stderr,c.stdout_truncated,c.stderr_truncated,c.expires_at <= now(),t.exit_code,extract(epoch from c.created_at)::bigint,extract(epoch from c.dispatched_at)::bigint,extract(epoch from c.finished_at)::bigint,extract(epoch from c.expires_at)::bigint,c.provider_resource";
fn record(r: &tokio_postgres::Row) -> Record {
    Record {
        id: r.get(0),
        status: r.get(1),
        digest: r.get(2),
        options: r.get(3),
        cancel: r.get(4),
        stdout: r.get(5),
        stderr: r.get(6),
        out_gap: r.get(7),
        err_gap: r.get(8),
        expired: r.get(9),
        exit_code: r.get(10),
        created: r.get(11),
        started: r.get(12),
        finished: r.get(13),
        expires: r.get(14),
        resource: r.get(15),
    }
}
impl Record {
    fn terminal(&self) -> bool {
        !matches!(self.status.as_str(), "queued" | "dispatching" | "running")
    }
    fn view(&self, computer: &str) -> Value {
        json!({"id":self.id,"computer":computer,"state":self.status,"digest":self.digest,"exit_code":self.exit_code,"cancel_requested":self.cancel,"created_unix":self.created,"started_unix":self.started,"finished_unix":self.finished,"output_expires_unix":self.expires,"output_limit_bytes":LIMIT,"stdout_truncated":self.out_gap,"stderr_truncated":self.err_gap,"stdout_bytes":self.stdout.len(),"stderr_bytes":self.stderr.len()})
    }
}

impl Store {
    pub async fn prune_command_output(&self) -> Result<(), DbError> {
        self.db.client().await?.execute("UPDATE computers.commands SET stdout='',stderr='' WHERE expires_at<=now() AND (octet_length(stdout)>0 OR octet_length(stderr)>0)",&[]).await?;
        Ok(())
    }

    pub async fn command_by_request(
        &self,
        owner: &str,
        computer: &str,
        key: &str,
    ) -> Result<Option<String>, DbError> {
        let client = self.db.client().await?;
        Ok(client.query_opt("SELECT turn FROM computers.commands WHERE owner=$1 AND computer=$2 AND request_key=$3",&[&owner,&computer,&key]).await?.map(|row|row.get(0)))
    }

    pub async fn command(
        &self,
        owner: &str,
        computer: &str,
        id: &str,
    ) -> Result<Option<Record>, DbError> {
        let c = self.db.client().await?;
        // Expired payloads are removed without freeing request identities.
        c.execute("UPDATE computers.commands SET stdout='',stderr='' WHERE owner=$1 AND computer=$2 AND turn=$3 AND expires_at <= now() AND (octet_length(stdout)>0 OR octet_length(stderr)>0)", &[&owner,&computer,&id]).await?;
        Ok(c.query_opt(&format!("SELECT {COLUMNS} FROM computers.commands c JOIN computers.turns t ON t.id=c.turn WHERE c.owner=$1 AND c.computer=$2 AND c.turn=$3"), &[&owner,&computer,&id]).await?.map(|r| record(&r)))
    }

    pub async fn add_command(
        &self,
        owner: &str,
        computer: &str,
        request: &Request,
    ) -> Result<(String, bool), &'static str> {
        self.add_command_admitted(owner, computer, request, Ok(()))
            .await
    }

    pub(crate) async fn add_command_admitted(
        &self,
        owner: &str,
        computer: &str,
        request: &Request,
        admission: Result<(), &'static str>,
    ) -> Result<(String, bool), &'static str> {
        let mut c = self.db.independent().await.map_err(|_| "unavailable")?;
        let tx = c.transaction().await.map_err(|_| "unavailable")?;
        tx.batch_execute("SET LOCAL lock_timeout='5s'; SET LOCAL statement_timeout='10s'")
            .await
            .map_err(|_| "unavailable")?;
        tx.query_one(
            "SELECT pg_advisory_xact_lock(hashtextextended($1,11802))",
            &[&format!("{owner}/{computer}")],
        )
        .await
        .map_err(|_| "unavailable")?;
        let digest = request.digest();
        if let Some(r) = tx.query_opt("SELECT turn,digest FROM computers.commands WHERE owner=$1 AND computer=$2 AND request_key=$3", &[&owner,&computer,&request.idempotency_key]).await.map_err(|_| "unavailable")? {
            if r.get::<_,String>(1) != digest { return Err("idempotency_conflict"); }
            return Ok((r.get(0),false));
        }
        // An existing request remains readable after a deployment disappears
        // or stops admitting commands. Only a new command needs admission.
        admission?;
        crate::usage::lock(&tx).await.map_err(|_| "unavailable")?;
        crate::usage::check_turn(&tx, owner, request.timeout_seconds as i32)
            .await
            .map_err(|e| {
                if e.0 == crate::usage::QUOTA_ERROR {
                    "quota_exhausted"
                } else {
                    "unavailable"
                }
            })?;
        if tx.query_opt("SELECT id FROM computers.computers WHERE id=$1 AND owner=$2 AND desired<>'deleted' FOR UPDATE", &[&computer,&owner]).await.map_err(|_| "unavailable")?.is_none() { return Err("not_found"); }
        let count:i64 = tx.query_one("SELECT count(*) FROM computers.turns WHERE computer=$1 AND state IN ('queued','running','command_queued','command_running')", &[&computer]).await.map_err(|_| "unavailable")?.get(0);
        if count >= 16 {
            return Err("quota_exhausted");
        }
        let mut bytes = [0u8; 10];
        getrandom::fill(&mut bytes).map_err(|_| "unavailable")?;
        let id = format!(
            "trn_{}",
            bytes.iter().map(|b| format!("{b:02x}")).collect::<String>()
        );
        tx.execute("INSERT INTO computers.turns(id,computer,owner,state,command,cwd,timeout_seconds) VALUES($1,$2,$3,'command_queued',$4,$5,$6)", &[&id,&computer,&owner,&request.command,&request.cwd,&(request.timeout_seconds as i32)]).await.map_err(|_| "unavailable")?;
        let options = json!({"env":request.env,"credential_ref":request.credential_ref,"request_key":request.idempotency_key});
        tx.execute("INSERT INTO computers.commands(turn,owner,computer,request_key,digest,options,status) VALUES($1,$2,$3,$4,$5,$6,'queued')", &[&id,&owner,&computer,&request.idempotency_key,&digest,&options]).await.map_err(|_| "unavailable")?;
        tx.execute("UPDATE computers.computers SET desired='running',intent_generation=intent_generation+1,usage_intent_generation=intent_generation+1,next_attempt_at=now(),attempts=0 WHERE id=$1", &[&computer]).await.map_err(|_| "unavailable")?;
        crate::usage::record_intent(&tx, owner, computer)
            .await
            .map_err(|e| {
                if e.0 == crate::usage::QUOTA_ERROR {
                    "quota_exhausted"
                } else {
                    "unavailable"
                }
            })?;
        tx.commit().await.map_err(|_| "unavailable")?;
        Ok((id, true))
    }
    async fn command_end(&self, id: &str, state: &str, code: Option<i64>) -> Result<(), DbError> {
        self.db.client().await?.execute(
            "WITH ended AS (UPDATE computers.commands SET status=$2,finished_at=now() WHERE turn=$1 AND finished_at IS NULL RETURNING turn)
             UPDATE computers.turns SET state=CASE WHEN $2='succeeded' THEN 'done' ELSE 'failed' END,exit_code=$3,finished_at=now() WHERE id IN (SELECT turn FROM ended)",
            &[&id,&state,&code]).await?;
        Ok(())
    }
    #[cfg(test)]
    async fn command_output(
        &self,
        id: &str,
        stdout: &[u8],
        stderr: &[u8],
        out_gap: bool,
        err_gap: bool,
    ) -> Result<(), DbError> {
        self.command_output_scoped(id, stdout, stderr, out_gap, err_gap, None)
            .await
    }
    async fn command_output_scoped(
        &self,
        id: &str,
        stdout: &[u8],
        stderr: &[u8],
        out_gap: bool,
        err_gap: bool,
        source: Option<CommandCursor>,
    ) -> Result<(), DbError> {
        if let Some(source) = source {
            let positions = json!({"source_stdout":source.stdout,"source_stderr":source.stderr});
            self.db.client().await?.execute("UPDATE computers.commands SET stdout=$2,stderr=$3,stdout_truncated=stdout_truncated OR $4,stderr_truncated=stderr_truncated OR $5,options=options || $6::jsonb WHERE turn=$1 AND expires_at>now() AND finished_at IS NULL AND octet_length(stdout)<=octet_length($2::bytea) AND octet_length(stderr)<=octet_length($3::bytea) AND COALESCE((options->>'source_stdout')::bigint,0)<=$7 AND COALESCE((options->>'source_stderr')::bigint,0)<=$8",&[&id,&stdout,&stderr,&out_gap,&err_gap,&positions,&(source.stdout as i64),&(source.stderr as i64)]).await?;
            return Ok(());
        }
        self.db.client().await?.execute("UPDATE computers.commands SET stdout=$2,stderr=$3,stdout_truncated=stdout_truncated OR $4,stderr_truncated=stderr_truncated OR $5 WHERE turn=$1 AND expires_at>now() AND finished_at IS NULL AND octet_length(stdout)<=octet_length($2::bytea) AND octet_length(stderr)<=octet_length($3::bytea)", &[&id,&stdout,&stderr,&out_gap,&err_gap]).await?;
        Ok(())
    }
}

fn owner(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(crate::api::ACCOUNT_HEADER)?
        .to_str()
        .ok()
        .filter(|s| !s.is_empty() && s.len() <= 256)
}
fn error(code: &str) -> Response {
    let status = match code {
        "not_found" => StatusCode::NOT_FOUND,
        "idempotency_conflict" => StatusCode::CONFLICT,
        "quota_exhausted" => StatusCode::TOO_MANY_REQUESTS,
        "invalid" => StatusCode::BAD_REQUEST,
        "expired" => StatusCode::GONE,
        "unsupported" => StatusCode::UNPROCESSABLE_ENTITY,
        _ => StatusCode::SERVICE_UNAVAILABLE,
    };
    (status,Json(json!({"error":{"code":code,"message":match code {"unsupported"=>"Commands aren't available for this computer.","invalid"=>"Check the command settings and try again.","not_found"=>"No such command.","expired"=>"This command's output has expired.","idempotency_conflict"=>"That request key already names another command.","quota_exhausted"=>"Wait for a command to finish before starting another.",_=>"We couldn't read that command. Try again later."}}}))).into_response()
}
async fn create(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path(computer): Path<String>,
    bytes: axum::body::Bytes,
) -> Response {
    let Some(owner) = owner(&headers) else {
        return error("invalid");
    };
    if bytes.len() > 73728 {
        return error("invalid");
    }
    let Ok(request) = serde_json::from_slice::<Request>(&bytes) else {
        return error("invalid");
    };
    if !request.valid() {
        return error("invalid");
    }
    let entry = match api.store.get(owner, &computer).await {
        Ok(Some(entry)) => entry,
        Ok(None) => return error("not_found"),
        Err(_) => return error("unavailable"),
    };
    let admission = match api.capabilities_for(&entry) {
        Ok(caps) if api.catalog.is_some() && !caps.command_sessions => Err("unsupported"),
        Ok(caps)
            if caps.workspace_attachment
                && (request.credential_ref.is_some()
                    || request.timeout_seconds > coder_access::enrolled::MAX_TIMEOUT
                    || request.command.len() > 32 * 1024) =>
        {
            Err("unsupported")
        }
        Ok(_) => Ok(()),
        Err(_) => Err("unavailable"),
    };
    let (id, new) = match api
        .store
        .add_command_admitted(owner, &computer, &request, admission)
        .await
    {
        Ok(r) => r,
        Err(e) => return error(e),
    };
    match api.store.command(owner, &computer, &id).await {
        Ok(Some(r)) => (
            if new {
                StatusCode::ACCEPTED
            } else {
                StatusCode::OK
            },
            Json(r.view(&computer)),
        )
            .into_response(),
        _ => error("unavailable"),
    }
}
async fn read(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((computer, id)): Path<(String, String)>,
) -> Response {
    let Some(owner) = owner(&headers) else {
        return error("invalid");
    };
    match api.store.command(owner, &computer, &id).await {
        Ok(Some(r)) => Json(r.view(&computer)).into_response(),
        Ok(None) => error("not_found"),
        _ => error("unavailable"),
    }
}
#[derive(Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct OutputQuery {
    pub stdout: u64,
    pub stderr: u64,
    pub max_bytes: u64,
    pub wait_ms: u64,
}
async fn output(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((computer, id)): Path<(String, String)>,
    Query(q): Query<OutputQuery>,
) -> Response {
    let Some(owner) = owner(&headers) else {
        return error("invalid");
    };
    if q.stdout > LIMIT as u64
        || q.stderr > LIMIT as u64
        || q.max_bytes > 65536
        || q.wait_ms > 20000
    {
        return error("invalid");
    }
    let until = tokio::time::Instant::now() + Duration::from_millis(q.wait_ms);
    loop {
        let r = match api.store.command(owner, &computer, &id).await {
            Ok(Some(r)) => r,
            Ok(None) => return error("not_found"),
            _ => return error("unavailable"),
        };
        if r.expired {
            return error("expired");
        }
        if q.stdout as usize > r.stdout.len() || q.stderr as usize > r.stderr.len() {
            return error("invalid");
        }
        if r.terminal()
            || r.stdout.len() > q.stdout as usize
            || r.stderr.len() > q.stderr as usize
            || tokio::time::Instant::now() >= until
        {
            let max = if q.max_bytes == 0 {
                65536
            } else {
                q.max_bytes as usize
            };
            let out = &r.stdout[q.stdout as usize..r.stdout.len().min(q.stdout as usize + max)];
            let err = &r.stderr[q.stderr as usize..r.stderr.len().min(q.stderr as usize + max)];
            let b64 = &base64::engine::general_purpose::STANDARD;
            return Json(json!({"command":r.view(&computer),"encoding":"base64","stdout":b64.encode(out),"stderr":b64.encode(err),"next_cursor":{"stdout":q.stdout+out.len() as u64,"stderr":q.stderr+err.len() as u64},"gaps":{"stdout":r.out_gap,"stderr":r.err_gap},"terminal":r.terminal(),"has_more":(q.stdout+out.len() as u64)<r.stdout.len() as u64 || (q.stderr+err.len() as u64)<r.stderr.len() as u64})).into_response();
        }
        tokio::time::sleep(Duration::from_millis(200)).await;
    }
}
pub(crate) async fn cancel(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((computer, id)): Path<(String, String)>,
) -> Response {
    let Some(owner) = owner(&headers) else {
        return error("invalid");
    };
    let r = match api.store.command(owner, &computer, &id).await {
        Ok(Some(r)) => r,
        Ok(None) => return error("not_found"),
        _ => return error("unavailable"),
    };
    if !r.terminal() {
        let Ok(c) = api.store.db.client().await else {
            return error("unavailable");
        };
        if c.execute(
            "WITH cancelled AS (
                UPDATE computers.commands SET cancel_requested=true,
                    status=CASE WHEN status='queued' THEN 'cancelled' ELSE status END,
                    finished_at=CASE WHEN status='queued' THEN now() ELSE finished_at END
                WHERE turn=$1 AND finished_at IS NULL RETURNING turn,status
             ) UPDATE computers.turns SET state='failed',finished_at=now()
               WHERE id IN (SELECT turn FROM cancelled WHERE status='cancelled')",
            &[&id],
        )
        .await
        .is_err()
        {
            return error("unavailable");
        }
        let _ = api.store.poke(&computer).await;
    }
    match api.store.command(owner, &computer, &id).await {
        Ok(Some(r)) => (StatusCode::ACCEPTED, Json(r.view(&computer))).into_response(),
        _ => error("unavailable"),
    }
}
async fn by_request(
    State(api): State<Arc<Api>>,
    headers: HeaderMap,
    Path((computer, key)): Path<(String, String)>,
) -> Response {
    let Some(owner) = owner(&headers) else {
        return error("invalid");
    };
    match api.store.command_by_request(owner, &computer, &key).await {
        Ok(id) => Json(json!({"id":id})).into_response(),
        Err(_) => error("unavailable"),
    }
}
pub fn routes() -> Router<Arc<Api>> {
    Router::new()
        .route(
            "/v1/computers/{id}/command-credentials",
            post(crate::command_credentials::prepare),
        )
        .route("/v1/computers/{id}/commands", post(create))
        .route(
            "/v1/computers/{id}/commands/by-request/{key}",
            get(by_request),
        )
        .route("/v1/computers/{id}/commands/{command}", get(read))
        .route("/v1/computers/{id}/commands/{command}/output", get(output))
        .route("/v1/computers/{id}/commands/{command}/cancel", post(cancel))
}

/// Run through the existing computer lease. A persisted dispatch marker means
/// recovery may read and cancel, but may never start the command again.
pub async fn run<D: Driver + Clone + 'static>(
    store: &Store,
    secrets: &crate::turns::Secrets,
    p: &OnDriver<D>,
    lease: &PgLease,
    e: &Entry,
    computer: Computer,
    t: Turn,
    heartbeat: Duration,
) -> coder_working_computer::store::Result<Settled> {
    let io = |_| coder_working_computer::store::StoreError::Io("The command store is unavailable.");
    let Some(mut r) = store.command(&e.owner, &e.id, &t.id).await.map_err(io)? else {
        return Ok(Settled::Idle(computer));
    };
    if r.status == "unknown" && p.driver.capabilities().memory_suspend {
        return crate::turns::uncertain_memory_turn(p, lease, e, computer).await;
    }
    if r.terminal() {
        // Crash between committing a command result and ending its turn.
        store
            .turn_ended(
                &t.id,
                r.status == "succeeded",
                r.exit_code,
                None,
                None,
                None,
            )
            .await
            .map_err(io)?;
    }
    let (generation, computer) = match computer.phase {
        Phase::Turn { generation } => (generation, computer),
        Phase::Awake => {
            match driver::settle(p, lease, Trigger::Prompt, crate::reconcile::now_ms()).await? {
                Settled::Dispatch {
                    generation,
                    computer,
                } => (generation, computer),
                other => return Ok(other),
            }
        }
        _ => return Ok(Settled::Idle(computer)),
    };
    store.turn_running(&t.id, generation).await.map_err(io)?;
    let resource = r
        .resource
        .clone()
        .unwrap_or_else(|| computer.resource().unwrap_or_default().to_owned());
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
    let result = async {
        if r.status == "queued" {
            if r.cancel {
                store.command_end(&t.id, "cancelled", None).await.map_err(io)?;
            } else {
                lease.read().await?;
                let c = store.db.client().await.map_err(io)?;
                let claimed = c.execute(
                    "UPDATE computers.commands SET status='dispatching',dispatched_at=now(),provider_resource=$2
                     WHERE turn=$1 AND status='queued' AND NOT cancel_requested",
                    &[&t.id,&resource]).await.map_err(|x| io(x.into()))?;
                if claimed == 1 {
                    let mut env: BTreeMap<String,String> = serde_json::from_value(r.options["env"].clone())
                        .map_err(|_| io(DbError("options".into())))?;
                    let mut credential_names = BTreeSet::new();
                    if let Some(reference) = r.options["credential_ref"].as_str() {
                        let Some(values) = secrets.2.take(reference, &e.owner, &e.id, r.options["request_key"].as_str().unwrap_or_default(), &format!("{:x}", Sha256::digest(t.command.as_bytes()))) else {
                            store.command_end(&t.id,"failed",None).await.map_err(io)?;
                            return Ok(());
                        };
                        for (name, value) in &values {
                            credential_names.insert(name.clone());
                            env.insert(name.clone(), value.expose().to_owned());
                        }
                    }
                    let mut spec = CommandSpec {
                        id:t.id.clone(), command:t.command.clone(),
                        cwd:t.cwd.clone().unwrap_or_else(default_cwd),
                        timeout_seconds:t.timeout_seconds as u64, env,
                        credential_names, digest:r.digest.clone(),
                    };
                    if e.binding.as_ref().is_some_and(|b|b.environment_attempt.is_some()) {
                        let patterns=redaction::patterns(&t.id,spec.credential_names.iter().filter_map(|name|spec.env.get(name).map(String::as_str)));
                        let metadata=json!({"redaction":patterns,"source_stdout":0,"source_stderr":0});
                        c.execute("UPDATE computers.commands SET options=options || $2::jsonb WHERE turn=$1 AND status='dispatching'",&[&t.id,&metadata]).await.map_err(|x|io(x.into()))?;
                        spec.env.insert("OA_ENVIRONMENT_VOLATILE_OUTPUT".into(),"1".into());
                    }
                    // A lost reply leaves the dispatch marker in place.
                    let dispatched = p.driver.start_command(&resource, &spec).await;
                    for name in &spec.credential_names {
                        if let Some(value) = spec.env.remove(name) { drop(computer_driver::Secret::new(value)); }
                    }
                    match dispatched {
                        Outcome::Done {value} => {
                            c.execute("UPDATE computers.commands SET status='running',provider_reference=$2
                                WHERE turn=$1 AND status='dispatching'", &[&t.id,&value])
                                .await.map_err(|x| io(x.into()))?;
                        }
                        Outcome::Failed {..} => store.command_end(&t.id,"failed",None).await.map_err(io)?,
                        Outcome::Unknown {..} => {}
                    }
                }
            }
        }
        let mut misses = 0;
        loop {
            lease.read().await?;
            r = store.command(&e.owner,&e.id,&t.id).await.map_err(io)?
                .ok_or_else(|| io(DbError("missing command".into())))?;
            if r.terminal() { break; }
            let current = store.get(&e.owner,&e.id).await.map_err(io)?;
            let stopping = r.cancel || current.is_none_or(|c| c.desired != Desired::Running);
            if stopping {
                if r.status == "queued" {
                    store.command_end(&t.id,"cancelled",None).await.map_err(io)?;
                    continue;
                }
                // A signal acknowledgement is not completion. Observe exit below.
                let _ = p.driver.stop_command(&resource,&t.id).await;
            }
            let cursor = CommandCursor {
                stdout:r.options["source_stdout"].as_u64().unwrap_or(r.stdout.len() as u64 + u64::from(r.out_gap)),
                stderr:r.options["source_stderr"].as_u64().unwrap_or(r.stderr.len() as u64 + u64::from(r.err_gap)),
            };
            match p.driver.read_command(&resource,&t.id,cursor,65536).await {
                Outcome::Done {value} if value.digest.as_deref() == Some(r.digest.as_str()) => {
                    misses = 0;
                    let full = value.stdout.len() == 65536 || value.stderr.len() == 65536;
                    let patterns:Vec<redaction::Pattern>=if e.binding.as_ref().is_some_and(|b|b.environment_attempt.is_some()) {
                        match serde_json::from_value(r.options["redaction"].clone()) {
                            Ok(patterns)=>patterns,
                            Err(_)=>{store.command_end(&t.id,"unknown",None).await.map_err(io)?;return Ok(())}
                        }
                    }else{Vec::new()};
                    let terminal=matches!(value.progress,CommandProgress::Exited{..}|CommandProgress::Lost|CommandProgress::Absent) && !full;
                    let (clean_out,out_used)=redaction::scrub(&value.stdout,&t.id,&patterns,terminal);
                    let (clean_err,err_used)=redaction::scrub(&value.stderr,&t.id,&patterns,terminal);
                    let mut out=r.stdout.clone();let mut err=r.stderr.clone();
                    let out_gap=r.out_gap || out.len()+clean_out.len()>LIMIT || cursor.stdout+value.stdout.len() as u64>LIMIT as u64;
                    let err_gap=r.err_gap || err.len()+clean_err.len()>LIMIT || cursor.stderr+value.stderr.len() as u64>LIMIT as u64;
                    out.extend(clean_out.iter().take(LIMIT-out.len()));err.extend(clean_err.iter().take(LIMIT-err.len()));
                    let source=r.options.get("source_stdout").is_some().then_some(CommandCursor{stdout:cursor.stdout+out_used as u64,stderr:cursor.stderr+err_used as u64});
                    store.command_output_scoped(&t.id,&out,&err,out_gap,err_gap,source).await.map_err(io)?;
                    match value.progress {
                        CommandProgress::Exited {code} if !full => {
                            let state = if stopping {"cancelled"}
                                else if value.timed_out {"timed_out"}
                                else if code == 0 {"succeeded"} else {"failed"};
                            store.command_end(&t.id,state,Some(code)).await.map_err(io)?;
                        }
                        CommandProgress::Lost | CommandProgress::Absent => {
                            store.command_end(&t.id,"unknown",None).await.map_err(io)?;
                        }
                        _ => {}
                    }
                    if full { continue; }
                }
                Outcome::Done {value} if value.progress == CommandProgress::Absent => {
                    // A detached wrapper may not yet have made its directory.
                    // Wait for it, but absence never authorizes another start.
                    misses += 1;
                    if misses >= 10 {
                        store.command_end(&t.id,"unknown",None).await.map_err(io)?;
                    }
                }
                _ => {
                    misses += 1;
                    if misses >= 5 {
                        store.command_end(&t.id,"unknown",None).await.map_err(io)?;
                    }
                }
            }
            if r.started.is_some_and(|start| crate::reconcile::now_ms()/1000 > start as u64 + t.timeout_seconds as u64 + 30) {
                let _ = p.driver.stop_command(&resource,&t.id).await;
                store.command_end(&t.id,"unknown",None).await.map_err(io)?;
            }
            tokio::time::sleep(Duration::from_millis(500)).await;
        }
        Ok::<(),coder_working_computer::store::StoreError>(())
    }.await;
    beat.abort();
    result?;
    if r.status == "unknown" && p.driver.capabilities().memory_suspend {
        return crate::turns::uncertain_memory_turn(p, lease, e, computer).await;
    }
    if p.driver.capabilities().workspace_attachment {
        let computer = lease
            .apply(
                &Command::FinishCommand { generation },
                crate::reconcile::now_ms(),
            )
            .await?;
        let _ = store.poke(&e.id).await;
        return Ok(Settled::Idle(computer));
    }
    let finished = driver::turn_finished(p, lease, generation, crate::reconcile::now_ms()).await?;
    if store.next_turn(&e.id).await.map_err(io)?.is_none() {
        let _ = store
            .settle_desired(
                &e.id,
                e.intent_generation,
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
mod tests {
    use super::*;

    #[tokio::test]
    async fn an_old_output_read_cannot_rewind_or_replace_a_finished_result() {
        let Ok(admin) = std::env::var("OA_COMPUTERS_TEST_DATABASE_URL") else {
            return;
        };
        let (db, _) = crate::Db::scratch(&admin).await.unwrap();
        let store = Store::new(db);
        let app = crate::app(
            store.clone(),
            computer_driver::Capabilities::new("fake", computer_driver::Profile::ALL, &["small"]),
            "synthetic-service-token",
            crate::turns::Secrets::default(),
        );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        let task = tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
        let computer: Value = reqwest::Client::new()
            .post(format!("http://{address}/v1/computers"))
            .bearer_auth("synthetic-service-token")
            .header("X-OpenAgents-Account", "test-account")
            .json(&json!({"idempotency_key":"output-race"}))
            .send()
            .await
            .unwrap()
            .json()
            .await
            .unwrap();
        let computer = computer["id"].as_str().unwrap();
        let request = Request {
            credential_ref: None,
            command: "true".into(),
            cwd: ".".into(),
            timeout_seconds: 10,
            env: Default::default(),
            idempotency_key: "once".into(),
        };
        let (id, _) = store
            .add_command("test-account", computer, &request)
            .await
            .unwrap();
        store
            .command_output(&id, b"first second", b"error", false, false)
            .await
            .unwrap();
        // An earlier provider read completes after a new lease owner has advanced.
        store
            .command_output(&id, b"first", b"err", false, false)
            .await
            .unwrap();
        let read = store
            .command("test-account", computer, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read.stdout, b"first second");
        assert_eq!(read.stderr, b"error");
        store
            .command_output(&id, b"first second", b"error", true, true)
            .await
            .unwrap();
        store
            .command_output(&id, b"first second", b"error", false, false)
            .await
            .unwrap();
        assert!(
            store
                .command("test-account", computer, &id)
                .await
                .unwrap()
                .unwrap()
                .out_gap
        );
        store.command_end(&id, "succeeded", Some(0)).await.unwrap();
        store
            .command_output(&id, b"first second extra", b"error extra", true, true)
            .await
            .unwrap();
        let read = store
            .command("test-account", computer, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read.stdout, b"first second");
        assert_eq!(read.stderr, b"error");
        assert!(read.out_gap);
        assert_eq!(read.status, "succeeded");
        let request = Request {
            idempotency_key: "raw-cursors".into(),
            ..request
        };
        let (id, _) = store
            .add_command("test-account", computer, &request)
            .await
            .unwrap();
        store
            .command_output_scoped(
                &id,
                b"clean",
                b"",
                false,
                false,
                Some(CommandCursor {
                    stdout: 100,
                    stderr: 0,
                }),
            )
            .await
            .unwrap();
        // A stale worker can have longer sanitized output but an older guest cursor.
        store
            .command_output_scoped(
                &id,
                b"stale private output",
                b"",
                false,
                false,
                Some(CommandCursor {
                    stdout: 99,
                    stderr: 0,
                }),
            )
            .await
            .unwrap();
        let read = store
            .command("test-account", computer, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read.stdout, b"clean");
        assert_eq!(read.options["source_stdout"], 100);
        store
            .command_output_scoped(
                &id,
                b"clean next",
                b"",
                false,
                false,
                Some(CommandCursor {
                    stdout: 120,
                    stderr: 0,
                }),
            )
            .await
            .unwrap();
        let read = store
            .command("test-account", computer, &id)
            .await
            .unwrap()
            .unwrap();
        assert_eq!(read.stdout, b"clean next");
        assert_eq!(read.options["source_stdout"], 120);

        task.abort();
    }
}
