//! RLM (Recursive Language Model) visualization page - interactive execution movie

use k256::schnorr::{Signature, VerifyingKey};
use k256::schnorr::signature::Verifier;
use serde::{Deserialize, Serialize};
use worker::*;

use crate::db::{rlm, users};
use crate::AuthenticatedUser;

/// View the RLM page: /rlm
/// Interactive visualization of Recursive Language Model execution showing
/// structure discovery, chunking, extraction, and synthesis phases.
pub async fn view_rlm(_env: Env) -> Result<Response> {
    let html = r#"<!DOCTYPE html>
<html>
<head>
    <meta charset="utf-8">
    <meta name="viewport" content="width=device-width, initial-scale=1">
    <title>RLM Execution Visualizer - OpenAgents</title>
    <meta name="description" content="Interactive visualization of Recursive Language Model execution - watch AI decompose documents and synthesize answers in real-time.">
    <meta property="og:title" content="RLM Execution Visualizer - OpenAgents">
    <meta property="og:description" content="Watch Recursive Language Models process documents: structure discovery, semantic chunking, parallel extraction, and synthesis.">
    <meta property="og:type" content="website">
    <meta property="og:url" content="https://openagents.com/rlm">
    <link rel="stylesheet" href="/static/MyWebfontsKit.css">
    <style>
        html, body {
            margin: 0;
            padding: 0;
            background: #0a0a0a;
            color: #e0e0e0;
            font-family: 'Square721StdRoman', sans-serif;
            overflow: hidden;
            width: 100%;
            height: 100%;
        }
        #hud-container {
            width: 100vw;
            height: 100vh;
            overflow: hidden;
        }
        canvas {
            width: 100%;
            height: 100%;
            display: block;
        }
    </style>
</head>
<body>
    <div id="hud-container">
        <canvas id="canvas"></canvas>
    </div>
    <script type="module">
        window.RLM_PAGE = true;

        import init, { start_demo } from '/pkg/openagents_web_client.js';

        async function run() {
            await init();
            await start_demo('canvas');
        }

        run().catch(console.error);
    </script>
</body>
</html>"#;

    let headers = Headers::new();
    headers.set("Content-Type", "text/html; charset=utf-8")?;
    headers.set("Cross-Origin-Opener-Policy", "same-origin")?;
    headers.set("Cross-Origin-Embedder-Policy", "require-corp")?;
    headers.set("X-Frame-Options", "SAMEORIGIN")?;

    Ok(Response::ok(html)?.with_headers(headers))
}

#[derive(Debug, Serialize)]
struct RlmRunResponse {
    id: String,
    query: String,
    status: String,
    fragment_count: i64,
    budget_sats: i64,
    total_cost_sats: i64,
    total_duration_ms: i64,
    output: Option<String>,
    error_message: Option<String>,
    created_at: i64,
    completed_at: Option<i64>,
}

#[derive(Debug, Serialize)]
struct TraceResponse {
    run_id: String,
    events: Vec<rlm::RlmTraceEventRecord>,
}

#[derive(Debug, Deserialize)]
struct SyncRequest {
    pubkey: String,
    sig: String,
    payload: String,
}

#[derive(Debug, Deserialize)]
struct SyncPayload {
    run: SyncRun,
    trace_events: Vec<SyncTraceEvent>,
}

#[derive(Debug, Deserialize)]
struct SyncRun {
    id: String,
    query: String,
    status: String,
    fragment_count: i64,
    budget_sats: i64,
    total_cost_sats: i64,
    total_duration_ms: i64,
    output: Option<String>,
    error_message: Option<String>,
    created_at: Option<i64>,
    completed_at: Option<i64>,
}

#[derive(Debug, Deserialize)]
struct SyncTraceEvent {
    seq: i64,
    event_type: String,
    timestamp_ms: i64,
    event_json: String,
}

/// GET /api/rlm/runs
pub async fn list_runs(
    user: AuthenticatedUser,
    env: Env,
    limit: u32,
    before: Option<i64>,
) -> Result<Response> {
    let db = env.d1("DB")?;
    let records = rlm::list_runs(&db, &user.user_id, limit, before).await?;
    let response: Vec<RlmRunResponse> = records.into_iter().map(to_run_response).collect();
    Response::from_json(&response)
}

/// GET /api/rlm/runs/:id
pub async fn get_run(user: AuthenticatedUser, env: Env, run_id: String) -> Result<Response> {
    let db = env.d1("DB")?;
    let record = rlm::get_run(&db, &user.user_id, &run_id).await?;
    match record {
        Some(record) => Response::from_json(&to_run_response(record)),
        None => Response::error("Run not found", 404),
    }
}

/// GET /api/rlm/runs/:id/trace
pub async fn get_trace(user: AuthenticatedUser, env: Env, run_id: String) -> Result<Response> {
    let db = env.d1("DB")?;
    let record = rlm::get_run(&db, &user.user_id, &run_id).await?;
    if record.is_none() {
        return Response::error("Run not found", 404);
    }

    let events = rlm::list_trace_events(&db, &run_id).await?;
    Response::from_json(&TraceResponse { run_id, events })
}

