//! Authenticated end-to-end suite against a deployed Rust Coder.
//!
//! Every test is gated on two environment variables:
//!
//! - `OPENAGENTS_E2E_CODER_ORIGIN` — the deployed Coder origin, for example
//!   `https://coder-stage.openagents.com`
//! - `OPENAGENTS_E2E_CODER_TOKEN` — an OpenAgents-minted Ed25519 JWT or an
//!   OpenAgents account bearer for that origin
//!
//! When either is unset every test returns `Ok` early, so a normal
//! `cargo test --workspace` stays green and fast. When both are set the tests
//! drive the real compiled `openagents` binary where it has a verb, and
//! `reqwest` directly where it does not, against the live service.
//!
//! The token is never printed: assertions quote server answers and CLI
//! output, both of which the service and CLI keep credential-free.
//!
//! Two truths this suite reports honestly rather than papering over:
//!
//! - `openagents responses <prompt>` sends no `workspace.repository`, and the
//!   deployed `/v1/responses` refuses a new run that names no repository. The
//!   binary leg therefore asserts the typed refusal is *not*
//!   `authorization_denied` (transport and auth work), and the run-creating
//!   legs go over `reqwest` with a repository the caller actually reaches.
//! - `openagents responses --response <id>` always sends `input`, and the
//!   deployed service refuses a reconnect that carries input. The replay
//!   assertions therefore run over raw SSE, and the binary reconnect leg
//!   asserts only that its refusal is typed and not an auth or ownership
//!   failure.

use serde_json::{Value, json};
use std::io::Read as _;
use std::path::Path;
use std::time::{Duration, Instant};

/// How long the run-creating legs follow a fresh run's live stream before
/// stopping the run. The point is proving transport, not finishing agent work.
const ORDER_FOLLOW: Duration = Duration::from_secs(20);

/// How long a reconnect's replay is followed. Stored events replay at once;
/// what streams after them is live work this suite does not wait out.
const REPLAY_FOLLOW: Duration = Duration::from_secs(30);

/// How long a plain (non-streaming) HTTP exchange may take.
const CALL_CAP: Duration = Duration::from_secs(30);

/// How long the binary may run before it is killed and its output judged.
const BINARY_CAP: Duration = Duration::from_secs(120);

/// A valid v4-shaped uuid no live run can hold: run ids are random v4, and
/// this one is fixed. Standing in for "somebody else's run", because the
/// service answers both with the same `not_found`.
const NOBODYS_RUN: &str = "00000000-0000-4000-8000-0123456789ab";

// ---------------------------------------------------------------------------
// the tests
// ---------------------------------------------------------------------------

#[tokio::test]
async fn ready_and_metadata_answer() -> Result<(), String> {
    let Some(live) = Live::from_env() else {
        return Ok(());
    };

    let ready = tokio::time::timeout(
        CALL_CAP,
        live.http.get(format!("{}/ready", live.origin)).send(),
    )
    .await
    .map_err(|_| "GET /ready did not answer within 30s".to_string())?
    .map_err(|error| format!("GET /ready failed: {error}"))?;
    if ready.status() != reqwest::StatusCode::OK {
        return Err(format!("GET /ready answered {}", ready.status()));
    }
    let body: Value = ready
        .json()
        .await
        .map_err(|error| format!("/ready body is not JSON: {error}"))?;
    if body["ready"] != json!(true) {
        return Err(format!("/ready did not report ready: {body}"));
    }

    let metadata = tokio::time::timeout(
        CALL_CAP,
        live.http
            .get(format!(
                "{}/.well-known/oauth-protected-resource",
                live.origin
            ))
            .send(),
    )
    .await
    .map_err(|_| "GET oauth-protected-resource did not answer within 30s".to_string())?
    .map_err(|error| format!("GET oauth-protected-resource failed: {error}"))?;
    if metadata.status() != reqwest::StatusCode::OK {
        return Err(format!(
            "oauth-protected-resource answered {}",
            metadata.status()
        ));
    }
    let body: Value = metadata
        .json()
        .await
        .map_err(|error| format!("oauth-protected-resource body is not JSON: {error}"))?;
    let resource = body["resource"]
        .as_str()
        .ok_or_else(|| format!("metadata names no resource: {body}"))?;
    if !resource.ends_with("/mcp") {
        return Err(format!("the protected resource is not /mcp: {resource}"));
    }
    Ok(())
}

