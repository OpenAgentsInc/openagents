//! Mac jobs through the actor runtime (#11253, step 1), behind
//! `OPENAGENTS_WEB_MAC_JOBS_ACTORS=1`.
//!
//! With the flag on, a new job is a `mac.job` actor
//! ([`mac_jobs::actor`]) in the account's own workspace: the submit route
//! creates it (the request's `Idempotency-Key` names it, so a retry is the
//! same job), the linked Mac claims it from the work queue with a long poll
//! and reports through calls fenced by its claim, and the owner's answers
//! come from the host's own pages only. Jobs made before the switch stay in
//! the chat store and finish there: every read here asks the actor first
//! and then the old store, and the Mac drains old jobs before it claims.
//! With the flag off nothing here runs.
//!
//! A job's files keep the old store's layout (its parts under the
//! account's folder); the actor records each part after its bytes are kept.

use std::sync::Arc;

use ::mac_jobs::Spec;
use ::mac_jobs::actor::{self as job_actor};
use actors::{ActionRequest, ActorError, ActorId, Envelope, Origin, WorkFence};
use axum::http::StatusCode;
use serde_json::{Value, json};

use crate::App;
use crate::actors_host::Host;
use crate::chat_store::{Error, Store, account_owner};
use crate::mac_jobs::{self, Answered, Job, Refused, Submitted};
use crate::phone_api::Item;

/// The most actor jobs listed.
const LIST: usize = 64;
/// The most jobs waiting for a Mac at once.
const MAX_WAITING: usize = 16;

/// One account's actor jobs.
pub(crate) struct Jobs {
    host: Arc<Host>,
    pub account: String,
    pub workspace: String,
}

fn failed(error: &ActorError) -> Error {
    eprintln!("openagents-web: mac jobs (actors): {error}");
    Error::Unavailable("Mac jobs are unavailable right now.")
}

impl Jobs {
    /// One account's jobs in `workspace` on `host` (tests).
    #[cfg(test)]
    pub(crate) fn new(host: Arc<Host>, account: &str, workspace: &str) -> Self {
        Self {
            host,
            account: account.into(),
            workspace: workspace.into(),
        }
    }
}

/// The account's actor jobs, when Mac jobs run through actors here.
pub(crate) async fn jobs(app: &App, account: &str) -> Option<Jobs> {
    let host = app.config.actors.clone().filter(|host| host.mac_jobs)?;
    match host.home(account).await {
        Ok(workspace) => Some(Jobs {
            host,
            account: account.to_owned(),
            workspace,
        }),
        Err(error) => {
            eprintln!("openagents-web: mac jobs: no workspace for the account: {error}");
            None
        }
    }
}

/// A job read from its actor's view: the shape the old store keeps.
fn job_of(view: Value) -> Option<Job> {
    serde_json::from_value(view).ok()
}

impl Jobs {
    fn id(&self, key: &str) -> ActorId {
        ActorId {
            workspace_id: self.workspace.clone(),
            actor_type: job_actor::TYPE.into(),
            key: key.into(),
        }
    }

    #[allow(clippy::too_many_arguments)]
    async fn call(
        &self,
        caller: &actors::Caller,
        key: &str,
        message: &str,
        args: Value,
        input: Option<Value>,
        idempotency_key: Option<String>,
        fence: Option<WorkFence>,
    ) -> actors::Result<Value> {
        self.host
            .store
            .call(
                caller,
                ActionRequest {
                    id: self.id(key),
                    message: Envelope {
                        name: message.into(),
                        args,
                        origin: Origin::Action,
                    },
                    input,
                    idempotency_key,
                    expected_version: None,
                    fence,
                },
            )
            .await
            .map(|reply| reply.reply)
    }

    fn owner(&self) -> actors::Caller {
        Host::owner_caller(&self.account, &self.workspace)
    }

    fn site(&self) -> actors::Caller {
        Host::host_caller(&self.account, &self.workspace)
    }

