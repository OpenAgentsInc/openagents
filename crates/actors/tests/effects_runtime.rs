use actors::*;
use futures_util::future::BoxFuture;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::{sync::Arc, time::Duration};

struct EffectActor;
impl Actor for EffectActor {
    const TYPE: &'static str = "test.effects";
    const STATE_VERSION: u32 = 1;
    type State = Counts;
    type Input = ();
    fn create(_: (), _: &mut Ctx) -> Result<Counts> {
        Ok(Counts::default())
    }
    fn wake(_: &Counts) -> Result<Self> {
        Ok(Self)
    }
    fn view(state: &Counts, _: &Caller) -> Result<Value> {
        Ok(serde_json::to_value(state)?)
    }
}
#[derive(Default, Serialize, Deserialize)]
struct Counts {
    done: u64,
    failed: u64,
}
#[derive(Serialize, Deserialize)]
#[serde(transparent)]
struct Start(EffectSpec);
impl Message for Start {
    const NAME: &'static str = "start@1";
    type Reply = String;
}
impl Handles<Start> for EffectActor {
    fn handle(&mut self, _: &mut Counts, msg: Start, ctx: &mut Ctx) -> Result<String> {
        ctx.effect(msg.0)
    }
}
#[derive(Deserialize)]
#[serde(transparent)]
struct Done(Value);
impl Message for Done {
    const NAME: &'static str = "EffectDone";
    const ACCESS: Access = Access::Internal;
    type Reply = ();
}
impl Handles<Done> for EffectActor {
    fn handle(&mut self, state: &mut Counts, msg: Done, _: &mut Ctx) -> Result<()> {
        assert!(msg.0.is_object());
        state.done += 1;
        Ok(())
    }
}
#[derive(Deserialize)]
#[serde(transparent)]
struct Failed(Value);
impl Message for Failed {
    const NAME: &'static str = "EffectFailed";
    const ACCESS: Access = Access::Internal;
    type Reply = ();
}
impl Handles<Failed> for EffectActor {
    fn handle(&mut self, state: &mut Counts, msg: Failed, _: &mut Ctx) -> Result<()> {
        assert!(msg.0.is_object());
        state.failed += 1;
        Ok(())
    }
}
struct Echo;
impl EffectExecutor for Echo {
    fn execute<'a>(&'a self, effect: &'a ClaimedEffect) -> BoxFuture<'a, Result<Value>> {
        Box::pin(async move { Ok(effect.payload.clone()) })
    }
}
struct Hang;
impl EffectExecutor for Hang {
    fn execute<'a>(&'a self, _: &'a ClaimedEffect) -> BoxFuture<'a, Result<Value>> {
        Box::pin(std::future::pending())
    }
}
#[tokio::test]
async fn runtime_executes_supported_effects_and_keeps_timeout_uncertain() {
    let Ok(dsn) = std::env::var("ACTORS_TEST_DATABASE_URL") else {
        eprintln!("Skipping isolated PostgreSQL test: ACTORS_TEST_DATABASE_URL is unset.");
        return;
    };
    let mut nonce = [0; 8];
    getrandom::fill(&mut nonce).unwrap();
    let ws = format!("runtime-effects-{}", u64::from_le_bytes(nonce));
    let mut registry = Registry::new();
    registry
        .register(
            Definition::<EffectActor>::new()
                .message::<Start>()
                .message::<Done>()
                .message::<Failed>(),
        )
        .unwrap();
    let pool = Pool::new(&dsn, 4).unwrap();
    let store = PgStore::new(pool.clone(), Arc::new(registry));
    store.migrate().await.unwrap();
    let caller = Caller {
        principal: "test".into(),
        workspace_id: ws.clone(),
        account_id: None,
        role: Role::Owner,
        executor: None,
    };
    let actor = Client::new(store.clone(), caller)
        .actor::<EffectActor>("effects")
        .unwrap()
        .with_input(&())
        .unwrap();
    let make = |kind: &str, timeout_ms| {
        Start(EffectSpec {
            id: String::new(),
            kind: kind.into(),
            payload: json!({"value":42}),
            timeout_ms,
            max_attempts: 3,
            retry: RetryPolicy::Reconcile,
        })
    };
    actor
        .call(&make("test.echo", 1000), CallOptions::default())
        .await
        .unwrap();
    let hung = actor
        .call(&make("test.hang", 80), CallOptions::default())
        .await
        .unwrap()
        .reply;
    let unknown = actor
        .call(&make("newer.kind", 1000), CallOptions::default())
        .await
        .unwrap()
        .reply;
    let mut effects = Effects::new();
    effects.register("test.echo", Arc::new(Echo)).unwrap();
    effects.register("test.hang", Arc::new(Hang)).unwrap();
    let runtime = Runtime::new(store)
        .effects(effects)
        .config(RuntimeConfig {
            poll_interval: Duration::from_millis(20),
            listen: false,
            ..Default::default()
        })
        .start()
        .unwrap();
    tokio::time::timeout(Duration::from_secs(5), async {
        loop {
            let view = actor.view().await.unwrap().view;
            if view["done"] == 1 && view["failed"] == 1 {
                break;
            }
            tokio::time::sleep(Duration::from_millis(20)).await;
        }
    })
    .await
    .unwrap();
    let connection = pool.acquire().await.unwrap();
    let rows=connection.query("SELECT e.effect_id,e.state,e.attempts FROM actor.effects e JOIN actor.instances a USING(uid) WHERE a.workspace_id=$1",&[&ws]).await.unwrap();
    let find = |id: &str| {
        rows.iter()
            .find(|r| r.get::<_, String>("effect_id") == id)
            .unwrap()
    };
    assert_eq!(find(&hung).get::<_, String>("state"), "uncertain");
    assert_eq!(find(&hung).get::<_, i32>("attempts"), 1);
    assert_eq!(find(&unknown).get::<_, String>("state"), "pending");
    assert_eq!(find(&unknown).get::<_, i32>("attempts"), 0);
    drop(connection);
    runtime.shutdown().await;
}
