//! Fenced actions, the long-poll claim, releasing cancelled work, and listing
//! one account's records. Runs only with `ACTORS_TEST_DATABASE_URL` set.
use actors::{
    core::{Actor, Ctx, Definition, Handles, Message, Registry},
    postgres::PgStore,
    *,
};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

/// A job that offers one work item and takes reports only from its claim.
struct Job;
#[derive(Serialize, Deserialize, Default)]
struct JobState {
    item: Option<String>,
    reports: Vec<(u64, String)>,
    expired: Vec<Value>,
}
impl Actor for Job {
    const TYPE: &'static str = "test.job";
    const STATE_VERSION: u32 = 1;
    const PRIVATE: bool = true;
    const VIEW_ACCESS: Access = Access::AccountOwner;
    type State = JobState;
    type Input = Value;
    fn create(_input: Value, _ctx: &mut Ctx) -> Result<JobState> {
        Ok(JobState::default())
    }
    fn wake(_state: &JobState) -> Result<Self> {
        Ok(Self)
    }
    fn view(state: &JobState, _caller: &Caller) -> Result<Value> {
        Ok(json!({"item": state.item, "reports": state.reports, "expired": state.expired}))
    }
}
#[derive(Deserialize)]
struct Offer {}
impl Message for Offer {
    const NAME: &'static str = "offer@1";
    const ACCESS: Access = Access::AccountOwner;
    type Reply = String;
}
impl Handles<Offer> for Job {
    fn handle(&mut self, state: &mut JobState, _: Offer, ctx: &mut Ctx) -> Result<String> {
        let item = ctx.work(WorkSpec {
            item_id: String::new(),
            queue: "jobs".into(),
            target: Some("mac".into()),
            payload: json!({}),
            lease_ms: 60_000,
            max_attempts: 3,
            retry: RetryPolicy::Idempotent,
        })?;
        state.item = Some(item.clone());
        Ok(item)
    }
}
#[derive(Deserialize)]
struct Report {
    text: String,
}
impl Message for Report {
    const NAME: &'static str = "report@1";
    const ACCESS: Access = Access::AccountOwner;
    type Reply = bool;
}
impl Handles<Report> for Job {
    fn handle(&mut self, state: &mut JobState, m: Report, ctx: &mut Ctx) -> Result<bool> {
        let fence = ctx
            .fence()
            .filter(|fence| state.item.as_deref() == Some(fence.item_id.as_str()))
            .ok_or_else(|| ActorError::new("forbidden", "Only the claim reports."))?
            .clone();
        state.reports.push((fence.epoch, m.text));
        Ok(fence.cancel)
    }
}
#[derive(Deserialize)]
struct Cancel {}
impl Message for Cancel {
    const NAME: &'static str = "cancel@1";
    const ACCESS: Access = Access::AccountOwner;
    type Reply = ();
}
impl Handles<Cancel> for Job {
    fn handle(&mut self, state: &mut JobState, _: Cancel, ctx: &mut Ctx) -> Result<()> {
        if let Some(item) = &state.item {
            ctx.cancel_work(item.clone())?;
        }
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(transparent)]
struct WorkExpired(Value);
impl Message for WorkExpired {
    const NAME: &'static str = "WorkExpired";
    const ACCESS: Access = Access::Internal;
    type Reply = ();
}
impl Handles<WorkExpired> for Job {
    fn handle(&mut self, state: &mut JobState, m: WorkExpired, _: &mut Ctx) -> Result<()> {
        state.expired.push(m.0);
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(transparent)]
struct WorkDone(#[allow(dead_code)] Value);
impl Message for WorkDone {
    const NAME: &'static str = "WorkDone";
    const ACCESS: Access = Access::Internal;
    type Reply = ();
}
impl Handles<WorkDone> for Job {
    fn handle(&mut self, _: &mut JobState, _: WorkDone, _: &mut Ctx) -> Result<()> {
        Ok(())
    }
}

fn registry() -> Registry {
    let mut registry = Registry::new();
    registry
        .register(
            Definition::<Job>::new()
                .message::<Offer>()
                .message::<Report>()
                .message::<Cancel>()
                .message::<WorkExpired>()
                .message::<WorkDone>(),
        )
        .unwrap();
    registry
}

fn person(ws: &str, account: &str) -> Caller {
    Caller {
        principal: format!("account:{account}"),
        workspace_id: ws.into(),
        account_id: Some(account.into()),
        role: Role::Owner,
        executor: None,
    }
}

fn mac(ws: &str, account: &str) -> Caller {
    Caller {
        executor: Some(ExecutorGrant {
            id: "mac:one".into(),
            queues: vec!["jobs".into()],
            targets: vec!["mac".into()],
            generation: 1,
            expires_at: i64::MAX / 2,
            max_claims: 1,
        }),
        ..person(ws, account)
    }
}

fn action(key: &str, message: &str, args: Value, fence: Option<WorkFence>) -> ActionRequest {
    ActionRequest {
        id: ActorId {
            workspace_id: String::new(),
            actor_type: "test.job".into(),
            key: key.into(),
        },
        message: Envelope {
            name: message.into(),
            args,
            origin: Origin::Action,
        },
        input: Some(json!({})),
        idempotency_key: None,
        expected_version: None,
        fence,
    }
}

fn at(ws: &str, mut request: ActionRequest) -> ActionRequest {
    request.id.workspace_id = ws.into();
    request
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn fenced_calls_long_poll_claims_cancel_release_and_own_listing() {
    let Ok(dsn) = std::env::var("ACTORS_TEST_DATABASE_URL") else {
        eprintln!("Skipping isolated PostgreSQL test: ACTORS_TEST_DATABASE_URL is unset.");
        return;
    };
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let ws = format!("fence-{:x}", u64::from_le_bytes(nonce));
    let pool = Pool::new(&dsn, 6).unwrap();
    let store = PgStore::new(pool.clone(), Arc::new(registry()));
    store.migrate().await.unwrap();
    let owner = person(&ws, "alice");
    let executor = mac(&ws, "alice");

    // A long-poll claim waits for work that arrives while it waits.
    let waiting = {
        let store = store.clone();
        let executor = executor.clone();
        tokio::spawn(async move {
            store
                .claim_work_wait(&executor, "jobs", Some("mac"), 1, Duration::from_secs(10))
                .await
        })
    };
    tokio::time::sleep(Duration::from_millis(400)).await;
    let item: String = serde_json::from_value(
        store
            .call(&owner, at(&ws, action("one", "offer@1", json!({}), None)))
            .await
            .unwrap()
            .reply,
    )
    .unwrap();
    let claim = waiting.await.unwrap().unwrap().remove(0);
    assert_eq!(claim.item_id, item);
    // An empty long poll returns empty when its wait ends.
    let started = std::time::Instant::now();
    let none = store
        .claim_work_wait(
            &executor,
            "jobs",
            Some("mac"),
            1,
            Duration::from_millis(600),
        )
        .await
        .unwrap();
    assert!(none.is_empty() && started.elapsed() >= Duration::from_millis(500));

    let fence = |epoch| {
        Some(WorkFence {
            item_id: item.clone(),
            epoch,
        })
    };
    // The claim's executor reports at its epoch, and that renews the lease.
    let reply = store
        .call(
            &executor,
            at(
                &ws,
                action(
                    "one",
                    "report@1",
                    json!({"text":"hello"}),
                    fence(claim.epoch),
                ),
            ),
        )
        .await
        .unwrap();
    assert_eq!(reply.reply, json!(false));
    // A wrong epoch, no fence, or the owner without a grant are refused.
    for (caller, fenced) in [
        (&executor, fence(claim.epoch + 1)),
        (&executor, None),
        (&owner, fence(claim.epoch)),
    ] {
        assert!(
            store
                .call(
                    caller,
                    at(&ws, action("one", "report@1", json!({"text":"no"}), fenced))
                )
                .await
                .is_err()
        );
    }
    // Another account's executor can't reach the record at all.
    assert!(
        store
            .call(
                &mac(&ws, "mallory"),
                at(
                    &ws,
                    action("one", "report@1", json!({"text":"no"}), fence(claim.epoch))
                )
            )
            .await
            .is_err()
    );

    // Cancelled: a fenced call still answers (so the executor learns it),
    // then the executor releases the claim, which ends it without
    // uncertainty and frees its capacity.
    store
        .call(&owner, at(&ws, action("one", "cancel@1", json!({}), None)))
        .await
        .unwrap();
    let reply = store
        .call(
            &executor,
            at(
                &ws,
                action(
                    "one",
                    "report@1",
                    json!({"text":"stopping"}),
                    fence(claim.epoch),
                ),
            ),
        )
        .await
        .unwrap();
    assert_eq!(reply.reply, json!(true));
    store
        .release_work(&executor, &claim.uid, &item, claim.epoch, "cancelled")
        .await
        .unwrap();
    let state: String = pool
        .acquire()
        .await
        .unwrap()
        .query_one("SELECT state FROM actor.work WHERE uid=$1", &[&claim.uid])
        .await
        .unwrap()
        .get(0);
    assert_eq!(state, "cancelled");
    assert!(
        store
            .call(
                &executor,
                at(
                    &ws,
                    action(
                        "one",
                        "report@1",
                        json!({"text":"late"}),
                        fence(claim.epoch)
                    )
                ),
            )
            .await
            .is_err(),
        "a released claim is fenced"
    );
    // The released claim no longer holds the executor's only slot.
    store
        .call(&owner, at(&ws, action("two", "offer@1", json!({}), None)))
        .await
        .unwrap();
    assert_eq!(
        store
            .claim_work_wait(&executor, "jobs", Some("mac"), 1, Duration::from_secs(5))
            .await
            .unwrap()
            .len(),
        1
    );
    store.dispatch_once(16).await.unwrap();
    store.dispatch_once(16).await.unwrap();

    // Listing: the account's own records only, newest first, with views.
    store
        .call(
            &person(&ws, "bob"),
            at(&ws, action("bobs", "offer@1", json!({}), None)),
        )
        .await
        .unwrap();
    let mine = store.list_own(&owner, "test.job", 10).await.unwrap();
    let keys: Vec<&str> = mine.iter().map(|(id, _)| id.key.as_str()).collect();
    assert_eq!(keys, ["two", "one"]);
    assert_eq!(mine[1].1.view["reports"].as_array().unwrap().len(), 2);
    assert_eq!(mine[1].1.view["expired"][0]["state"], "cancelled");
    assert!(store.list_own(&owner, "example.counter", 10).await.is_err());
}
