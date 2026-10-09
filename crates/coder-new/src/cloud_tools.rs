//! Structured terminal cloud tools over the same executor as the CLI.
use crate::{bundled_runtime::RuntimeEvent, cloud_settings::Configuration};
use coder_cloud::{Mode, Placement, Store};
use serde::Deserialize;
use serde_json::{Value, json};
use std::{
    collections::BTreeSet,
    path::PathBuf,
    sync::{Arc, atomic::AtomicBool},
};
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Delegate {
    agent: String,
    task: String,
    mode: Option<Mode>,
    model: Option<String>,
    reasoning: Option<String>,
    job: Option<String>,
    #[serde(default)]
    credential_names: Vec<String>,
    #[serde(default)]
    workspace_paths: Vec<String>,
    #[serde(default)]
    include: Vec<String>,
    #[serde(default)]
    no_workspace: bool,
    timeout_seconds: Option<u64>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Job {
    operation: String,
    job: String,
    message: Option<String>,
}
pub fn definition(p: Placement, control: bool) -> Value {
    let prefix = if p == Placement::Boat { "boat" } else { "gce" };
    let (name, description, parameters) = if control {
        (
            format!("{prefix}_job"),
            "Read, follow, cancel, steer, or continue a saved remote job. Continuation preserves its workspace. Applying a patch is an explicit caller CLI action.",
            json!({"type":"object","properties":{"operation":{"type":"string","enum":["status","follow","cancel","steer","continue","artifacts"]},"job":{"type":"string"},"message":{"type":"string"}},"required":["operation","job"],"additionalProperties":false}),
        )
    } else {
        (
            format!("{prefix}_delegate"),
            "Delegate requested cloud work to an exact remote agent ID: agent@boat or agent@gce. Boat supports integrated agents or the Coder runtime; GCE runs Coder. Use configured credential variable names only. Select workspace paths before sending a large repository. Results retain a patch, ATIF, logs, placement, state, and usage.",
            json!({"type":"object","properties":{"agent":{"type":"string"},"task":{"type":"string"},"mode":{"type":"string","enum":if p==Placement::Boat{vec!["integrated","coder"]}else{vec!["coder"]}},"model":{"type":"string"},"reasoning":{"type":"string"},"job":{"type":"string"},"credential_names":{"type":"array","items":{"type":"string"}},"workspace_paths":{"type":"array","items":{"type":"string"}},"include":{"type":"array","items":{"type":"string"}},"no_workspace":{"type":"boolean"},"timeout_seconds":{"type":"integer","minimum":1,"maximum":43200}},"required":["agent","task"],"additionalProperties":false}),
        )
    };
    json!({"type":"function","function":{"name":name,"description":description,"parameters":parameters}})
}
pub fn arguments(
    p: Placement,
    config: &Configuration,
    value: Value,
    targets: &BTreeSet<String>,
) -> Result<Vec<String>, String> {
    if !config.enabled || !config.valid(p) {
        return Err("This cloud plugin is disabled or has invalid settings.".into());
    }
    let a: Delegate =
        serde_json::from_value(value).map_err(|_| "Invalid cloud delegation fields.")?;
    let suffix = if p == Placement::Boat {
        "@boat"
    } else {
        "@gce"
    };
    let agent = a
        .agent
        .strip_suffix(suffix)
        .ok_or("Use an exact agent ID with its @boat or @gce placement.")?;
    if !targets.is_empty() && !targets.contains(agent) {
        return Err(
            "The remote agent isn't the one named in this request, so nothing was started.".into(),
        );
    }
    let credentials = if a.credential_names.is_empty() {
        config.credential_names.clone()
    } else {
        a.credential_names
    };
    if credentials
        .iter()
        .any(|n| !config.credential_names.contains(n))
    {
        return Err(
            "A requested credential variable isn't allowed in this cloud plugin's settings.".into(),
        );
    }
    let mode = a.mode.unwrap_or(config.mode);
    if p == Placement::Gce && mode != Mode::Coder {
        return Err("GCE requires the Coder runtime.".into());
    }
    let mut args = vec![
        agent.into(),
        "--task".into(),
        a.task,
        "--on".into(),
        if p == Placement::Boat {
            "boat".into()
        } else {
            "gce".into()
        },
        "--mode".into(),
        if mode == Mode::Coder {
            "coder".into()
        } else {
            "integrated".into()
        },
        "--size".into(),
        config.size.clone(),
    ];
    for (name, value) in [
        ("--model", a.model),
        ("--reasoning", a.reasoning),
        ("--job", a.job),
        ("--template", config.template.clone()),
        ("--timeout", a.timeout_seconds.map(|n| n.to_string())),
    ] {
        if let Some(value) = value {
            args.extend([name.into(), value]);
        }
    }
    for n in credentials {
        args.extend(["--credential-env".into(), n]);
    }
    let paths = if a.workspace_paths.is_empty() {
        config.workspace_paths.clone()
    } else {
        a.workspace_paths
    };
    for p in paths {
        args.extend(["--workspace-path".into(), p]);
    }
    for p in a.include {
        args.extend(["--include".into(), p]);
    }
    if a.no_workspace {
        args.push("--no-workspace".into());
    }
    Ok(args)
}
pub async fn execute(
    p: Placement,
    control: bool,
    config: Configuration,
    root: PathBuf,
    cwd: PathBuf,
    value: Value,
    targets: &BTreeSet<String>,
    cancel: Arc<AtomicBool>,
    emit: &mut dyn FnMut(RuntimeEvent),
) -> Result<Value, String> {
    if !config.enabled {
        return Err("This cloud plugin is turned off.".into());
    }
    if crate::approval::gated()
        && !(control && matches!(value["operation"].as_str(), Some("status" | "artifacts")))
    {
        return Err("Remote execution requires an ungated host.".into());
    }
    let (command, args) = if control {
        let a: Job = serde_json::from_value(value).map_err(|_| "Invalid remote job fields.")?;
        let r = Store::under(root.join("remote")).read(&a.job)?;
        if r.spec.placement != p {
            return Err("This remote job belongs to another cloud backend.".into());
        }
        if !matches!(
            a.operation.as_str(),
            "status" | "follow" | "cancel" | "steer" | "continue" | "artifacts"
        ) {
            return Err("Unknown remote job operation.".into());
        }
        let mut args = vec![a.operation.clone(), a.job];
        if matches!(a.operation.as_str(), "steer" | "continue") {
            args.extend([
                if a.operation == "steer" {
                    "--message".into()
                } else {
                    "--task".into()
                },
                a.message
                    .ok_or("Supply the steering or continuation message.")?,
            ]);
        } else if a.message.is_some() {
            return Err("This remote job operation takes no message.".into());
        }
        ("remote", args)
    } else {
        ("delegate", arguments(p, &config, value, targets)?)
    };
    let (sender, mut receiver) = tokio::sync::mpsc::unbounded_channel();
    let worker = tokio::task::spawn_blocking(move || {
        let context = crate::programmatic::Context {
            root,
            cwd,
            environment: crate::programmatic::command_environment(|n| std::env::var(n).ok()),
            input: None,
            canceled: Some(cancel),
            approvals: None,
        };
        crate::cloud::execute(command, &args, &context, &mut |v| {
            let _ = sender.send(v);
        })
    });
    while let Some(v) = receiver.recv().await {
        if let Some(event) = crate::delegation_events::decode(&v, 0) {
            emit(event);
        }
    }
    worker
        .await
        .map_err(|_| "The cloud worker stopped unexpectedly. Follow the job to reconnect.")?
}