    /// Job `id`, when it is an actor job of this account.
    pub(crate) async fn load(&self, id: &str) -> Result<Option<Job>, Error> {
        if !::mac_jobs::valid_job_id(id) {
            return Ok(None);
        }
        match self.host.store.view(&self.owner(), &self.id(id)).await {
            Ok(reply) => Ok(job_of(reply.view)),
            Err(error) if error.code == "not_found" => Ok(None),
            Err(error) => Err(failed(&error)),
        }
    }

    /// The account's actor jobs, newest first.
    pub(crate) async fn list(&self) -> Result<Vec<Job>, Error> {
        let found = self
            .host
            .store
            .list_own(&self.owner(), job_actor::TYPE, LIST)
            .await
            .map_err(|e| failed(&e))?;
        Ok(found
            .into_iter()
            .filter_map(|(_, reply)| job_of(reply.view))
            .collect())
    }

    /// The account's actor jobs as their readers see them, newest first:
    /// what the live pages watch. Views, not versions: a Mac's report with
    /// nothing new (its heartbeat) changes the version but not the view.
    pub(crate) async fn fingerprint(&self) -> Result<String, Error> {
        let found = self
            .host
            .store
            .list_own(&self.owner(), job_actor::TYPE, LIST)
            .await
            .map_err(|e| failed(&e))?;
        Ok(found
            .iter()
            .map(|(id, reply)| format!("{}:{}", id.key, reply.view))
            .collect::<Vec<_>>()
            .join(","))
    }

    /// Queue `spec` on `computer` as a new job; `request` (the caller's
    /// `Idempotency-Key`) names it, so the same request again is the same
    /// job.
    pub(crate) async fn submit(
        &self,
        spec: Spec,
        computer: &str,
        request: Option<&str>,
    ) -> Result<Result<Submitted, Refused>, Error> {
        let key = match request {
            Some(key) => key.to_owned(),
            None => {
                let bytes: [u8; 16] = secp256k1::rand::random();
                bytes.iter().map(|b| format!("{b:02x}")).collect()
            }
        };
        let id = job_actor::job_id(&self.account, &key);
        if self.load(&id).await?.is_none() {
            let waiting = self
                .list()
                .await?
                .iter()
                .filter(|job| job.state == mac_jobs::JobState::Waiting)
                .count();
            if waiting >= MAX_WAITING {
                return Ok(Err(Refused::Busy));
            }
        }
        let input = json!({"id": id, "computer": computer, "spec": spec});
        match self
            .call(
                &self.site(),
                &id,
                "submit@1",
                json!({}),
                Some(input),
                Some(format!("submit:{key}")),
                None,
            )
            .await
        {
            Ok(reply) => {
                let submitted: job_actor::Submitted = serde_json::from_value(reply)
                    .map_err(|_| Error::Corrupt("A Mac job is invalid."))?;
                Ok(Ok(Submitted {
                    id: submitted.id,
                    computer: submitted.computer,
                    kind: submitted.kind,
                    approval: submitted.approval,
                    online: false,
                }))
            }
            Err(error) if error.code == "idempotency_conflict" => Ok(Err(Refused::Invalid(
                "That request key was already used for a different job.".into(),
            ))),
            Err(error) if error.code == "bad_args" => Ok(Err(Refused::Invalid(error.message))),
            Err(error) => Err(failed(&error)),
        }
    }

    /// Stop job `id`; `None` when it isn't an actor job.
    pub(crate) async fn cancel(&self, id: &str) -> Result<Option<()>, Error> {
        if self.load(id).await?.is_none() {
            return Ok(None);
        }
        self.call(&self.owner(), id, "cancel@1", json!({}), None, None, None)
            .await
            .map(|_| Some(()))
            .map_err(|e| failed(&e))
    }

