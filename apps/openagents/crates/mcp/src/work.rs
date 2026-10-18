//! Issue work through the public SDK and the connected engine.
use std::sync::Arc;
use std::time::Duration;

use futures::{StreamExt as _, stream};
use openagents_sdk::{
    CancellationToken,
    work::{self as api, LandMode, Placement, Pool, Work, WorkRequest},
};
use serde::Deserialize;
use serde_json::{Value, json};

use crate::{OpenAgents, ToolDef};

pub const NAME: &str = "work_issues";

pub fn tool() -> ToolDef {
    ToolDef {
        name: NAME,
        description: "Start issue work on this computer's granted GCE pool and return run IDs. Read progress, list runs, cancel your runs, or check the pool. Uses your saved agent's compute budget. A missing grant returns the command the owner must run. Reuse request_id after a lost response. Work continues when the tool call ends.",
        input_schema: json!({"type":"object","additionalProperties":false,"properties":{
            "action":{"type":"string","enum":["start","list","events","cancel","pool_status","pool_up","pool_down"],"default":"start"},
            "request_id":{"type":"string","description":"A stable ID for this batch; reuse it for retries."},
            "repo":{"type":"string","default":"OpenAgentsInc/openagents"},
            "issues":{"type":"array","items":{"type":"integer","minimum":1},"maxItems":32},
            "ids":{"type":"array","items":{"type":"string"},"maxItems":32},
            "after":{"type":"integer","minimum":0,"default":0},
            "land":{"type":"string","enum":["queue","pr","none"],"default":"queue"},
            "engine":{"type":"string"},"on":{"type":"string","enum":["gce"],"default":"gce"},
            "agent":{"type":"string","description":"Saved agent ID; defaults to this chat's agent. It must match when called from a saved agent chat."},
            "hosts":{"type":"integer","minimum":1,"maximum":32,"default":1}
        }}),
    }
}

#[derive(Default, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Action {
    #[default]
    Start,
    List,
    Events,
    Cancel,
    PoolStatus,
    PoolUp,
    PoolDown,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Args {
    #[serde(default)]
    action: Action,
    request_id: Option<String>,
    #[serde(default = "repo")]
    repo: String,
    #[serde(default)]
    issues: Vec<u64>,
    #[serde(default)]
    ids: Vec<String>,
    #[serde(default)]
    after: u64,
    #[serde(default)]
    land: LandMode,
    engine: Option<String>,
    on: Option<Placement>,
    agent: Option<String>,
    #[serde(default = "one")]
    hosts: u32,
}
fn repo() -> String {
    "OpenAgentsInc/openagents".into()
}
fn one() -> u32 {
    1
}

struct Connection(Arc<OpenAgents>);
impl api::Transport for Connection {
    fn request<'a>(
        &'a self,
        method: &'static str,
        params: Value,
        _: &'a CancellationToken,
    ) -> api::Reply<'a> {
        Box::pin(async move {
            let reply = tokio::time::timeout(Duration::from_secs(115), self.0.call(method, params)).await
                .map_err(|_| api::Error::refused("The work request timed out. Check its status or retry with the same request ID."))?
                .map_err(|error| api::Error::refused(error.to_string()))?;
            serde_json::from_value::<Result<Value, api::Error>>(reply)
                .map_err(|_| api::Error::refused("The work response could not be read."))?
        })
    }
}

fn actor(saved: Option<&str>, requested: Option<&str>) -> Result<String, String> {
    if let (Some(saved), Some(requested)) = (saved, requested) {
        if saved != requested {
            return Err("Use this chat's saved agent ID for its compute budget.".into());
        }
    }
    saved
        .or(requested)
        .filter(|agent| api::identifier(agent))
        .map(str::to_owned)
        .ok_or_else(|| "Choose a saved agent ID with an owner-granted compute budget.".into())
}

