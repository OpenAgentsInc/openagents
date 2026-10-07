//! Boat's integrated-agent API and the headless Coder runtime.
use crate::{
    Backend, Mode, Observation, Record, Result, State, Task,
    runtime::{self, Credentials},
};
use base64::Engine;
use boat::{Client, Nullable, WaitOptions, models::*};
use serde_json::{Value, json};
use std::time::Duration;

pub struct Boat {
    pub client: Client,
    pub credentials: Credentials,
}
impl Boat {
    pub async fn from_env(names: &[String]) -> Result<Self> {
        let client = Client::from_env().await.map_err(|e| e.to_string())?;
        let credentials = Credentials::from_names(names, |n| std::env::var(n).ok())?;
        Ok(Self {
            client,
            credentials,
        })
    }
    fn resource<'a>(&self, r: &'a Record) -> Result<&'a str> {
        if r.binding
            .get("origin")
            .and_then(Value::as_str)
            .is_some_and(|v| v != self.client.origin())
        {
            return Err("The configured Boat origin differs from the retained job.".into());
        }
        r.resource
            .as_deref()
            .ok_or("The Boat sandbox is not yet known.".into())
    }
    pub async fn command(&self, r: &Record, command: String) -> Result<String> {
        match self
            .client
            .command(&CommandParams {
                sandbox_id: self.resource(r)?.into(),
                body: CommandRequest {
                    command,
                    timeout_seconds: Some(600),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?
        {
            CommandResponseBody::Finished(reply) if reply.success && reply.exit_code == Some(0) => {
                Ok(reply.stdout)
            }
            _ => Err("The remote setup or observation command failed.".into()),
        }
    }
    fn prompt(&self, r: &Record) -> String {
        format!(
            "{}\n\nWork in {}. OpenAgents remote job: {}.",
            r.spec.task,
            runtime::directory(r) + "/workspace",
            r.id
        )
    }
    pub async fn steer(&self, r: &Record, message: &str) -> Result<()> {
        if r.spec.mode != Mode::Integrated || r.state != State::Running {
            return Err("Steering requires a running Boat integrated-agent job.".into());
        }
        let task = r
            .remote_task
            .as_ref()
            .ok_or("The remote task is unknown.")?;
        self.client
            .steer(&SteerParams {
                sandbox_id: self.resource(r)?.into(),
                conversation: task.conversation.clone(),
                body: SteerRequest {
                    message: message.into(),
                    ..Default::default()
                },
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(())
    }
    async fn integrated_poll(&self, r: &Record) -> Result<Observation> {
        let task = r
            .remote_task
            .as_ref()
            .ok_or("The Boat prompt is unknown.")?;
        let page = self
            .client
            .events(&EventsParams {
                sandbox_id: self.resource(r)?.into(),
                conversation: task.conversation.clone(),
                cursor: r.cursor.clone(),
                sort: Some("asc".into()),
                limit: Some(100),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        let events = normalize(r, &page.events);
        let cursor = page
            .page_info
            .as_ref()
            .and_then(|p| p.next_cursor.clone())
            .or_else(|| {
                page.events.last().and_then(|e| {
                    Some(
                        base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(format!(
                            "{}:{}",
                            e.timestamp?,
                            e.id.as_ref()?
                        )),
                    )
                })
            });
        let more = page.page_info.as_ref().is_some_and(|p| p.has_more);
        let status = self
            .client
            .prompt_run_status(&PromptRunStatusParams {
                sandbox_id: self.resource(r)?.into(),
                prompt_id: task.id.clone(),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?
            .prompt_run;
        let end = if status.done && !more {
            if matches!(
                status.status.as_str(),
                "failed" | "cancelled" | "interrupted"
            ) {
                Some(Err(format!(
                    "The Boat agent ended with status {}.",
                    status.status
                )))
            } else {
                let reply = r
                    .events
                    .iter()
                    .chain(events.iter())
                    .filter(|v| v["event"] == "delta")
                    .filter_map(|v| v["text"].as_str())
                    .collect::<String>();
                Some(Ok(
                    json!({"reply":reply,"model":status.model,"conversation":task.conversation,"transport":"boat-integrated"}),
                ))
            }
        } else {
            None
        };
        Ok(Observation {
            events,
            cursor,
            end,
        })
    }
}
impl Backend for Boat {
    async fn provision(&self, r: &mut Record) -> Result<String> {
        if let Some(origin) = r.binding.get("origin").and_then(Value::as_str) {
            if origin != self.client.origin() {
                return Err("The configured Boat origin differs from the retained job.".into());
            }
        }
        r.binding = json!({"origin":self.client.origin()});
        let reply = self
            .client
            .create(&CreateParams {
                idempotency_key: Some(format!("oa-coder-{}", r.id)),
                body: Some(CreateSandboxRequest {
                    type_: Some(r.spec.size.clone()),
                    ttl_seconds: Nullable::Value(r.spec.timeout_seconds as i64),
                    no_env: Some(true),
                    env: Some(self.credentials.environment()),
                    from_: r.spec.template.clone(),
                    setup_script: Some(format!(
                        "mkdir -p {}",
                        boat::shell_quote(&(runtime::directory(r) + "/workspace"))
                    )),
                    ..Default::default()
                }),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        Ok(reply.sandbox.id)
    }
    async fn prepare(&self, r: &Record) -> Result<()> {
        let id = self.resource(r)?;
        let opts = WaitOptions {
            timeout: Duration::from_secs(r.spec.timeout_seconds.min(900)),
            ..Default::default()
        };
        self.client
            .wait_until_ready(id, &opts)
            .await
            .map_err(|e| e.to_string())?;
        if r.spec.template.is_some() {
            self.client
                .wait_until_hydrated(id, &opts)
                .await
                .map_err(|e| e.to_string())?;
        }
        let dir = runtime::directory(r);
        self.client
            .write_text(id, &format!("{dir}/task"), &r.spec.task)
            .await
            .map_err(|e| e.to_string())?;
        self.client
            .write_text(
                id,
                &format!("/tmp/oa-coder-{}.env", r.id),
                &self.credentials.shell(),
            )
            .await
            .map_err(|e| e.to_string())?;
        let script = runtime::prepare_script(r, &dir);
        self.command(r, script).await?;
        if r.spec.mode == Mode::Coder {
            self.client
                .write_text(
                    id,
                    &format!("{dir}/run.sh"),
                    &runtime::launch_script(r, &dir),
                )
                .await
                .map_err(|e| e.to_string())?;
        }
        Ok(())
    }
    async fn dispatch(&self, r: &Record) -> Result<Task> {
        if r.spec.mode == Mode::Integrated {
            let reply = self
                .client
                .prompt(&PromptParams {
                    sandbox_id: self.resource(r)?.into(),
                    body: PromptRequest {
                        provider: r.spec.agent.clone(),
                        prompt: self.prompt(r),
                        new: Some(true),
                        model: r
                            .spec
                            .model
                            .clone()
                            .map_or(Nullable::Unset, Nullable::Value),
                        reasoning_effort: r
                            .spec
                            .reasoning
                            .clone()
                            .map_or(Nullable::Unset, Nullable::Value),
                        ..Default::default()
                    },
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            let conversation = match reply.conversation_id {
                Nullable::Value(v) => Some(v),
                _ => None,
            };
            Ok(Task {
                id: reply.prompt_id,
                conversation,
            })
        } else {
            let dir = runtime::directory(r);
            self.command(r,format!("d={}; setsid nohup sh \"$d/run.sh\" >\"$d/launcher.out\" 2>\"$d/launcher.err\" </dev/null &",boat::shell_quote(&dir))).await?;
            Ok(Task {
                id: r.id.clone(),
                conversation: None,
            })
        }
    }
    async fn recover(&self, r: &Record) -> Result<Option<Task>> {
        if r.spec.mode == Mode::Coder {
            let observation = self
                .command(r, runtime::poll_script(&runtime::directory(r), 0))
                .await?;
            let v: Value = serde_json::from_str(&observation)
                .map_err(|_| "Invalid remote process evidence.")?;
            return Ok((v["started"] == true).then(|| Task {
                id: r.id.clone(),
                conversation: None,
            }));
        }
        let mut cursor = None;
        for _ in 0..20 {
            let page = self
                .client
                .events(&EventsParams {
                    sandbox_id: self.resource(r)?.into(),
                    sort: Some("asc".into()),
                    cursor: cursor.clone(),
                    limit: Some(100),
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            for event in page.events {
                if event.type_ == "prompt"
                    && event
                        .data
                        .as_ref()
                        .and_then(|d| d.get("prompt"))
                        .and_then(Value::as_str)
                        == Some(self.prompt(r).as_str())
                {
                    let id = match event.task_id {
                        Nullable::Value(v) => v,
                        _ => event.id.ok_or("The recovered prompt has no identity.")?,
                    };
                    let conversation = match event.conversation_id {
                        Nullable::Value(v) => Some(v),
                        _ => None,
                    };
                    return Ok(Some(Task { id, conversation }));
                }
            }
            let Some(page_info) = page.page_info else {
                break;
            };
            if !page_info.has_more {
                break;
            }
            cursor = page_info.next_cursor;
        }
        Ok(None)
    }
    async fn poll(&self, r: &Record) -> Result<Observation> {
        let mut observation = if r.spec.mode == Mode::Integrated {
            self.integrated_poll(r).await?
        } else {
            let offset = r
                .cursor
                .as_deref()
                .unwrap_or("0")
                .parse()
                .map_err(|_| "Invalid remote cursor.")?;
            runtime::parse_poll(
                r,
                &self
                    .command(r, runtime::poll_script(&runtime::directory(r), offset))
                    .await?,
            )?
        };
        for event in &mut observation.events {
            self.credentials.redact(event);
        }
        if let Some(Ok(result)) = &mut observation.end {
            self.credentials.redact(result);
        }
        Ok(observation)
    }
    async fn cancel(&self, r: &Record) -> Result<()> {
        if r.spec.mode == Mode::Integrated {
            self.client
                .interrupt(&InterruptParams {
                    sandbox_id: self.resource(r)?.into(),
                    conversation: r.remote_task.as_ref().and_then(|t| t.conversation.clone()),
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
        } else {
            let dir = runtime::directory(r);
            self.command(r,format!("d={}; if [ -f \"$d/pid\" ]; then kill -TERM -- -\"$(cat \"$d/pid\")\" 2>/dev/null || true; fi",boat::shell_quote(&dir))).await?;
        }
        Ok(())
    }
    async fn cleanup(&self, r: &Record) -> Result<Option<Value>> {
        let id = self.resource(r)?;
        let info = self
            .client
            .get(&GetParams {
                sandbox_id: id.into(),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?
            .sandbox;
        if !matches!(info.state.as_str(), "stopped" | "archived") {
            let reply = self
                .client
                .stop(&StopParams {
                    sandbox_id: id.into(),
                    ..Default::default()
                })
                .await
                .map_err(|e| e.to_string())?;
            if let Nullable::Value(sandbox) = reply.sandbox {
                if let Nullable::Value(stop) = sandbox.stop {
                    self.client
                        .wait_for_stop(
                            id,
                            &stop.id,
                            &WaitOptions {
                                timeout: Duration::from_secs(600),
                                ..Default::default()
                            },
                        )
                        .await
                        .map_err(|e| e.to_string())?;
                } else {
                    return Err(
                        "Boat did not return a stop operation; follow this job to verify cleanup."
                            .into(),
                    );
                }
            } else {
                return Err(
                    "Boat did not return stop evidence; follow this job to verify cleanup.".into(),
                );
            }
        }
        let usage = self
            .client
            .usage(&UsageParams {
                sandbox_id: id.into(),
                ..Default::default()
            })
            .await
            .map_err(|e| e.to_string())?;
        if usage.running {
            return Err("Boat still reports a running meter after stop.".into());
        }
        Ok(Some(
            json!({"cost_usd":usage.dollars,"machine_seconds":usage.seconds,"basis":"provider","running":false}),
        ))
    }
}

fn normalize(r: &Record, events: &[SandboxEvent]) -> Vec<Value> {
    let mut out: Vec<Value> = vec![];
    for event in events {
        let Some(data) = &event.data else {
            continue;
        };
        if event.type_ != "response" {
            continue;
        }
        let id = event.id.as_deref().unwrap_or("");
        if let Some(model) = data.get("model").and_then(Value::as_str) {
            out.push(json!({"event":"model","model":model}));
        }
        if let Some(text) = data.get("content").and_then(Value::as_str) {
            let previous = out
                .iter()
                .rev()
                .chain(r.events.iter().rev())
                .find(|v| v["source_id"] == id && v.get("source_content").is_some())
                .and_then(|v| v["source_content"].as_str())
                .unwrap_or("");
            let delta = text.strip_prefix(previous).unwrap_or(text);
            if !delta.is_empty() {
                out.push(
                    json!({"event":"delta","text":delta,"source_id":id,"source_content":text}),
                );
            }
        }
        if let Some(tools) = data.get("tools").and_then(Value::as_array) {
            for tool in tools {
                let call = &tool["use"];
                let result = &tool["result"];
                if let Some(name) = call["name"].as_str() {
                    out.push(json!({"event":"tool","name":name,"input":call["input"],"output":result,"running":result.is_null(),"call_id":call["id"]}));
                }
            }
        }
    }
    out
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn normalization_deduplicates_cumulative_text_and_keeps_tools() {
        let spec = crate::Spec {
            placement: crate::Placement::Boat,
            mode: Mode::Integrated,
            agent: "codex".into(),
            task: "test".into(),
            model: None,
            reasoning: None,
            cwd: "fixture".into(),
            timeout_seconds: 600,
            size: "small".into(),
            template: None,
            credential_names: vec![],
        };
        let mut r = Record::new("j1", spec).unwrap();
        r.events.push(
            json!({"event":"delta","text":"Hello","source_id":"e1","source_content":"Hello"}),
        );
        let event:SandboxEvent=serde_json::from_value(json!({"id":"e1","type":"response","timestamp":2,"data":{"content":"Hello world","tools":[{"use":{"id":"c1","name":"Bash","input":{"command":"true"}},"result":{"content":"ok"}}]}})).unwrap();
        let out = normalize(&r, &[event]);
        assert_eq!(out[0]["text"], " world");
        assert_eq!(out[1]["running"], false);
    }
}