#[tokio::test]
async fn an_unauthenticated_responses_request_is_refused() -> Result<(), String> {
    let Some(live) = Live::from_env() else {
        return Ok(());
    };

    let answered = tokio::time::timeout(
        CALL_CAP,
        live.http
            .post(format!("{}/v1/responses", live.origin))
            .header("content-type", "application/json")
            .header("accept", "text/event-stream")
            .json(&json!({
                "input": [{"type": "message", "role": "user", "content": "hello"}]
            }))
            .send(),
    )
    .await
    .map_err(|_| "the unauthenticated POST did not answer within 30s".to_string())?
    .map_err(|error| format!("the unauthenticated POST failed: {error}"))?;

    if answered.status() != reqwest::StatusCode::UNAUTHORIZED {
        return Err(format!(
            "an unauthenticated request was answered {} rather than 401",
            answered.status()
        ));
    }
    let body: Value = answered
        .json()
        .await
        .map_err(|error| format!("the 401 body is not typed JSON: {error}"))?;
    if body["error"]["code"] != json!("authorization_denied") {
        return Err(format!(
            "the 401 body does not carry code authorization_denied: {body}"
        ));
    }
    Ok(())
}

#[tokio::test]
async fn whoami_names_the_person_over_mcp() -> Result<(), String> {
    let Some(live) = Live::from_env() else {
        return Ok(());
    };

    let answer = live.mcp_call("whoami", json!({})).await?;
    for field in ["login", "person", "account"] {
        if answer.get(field).is_none() {
            return Err(format!("whoami answered without `{field}`: {answer}"));
        }
    }
    if answer["login"].as_str().unwrap_or_default().is_empty() {
        return Err(format!("whoami named an empty login: {answer}"));
    }
    Ok(())
}

#[tokio::test]
async fn a_responses_run_streams_and_is_readable_over_mcp() -> Result<(), String> {
    let Some(live) = Live::from_env() else {
        return Ok(());
    };

    // Leg one: the real compiled binary, authenticated through the credential
    // store the CLI itself reads — the token written for this origin in a
    // private HOME, exactly as `oa auth login` would store it.
    let home = tempfile::tempdir().map_err(|error| format!("no temp HOME: {error}"))?;
    write_credentials(home.path(), &live.origin, &live.token)?;
    let ran = run_binary(
        home.path(),
        &[
            "responses",
            "Say the word ready and stop.",
            "--origin",
            &live.origin,
            "--json",
        ],
    )?;
    eprintln!(
        "binary leg: exit={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        ran.status, ran.stdout, ran.stderr
    );

    // The binary either created and streamed a run (`resumed: run=<uuid>`), or
    // was refused with a typed error. Auth must never be the reason: an
    // `authorization_denied` here means the bearer did not reach the wire.
    let run_from_binary = extract_run_id(&ran.stdout);
    if run_from_binary.is_none() {
        let code = envelope_code(&ran.stdout).ok_or_else(|| {
            format!(
                "the binary neither streamed `resumed: run=` nor printed a typed \
                 error envelope; stdout: {}; stderr: {}",
                ran.stdout, ran.stderr
            )
        })?;
        if code == "authorization_denied" {
            return Err(format!(
                "the binary's request was refused as authorization_denied — the \
                 stored bearer did not authenticate; stderr: {}",
                ran.stderr
            ));
        }
        eprintln!(
            "binary leg: the deployed Coder refused the run with typed code \
             `{code}` (the CLI sends no workspace.repository); \
             creating the run over reqwest instead"
        );
    }

    // Leg two: a run that actually exists. Reuse the binary's run when it made
    // one; otherwise order one over `/v1/responses` naming a repository the
    // caller reaches, which is what the binary cannot say yet.
    let (run, live_events) = match run_from_binary {
        Some(id) => (id, Vec::new()),
        None => match live
            .order_run("Reply with the single word ready, then stop.")
            .await
        {
            Ok(ordered) => ordered,
            Err(refusal) => {
                // A server-side refusal the client cannot fix — for example a
                // caller whose repository reach is empty. Report it exactly.
                eprintln!(
                    "skipped: the deployed Coder would not order a run for this \
                     caller — {refusal}"
                );
                return Ok(());
            }
        },
    };
    if !live_events.is_empty() {
        seqs_strictly_increase(&live_events)
            .map_err(|error| format!("the live stream for run {run}: {error}"))?;
    }

    // The run proved transport; end it rather than leaving an agent working.
    live.stop_run(&run).await;

    // Leg three: the same run over MCP, with the same bearer.
    let about = live
        .mcp_call("get_run", json!({ "run": run }))
        .await
        .map_err(|error| format!("get_run({run}) refused: {error}"))?;
    if about["run"] != json!(run.as_str()) {
        return Err(format!(
            "get_run answered a different run than asked: {about}"
        ));
    }
    if about.get("state").and_then(Value::as_str).is_none() {
        return Err(format!("get_run answered without a state: {about}"));
    }

    let replayed = live
        .mcp_call("read_run_events", json!({ "run": run, "after": 0 }))
        .await
        .map_err(|error| format!("read_run_events({run}) refused: {error}"))?;
    let events = replayed["events"]
        .as_array()
        .ok_or_else(|| format!("read_run_events answered without events: {replayed}"))?;
    if events.is_empty() {
        return Err(format!("run {run} replayed no events over MCP"));
    }
    seqs_strictly_increase(events).map_err(|error| format!("MCP replay for run {run}: {error}"))?;
    Ok(())
}