pub async fn call(openagents: &Arc<OpenAgents>, value: Value) -> Result<Value, String> {
    tokio::time::timeout(Duration::from_secs(115), call_inner(openagents, value))
        .await
        .map_err(|_| {
            "The work request timed out. Check status or retry the same request ID.".to_owned()
        })?
}

async fn call_inner(openagents: &Arc<OpenAgents>, value: Value) -> Result<Value, String> {
    let args: Args = serde_json::from_value(value)
        .map_err(|error| format!("Check the work request: {error}"))?;
    let saved = if let Some(id) = &openagents.origin().chat_id {
        let chat = openagents
            .resolve_chat(id)
            .await
            .map_err(|error| error.to_string())?;
        chat.config
            .as_ref()
            .and_then(|config| {
                config
                    .model_options
                    .get(openagents_proto::saved_agent::AGENT)
            })
            .and_then(Value::as_str)
            .map(str::to_owned)
    } else {
        None
    };
    let agent = actor(saved.as_deref(), args.agent.as_deref())?;
    execute(&Connection(openagents.clone()), &agent, args).await
}

async fn execute(
    connection: &dyn api::Transport,
    agent: &str,
    args: Args,
) -> Result<Value, String> {
    let cancel = CancellationToken::new();
    let work = Work::new(connection);
    let error = |error: api::Error| {
        error.owner_command.as_ref().map_or_else(
            || error.message.clone(),
            |command| format!("{} Owner command: {command}", error.message),
        )
    };
    match args.action {
        Action::Start => {
            let request_id = args.request_id.filter(|id| api::identifier(id) && id.len() <= 100)
                .ok_or("Choose a stable request_id of up to 100 letters, numbers, dots, underscores, or hyphens.")?;
            let unique: std::collections::BTreeSet<_> = args.issues.iter().copied().collect();
            if args.issues.is_empty() || args.issues.len() > 32 || unique.len() != args.issues.len()
            {
                return Err("Choose 1 to 32 different issue numbers.".into());
            }
            let mut requests = vec![];
            for issue in args.issues {
                let request = WorkRequest {
                    repo: args.repo.clone(),
                    issue,
                    land: args.land,
                    engine: args.engine.clone(),
                    on: args.on.clone().or(Some(Placement::Gce)),
                    agent: agent.into(),
                };
                request.validate().map_err(&error)?;
                requests.push((format!("{request_id}-{issue}"), request));
            }
            let mut runs = stream::iter(requests.into_iter().map(|(id, request)| {
                let work = &work;
                let cancel = &cancel;
                async move {
                    let issue = request.issue;
                    match work.start(&id, request, cancel).await {
                        Ok(run) => json!({"issue":issue,"run":run}),
                        Err(error) => json!({"issue":issue,"error":error}),
                    }
                }
            }))
            .buffer_unordered(4)
            .collect::<Vec<_>>()
            .await;
            runs.sort_by_key(|run| run["issue"].as_u64());
            Ok(json!({"runs":runs}))
        }
        Action::List => Ok(json!(
            work.list(&cancel)
                .await
                .map_err(&error)?
                .into_iter()
                .filter(|run| run.request.agent == agent)
                .collect::<Vec<_>>()
        )),
        Action::Events | Action::Cancel => {
            if args.ids.is_empty() || args.ids.len() > 32 {
                return Err("Choose 1 to 32 work run IDs.".into());
            }
            let mut pages = vec![];
            for id in args.ids {
                let after = if matches!(args.action, Action::Cancel) {
                    0
                } else {
                    args.after
                };
                let page = work.events(&id, after, &cancel).await.map_err(&error)?;
                if page.run.request.agent != agent {
                    return Err("Choose a work run started by this saved agent.".into());
                }
                pages.push((id, page));
            }
            let mut replies = vec![];
            for (id, page) in pages {
                if matches!(args.action, Action::Cancel) {
                    work.cancel(&id, &cancel).await.map_err(&error)?;
                    replies.push(json!({"id":id,"canceled":true}));
                } else {
                    replies.push(json!(page));
                }
            }
            Ok(json!({"runs":replies}))
        }
        Action::PoolStatus | Action::PoolUp | Action::PoolDown => {
            let pool = Pool::new(connection, agent).map_err(&error)?;
            match args.action {
                Action::PoolUp => Ok(json!(pool.up(args.hosts, &cancel).await.map_err(&error)?)),
                Action::PoolDown => {
                    pool.down(&cancel).await.map_err(&error)?;
                    Ok(json!({"stopped":true}))
                }
                _ => Ok(json!(pool.status(&cancel).await.map_err(&error)?)),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn any_saved_agent_can_work_but_cannot_borrow_another_agents_budget() {
        assert_eq!(actor(Some("agt_reviewer"), None).unwrap(), "agt_reviewer");
        assert!(actor(Some("agt_reviewer"), Some("agt_other")).is_err());
        assert!(actor(None, None).is_err());
        assert_eq!(actor(None, Some("agt_builder")).unwrap(), "agt_builder");
    }
}

#[cfg(test)]
mod connection_tests {
    use super::*;
    use std::sync::Mutex;
    #[derive(Default)]
    struct Fake(Mutex<Vec<(&'static str, Value)>>);
    impl api::Transport for Fake {
        fn request<'a>(
            &'a self,
            method: &'static str,
            params: Value,
            _: &'a CancellationToken,
        ) -> api::Reply<'a> {
            self.0.lock().unwrap().push((method, params.clone()));
            Box::pin(async move {
                match method {
                    api::START => Ok(json!(api::WorkRun {
                        id: params["request_id"].as_str().unwrap().into(),
                        request: serde_json::from_value(params["request"].clone()).unwrap(),
                        state: "created".into(),
                        created_unix: 1,
                        updated_unix: 1,
                        host: None,
                        result: None,
                        error: None,
                        usage: None
                    })),
                    api::POOL_STATUS => Ok(json!(api::PoolStatus {
                        granted: true,
                        ..Default::default()
                    })),
                    api::POOL_UP => Err(api::Error {
                        code: "refused".into(),
                        message: "Ask the owner to grant a pool.".into(),
                        owner_command: Some(
                            "openagents cloud up --hosts 1 --agent agt_reviewer".into(),
                        ),
                    }),
                    _ => Err(api::Error::invalid("Unexpected request.")),
                }
            })
        }
    }
    #[tokio::test]
    async fn tool_dispatches_through_sdk_with_stable_ids_and_the_saved_agent_budget() {
        let connection = Fake::default();
        let args = serde_json::from_value(
            json!({"issues":[12,13],"request_id":"batch","engine":"briefed:codex"}),
        )
        .unwrap();
        let result = execute(&connection, "agt_reviewer", args).await.unwrap();
        assert_eq!(result["runs"][0]["run"]["id"], "batch-12");
        let calls = connection.0.lock().unwrap();
        assert_eq!(calls.len(), 2);
        for (method, params) in calls.iter() {
            assert_eq!(*method, api::START);
            assert_eq!(params["request"]["agent"], "agt_reviewer");
            assert_eq!(params["request"]["on"], "gce");
            assert_eq!(params["request"]["land"], "queue");
        }
    }
    #[tokio::test]
    async fn tool_preserves_owner_commands_and_validates_the_whole_batch_first() {
        let connection = Fake::default();
        let args = serde_json::from_value(json!({"action":"pool_up"})).unwrap();
        assert!(
            execute(&connection, "agt_reviewer", args)
                .await
                .unwrap_err()
                .contains("openagents cloud up --hosts 1 --agent agt_reviewer")
        );
        connection.0.lock().unwrap().clear();
        let args = serde_json::from_value(json!({"issues":[12,0],"request_id":"batch"})).unwrap();
        assert!(execute(&connection, "agt_reviewer", args).await.is_err());
        assert!(
            connection.0.lock().unwrap().is_empty(),
            "invalid batch must start no work"
        );
    }
}
