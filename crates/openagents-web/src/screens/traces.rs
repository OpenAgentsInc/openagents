//! Traces: the signed-in list, one trace, one of its agents, and the
//! shared pages anyone with the link sees, over a sample run read back
//! through the live upload check.

use axum::http::HeaderMap;
use axum::response::Response;
use serde_json::{Value, json};

use super::{Group, Screen, state};
use crate::traces::{Agent, PAGE, PUBLIC, Stats, Summary};

const TICKET: (&str, &str) = ("sample-ticket", "sample-request");
/// October 9, 2026, 12:00 UTC.
const NOW: u64 = 1_791_547_200;
const MODEL: &str = "sample-model";

pub(super) fn screens() -> Vec<Screen> {
    vec![Screen {
        id: "traces",
        title: "Traces",
        group: Group::Page,
        routes: &[
            "/settings/traces",
            "/settings/traces/{id}",
            "/settings/traces/{id}/agents/{agent}",
            "/trace/{id}",
            "/trace/{id}/agents/{agent}",
        ],
        states: vec![
            state("none", "List: no traces", list_none),
            state("list", "List: traces", list_some),
            state("trace", "One trace", one),
            state("shared", "One trace, shared", one_shared),
            state("agent", "One of its agents", agent),
            state("public", "Shared page", public),
            state("public-agent", "Shared page: an agent", public_agent),
            state("run", "Shared page: a real run with agents", run_folded),
            state(
                "run-open",
                "Shared page: a real run, every step open",
                run_open,
            ),
        ],
    }]
}

fn document(title: &str) -> Value {
    json!({
        "schema_version": "ATIF-v1.7",
        "session_id": "sample-session",
        "agent": {"name": "openagents-coder", "version": "1", "model_name": MODEL},
        "steps": [
            {"step_id": 1, "source": "user", "message": title},
            {"step_id": 2, "source": "agent",
             "message": "The login button moved, so the test clicked nothing. I updated the test.",
             "model_name": MODEL,
             "tool_calls": [{"tool_call_id": "c1", "function_name": "shell", "arguments": {"command": "cargo test -p sample-app"}}],
             "observation": {"results": [{"source_call_id": "c1", "content": "test result: ok. 48 passed"}]}},
            {"step_id": 3, "source": "agent", "message": "Done. **All 48 tests pass.**", "model_name": MODEL}
        ]
    })
}

/// A sample trace as the upload check reads it.
fn trace(title: &str, shared: bool, agents: usize) -> (Summary, Value) {
    let bytes = serde_json::to_vec(&document(title)).unwrap();
    let (document, mut summary) = crate::traces::check(&bytes).expect("a sample trace");
    summary.id = "sample-trace-1".into();
    summary.uploaded_unix = NOW - 86_400;
    summary.shared = shared;
    summary.agents = agents;
    (summary, document)
}

fn helper(stats: Stats) -> Agent {
    Agent {
        id: "sample-agent-1".into(),
        parent: None,
        title: "Find where the login button moved".into(),
        agent: "openagents-coder".into(),
        model: MODEL.into(),
        steps: 3,
        bytes: 2_048,
        digest: "sample".into(),
        uploaded_unix: NOW - 86_400,
        call: Some("c1".into()),
        stats,
    }
}

fn page(headers: &HeaderMap, title: &str, path: &str, body: maud::Markup) -> Response {
    crate::settings::page_as(headers, super::auth::signed_in(), title, path, body)
}

fn list_none(headers: &HeaderMap) -> Response {
    page(headers, "Traces", PAGE, crate::traces::list_content(&[]))
}

fn list_some(headers: &HeaderMap) -> Response {
    let (first, _) = trace("Fix the failing login test", true, 1);
    let (mut second, _) = trace("Plan the sample release", false, 0);
    second.id = "sample-trace-2".into();
    second.uploaded_unix = NOW - 3 * 86_400;
    page(
        headers,
        "Traces",
        PAGE,
        crate::traces::list_content(&[first, second]),
    )
}

fn one_with(headers: &HeaderMap, shared: bool) -> Response {
    let (summary, document) = trace("Fix the failing login test", shared, 1);
    let agents = [helper(Stats::of(&document))];
    let path = format!("{PAGE}/{}", summary.id);
    let body = crate::traces::trace_content(
        &summary,
        &document,
        &agents,
        &format!("https://openagents.com{PUBLIC}/{}", summary.id),
        TICKET,
        TICKET,
        false,
    );
    page(headers, &summary.title, &path, body)
}

fn one(headers: &HeaderMap) -> Response {
    one_with(headers, false)
}

fn one_shared(headers: &HeaderMap) -> Response {
    one_with(headers, true)
}

fn agent_body(base: &str) -> (Agent, maud::Markup) {
    let (summary, root) = trace("Fix the failing login test", true, 1);
    let agents = [helper(Stats::of(&root))];
    let own = document("Find where the login button moved");
    let body = crate::traces::agent_content(
        &summary,
        &Stats::of(&root),
        &agents,
        &agents[0],
        &own,
        base,
        false,
    );
    (agents[0].clone(), body)
}

fn agent(headers: &HeaderMap) -> Response {
    let base = format!("{PAGE}/sample-trace-1");
    let (agent, body) = agent_body(&base);
    page(
        headers,
        &agent.title,
        &format!("{base}/agents/{}", agent.id),
        body,
    )
}

fn public(headers: &HeaderMap) -> Response {
    let (summary, document) = trace("Fix the failing login test", true, 1);
    let agents = [helper(Stats::of(&document))];
    crate::traces::public_response(
        headers,
        &summary.title,
        &format!("{PUBLIC}/{}", summary.id),
        crate::traces::public_content(&summary, &document, &agents, false),
    )
}

fn public_agent(headers: &HeaderMap) -> Response {
    let base = format!("{PUBLIC}/sample-trace-1");
    let (agent, body) = agent_body(&base);
    crate::traces::public_response(
        headers,
        &agent.title,
        &format!("{base}/agents/{}", agent.id),
        body,
    )
}

/// A real export from Coder: a person asks it to try three agents
/// read-only; two fail, one answers, and Microcoder answers too
/// (`delegation.atif.json`, #11621).
const RUN: &str = include_str!("delegation.atif.json");

fn run(headers: &HeaderMap, expand: bool) -> Response {
    let (document, mut summary) = crate::traces::check(RUN.as_bytes()).expect("the sample run");
    summary.id = "sample-run".into();
    summary.uploaded_unix = NOW - 3_600;
    summary.shared = true;
    crate::traces::public_response(
        headers,
        &summary.title,
        &format!("{PUBLIC}/{}", summary.id),
        crate::traces::public_content(&summary, &document, &[], expand),
    )
}

fn run_folded(headers: &HeaderMap) -> Response {
    run(headers, false)
}

fn run_open(headers: &HeaderMap) -> Response {
    run(headers, true)
}