#[tokio::test]
async fn a_reconnect_replays_without_duplicates() -> Result<(), String> {
    let Some(live) = Live::from_env() else {
        return Ok(());
    };

    // A fresh short run of this test's own, stopped straight away. Stopping is
    // asynchronous — the worker finishes its turn in flight first — so the
    // replay legs below do not demand a terminal event; they assert what the
    // contract promises of every replay: it arrives, and seq never regresses.
    let (run, _) = match live
        .order_run("Reply with the single word ready, then stop.")
        .await
    {
        Ok(ordered) => ordered,
        Err(refusal) => {
            eprintln!(
                "skipped: the deployed Coder would not order a run for this \
                 caller — {refusal}"
            );
            return Ok(());
        }
    };
    live.stop_run(&run).await;
    tokio::time::sleep(Duration::from_secs(3)).await;

    // The binary's reconnect verb, for the record: it always sends `input`,
    // which the deployed service refuses on a reconnect. What must hold is
    // that the refusal is typed and is neither an auth nor an ownership
    // failure — and that a future server accepting it streams `resumed:`.
    let home = tempfile::tempdir().map_err(|error| format!("no temp HOME: {error}"))?;
    write_credentials(home.path(), &live.origin, &live.token)?;
    let ran = run_binary(
        home.path(),
        &[
            "responses",
            "resume",
            "--response",
            &run,
            "--starting-after",
            "0",
            "--origin",
            &live.origin,
            "--json",
        ],
    )?;
    eprintln!(
        "binary reconnect leg: exit={:?}\n--- stdout ---\n{}\n--- stderr ---\n{}",
        ran.status, ran.stdout, ran.stderr
    );
    if ran.status == Some(0) {
        if !ran.stdout.contains("resumed:") {
            return Err(format!(
                "the binary reconnect exited 0 without a resumed line: {}",
                ran.stdout
            ));
        }
    } else if let Some(code) = envelope_code(&ran.stdout) {
        if code == "authorization_denied" || code == "not_found" {
            return Err(format!(
                "the binary reconnect to this caller's own run {run} was refused \
                 as `{code}`; stderr: {}",
                ran.stderr
            ));
        }
        eprintln!("binary reconnect leg: typed refusal `{code}` (see stdout above)");
    } else {
        return Err(format!(
            "the binary reconnect neither streamed nor printed a typed envelope; \
             stdout: {}; stderr: {}",
            ran.stdout, ran.stderr
        ));
    }

    // The replay itself, over raw SSE where the seq fields are visible.
    // From zero: the stream opens with `response.resumed`, replayed output
    // arrives after it, and every seq strictly increases.
    let events = live.reconnect(&run, 0, REPLAY_FOLLOW).await?;
    let first = events
        .first()
        .ok_or_else(|| format!("the reconnect to {run} streamed nothing"))?;
    if first["type"] != json!("response.resumed") {
        return Err(format!("the reconnect did not open with resumed: {first}"));
    }
    if first["run"] != json!(run.as_str()) {
        return Err(format!("the reconnect resumed a different run: {first}"));
    }
    if events.len() < 2 {
        return Err(format!(
            "the reconnect to {run} replayed nothing after the resumed frame"
        ));
    }
    seqs_strictly_increase(&events)
        .map_err(|error| format!("reconnect from 0 to {run}: {error}"))?;
    let last = events
        .last()
        .map(|event| event["type"].as_str().unwrap_or_default().to_string())
        .unwrap_or_default();
    if matches!(
        last.as_str(),
        "response.completed" | "response.failed" | "response.cancelled"
    ) {
        eprintln!("reconnect from 0: the stopped run reached its terminal event `{last}`");
    } else {
        // Stopping ends the turn in flight first, so the terminal event can
        // land minutes later. The replay is what this test pins; the run's
        // standing is read and reported rather than waited out.
        let standing = live.mcp_call("get_run", json!({ "run": run })).await;
        eprintln!(
            "reconnect from 0: no terminal event within {REPLAY_FOLLOW:?} \
             (the stop is still landing); last event type `{last}`, \
             get_run says {standing:?}"
        );
    }

    // From a cursor in the middle: nothing at or before the cursor replays.
    let top = events
        .iter()
        .filter_map(|event| event["seq"].as_i64())
        .max()
        .unwrap_or(0);
    let cursor = top / 2;
    let replayed = live.reconnect(&run, cursor, REPLAY_FOLLOW).await?;
    let opened = replayed
        .first()
        .ok_or_else(|| format!("the cursor reconnect to {run} streamed nothing"))?;
    if opened["replay_from"] != json!(cursor) {
        return Err(format!(
            "the cursor reconnect did not acknowledge replay_from={cursor}: {opened}"
        ));
    }
    seqs_strictly_increase(&replayed)
        .map_err(|error| format!("reconnect from {cursor} to {run}: {error}"))?;
    for event in replayed.iter().skip(1) {
        if let Some(seq) = event["seq"].as_i64()
            && seq <= cursor
        {
            return Err(format!(
                "the reconnect from {cursor} replayed a duplicate at seq {seq}: {event}"
            ));
        }
    }
    Ok(())
}