/// POST /api/rlm/runs/sync
pub async fn sync(env: Env, body: String) -> Result<Response> {
    let req: SyncRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;

    verify_signature(&req.pubkey, &req.sig, &req.payload)?;

    let payload: SyncPayload = serde_json::from_str(&req.payload)
        .map_err(|e| Error::RustError(format!("Invalid payload: {}", e)))?;

    let db = env.d1("DB")?;
    let user_id = users::get_user_id_by_nostr_pubkey(&db, &req.pubkey).await?;
    let user_id = match user_id {
        Some(id) => id,
        None => return Response::error("Unknown nostr pubkey", 401),
    };

    let created_at = payload.run.created_at.unwrap_or_else(now_ts);
    let run_record = rlm::RlmRunRecord {
        id: payload.run.id.clone(),
        user_id,
        query: payload.run.query,
        status: payload.run.status,
        fragment_count: payload.run.fragment_count,
        budget_sats: payload.run.budget_sats,
        total_cost_sats: payload.run.total_cost_sats,
        total_duration_ms: payload.run.total_duration_ms,
        output: payload.run.output,
        error_message: payload.run.error_message,
        created_at,
        completed_at: payload.run.completed_at,
    };

    rlm::upsert_run(&db, &run_record).await?;

    let events: Vec<rlm::RlmTraceEventRecord> = payload
        .trace_events
        .into_iter()
        .map(|event| rlm::RlmTraceEventRecord {
            run_id: run_record.id.clone(),
            seq: event.seq,
            event_type: event.event_type,
            timestamp_ms: event.timestamp_ms,
            event_json: event.event_json,
        })
        .collect();

    rlm::replace_trace_events(&db, &run_record.id, &events).await?;

    Response::from_json(&serde_json::json!({
        "ok": true,
        "run_id": run_record.id,
        "events": events.len()
    }))
}

/// GET /api/rlm/ws/:type
pub async fn websocket(req: Request, env: Env) -> Result<Response> {
    let url = req.url()?;
    let path = url.path();

    let conn_type = if path.contains("/browser") {
        "browser"
    } else if path.contains("/pylon") {
        "pylon"
    } else {
        return Response::error("Invalid WebSocket path", 400);
    };

    let run_id = url
        .query_pairs()
        .find(|(k, _)| k == "run_id")
        .map(|(_, v)| v.to_string())
        .ok_or_else(|| Error::RustError("Missing run_id".to_string()))?;

    if conn_type == "browser" {
        let user = crate::middleware::auth::authenticate(&req, &env).await
            .map_err(|e| Error::RustError(format!("Unauthorized: {}", e)))?;
        let db = env.d1("DB")?;
        let record = rlm::get_run(&db, &user.user_id, &run_id).await?;
        if record.is_none() {
            return Response::error("Run not found", 404);
        }
    } else {
        let pubkey = url
            .query_pairs()
            .find(|(k, _)| k == "pubkey")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| Error::RustError("Missing pubkey".to_string()))?;
        let sig = url
            .query_pairs()
            .find(|(k, _)| k == "sig")
            .map(|(_, v)| v.to_string())
            .ok_or_else(|| Error::RustError("Missing sig".to_string()))?;

        verify_signature(&pubkey, &sig, &run_id)?;
        let db = env.d1("DB")?;
        if users::get_user_id_by_nostr_pubkey(&db, &pubkey).await?.is_none() {
            return Response::error("Unknown nostr pubkey", 401);
        }
    }

    let namespace = env.durable_object("RLM_RUN_DO")?;
    let id = namespace.id_from_name(&run_id)?;
    let stub = id.get_stub()?;

    let do_url = format!("/ws/{}?run_id={}", conn_type, run_id);
    let mut new_req = Request::new(&do_url, Method::Get)?;
    for (key, value) in req.headers() {
        new_req.headers_mut()?.set(&key, &value)?;
    }

    stub.fetch_with_request(new_req).await
}

fn to_run_response(record: rlm::RlmRunRecord) -> RlmRunResponse {
    RlmRunResponse {
        id: record.id,
        query: record.query,
        status: record.status,
        fragment_count: record.fragment_count,
        budget_sats: record.budget_sats,
        total_cost_sats: record.total_cost_sats,
        total_duration_ms: record.total_duration_ms,
        output: record.output,
        error_message: record.error_message,
        created_at: record.created_at,
        completed_at: record.completed_at,
    }
}

fn verify_signature(pubkey_hex: &str, sig_hex: &str, payload: &str) -> Result<()> {
    let pubkey_bytes = hex::decode(pubkey_hex)
        .map_err(|e| Error::RustError(format!("Invalid pubkey hex: {}", e)))?;
    if pubkey_bytes.len() != 32 {
        return Err(Error::RustError("Invalid pubkey length".to_string()));
    }
    let verifying_key = VerifyingKey::from_bytes(pubkey_bytes.as_slice())
        .map_err(|_| Error::RustError("Invalid pubkey".to_string()))?;

    let sig_bytes = hex::decode(sig_hex)
        .map_err(|e| Error::RustError(format!("Invalid signature hex: {}", e)))?;
    let signature = Signature::try_from(sig_bytes.as_slice())
        .map_err(|_| Error::RustError("Invalid signature".to_string()))?;

    verifying_key
        .verify(payload.as_bytes(), &signature)
        .map_err(|_| Error::RustError("Signature verification failed".to_string()))?;

    Ok(())
}

fn now_ts() -> i64 {
    (js_sys::Date::now() / 1000.0) as i64
}
