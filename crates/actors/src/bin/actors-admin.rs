//! Local operator tool. Database credentials come only from the environment.
use actors::{example, postgres::PgStore, *};
use serde_json::{Value, json};
use std::{io::Read, sync::Arc};

fn argument(args: &[String], n: usize, name: &str) -> Result<String> {
    args.get(n)
        .cloned()
        .ok_or_else(|| ActorError::new("bad_args", format!("Provide {name}.")))
}
fn input() -> Result<Value> {
    let mut bytes = Vec::new();
    std::io::stdin()
        .take(128 * 1024 + 1)
        .read_to_end(&mut bytes)
        .map_err(|_| ActorError::new("bad_args", "The input could not be read."))?;
    if bytes.len() > 128 * 1024 {
        return Err(ActorError::new("bad_args", "The input is too large."));
    }
    Ok(serde_json::from_slice(&bytes)?)
}
#[tokio::main]
async fn main() {
    if let Err(error) = run().await {
        eprintln!("{}", error.message);
        std::process::exit(1);
    }
}
async fn run() -> Result<()> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.is_empty() || args[0] == "--help" || args[0] == "help" {
        println!(
            "actors-admin COMMAND [WORKSPACE TYPE KEY]\n\nCommands: migrate, contract, list, inspect, history, block, unblock, destroy, export, call, enqueue, tick, retry-inbox, resolve-work, resolve-effect\n\nSet ACTORS_DATABASE_URL to a local socket or authenticated proxy connection.\nSet ACTORS_OPERATOR for the audit identity and ACTORS_ACCOUNT_ID for private actors.\ncall: append MESSAGE; stdin is {{\"args\":...,\"input\":...,\"idempotency_key\":...}}.\nenqueue: append MESSAGE; stdin is {{\"args\":...,\"idempotency_key\":...}}.\nretry-inbox: WORKSPACE TYPE KEY SEQUENCE.\nresolve-work/effect: WORKSPACE UID ITEM; stdin includes retry, outcome, and expected_epoch (work) or expected_attempt (effect).\nThe bundled registry contains only example.counter. Applications supply their own registry."
        );
        return Ok(());
    }
    let registry = Arc::new(example::registry()?);
    if args[0] == "contract" {
        println!("{}", serde_json::to_string_pretty(&registry.contract())?);
        return Ok(());
    }
    let dsn = std::env::var("ACTORS_DATABASE_URL")
        .map_err(|_| ActorError::new("bad_args", "Set ACTORS_DATABASE_URL."))?;
    let pool = Pool::new(&dsn, 4)?;
    let store = PgStore::new(pool, registry);
    if args[0] == "migrate" {
        store.migrate().await?;
        println!("{{\"ok\":true}}");
        return Ok(());
    }
    if args[0] == "tick" {
        let report = json!({"alarms":store.fire_alarms(32).await?,"expired_work":store.expire_work(32).await?,"expired_effects":store.expire_effects(32).await?,"messages":store.dispatch_once(32).await?});
        println!("{}", serde_json::to_string_pretty(&report)?);
        return Ok(());
    }
    let ws = argument(&args, 1, "a workspace")?;
    let caller = Caller {
        principal: std::env::var("ACTORS_OPERATOR").unwrap_or_else(|_| "local-operator".into()),
        workspace_id: ws.clone(),
        account_id: std::env::var("ACTORS_ACCOUNT_ID").ok(),
        role: Role::Admin,
        executor: None,
    };
    if args[0] == "list" {
        println!(
            "{}",
            serde_json::to_string_pretty(
                &store
                    .list(&caller, args.get(2).map(String::as_str), 100)
                    .await?
            )?
        );
        return Ok(());
    }
    if args[0] == "resolve-work" || args[0] == "resolve-effect" {
        let uid = argument(&args, 2, "an actor UID")?;
        let item = argument(&args, 3, "an item ID")?;
        let body = input()?;
        let retry = body
            .get("retry")
            .and_then(Value::as_bool)
            .ok_or_else(|| ActorError::new("bad_args", "Provide retry as true or false."))?;
        let outcome = body.get("outcome").cloned();
        if args[0] == "resolve-work" {
            let epoch = body
                .get("expected_epoch")
                .and_then(Value::as_u64)
                .ok_or_else(|| {
                    ActorError::new(
                        "bad_args",
                        "Provide expected_epoch from the item being resolved.",
                    )
                })?;
            store
                .resolve_work(&caller, &uid, &item, epoch, retry, outcome)
                .await?;
        } else {
            let attempt = body
                .get("expected_attempt")
                .and_then(Value::as_u64)
                .and_then(|n| u32::try_from(n).ok())
                .ok_or_else(|| {
                    ActorError::new(
                        "bad_args",
                        "Provide expected_attempt from the operation being resolved.",
                    )
                })?;
            store
                .resolve_effect(&caller, &uid, &item, attempt, retry, outcome)
                .await?;
        }
        println!("{{\"ok\":true}}");
        return Ok(());
    }
    let id = ActorId {
        workspace_id: ws,
        actor_type: argument(&args, 2, "an actor type")?,
        key: argument(&args, 3, "an actor key")?,
    };
    let output = match args[0].as_str() {
        "inspect" => serde_json::to_value(store.inspect(&caller, &id).await?)?,
        "history" => serde_json::to_value(store.history(&caller, &id, 0, 100).await?)?,
        "export" => store.export(&caller, &id).await?,
        "block" => {
            store.admin_block(&caller, &id, true).await?;
            json!({"ok":true})
        }
        "unblock" => {
            store.admin_block(&caller, &id, false).await?;
            json!({"ok":true})
        }
        "destroy" => {
            store.destroy(&caller, &id).await?;
            json!({"ok":true})
        }
        "retry-inbox" => {
            let seq = argument(&args, 4, "a message sequence")?
                .parse::<u64>()
                .map_err(|_| ActorError::new("bad_args", "Provide a positive message sequence."))?;
            store.retry_inbox(&caller, &id, seq).await?;
            json!({"ok":true})
        }
        "call" => {
            let body = input()?;
            let name = argument(&args, 4, "a message name")?;
            serde_json::to_value(
                store
                    .call(
                        &caller,
                        ActionRequest {
                            id,
                            message: Envelope {
                                name,
                                args: body.get("args").cloned().unwrap_or(Value::Null),
                                origin: Origin::Action,
                            },
                            input: body.get("input").cloned(),
                            idempotency_key: body
                                .get("idempotency_key")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            expected_version: body.get("expected_version").and_then(Value::as_u64),
                        },
                    )
                    .await?,
            )?
        }
        "enqueue" => {
            let body = input()?;
            let name = argument(&args, 4, "a message name")?;
            serde_json::to_value(
                store
                    .enqueue(
                        &caller,
                        &id,
                        Envelope {
                            name,
                            args: body.get("args").cloned().unwrap_or(Value::Null),
                            origin: Origin::Inbox,
                        },
                        body.get("idempotency_key").and_then(Value::as_str),
                    )
                    .await?,
            )?
        }
        _ => {
            return Err(ActorError::new(
                "bad_args",
                "That command is not available. Run actors-admin --help.",
            ));
        }
    };
    println!("{}", serde_json::to_string_pretty(&output)?);
    Ok(())
}
