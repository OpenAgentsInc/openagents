//! CLI placement over the shared durable remote executor.
use crate::programmatic::Context;
use coder_cloud::{Mode, Placement, Record, Spec, State, Store, boat_backend::Boat, drive};
use serde_json::{Value, json};
use std::sync::atomic::AtomicBool;
use std::time::Duration;

pub fn execute(
    command: &str,
    args: &[String],
    context: &Context,
    emit: &mut dyn FnMut(Value),
) -> Result<Value, String> {
    if context.approvals.is_some() {
        return Err(
            "Remote execution requires an explicit cloud delegation from an ungated host.".into(),
        );
    }
    let store = Store::under(context.root.join("remote"));
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .map_err(|_| "Cannot start the cloud runtime.")?;
    if command == "remote" {
        match args {
            [op] if op == "list" => {
                return serde_json::to_value(store.list()?)
                    .map_err(|_| "Cannot encode remote jobs.".into());
            }
            [op, id] if op == "status" => {
                return serde_json::to_value(store.read(id)?)
                    .map_err(|_| "Cannot encode the remote job.".into());
            }
            [op, id] if op == "cancel" => {
                store.cancel(id)?;
                if let Ok(lease) = store.lease(id) {
                    let mut record = lease.read(id)?;
                    return run(&runtime, &lease, &mut record, context, emit);
                }
                return Ok(json!({"job":id,"cancel_requested":true}));
            }
            [op, id, flag, message] if op == "steer" && flag == "--message" => {
                let record = store.read(id)?;
                runtime.block_on(async {
                    let backend = Boat::from_env(&record.spec.credential_names).await?;
                    backend.steer(&record, message).await
                })?;
                return Ok(json!({"job":id,"steered":true}));
            }
            [op, id] if op == "follow" => {
                let lease = store.lease(id)?;
                let mut record = lease.read(id)?;
                return run(&runtime, &lease, &mut record, context, emit);
            }
            _ => {
                return Err(
                    "Use remote list, status ID, follow ID, cancel ID, or steer ID --message TEXT."
                        .into(),
                );
            }
        }
    }
    let (id, spec) = parse(args, context)?;
    let lease = store.lease(&id)?;
    let mut record = if lease.exists() {
        let record = lease.read(&id)?;
        if record.spec != spec {
            return Err("This remote job ID already belongs to a different task. Follow it or choose another ID.".into());
        }
        record
    } else {
        let record = Record::new(&id, spec)?;
        lease.save(&record)?;
        record
    };
    run(&runtime, &lease, &mut record, context, emit)
}
fn run(
    runtime: &tokio::runtime::Runtime,
    lease: &coder_cloud::Lease,
    record: &mut Record,
    context: &Context,
    emit: &mut dyn FnMut(Value),
) -> Result<Value, String> {
    let id = record.id.clone();
    let task = record.spec.task.clone();
    let name = format!(
        "{}@{}",
        record.spec.agent,
        match record.spec.placement {
            Placement::Boat => "boat",
            Placement::Gce => "gce",
        }
    );
    emit(
        json!({"event":"remote_job","job":id,"placement":record.spec.placement,"mode":record.spec.mode,"state":record.state,"resource":record.resource}),
    );
    emit(json!({"event":"delegation","id":id,"name":name,"task":task,
        "update":{"event":"tool","name":"remote_delegate","input":{"placement":record.spec.placement,"mode":record.spec.mode},"output":null,"running":true}}));
    let cancel = AtomicBool::new(false);
    let flag = context.canceled.as_deref().unwrap_or(&cancel);
    let result = if record.state.terminal() && record.cleanup_complete {
        Ok(())
    } else {
        runtime.block_on(async {
        match record.spec.placement {
            Placement::Boat=>{let backend=Boat::from_env(&record.spec.credential_names).await?;
                drive(&backend,lease,record,flag,Duration::from_secs(2),&mut |event| {
                    emit(json!({"event":"delegation","id":id,"name":name,"task":task,"update":event}));
                }).await
            },
            Placement::Gce=>Err("The GCE adapter is not installed in this build.".into())
        }
    })
    };
    emit(json!({"event":"delegation","id":id,"name":name,"task":task,
        "update":{"event":"tool","name":"remote_delegate","input":null,"output":{"state":record.state,"cleanup_complete":record.cleanup_complete,"error":result.as_ref().err()},"running":false}}));
    if let Err(error) = result {
        emit(
            json!({"event":"remote_job","job":id,"state":record.state,"error":error,"resource":record.resource}),
        );
        return Err(format!(
            "{error} Remote job: {id}. Use remote follow {id} to reconnect or verify cleanup."
        ));
    }
    let reply = record
        .result
        .as_ref()
        .and_then(|v| v["reply"].as_str())
        .unwrap_or("");
    let out = json!({"event":"finished","job":id,"state":record.state,"resource":record.resource,"reply":reply,"result":record.result,"usage":record.usage,"cleanup_complete":record.cleanup_complete});
    if record.state == State::Failed {
        return Err(format!(
            "{} Remote job: {id}.",
            record
                .error
                .as_deref()
                .unwrap_or("Remote execution failed.")
        ));
    }
    Ok(out)
}
fn take(args: &mut Vec<String>, name: &str) -> Result<Option<String>, String> {
    if let Some(i) = args.iter().position(|s| s == name) {
        if i + 1 >= args.len() {
            return Err(format!("{name} requires a value."));
        }
        args.remove(i);
        Ok(Some(args.remove(i)))
    } else {
        Ok(None)
    }
}
fn parse(args: &[String], context: &Context) -> Result<(String, Spec), String> {
    let mut args = args.to_vec();
    let placement = match take(&mut args, "--on")?.as_deref() {
        Some("boat") => Placement::Boat,
        Some("gce" | "cloud") => Placement::Gce,
        _ => return Err("Use --on boat or --on gce.".into()),
    };
    let mode = match take(&mut args, "--mode")?.as_deref() {
        Some("integrated") => Mode::Integrated,
        Some("coder") => Mode::Coder,
        None => {
            if placement == Placement::Boat {
                Mode::Integrated
            } else {
                Mode::Coder
            }
        }
        _ => return Err("Use --mode integrated or --mode coder.".into()),
    };
    let task = take(&mut args, "--task")?.ok_or("Supply --task TEXT.")?;
    let id = take(&mut args, "--job")?
        .or(take(&mut args, "--session")?)
        .unwrap_or_else(|| atif::log::session_id(atif::now_ms()));
    let timeout = take(&mut args, "--timeout")?
        .map(|s| s.parse::<u64>())
        .transpose()
        .map_err(|_| "Invalid remote deadline.")?
        .unwrap_or(3600);
    let model = take(&mut args, "--model")?;
    let reasoning = take(&mut args, "--reasoning")?;
    let size = take(&mut args, "--size")?.unwrap_or("default".into());
    let template = take(&mut args, "--template")?;
    if !matches!(size.as_str(), "small" | "default" | "large" | "xlarge") {
        return Err("Unknown remote machine size.".into());
    }
    let mut credential_names = vec![];
    while let Some(name) = take(&mut args, "--credential-env")? {
        if matches!(
            name.as_str(),
            "BOAT_API_KEY" | "GOOGLE_APPLICATION_CREDENTIALS"
        ) {
            return Err("Cloud control credentials cannot be sent to an agent.".into());
        }
        credential_names.push(name);
    }
    if args.len() != 1 {
        return Err("Use delegate AGENT --task TEXT --on boat|gce and supported options.".into());
    }
    let spec = Spec {
        placement,
        mode,
        agent: args.remove(0),
        task,
        model,
        reasoning,
        cwd: context.cwd.clone(),
        timeout_seconds: timeout,
        size,
        template,
        credential_names,
    };
    spec.validate()?;
    if spec.mode == Mode::Integrated {
        if !matches!(
            spec.agent.as_str(),
            "codex"
                | "claude-code"
                | "claude"
                | "pi"
                | "opencode"
                | "prime-agent"
                | "prime"
                | "kimi"
                | "kimi-code"
                | "mistral"
        ) {
            return Err("Unknown Boat integrated agent.".into());
        }
    } else if spec.agent != "codex"
        && spec.agent != "microcoder"
        && (spec.model.is_some() || spec.reasoning.is_some())
    {
        return Err("Explicit remote runtime model settings require codex or microcoder.".into());
    }
    coder_cloud::validate_id(&id)?;
    Ok((id, spec))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn context() -> Context {
        Context {
            root: "fixture-state".into(),
            cwd: "fixture-workspace".into(),
            environment: Default::default(),
            input: None,
            canceled: None,
            approvals: None,
        }
    }
    #[test]
    fn explicit_placement_keeps_agent_model_and_credentials_separate() {
        let words = [
            "codex",
            "--task",
            "Review the patch",
            "--on",
            "boat",
            "--mode",
            "integrated",
            "--model",
            "chosen-model",
            "--credential-env",
            "OPENAI_API_KEY",
        ];
        let (_, spec) = parse(&words.map(String::from), &context()).unwrap();
        assert_eq!(spec.agent, "codex");
        assert_eq!(spec.model.as_deref(), Some("chosen-model"));
        assert_eq!(spec.credential_names, ["OPENAI_API_KEY"]);
        assert!(
            parse(
                &[
                    "codex",
                    "--task",
                    "test",
                    "--on",
                    "gce",
                    "--mode",
                    "integrated"
                ]
                .map(String::from),
                &context()
            )
            .is_err()
        );
        assert!(
            parse(
                &[
                    "codex",
                    "--task",
                    "test",
                    "--on",
                    "boat",
                    "--credential-env",
                    "BOAT_API_KEY"
                ]
                .map(String::from),
                &context()
            )
            .is_err()
        );
    }
}
