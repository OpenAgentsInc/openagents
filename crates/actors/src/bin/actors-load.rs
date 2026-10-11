//! Explicit opt-in database load probe for the example actor; never selects a production DSN.
use actors::{example, *};
use serde_json::json;
use std::{sync::Arc, time::Instant};
#[tokio::main]
async fn main() {
    if let Err(e) = run().await {
        eprintln!("{}", e.message);
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.first().map(String::as_str) != Some("--confirm-scratch") {
        println!(
            "actors-load --confirm-scratch [operations=100] [concurrency=4] [actors=1]\nSet ACTORS_TEST_DATABASE_URL to an isolated database. This creates example actors and leaves their records for inspection."
        );
        return Ok(());
    }
    let read = |i: usize, default: usize| max_arg(args.get(i).map(String::as_str), default);
    let operations = read(1, 100)?;
    let concurrency = read(2, 4)?.min(32);
    let actor_count = read(3, 1)?;
    let dsn = std::env::var("ACTORS_TEST_DATABASE_URL").map_err(|_| {
        ActorError::new(
            "bad_args",
            "Set ACTORS_TEST_DATABASE_URL to an isolated database.",
        )
    })?;
    let store = PgStore::new(
        Pool::new(&dsn, concurrency)?,
        Arc::new(example::registry()?),
    );
    store.migrate().await?;
    let mut nonce = [0u8; 8];
    getrandom::fill(&mut nonce)
        .map_err(|_| ActorError::new("unavailable", "Could not create the run ID."))?;
    let workspace = format!("actor-load-{}", u64::from_le_bytes(nonce));
    let caller = Caller {
        principal: "load-probe".into(),
        workspace_id: workspace.clone(),
        account_id: Some("load-probe".into()),
        role: Role::Owner,
        executor: None,
    };
    let semaphore = Arc::new(tokio::sync::Semaphore::new(concurrency));
    let mut tasks = tokio::task::JoinSet::new();
    let start = Instant::now();
    for n in 0..operations {
        let permit = semaphore
            .clone()
            .acquire_owned()
            .await
            .map_err(|_| ActorError::new("unavailable", "The probe stopped."))?;
        let store = store.clone();
        let caller = caller.clone();
        tasks.spawn(async move {
            let _permit = permit;
            let started = Instant::now();
            let reply = store
                .call(
                    &caller,
                    ActionRequest {
                        id: ActorId {
                            workspace_id: caller.workspace_id.clone(),
                            actor_type: "example.counter".into(),
                            key: format!("counter-{}", n % actor_count),
                        },
                        message: Envelope {
                            name: "add@1".into(),
                            args: json!({"delta":1}),
                            origin: Origin::Action,
                        },
                        input: Some(json!({"initial":0})),
                        idempotency_key: Some(format!("operation-{n}")),
                        expected_version: None,
                        fence: None,
                    },
                )
                .await;
            (started.elapsed().as_micros() as u64, reply)
        });
    }
    let mut latencies = Vec::new();
    let mut failed = 0;
    while let Some(result) = tasks.join_next().await {
        match result {
            Ok((latency, Ok(_))) => latencies.push(latency),
            _ => failed += 1,
        }
    }
    latencies.sort_unstable();
    let percentile = |p: usize| {
        latencies
            .get(latencies.len().saturating_sub(1) * p / 100)
            .copied()
    };
    println!(
        "{}",
        serde_json::to_string_pretty(
            &json!({"workspace":workspace,"attempted":operations,"completed":latencies.len(),"failed":failed,"concurrency":concurrency,"actors":actor_count,"elapsed_ms":start.elapsed().as_millis(),"p50_us":percentile(50),"p99_us":percentile(99)})
        )?
    );
    if failed > 0 {
        return Err(ActorError::new(
            "probe_failed",
            "Some requests failed. Inspect the database and retry the probe.",
        ));
    }
    Ok(())
}
fn max_arg(value: Option<&str>, default: usize) -> Result<usize> {
    let n = match value {
        Some(value) => value
            .parse()
            .map_err(|_| ActorError::new("bad_args", "Use a positive integer."))?,
        None => default,
    };
    if !(1..=100_000).contains(&n) {
        return Err(ActorError::new(
            "bad_args",
            "Choose a value between 1 and 100000.",
        ));
    }
    Ok(n)
}