    /// The owner answers job `id`'s question on `via` (`web`, `phone`).
    pub(crate) async fn answer(
        &self,
        id: &str,
        question: &str,
        approve: bool,
        via: &str,
    ) -> Result<Answered, Error> {
        if self.load(id).await?.is_none() {
            return Ok(Answered::Unknown);
        }
        let message = if approve { "approve@1" } else { "deny@1" };
        match self
            .call(
                &self.site(),
                id,
                message,
                json!({"question": question, "via": via}),
                None,
                None,
                None,
            )
            .await
        {
            Ok(reply) if reply == "recorded" => Ok(Answered::Recorded),
            Ok(_) => Ok(Answered::NotAsking),
            Err(error) => Err(failed(&error)),
        }
    }

    /// Keep part `part` of file `name` of job `id`, sent by `computer`
    /// under its claim `fence`: the bytes first, then the actor's record.
    #[allow(clippy::too_many_arguments)]
    pub(crate) async fn save_part(
        &self,
        store: &Store,
        computer: &str,
        id: &str,
        fence: WorkFence,
        name: &str,
        part: u32,
        last: bool,
        bytes: Vec<u8>,
    ) -> Result<Result<(), (StatusCode, &'static str, String)>, Error> {
        let Some(job) = self.load(id).await? else {
            return Ok(Err((
                StatusCode::NOT_FOUND,
                "unknown",
                "There is no such job.".into(),
            )));
        };
        if job.computer != computer || job.state.finished() || job.cancel {
            return Ok(Err((
                StatusCode::NOT_FOUND,
                "unknown",
                "There is no such job.".into(),
            )));
        }
        let owner = account_owner(&self.account);
        let key = mac_jobs::part_key(&owner, id, name, part)?;
        let size = bytes.len() as u64;
        if !job_actor::valid_artifact(name) || size > job_actor::PART_BYTES {
            return Ok(Err((
                StatusCode::BAD_REQUEST,
                "invalid",
                "That part can't be kept.".into(),
            )));
        }
        let generation = store
            .read_key(&key)
            .await?
            .map(|(_, generation)| generation);
        store.write_key(&key, bytes, generation.as_deref()).await?;
        let caller = Host::mac_caller(&self.account, &self.workspace, computer);
        match self
            .call(
                &caller,
                id,
                "artifact@1",
                json!({"name": name, "part": part, "size": size, "last": last}),
                None,
                None,
                Some(fence),
            )
            .await
        {
            Ok(_) => Ok(Ok(())),
            Err(error) if error.code == "bad_args" => {
                Ok(Err((StatusCode::BAD_REQUEST, "invalid", error.message)))
            }
            Err(error) if !error.retryable => {
                Ok(Err((StatusCode::CONFLICT, "fenced", error.message)))
            }
            Err(error) => Err(failed(&error)),
        }
    }
}

// ------------------------------------------- both stores, for the routes

/// Every job of the account, newest first: actor jobs and the old store's.
pub(crate) async fn all(app: &App, account: &str) -> Result<Vec<Job>, Error> {
    let owner = account_owner(account);
    let mut jobs = mac_jobs::list(&app.config.chat_store, &owner).await?;
    if let Some(actor_jobs) = jobs_of(app, account).await {
        jobs.extend(actor_jobs?);
    }
    jobs.sort_by_key(|job| std::cmp::Reverse(job.created_unix));
    Ok(jobs)
}

async fn jobs_of(app: &App, account: &str) -> Option<Result<Vec<Job>, Error>> {
    Some(jobs(app, account).await?.list().await)
}

/// Job `id` of the account, from whichever store has it.
pub(crate) async fn find(app: &App, account: &str, id: &str) -> Result<Option<Job>, Error> {
    if let Some(jobs) = jobs(app, account).await
        && let Some(job) = jobs.load(id).await?
    {
        return Ok(Some(job));
    }
    mac_jobs::load(&app.config.chat_store, &account_owner(account), id).await
}

/// Stop job `id`; `None` when there is no such job.
pub(crate) async fn cancel(app: &App, account: &str, id: &str) -> Result<Option<()>, Error> {
    if let Some(jobs) = jobs(app, account).await
        && let Some(done) = jobs.cancel(id).await?
    {
        return Ok(Some(done));
    }
    mac_jobs::cancel(&app.config.chat_store, &account_owner(account), id).await
}

