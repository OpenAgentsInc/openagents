//! Synthetic ATIF export from the same original records shown by the demo.
use super::{DemoState, agents::*, onboarding, tools::*};
use atif::{Call, Outcome, Session, Source, Step};
use serde_json::{Map, Value, json};

fn said(source: Source, message: &str) -> Step {
    Step {
        at: 0,
        source,
        message: message.into(),
        reasoning: None,
        model: (source == Source::Agent).then(|| "demo/local".into()),
        call: None,
        tokens: None,
        milliseconds: None,
        extensions: Map::new(),
    }
}
fn called(index: usize, name: &str, input: &str, output: &str, state: ToolState) -> Step {
    let output = if state == ToolState::Failed {
        json!({"error":output})
    } else {
        json!(output)
    };
    let running = state == ToolState::Running;
    let mut extra = Map::new();
    extra.insert("running".into(), json!(running));
    if running {
        extra.insert("pending_output".into(), output.clone());
    }
    Step {
        call: Some(Call {
            id: format!("call-{}", index + 1),
            name: name.into(),
            arguments: json!(input),
            output: output.to_string(),
            outcome: if running {
                Outcome::Cancelled
            } else if state == ToolState::Failed {
                Outcome::Failed
            } else {
                Outcome::Completed
            },
            milliseconds: 0,
            purpose: None,
            extra,
        }),
        ..said(Source::Agent, "")
    }
}
fn tool(index: usize, call: &ToolCall) -> Step {
    called(
        index,
        match call.kind {
            ToolKind::Read => "Read",
            ToolKind::Search => "Search",
            ToolKind::Edit => "Edit",
            ToolKind::Run => "Run",
        },
        call.input,
        call.output,
        call.state,
    )
}
fn plugin(index: usize, call: &PluginCall) -> Step {
    called(
        index,
        &format!("{}.{}", call.plugin, call.operation),
        call.input,
        call.output,
        call.state,
    )
}
fn append(steps: &mut Vec<Step>, messages: &[String]) {
    for text in messages {
        steps.push(said(Source::User, text));
        steps.push(said(
            Source::Agent,
            "Preview message added. No agent is connected.",
        ));
    }
}
fn base(id: &str, steps: &[Step], tokens: u64, notice: Option<&str>) -> Value {
    let mut session = Session::opening(
        id,
        "demo/local",
        "coder-new",
        "openagents / main",
        env!("CARGO_PKG_VERSION"),
    );
    session.state = "ended".into();
    session.directive = steps
        .iter()
        .find(|s| s.source == Source::User)
        .map(|s| s.message.clone())
        .unwrap_or_default();
    let mut value = atif::document_at(&session, steps, 0);
    for step in value["steps"].as_array_mut().into_iter().flatten() {
        if step
            .pointer("/tool_calls/0/extra/running")
            .and_then(Value::as_bool)
            == Some(true)
        {
            step.as_object_mut().unwrap().remove("observation");
        }
    }
    value["extra"]["demo"] = json!(true);
    value["extra"]["timestamps"] = json!("synthetic-zero");
    value["final_metrics"]["extra"]["reported_total_tokens"] = json!(tokens);
    if let Some(notice) = notice {
        value["extra"]["notice"] = json!(notice);
    }
    value
}
fn child(agent: &DemoAgent, messages: &[String], elapsed: u64, notice: Option<&str>) -> Value {
    let mut steps = vec![];
    for message in agent.conversation {
        let index = steps.len();
        steps.push(match message {
            DemoMessage::User(text) => said(Source::User, text),
            DemoMessage::Assistant(text) => said(Source::Agent, text),
            DemoMessage::Tool(call) => tool(index, call),
            DemoMessage::Plugin(call) => plugin(index, call),
        });
    }
    append(&mut steps, messages);
    let tokens = (agent
        .tokens
        .trim_end_matches('k')
        .parse::<f64>()
        .unwrap_or(0.0)
        * if agent.tokens.ends_with('k') {
            1000.0
        } else {
            1.0
        }) as u64;
    let mut value = base(&format!("demo-{}", agent.name), &steps, tokens, notice);
    value["extra"]["agent"] = json!(agent.name);
    value["extra"]["task"] = json!(agent.task);
    value["final_metrics"]["extra"]["wall_seconds"] =
        json!(agent.elapsed_seconds.saturating_add(elapsed));
    value
}
pub(super) fn document(app: &DemoState) -> Value {
    if app.onboarding {
        return child(
            &onboarding::DEMO,
            &app.messages,
            app.elapsed_seconds,
            app.notice.as_deref(),
        );
    }
    if let Some(index) = app.selected_agent {
        return child(
            &DEMOS[index],
            &app.messages,
            app.elapsed_seconds,
            app.notice.as_deref(),
        );
    }
    let mut steps = vec![said(Source::User, "Review the terminal with four agents.")];
    for call in MAIN_TOOLS {
        steps.push(tool(steps.len(), &call));
    }
    for call in MAIN_PLUGINS {
        steps.push(plugin(steps.len(), &call));
    }
    for agent in &DEMOS {
        let mut extra = Map::new();
        extra.insert("schema".into(), json!("openagents.delegation.v1"));
        extra.insert("agent".into(), json!(agent.name));
        extra.insert("running".into(), json!(true));
        steps.push(Step {
            call: Some(Call {
                id: format!("demo-{}", agent.name),
                name: "delegate".into(),
                arguments: json!({"agent":agent.name,"task":agent.task}),
                output: "null".into(),
                outcome: Outcome::Cancelled,
                milliseconds: 0,
                purpose: Some(agent.task.into()),
                extra,
            }),
            ..said(Source::Agent, "")
        });
    }
    append(&mut steps, &app.messages);
    let mut value = base("demo-main", &steps, 0, app.notice.as_deref());
    value["subagent_trajectories"] = json!(
        DEMOS
            .iter()
            .enumerate()
            .map(|(index, agent)| child(
                agent,
                &app.saved_chats[index + 1].messages,
                app.elapsed_seconds,
                None
            ))
            .collect::<Vec<_>>()
    );
    value
}