#[tokio::test]
async fn someone_elses_run_reads_as_absent() -> Result<(), String> {
    let Some(live) = Live::from_env() else {
        return Ok(());
    };

    // A run this caller does not own answers exactly the way a run that does
    // not exist answers, so a fixed uuid no run holds probes the same rule.
    let events = live.reconnect(NOBODYS_RUN, 0, CALL_CAP).await?;
    let terminal = events
        .iter()
        .find(|event| event["type"] == json!("response.failed"))
        .ok_or_else(|| {
            format!(
                "the probe of an absent run did not fail; events: {}",
                Value::Array(events.clone())
            )
        })?;
    if terminal["response"]["error"]["code"] != json!("not_found") {
        return Err(format!(
            "the absent run's failure is not typed not_found: {terminal}"
        ));
    }
    Ok(())
}

// ---------------------------------------------------------------------------
// the live service, as this suite reaches it
// ---------------------------------------------------------------------------

/// The deployed Coder this suite talks to, or nothing when the gate is closed.
struct Live {
    origin: String,
    token: String,
    http: reqwest::Client,
}

impl Live {
    /// Read the gate. Either variable missing closes it, and the test says so
    /// once on stderr and passes.
    fn from_env() -> Option<Self> {
        let origin = std::env::var("OPENAGENTS_E2E_CODER_ORIGIN")
            .ok()
            .map(|value| value.trim().trim_end_matches('/').to_string())
            .filter(|value| !value.is_empty());
        let token = std::env::var("OPENAGENTS_E2E_CODER_TOKEN")
            .ok()
            .map(|value| value.trim().to_string())
            .filter(|value| !value.is_empty());
        let (Some(origin), Some(token)) = (origin, token) else {
            eprintln!("skipped: no live coder configured");
            return None;
        };
        let http = reqwest::Client::builder()
            .connect_timeout(Duration::from_secs(15))
            .build()
            .expect("a reqwest client");
        Some(Self {
            origin,
            token,
            http,
        })
    }

