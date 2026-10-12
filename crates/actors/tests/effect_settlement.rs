//! Effect settlement across processes: a holder that loses its claim is
//! never re-dispatched, and the answer it finally produces files as
//! evidence (`result.settled`, `EffectSettled`), never as an outcome. Runs
//! only with `ACTORS_TEST_DATABASE_URL` set to a scratch database.
use actors::{example, postgres::PgStore, *};
use serde_json::{Value, json};
use std::{
    io::{BufRead, BufReader},
    process::{Child, Command, Stdio},
    sync::Arc,
};

const BIN: &str = env!("CARGO_BIN_EXE_actors-crash");

fn dsn() -> Option<String> {
    match std::env::var("ACTORS_TEST_DATABASE_URL") {
        Ok(dsn) => Some(dsn),
        Err(_) => {
            eprintln!("Skipping effect-settlement test: ACTORS_TEST_DATABASE_URL is unset.");
            None
        }
    }
}

fn unique(prefix: &str) -> String {
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    format!("{prefix}-{:x}", u64::from_le_bytes(nonce))
}

fn store(dsn: &str) -> (PgStore, Pool) {
    let pool = Pool::new(dsn, 4).unwrap();
    (
        PgStore::new(pool.clone(), Arc::new(example::registry().unwrap())),
        pool,
    )
}

fn owner(ws: &str) -> Caller {
    Caller {
        principal: "account:settle-owner".into(),
        workspace_id: ws.into(),
        account_id: Some("settle-owner".into()),
        role: Role::Owner,
        executor: None,
    }
}

fn admin(ws: &str) -> Caller {
    Caller {
        role: Role::Admin,
        principal: "operator".into(),
        ..owner(ws)
    }
}

/// Stage one reconcile-policy effect through `charge@1`; the reply is its id.
async fn stage(store: &PgStore, ws: &str, kind: &str, key: &str) -> (ActorId, String) {
    let id = ActorId {
        workspace_id: ws.into(),
        actor_type: "example.counter".into(),
        key: "paid".into(),
    };
    let reply = store
        .call(
            &owner(ws),
            ActionRequest {
                id: id.clone(),
                message: Envelope {
                    name: "charge@1".into(),
                    args: json!({"kind": kind}),
                    origin: Origin::Action,
                },
                input: Some(json!({})),
                idempotency_key: Some(key.into()),
                expected_version: None,
                fence: None,
            },
        )
        .await
        .unwrap();
    (id, reply.reply.as_str().unwrap().to_owned())
}

/// An `actors-crash effect-hold` process, its claim read from its first line.
fn spawn_holder(dsn: &str, kind: &str) -> (Child, Value) {
    let mut child = Command::new(BIN)
        .args(["effect-hold", kind])
        .env("ACTORS_TEST_DATABASE_URL", dsn)
        .stdout(Stdio::piped())
        .spawn()
        .unwrap();
    let mut line = String::new();
    BufReader::new(child.stdout.take().unwrap())
        .read_line(&mut line)
        .unwrap();
    let claim: Value = serde_json::from_str(&line).unwrap();
    assert_eq!(claim["claimed"], true, "nothing pending to claim");
    (child, claim)
}

fn kill(child: &mut Child) {
    child.kill().unwrap();
    child.wait().unwrap();
}

async fn effect_row(pool: &Pool, uid: &str) -> (String, i32, Value) {
    let row = pool
        .acquire()
        .await
        .unwrap()
        .query_one(
            "SELECT state,attempts,result FROM actor.effects WHERE uid=$1",
            &[&uid],
        )
        .await
        .unwrap();
    (
        row.get(0),
        row.get(1),
        row.get::<_, Option<Value>>(2).unwrap_or_default(),
    )
}

/// Fast-forward the claim's deadline, then run one expiry pass. The row's
/// new state is asserted by the caller, not by the pass's count, which
/// covers every workspace.
async fn expire(store: &PgStore, pool: &Pool, uid: &str, id: &str) {
    pool.acquire()
        .await
        .unwrap()
        .execute(
            "UPDATE actor.effects SET claimed_until=1 WHERE uid=$1 AND effect_id=$2",
            &[&uid, &id],
        )
        .await
        .unwrap();
    store.expire_effects(8).await.unwrap();
}

async fn inbox_names(pool: &Pool, uid: &str) -> Vec<String> {
    pool.acquire()
        .await
        .unwrap()
        .query(
            "SELECT message->>'name' FROM actor.inbox WHERE uid=$1",
            &[&uid],
        )
        .await
        .unwrap()
        .iter()
        .map(|row| row.get(0))
        .collect()
}