/// The owner's answer to job `id`'s question.
pub(crate) async fn answer(
    app: &App,
    account: &str,
    id: &str,
    question: &str,
    approve: bool,
    via: &str,
) -> Result<Answered, Error> {
    if let Some(jobs) = jobs(app, account).await {
        match jobs.answer(id, question, approve, via).await? {
            Answered::Unknown => {}
            answered => return Ok(answered),
        }
    }
    mac_jobs::answer_question(
        &app.config.chat_store,
        &account_owner(account),
        id,
        question,
        approve,
        via,
    )
    .await
}

/// The jobs as items on their Macs' boards, from both stores.
pub(crate) async fn board_items(app: &App, account: &str) -> Vec<(String, Item)> {
    match all(app, account).await {
        Ok(jobs) => mac_jobs::board_items_of(&jobs),
        Err(_) => Vec::new(),
    }
}

/// The phone's Approve, Deny, or Stop on a job's board item.
pub(crate) async fn act(
    app: &App,
    account: &str,
    id: &str,
    action: &str,
    question: Option<&str>,
) -> Result<Result<(), (StatusCode, &'static str, &'static str)>, Error> {
    match action {
        "approve" | "deny" => Ok(
            match answer(
                app,
                account,
                id,
                question.unwrap_or_default(),
                action == "approve",
                "phone",
            )
            .await?
            {
                Answered::Recorded => Ok(()),
                Answered::NotAsking => Err((
                    StatusCode::CONFLICT,
                    "not_asking",
                    "That job isn't waiting for an answer anymore.",
                )),
                Answered::Unknown => Err((
                    StatusCode::NOT_FOUND,
                    "unknown",
                    "That job isn't there anymore.",
                )),
            },
        ),
        "stop" => Ok(match cancel(app, account, id).await? {
            Some(()) => Ok(()),
            None => Err((
                StatusCode::NOT_FOUND,
                "unknown",
                "That job isn't there anymore.",
            )),
        }),
        _ => Ok(Err((
            StatusCode::CONFLICT,
            "unsupported",
            "A Mac job takes Approve, Deny, or Stop.",
        ))),
    }
}

/// A file job `id` made, from whichever store has the job.
pub(crate) async fn artifact(
    app: &App,
    account: &str,
    id: &str,
    name: &str,
) -> Result<Option<(u64, axum::body::Body)>, Error> {
    let Some(job) = find(app, account, id).await? else {
        return Ok(None);
    };
    mac_jobs::artifact_body_of(&app.config.chat_store, &account_owner(account), &job, name).await
}

#[cfg(test)]
mod db_tests {
    //! Against a scratch PostgreSQL (`ACTORS_TEST_DATABASE_URL`), with the
    //! account store's membership tables made there for the test.
    use super::*;
    use crate::mac_jobs::JobState;
    use ::mac_jobs::Recipe;
    use std::time::Duration;

    async fn setup() -> Option<(Arc<Host>, String, String)> {
        let dsn = std::env::var("ACTORS_TEST_DATABASE_URL").ok()?;
        let host = Host::open_for_test(&dsn).await;
        let connection = host.pool().acquire().await.unwrap();
        connection
            .batch_execute(
                "BEGIN; SELECT pg_advisory_xact_lock(4242);
                 CREATE SCHEMA IF NOT EXISTS workspace;
                 CREATE TABLE IF NOT EXISTS workspace.workspaces (id text PRIMARY KEY, kind text);
                 CREATE TABLE IF NOT EXISTS workspace.memberships (workspace_id text, account_id text,
                   role text, status text, PRIMARY KEY (workspace_id, account_id)); COMMIT;",
            )
            .await
            .unwrap();
        let bytes: [u8; 8] = secp256k1::rand::random();
        let n: String = bytes.iter().map(|b| format!("{b:02x}")).collect();
        let (account, workspace) = (format!("acct_{n}"), format!("ws_{n}"));
        connection
            .execute(
                "INSERT INTO workspace.workspaces VALUES ($1, 'personal')",
                &[&workspace],
            )
            .await
            .unwrap();
        connection
            .execute(
                "INSERT INTO workspace.memberships VALUES ($1, $2, 'owner', 'active')",
                &[&workspace, &account],
            )
            .await
            .unwrap();
        Some((host, account, workspace))
    }