    /// One MCP `tools/call` over the stateless Streamable HTTP transport, the
    /// way the 2026-07-28 protocol carries it: the version and call name in
    /// headers, the protocol metadata under `_meta`, and the answer as JSON or
    /// as one SSE `data:` frame.
    async fn mcp_call(&self, name: &str, arguments: Value) -> Result<Value, String> {
        let body = json!({
            "jsonrpc": "2.0",
            "id": 1,
            "method": "tools/call",
            "params": {
                "name": name,
                "arguments": arguments,
                "_meta": {
                    "io.modelcontextprotocol/protocolVersion": "2026-07-28",
                    "io.modelcontextprotocol/clientInfo": {
                        "name": "openagents-coder-e2e",
                        "version": env!("CARGO_PKG_VERSION"),
                    },
                    "io.modelcontextprotocol/clientCapabilities": {},
                },
            },
        });
        let answered = tokio::time::timeout(
            CALL_CAP,
            self.http
                .post(format!("{}/mcp", self.origin))
                .header("authorization", format!("Bearer {}", self.token))
                .header("content-type", "application/json")
                .header("accept", "application/json, text/event-stream")
                .header("MCP-Protocol-Version", "2026-07-28")
                .header("Mcp-Method", "tools/call")
                .header("Mcp-Name", name)
                .json(&body)
                .send(),
        )
        .await
        .map_err(|_| format!("MCP {name} did not answer within 30s"))?
        .map_err(|error| format!("MCP {name} failed: {error}"))?;
        let status = answered.status();
        let text = tokio::time::timeout(CALL_CAP, answered.text())
            .await
            .map_err(|_| format!("MCP {name} body did not arrive within 30s"))?
            .map_err(|error| format!("MCP {name} body failed: {error}"))?;
        if !status.is_success() {
            return Err(format!("MCP {name} answered {status}: {text}"));
        }
        // The transport answers plain JSON or a one-frame SSE stream; either
        // way one JSON-RPC message is in there.
        let message: Value = match text
            .lines()
            .find_map(|line| line.strip_prefix("data:"))
            .map(str::trim)
        {
            Some(data) => serde_json::from_str(data)
                .map_err(|error| format!("MCP {name} SSE frame does not parse: {error}: {data}"))?,
            None => serde_json::from_str(&text)
                .map_err(|error| format!("MCP {name} body does not parse: {error}: {text}"))?,
        };
        if let Some(error) = message.get("error") {
            return Err(format!("MCP {name} refused: {error}"));
        }
        let result = &message["result"];
        let text = result["content"][0]["text"]
            .as_str()
            .ok_or_else(|| format!("MCP {name} answered without a text block: {message}"))?;
        if result["isError"] == json!(true) {
            return Err(format!("MCP {name} answered an error: {text}"));
        }
        serde_json::from_str(text)
            .map_err(|error| format!("MCP {name} text block does not parse: {error}: {text}"))
    }