/// A paid effect claimed by a process that dies mid-dispatch: it goes
/// uncertain with the lost claim kept as custody proof, is never
/// re-dispatched, files the late answer as settlement evidence, refuses a
/// forged one, and ends only by operator resolve.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_killed_holder_files_settlement_and_the_paid_effect_dispatches_once() {
    let Some(dsn) = dsn() else { return };
    let (store, pool) = store(&dsn);
    store.migrate().await.unwrap();
    let ws = unique("settle-crash");
    // Kinds are run-unique: leftover pending effects from earlier runs can
    // never collide with this test's claims.
    let kind = format!("test.crash-{ws}");
    let (_id, effect_id) = stage(&store, &ws, &kind, "stage-1").await;

    let (mut holder, claim) = spawn_holder(&dsn, &kind);
    let uid = claim["uid"].as_str().unwrap().to_owned();
    let token = claim["token"].as_str().unwrap().to_owned();
    assert_eq!(claim["attempts"], 1, "the first dispatch");
    kill(&mut holder);

    // The dead claim expires on the next pass; the lost claim is retained
    // as custody proof, with the reason it went uncertain.
    expire(&store, &pool, &uid, &effect_id).await;
    let (state, attempts, result) = effect_row(&pool, &uid).await;
    assert_eq!(state, "uncertain");
    assert_eq!(attempts, 1, "never dispatched a second time");
    assert_eq!(result["error"], "lease_expired");
    assert_eq!(result["uncertain"], true);
    assert_eq!(result["lost_claim"]["token"], token);
    assert_eq!(result["lost_claim"]["attempt"], 1);
    assert!(
        store
            .claim_effects_for_kinds(8, &[kind.clone()])
            .await
            .unwrap()
            .is_empty(),
        "an uncertain effect is never re-dispatched"
    );

    // A forged report is refused; the real holder's late answer files.
    let forged = store
        .settle_effect(
            &uid,
            &effect_id,
            "not-the-token",
            EffectSettlement::Done(json!({})),
        )
        .await
        .unwrap_err();
    assert_eq!(forged.code, "stale_claim");
    store
        .settle_effect(
            &uid,
            &effect_id,
            &token,
            EffectSettlement::Done(json!({"receipt": "rcpt-1"})),
        )
        .await
        .unwrap();
    let (_, _, result) = effect_row(&pool, &uid).await;
    assert_eq!(result["settled"]["kind"], "done");
    assert_eq!(result["settled"]["outcome"]["receipt"], "rcpt-1");
    assert!(
        inbox_names(&pool, &uid)
            .await
            .contains(&"EffectSettled".to_string()),
        "the actor is told the late answer landed"
    );
    let (state, _, _) = effect_row(&pool, &uid).await;
    assert_eq!(state, "uncertain", "evidence does not decide");

    // The same settle is a no-op; a disagreeing one conflicts.
    store
        .settle_effect(
            &uid,
            &effect_id,
            &token,
            EffectSettlement::Done(json!({"receipt": "rcpt-1"})),
        )
        .await
        .unwrap();
    let conflict = store
        .settle_effect(
            &uid,
            &effect_id,
            &token,
            EffectSettlement::Failed("lost".into()),
        )
        .await
        .unwrap_err();
    assert_eq!(conflict.code, "settle_conflict");

    // Only an operator decision ends it.
    store
        .resolve_effect(
            &admin(&ws),
            &uid,
            &effect_id,
            1,
            false,
            Some(json!({"receipt": "rcpt-1"})),
        )
        .await
        .unwrap();
    let (state, _, _) = effect_row(&pool, &uid).await;
    assert_eq!(state, "completed");
    let late = store
        .settle_effect(&uid, &effect_id, &token, EffectSettlement::Done(json!({})))
        .await
        .unwrap_err();
    assert_eq!(late.code, "conflict");
}

/// The report a live executor produces after its claim expired: finish is
/// refused `stale_claim`, then `settle_effect` files the same answer as
/// evidence — the sequence the runtime's late-answer fallback runs.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn a_late_answer_lands_as_settlement_evidence() {
    let Some(dsn) = dsn() else { return };
    let (store, pool) = store(&dsn);
    store.migrate().await.unwrap();
    let ws = unique("settle-late");
    let kind = format!("test.late-{ws}");
    let (_id, effect_id) = stage(&store, &ws, &kind, "stage-1").await;

    let claim = store
        .claim_effects_for_kinds(1, &[kind.clone()])
        .await
        .unwrap()
        .into_iter()
        .next()
        .unwrap();
    expire(&store, &pool, &claim.uid, &effect_id).await;

    let refused = store
        .finish_effect(&claim.uid, &effect_id, &claim.token, json!({"done": true}))
        .await
        .unwrap_err();
    assert_eq!(refused.code, "stale_claim");
    store
        .settle_effect(
            &claim.uid,
            &effect_id,
            &claim.token,
            EffectSettlement::Done(json!({"done": true})),
        )
        .await
        .unwrap();
    let (state, _, result) = effect_row(&pool, &claim.uid).await;
    assert_eq!(state, "uncertain");
    assert_eq!(result["settled"]["kind"], "done");
    assert_eq!(result["settled"]["outcome"]["done"], true);

    // On a live claim nothing is uncertain to settle.
    let (_id, pending_id) = stage(&store, &ws, &kind, "stage-2").await;
    let early = store
        .settle_effect(
            &claim.uid,
            &pending_id,
            "any",
            EffectSettlement::Done(json!({})),
        )
        .await
        .unwrap_err();
    assert_eq!(early.code, "conflict");
}
