//! The `workflow` tool. It submits an inline script as one workflow run.

use std::future::Future;
use std::path::Path;
use std::pin::Pin;
use std::sync::Arc;
use std::sync::atomic::AtomicBool;

use serde::Deserialize;
use serde_json::{Value, json};
use workflow_run::{RunRequest, ThreadCall, ThreadResult, ThreadRunner};

use crate::plugin_tools::GenerationProvider;

pub const TOOL: &str = "workflow";

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Arguments {
    script: String,
    #[serde(default)]
    args: Option<Value>,
}

pub fn definition() -> Value {
    json!({
        "type": "function",
        "function": {
            "name": TOOL,
            "description": "Start a workflow run from an inline script. Use only when the person asked for a workflow, a fan-out, or multi-agent orchestration. The script is plain JavaScript and begins with export const meta. Pass args when the script reads them.",
            "parameters": {
                "type": "object",
                "additionalProperties": false,
                "required": ["script"],
                "properties": {
                    "script": {"type": "string", "minLength": 1},
                    "args": {}
                }
            }
        }
    })
}

pub async fn execute(
    arguments: Value,
    cwd: &Path,
    provider: Option<GenerationProvider>,
    cancel: &Arc<AtomicBool>,
) -> Result<Value, String> {
    let arguments: Arguments =
        serde_json::from_value(arguments).map_err(|_| "Pass the workflow script.".to_string())?;
    if arguments.script.trim().is_empty() {
        return Err("Pass the workflow script.".into());
    }
    let runner = match provider {
        Some(provider) => model_runner(provider, cwd),
        None => dead_runner(),
    };
    let mut request = RunRequest::new(arguments.script).args(arguments.args.unwrap_or(Value::Null));
    request.cancel = Some(Arc::clone(cancel));
    let outcome = session().start(request, runner).await;
    Ok(json!({
        "workflow_run_id": outcome.id,
        "status": outcome.status,
        "result": outcome.result.as_ref().map(|result| json!({
            "type": result.kind,
            "error_type": result.error_type,
            "message": result.message,
        })),
        "value": outcome.value,
        "notice": outcome.notice,
        "events": outcome.events,
    }))
}

fn session() -> &'static workflow_run::Session {
    use std::sync::OnceLock;
    static SESSION: OnceLock<workflow_run::Session> = OnceLock::new();
    SESSION.get_or_init(|| workflow_run::Session::new(None))
}

fn dead_runner() -> ThreadRunner {
    Arc::new(|_call: ThreadCall| {
        Box::pin(async { ThreadResult::dead() })
            as Pin<Box<dyn Future<Output = ThreadResult> + Send>>
    })
}

fn model_runner(provider: GenerationProvider, cwd: &Path) -> ThreadRunner {
    let cwd = cwd.to_path_buf();
    Arc::new(move |call: ThreadCall| {
        let provider = provider.clone();
        let cwd = cwd.clone();
        Box::pin(async move {
            if call.cancel.load(std::sync::atomic::Ordering::Relaxed) {
                return ThreadResult::dead();
            }
            let directory = cwd.display();
            let shared = format!(
                "You are one thread in a workflow run. Answer the request. The working directory is {directory}. Share this session's files. Do not start a workflow run."
            );
            let system = match &call.agent {
                Some(agent) if !agent.system.trim().is_empty() => {
                    format!("{}\n{shared}", agent.system)
                }
                _ => shared,
            };
            let model = call.model.clone().unwrap_or_else(|| provider.model.clone());
            let mut request = openrouter::ChatRequest::new(
                &model,
                vec![
                    openrouter::Message::system(system),
                    openrouter::Message::user(call.prompt),
                ],
            );
            if let Some(effort) = call.effort.as_deref().or(provider.effort.as_deref()) {
                request = request.effort(effort);
            }
            if let Some(schema) = call.schema {
                return match provider
                    .client
                    .structured::<Value>(request, "thread", schema)
                    .await
                {
                    Ok(structured) => ThreadResult {
                        value: Some(structured.value),
                        cost_sats: 0,
                    },
                    Err(_) => ThreadResult::dead(),
                };
            }
            match provider.client.chat(&request).await {
                Ok(response) => {
                    let text = response
                        .choices
                        .first()
                        .and_then(|choice| choice.message.content.clone())
                        .unwrap_or_default();
                    ThreadResult {
                        value: Some(Value::String(text)),
                        cost_sats: 0,
                    }
                }
                Err(_) => ThreadResult::dead(),
            }
        }) as Pin<Box<dyn Future<Output = ThreadResult> + Send>>
    })
}