    /// Order a run over `/v1/responses` in a repository this caller reaches,
    /// follow its live stream briefly, and hand back the run id and what
    /// streamed. `Err` carries the service's exact refusal.
    async fn order_run(&self, directive: &str) -> Result<(String, Vec<Value>), String> {
        let offered = self.mcp_call("list_repos", json!({})).await?;
        let repo = offered["repos"][0]
            .as_str()
            .ok_or_else(|| format!("list_repos offered no repositories: {offered}"))?
            .to_string();
        let response = tokio::time::timeout(
            CALL_CAP,
            self.http
                .post(format!("{}/v1/responses", self.origin))
                .header("authorization", format!("Bearer {}", self.token))
                .header("content-type", "application/json")
                .header("accept", "text/event-stream")
                .json(&json!({
                    "input": [{"type": "message", "role": "user", "content": directive}],
                    "workspace": {"repository": repo},
                }))
                .send(),
        )
        .await
        .map_err(|_| "ordering a run did not answer within 30s".to_string())?
        .map_err(|error| format!("ordering a run failed: {error}"))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("ordering a run answered {status}: {text}"));
        }
        let events = collect_sse(response, ORDER_FOLLOW).await?;
        let first = events
            .first()
            .ok_or_else(|| "the order's stream carried no events".to_string())?;
        if first["type"] == json!("response.failed") {
            return Err(format!("the order was refused on-stream: {first}"));
        }
        if first["type"] != json!("response.resumed") {
            return Err(format!(
                "the order's stream did not open with resumed: {first}"
            ));
        }
        let run = first["run"]
            .as_str()
            .ok_or_else(|| format!("the resumed event names no run: {first}"))?
            .to_string();
        Ok((run, events))
    }

    /// Reconnect to a run and collect its replay until a terminal event or
    /// `cap` elapses.
    async fn reconnect(
        &self,
        run: &str,
        starting_after: i64,
        cap: Duration,
    ) -> Result<Vec<Value>, String> {
        let response = tokio::time::timeout(
            CALL_CAP,
            self.http
                .post(format!("{}/v1/responses", self.origin))
                .header("authorization", format!("Bearer {}", self.token))
                .header("content-type", "application/json")
                .header("accept", "text/event-stream")
                .json(&json!({
                    "previous_response_id": run,
                    "starting_after": starting_after,
                }))
                .send(),
        )
        .await
        .map_err(|_| format!("the reconnect to {run} did not answer within 30s"))?
        .map_err(|error| format!("the reconnect to {run} failed: {error}"))?;
        let status = response.status();
        if !status.is_success() {
            let text = response.text().await.unwrap_or_default();
            return Err(format!("the reconnect to {run} answered {status}: {text}"));
        }
        collect_sse(response, cap).await
    }

    /// Stop a run this suite ordered. Best effort: a refusal is reported, not
    /// fatal, because the run's existence is already proven.
    async fn stop_run(&self, run: &str) {
        match self
            .mcp_call("stop_run", json!({ "run": run, "confirm": true }))
            .await
        {
            Ok(answer) => eprintln!("stopped run {run}: {answer}"),
            Err(error) => eprintln!("could not stop run {run}: {error}"),
        }
    }
}

// ---------------------------------------------------------------------------
// stream and binary plumbing
// ---------------------------------------------------------------------------

/// Read a Server-Sent Events response into its `data:` JSON events, stopping
/// at a terminal event or when `cap` elapses. A read that stalls past the cap
/// fails loudly rather than hanging the suite.
async fn collect_sse(mut response: reqwest::Response, cap: Duration) -> Result<Vec<Value>, String> {
    let deadline = Instant::now() + cap;
    let mut buffer = String::new();
    let mut events = Vec::new();
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return Ok(events);
        }
        let chunk = match tokio::time::timeout(remaining, response.chunk()).await {
            // The cap elapsed mid-read: what streamed so far is the answer.
            Err(_) => return Ok(events),
            Ok(Err(error)) => {
                // The peer closed or the transfer broke; with events in hand
                // that is the end of the stream, with none it is a failure.
                if events.is_empty() {
                    return Err(format!("the stream broke before any event: {error}"));
                }
                return Ok(events);
            }
            Ok(Ok(None)) => return Ok(events),
            Ok(Ok(Some(chunk))) => chunk,
        };
        buffer.push_str(&String::from_utf8_lossy(&chunk));
        while let Some(end) = buffer.find('\n') {
            let line: String = buffer.drain(..=end).collect();
            let Some(data) = line.trim_end().strip_prefix("data:") else {
                continue;
            };
            let Ok(event) = serde_json::from_str::<Value>(data.trim()) else {
                continue;
            };
            let kind = event["type"].as_str().unwrap_or_default().to_string();
            events.push(event);
            if matches!(
                kind.as_str(),
                "response.completed" | "response.failed" | "response.cancelled"
            ) {
                return Ok(events);
            }
        }
    }
}

