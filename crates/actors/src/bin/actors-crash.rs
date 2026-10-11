//! A separate process for the crash and restart tests (`tests/multiprocess.rs`).
//! It is killed with SIGKILL in the middle of its work, so nothing it holds
//! is released cleanly. It uses the bundled example registry and only the
//! scratch database in `ACTORS_TEST_DATABASE_URL`.
//!
//!   actors-crash executor WORKSPACE ACCOUNT EXECUTOR QUEUE TARGET
//!       Claim one item (waiting up to 10 s), print it as one JSON line, then
//!       heartbeat it every 200 ms until killed.
//!   actors-crash finish WORKSPACE ACCOUNT EXECUTOR QUEUE TARGET UID ITEM EPOCH
//!       Try to complete a claim; print `ok` or the refusal's code.
//!   actors-crash runtime MILLISECONDS
//!       Run the background runtime (inboxes, alarms, expiry) that long.
use actors::{example, http::Authenticator, postgres::PgStore, *};
use futures_util::future::BoxFuture;
use serde_json::json;
use std::{sync::Arc, time::Duration};

/// The tests' queued messages come from callers this process trusts as they
/// were saved; a host checks current membership here instead.
struct Trusting;
impl Authenticator for Trusting {
    fn authenticate<'a>(
        &'a self,
        _headers: &'a axum::http::HeaderMap,
        _workspace: &'a str,
    ) -> BoxFuture<'a, Result<Caller>> {
        Box::pin(async { Err(ActorError::new("unauthorized", "No HTTP here.")) })
    }
    fn revalidate<'a>(&'a self, caller: &'a Caller) -> BoxFuture<'a, Result<Caller>> {
        Box::pin(async move { Ok(caller.clone()) })
    }
}

fn executor(args: &[String]) -> Caller {
    Caller {
        principal: format!("executor:{}", args[3]),
        workspace_id: args[1].clone(),
        account_id: Some(args[2].clone()),
        role: Role::Service,
        executor: Some(ExecutorGrant {
            id: args[3].clone(),
            queues: vec![args[4].clone()],
            targets: vec![args[5].clone()],
            generation: 1,
            expires_at: i64::MAX / 2,
            max_claims: 1,
        }),
    }
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let dsn = std::env::var("ACTORS_TEST_DATABASE_URL").expect("ACTORS_TEST_DATABASE_URL");
    let registry = Arc::new(example::registry().expect("registry"));
    let store = actors::http::with_authenticator(
        PgStore::new(Pool::new(&dsn, 4).expect("pool"), registry),
        Arc::new(Trusting),
    );
    match args.first().map(String::as_str) {
        Some("executor") if args.len() == 6 => {
            let caller = executor(&args);
            let claimed = store
                .claim_work_wait(
                    &caller,
                    &args[4],
                    Some(&args[5]),
                    1,
                    Duration::from_secs(10),
                )
                .await
                .expect("claim");
            let Some(work) = claimed.into_iter().next() else {
                println!("{}", json!({"claimed": false}));
                return;
            };
            println!(
                "{}",
                json!({"claimed": true, "uid": work.uid, "item": work.item_id, "epoch": work.epoch})
            );
            loop {
                let _ = store
                    .heartbeat(&caller, &work.uid, &work.item_id, work.epoch, None)
                    .await;
                tokio::time::sleep(Duration::from_millis(200)).await;
            }
        }
        Some("finish") if args.len() == 9 => {
            let caller = executor(&args);
            let epoch: u64 = args[8].parse().expect("epoch");
            match store
                .finish_work(&caller, &args[6], &args[7], epoch, json!({"done": true}))
                .await
            {
                Ok(()) => println!("ok"),
                Err(error) => println!("{}", error.code),
            }
        }
        Some("runtime") if args.len() == 2 => {
            let millis: u64 = args[1].parse().expect("milliseconds");
            let runtime = Runtime::new(store)
                .config(RuntimeConfig {
                    poll_interval: Duration::from_millis(50),
                    ..RuntimeConfig::default()
                })
                .start()
                .expect("runtime");
            tokio::time::sleep(Duration::from_millis(millis)).await;
            println!("{}", json!(runtime.stats()));
            runtime.shutdown().await;
        }
        _ => {
            eprintln!("usage: see the file's header");
            std::process::exit(2);
        }
    }
}