    fn spec(recipe: Recipe) -> Spec {
        Spec {
            repo: "OpenAgentsInc/openagents".into(),
            git_ref: "main".into(),
            recipe,
            args: Vec::new(),
        }
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn jobs_run_through_the_actor_and_answers_come_only_from_the_site() {
        let Some((host, account, workspace)) = setup().await else {
            eprintln!("Skipping: ACTORS_TEST_DATABASE_URL is unset.");
            return;
        };
        assert_eq!(host.home(&account).await.unwrap(), workspace);
        let jobs = Jobs::new(host.clone(), &account, &workspace);
        let first = jobs
            .submit(spec(Recipe::IosTestflight), "Studio", Some("key-1"))
            .await
            .unwrap()
            .unwrap();
        let again = jobs
            .submit(spec(Recipe::IosTestflight), "Studio", Some("key-1"))
            .await
            .unwrap()
            .unwrap();
        assert_eq!(first.id, again.id, "a retry is the same job");
        assert!(first.approval);
        assert_eq!(jobs.list().await.unwrap().len(), 1);
        // The Mac claims it and asks.
        let mac = Host::mac_caller(&account, &workspace, "Studio");
        let claim = host
            .store
            .claim_work_wait(
                &mac,
                ::mac_jobs::actor::QUEUE,
                Some(&::mac_jobs::actor::target("Studio")),
                1,
                Duration::from_secs(2),
            )
            .await
            .unwrap()
            .remove(0);
        let fence = WorkFence {
            item_id: claim.item_id.clone(),
            epoch: claim.epoch,
        };
        jobs.call(
            &mac,
            &first.id,
            "report@1",
            json!({"ask": {"id": "q1", "text": "Upload?", "subject": "s"}}),
            None,
            None,
            Some(fence.clone()),
        )
        .await
        .unwrap();
        let job = jobs.load(&first.id).await.unwrap().unwrap();
        assert_eq!(job.state, JobState::Asking);
        assert_eq!(job.question.as_ref().unwrap().id, "q1");
        // The account through the API can't answer; the site can, once.
        assert!(
            jobs.call(
                &jobs.owner(),
                &first.id,
                "approve@1",
                json!({"question": "q1", "via": "web"}),
                None,
                None,
                None
            )
            .await
            .is_err()
        );
        assert_eq!(
            jobs.answer(&first.id, "q1", false, "web").await.unwrap(),
            Answered::Recorded
        );
        assert_eq!(
            jobs.answer(&first.id, "q1", true, "web").await.unwrap(),
            Answered::NotAsking
        );
        assert_eq!(
            jobs.answer("mjob0000000000000000000000000000000f", "q1", true, "web")
                .await
                .unwrap(),
            Answered::Unknown
        );
        // A file part, kept and recorded under the claim.
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        jobs.save_part(
            &store,
            "Studio",
            &first.id,
            fence.clone(),
            "log.txt",
            0,
            true,
            b"hello".to_vec(),
        )
        .await
        .unwrap()
        .unwrap();
        let job = jobs.load(&first.id).await.unwrap().unwrap();
        let (size, _) =
            crate::mac_jobs::artifact_body_of(&store, &account_owner(&account), &job, "log.txt")
                .await
                .unwrap()
                .unwrap();
        assert_eq!(size, 5);
        // A stale epoch can't add a file.
        let stale = WorkFence {
            epoch: fence.epoch + 1,
            ..fence
        };
        assert!(
            jobs.save_part(
                &store,
                "Studio",
                &first.id,
                stale,
                "x.txt",
                0,
                true,
                vec![1]
            )
            .await
            .unwrap()
            .is_err()
        );
        assert!(jobs.cancel(&first.id).await.unwrap().is_some());
        assert!(
            jobs.cancel("mjob0000000000000000000000000000000f")
                .await
                .unwrap()
                .is_none()
        );
        // Live pages watch the versions.
        let before = jobs.fingerprint().await.unwrap();
        jobs.submit(spec(Recipe::DesktopCapture), "Studio", None)
            .await
            .unwrap()
            .unwrap();
        assert_ne!(jobs.fingerprint().await.unwrap(), before);
        // Another account sees none of it.
        let other = Jobs::new(host.clone(), "acct_other", &workspace);
        assert!(other.load(&first.id).await.unwrap().is_none());
    }

    /// The actor routes are mounted in the site's router itself: their own
    /// path parameters, our authentication (none here: refused, never a 500),
    /// and the contract.
    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn the_site_serves_the_actor_routes_behind_its_auth() {
        use axum::body::{Body, to_bytes};
        use axum::http::{Request, header};
        use tower::ServiceExt;
        let Some((host, _, workspace)) = setup().await else {
            return;
        };
        let dir = tempfile::tempdir().unwrap();
        let mut config = crate::Config::development(dir.path().join("tasks"));
        config.actors = Some(host);
        let router = crate::router(config);
        let ask = |method: &str, uri: String, body: &str| {
            Request::builder()
                .method(method)
                .uri(uri)
                .header(header::HOST, "127.0.0.1:4300")
                .header(header::CONTENT_TYPE, "application/json")
                .header(header::COOKIE, "oa_cloud_session=sess_forged")
                .body(Body::from(body.to_owned()))
                .unwrap()
        };
        let claim = router
            .clone()
            .oneshot(ask(
                "POST",
                format!("/v1/w/{workspace}/work/mac-jobs/claim"),
                r#"{"max":1}"#,
            ))
            .await
            .unwrap();
        let status = claim.status();
        let body = to_bytes(claim.into_body(), 1 << 20).await.unwrap();
        assert_eq!(
            status,
            StatusCode::NOT_FOUND,
            "{}",
            String::from_utf8_lossy(&body)
        );
        assert!(String::from_utf8_lossy(&body).contains("\"group\":\"actor\""));
        let contract = router
            .oneshot(ask("GET", "/v1/actors/contract.json".into(), ""))
            .await
            .unwrap();
        assert_eq!(contract.status(), StatusCode::OK);
        let body = to_bytes(contract.into_body(), 1 << 20).await.unwrap();
        assert!(String::from_utf8_lossy(&body).contains("mac.job"));
    }

    #[tokio::test(flavor = "multi_thread", worker_threads = 2)]
    async fn queued_work_runs_only_while_the_account_still_belongs() {
        let Some((host, account, workspace)) = setup().await else {
            return;
        };
        let site = Host::host_caller(&account, &workspace);
        let fresh = host.revalidate(&site).await.unwrap();
        assert_eq!(
            fresh.role,
            actors::Role::Service,
            "the site's own alarms keep its authority"
        );
        let mac = Host::mac_caller(&account, &workspace, "Studio");
        let fresh = host.revalidate(&mac).await.unwrap();
        assert!(
            fresh.executor.is_none(),
            "queued work never carries a grant"
        );
        let forged = actors::Caller {
            principal: "account:someone-else".into(),
            ..site.clone()
        };
        assert!(host.revalidate(&forged).await.is_err());
        host.pool()
            .acquire()
            .await
            .unwrap()
            .execute(
                "UPDATE workspace.memberships SET status='revoked' WHERE account_id=$1",
                &[&account],
            )
            .await
            .unwrap();
        // The membership cache is short; a revoked member stops within it.
        tokio::time::sleep(Duration::from_secs(21)).await;
        assert!(host.revalidate(&site).await.is_err());
        host.shutdown().await;
    }
}