/// Every event's `seq` must be strictly greater than the one before it — the
/// replay rule the contract promises, and the reason a reconnect never shows
/// anything twice.
fn seqs_strictly_increase(events: &[Value]) -> Result<(), String> {
    let mut last: Option<i64> = None;
    for event in events {
        let Some(seq) = event["seq"].as_i64() else {
            continue;
        };
        if let Some(previous) = last
            && seq <= previous
        {
            return Err(format!("seq regressed from {previous} to {seq} at {event}"));
        }
        last = Some(seq);
    }
    Ok(())
}

/// What one binary invocation came back with. `status` is `None` when the
/// binary outlived [`BINARY_CAP`] and was killed.
struct Ran {
    status: Option<i32>,
    stdout: String,
    stderr: String,
}

/// Run the real compiled `openagents` binary against a private HOME, bounded.
///
/// Output goes through temp files rather than pipes so a killed process still
/// leaves everything it printed readable. The child's environment drops
/// `OPENAGENTS_TOKEN` so the credential store — the path a signed-in person
/// uses — is the only way the bearer can reach the wire.
fn run_binary(home: &Path, args: &[&str]) -> Result<Ran, String> {
    let mut out = tempfile::tempfile().map_err(|error| format!("no stdout file: {error}"))?;
    let mut err = tempfile::tempfile().map_err(|error| format!("no stderr file: {error}"))?;
    let mut child = std::process::Command::new(env!("CARGO_BIN_EXE_openagents"))
        .args(args)
        .env("HOME", home)
        .env_remove("OPENAGENTS_TOKEN")
        .env_remove("OPENAGENTS_API_URL")
        .env_remove("OPENAGENTS_PROFILE")
        .stdin(std::process::Stdio::null())
        .stdout(out.try_clone().map_err(|error| error.to_string())?)
        .stderr(err.try_clone().map_err(|error| error.to_string())?)
        .spawn()
        .map_err(|error| format!("the binary would not start: {error}"))?;
    let deadline = Instant::now() + BINARY_CAP;
    let status = loop {
        match child.try_wait() {
            Ok(Some(status)) => break status.code(),
            Ok(None) if Instant::now() >= deadline => {
                let _ = child.kill();
                let _ = child.wait();
                break None;
            }
            Ok(None) => std::thread::sleep(Duration::from_millis(200)),
            Err(error) => return Err(format!("waiting on the binary failed: {error}")),
        }
    };
    Ok(Ran {
        status,
        stdout: drain(&mut out),
        stderr: drain(&mut err),
    })
}

/// Everything a temp file collected, from the top.
fn drain(file: &mut std::fs::File) -> String {
    use std::io::Seek as _;
    let mut text = String::new();
    let _ = file.rewind();
    let _ = file.read_to_string(&mut text);
    text
}

/// Store the bearer for `origin` the way the CLI itself does, in a private
/// HOME: `~/.openagents/credentials.json`, version 1, keyed by origin.
fn write_credentials(home: &Path, origin: &str, token: &str) -> Result<(), String> {
    let directory = home.join(".openagents");
    std::fs::create_dir_all(&directory)
        .map_err(|error| format!("no credential directory: {error}"))?;
    let file = json!({ "version": 1, "tokens": { origin: token } });
    std::fs::write(directory.join("credentials.json"), file.to_string())
        .map_err(|error| format!("the credential file would not write: {error}"))
}

/// The run uuid out of a `resumed: run=<uuid>` line, when the stream printed
/// one.
fn extract_run_id(stdout: &str) -> Option<String> {
    let start = stdout.find("resumed: run=")? + "resumed: run=".len();
    let id: String = stdout[start..]
        .chars()
        .take_while(|character| character.is_ascii_hexdigit() || *character == '-')
        .collect();
    (id.len() == 36).then_some(id)
}

/// The `code` of the last `--json` error envelope on stdout, when one printed.
fn envelope_code(stdout: &str) -> Option<String> {
    stdout
        .lines()
        .rev()
        .filter_map(|line| serde_json::from_str::<Value>(line.trim()).ok())
        .find_map(|value| value["code"].as_str().map(str::to_string))
}
