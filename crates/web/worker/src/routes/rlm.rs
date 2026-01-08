//! RLM (Recursive Language Model) visualization page - interactive execution movie

use k256::schnorr::{Signature, VerifyingKey};
use k256::schnorr::signature::Verifier;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use uuid::Uuid;
use worker::*;

use crate::db::{rlm, users};
use crate::AuthenticatedUser;

/// View the RLM dashboard: /rlm
pub async fn view_rlm(_env: Env) -> Result<Response> {
    render_rlm_page("list", None, None)
}

/// View the RLM demo page: /rlm/demo
pub async fn view_rlm_demo(_env: Env) -> Result<Response> {
    render_rlm_page("demo", None, None)
}

/// View the RLM run detail page: /rlm/runs/:id
pub async fn view_rlm_detail(_env: Env, run_id: String) -> Result<Response> {
    render_rlm_page("detail", Some(&run_id), None)
}

/// View the RLM experiments page: /rlm/experiments
pub async fn view_rlm_experiments(_env: Env) -> Result<Response> {
    render_rlm_page("experiments", None, None)
}

/// View the RLM experiment detail page: /rlm/experiments/:id
pub async fn view_rlm_experiment_detail(_env: Env, experiment_id: String) -> Result<Response> {
    render_rlm_page("experiment_detail", None, Some(&experiment_id))
}

/// View the RLM providers leaderboard page: /rlm/providers
pub async fn view_rlm_providers(_env: Env) -> Result<Response> {
    render_rlm_page("providers", None, None)
}

fn render_rlm_page(mode: &str, run_id: Option<&str>, experiment_id: Option<&str>) -> Result<Response> {
    let mode_json = serde_json::to_string(mode).unwrap_or_else(|_| "\"list\"".to_string());
    let run_id_json = run_id
        .map(|id| serde_json::to_string(id).unwrap_or_else(|_| "null".to_string()))
        .unwrap_or_else(|| "null".to_string());
    let experiment_id_json = experiment_id
        .map(|id| serde_json::to_string(id).unwrap_or_else(|_| "null".to_string()))
        .unwrap_or_else(|| "null".to_string());

    let html = format!(r#"<!DOCTYPE html>
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
        html, body {{
            margin: 0;
            padding: 0;
            background: #0a0a0a;
            color: #e0e0e0;
            font-family: 'Square721StdRoman', sans-serif;
            overflow: hidden;
            width: 100%;
            height: 100%;
        }}
        #hud-container {{
            width: 100vw;
            height: 100vh;
            overflow: hidden;
        }}
        canvas {{
            width: 100%;
            height: 100%;
            display: block;
        }}
    </style>
</head>
<body>
    <div id="hud-container">
        <canvas id="canvas"></canvas>
    </div>
    <script type="module">
        window.RLM_MODE = {mode_json};
        window.RLM_RUN_ID = {run_id_json};
        window.RLM_EXPERIMENT_ID = {experiment_id_json};
        if (window.RLM_MODE === "demo") {{
            window.RLM_PAGE = true;
        }}

        import init, {{ start_demo }} from '/pkg/openagents_web_client.js';

        async function run() {{
            await init();
            await start_demo('canvas');
        }}

        run().catch(console.error);
</script>
</body>
</html>"#);

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

#[derive(Debug, Serialize)]
struct ExperimentSummaryResponse {
    id: String,
    name: String,
    description: Option<String>,
    created_at: i64,
    updated_at: i64,
    run_count: i64,
}

#[derive(Debug, Serialize)]
struct ExperimentRunResponse {
    run: RlmRunResponse,
    label: Option<String>,
    added_at: i64,
}

#[derive(Debug, Serialize)]
struct ExperimentDetailResponse {
    experiment: ExperimentSummaryResponse,
    runs: Vec<ExperimentRunResponse>,
}

#[derive(Debug, Deserialize)]
struct ExperimentCreateRequest {
    name: String,
    description: Option<String>,
}

#[derive(Debug, Deserialize)]
struct ExperimentRunsRequest {
    runs: Vec<rlm::RlmExperimentRunLink>,
}

#[derive(Debug, Serialize)]
struct ProviderStatResponse {
    provider_id: String,
    venue: Option<String>,
    total_queries: i64,
    success_count: i64,
    success_rate: f64,
    total_cost_sats: i64,
    total_duration_ms: i64,
    avg_duration_ms: f64,
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

/// GET /api/rlm/experiments
pub async fn list_experiments(
    user: AuthenticatedUser,
    env: Env,
    limit: u32,
    before: Option<i64>,
) -> Result<Response> {
    let db = env.d1("DB")?;
    let records = rlm::list_experiments(&db, &user.user_id, limit, before).await?;
    let response: Vec<ExperimentSummaryResponse> = records
        .into_iter()
        .map(|record| ExperimentSummaryResponse {
            id: record.id,
            name: record.name,
            description: record.description,
            created_at: record.created_at,
            updated_at: record.updated_at,
            run_count: record.run_count,
        })
        .collect();
    Response::from_json(&response)
}

/// POST /api/rlm/experiments
pub async fn create_experiment(
    user: AuthenticatedUser,
    env: Env,
    body: String,
) -> Result<Response> {
    let req: ExperimentCreateRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;
    let name = req.name.trim();
    if name.is_empty() {
        return Response::error("Name is required", 400);
    }

    let now = now_ts();
    let experiment = rlm::RlmExperimentRecord {
        id: Uuid::new_v4().to_string(),
        user_id: user.user_id.clone(),
        name: name.to_string(),
        description: req.description,
        created_at: now,
        updated_at: now,
    };

    let db = env.d1("DB")?;
    rlm::create_experiment(&db, &experiment).await?;

    Response::from_json(&ExperimentSummaryResponse {
        id: experiment.id,
        name: experiment.name,
        description: experiment.description,
        created_at: experiment.created_at,
        updated_at: experiment.updated_at,
        run_count: 0,
    })
}

/// GET /api/rlm/experiments/:id
pub async fn get_experiment(
    user: AuthenticatedUser,
    env: Env,
    experiment_id: String,
) -> Result<Response> {
    let db = env.d1("DB")?;
    let experiment = rlm::get_experiment(&db, &user.user_id, &experiment_id).await?;
    let Some(experiment) = experiment else {
        return Response::error("Experiment not found", 404);
    };

    let runs = rlm::list_experiment_runs(&db, &user.user_id, &experiment_id).await?;
    let run_responses = runs
        .into_iter()
        .map(|record| ExperimentRunResponse {
            run: RlmRunResponse {
                id: record.run_id,
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
            },
            label: record.label,
            added_at: record.added_at,
        })
        .collect::<Vec<_>>();

    let response = ExperimentDetailResponse {
        experiment: ExperimentSummaryResponse {
            id: experiment.id,
            name: experiment.name,
            description: experiment.description,
            created_at: experiment.created_at,
            updated_at: experiment.updated_at,
            run_count: run_responses.len() as i64,
        },
        runs: run_responses,
    };

    Response::from_json(&response)
}

/// POST /api/rlm/experiments/:id/runs
pub async fn add_experiment_runs(
    user: AuthenticatedUser,
    env: Env,
    experiment_id: String,
    body: String,
) -> Result<Response> {
    let req: ExperimentRunsRequest = serde_json::from_str(&body)
        .map_err(|e| Error::RustError(format!("Invalid request: {}", e)))?;
    if req.runs.is_empty() {
        return Response::error("No runs provided", 400);
    }

    let db = env.d1("DB")?;
    let experiment = rlm::get_experiment(&db, &user.user_id, &experiment_id).await?;
    if experiment.is_none() {
        return Response::error("Experiment not found", 404);
    }

    for link in &req.runs {
        let run = rlm::get_run(&db, &user.user_id, &link.run_id).await?;
        if run.is_none() {
            return Response::error("Run not found", 404);
        }
    }

    rlm::add_runs_to_experiment(&db, &experiment_id, &req.runs).await?;
    rlm::touch_experiment(&db, &experiment_id, now_ts()).await?;

    Response::from_json(&serde_json::json!({
        "ok": true,
        "experiment_id": experiment_id,
        "added": req.runs.len(),
    }))
}

/// DELETE /api/rlm/experiments/:id/runs/:run_id
pub async fn remove_experiment_run(
    user: AuthenticatedUser,
    env: Env,
    experiment_id: String,
    run_id: String,
) -> Result<Response> {
    let db = env.d1("DB")?;
    let experiment = rlm::get_experiment(&db, &user.user_id, &experiment_id).await?;
    if experiment.is_none() {
        return Response::error("Experiment not found", 404);
    }

    rlm::remove_run_from_experiment(&db, &experiment_id, &run_id).await?;
    rlm::touch_experiment(&db, &experiment_id, now_ts()).await?;

    Response::from_json(&serde_json::json!({
        "ok": true,
        "experiment_id": experiment_id,
        "run_id": run_id,
    }))
}

/// GET /api/rlm/providers
pub async fn list_providers(user: AuthenticatedUser, env: Env, limit: u32) -> Result<Response> {
    #[derive(Debug, Deserialize)]
    struct ProviderRow {
        run_id: String,
        event_type: String,
        event_json: String,
    }

    let db = env.d1("DB")?;
    let rows = db
        .prepare(
            "SELECT t.run_id, t.event_type, t.event_json
             FROM rlm_trace_events t
             JOIN rlm_runs r ON r.id = t.run_id
             WHERE r.user_id = ? AND t.event_type IN (?, ?)
             ORDER BY t.run_id, t.seq ASC
             LIMIT ?",
        )
        .bind(&[
            user.user_id.into(),
            "subquery_execute".into(),
            "subquery_return".into(),
            (limit as i64).into(),
        ])?
        .all()
        .await?
        .results::<ProviderRow>()?;

    let mut query_providers: HashMap<String, (String, Option<String>)> = HashMap::new();
    let mut stats: HashMap<String, ProviderStatResponse> = HashMap::new();

    for row in rows {
        let parsed: serde_json::Value = match serde_json::from_str(&row.event_json) {
            Ok(value) => value,
            Err(_) => continue,
        };

        match row.event_type.as_str() {
            "subquery_execute" => {
                let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("");
                let provider_id = parsed.get("provider_id").and_then(|v| v.as_str()).unwrap_or("");
                if query_id.is_empty() || provider_id.is_empty() {
                    continue;
                }
                let venue = parsed.get("venue").and_then(|v| v.as_str()).map(|s| s.to_string());
                let key = format!("{}:{}", row.run_id, query_id);
                query_providers.insert(key, (provider_id.to_string(), venue));
            }
            "subquery_return" => {
                let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("");
                if query_id.is_empty() {
                    continue;
                }
                let key = format!("{}:{}", row.run_id, query_id);
                let Some((provider_id, venue)) = query_providers.get(&key).cloned() else {
                    continue;
                };
                let cost = parsed.get("cost_sats").and_then(|v| v.as_i64()).unwrap_or(0);
                let duration = parsed.get("duration_ms").and_then(|v| v.as_i64()).unwrap_or(0);
                let success = parsed.get("success").and_then(|v| v.as_bool()).unwrap_or(false);

                let key = format!(
                    "{}::{}",
                    provider_id,
                    venue.clone().unwrap_or_default()
                );
                let entry = stats.entry(key).or_insert_with(|| ProviderStatResponse {
                    provider_id,
                    venue: venue.clone(),
                    total_queries: 0,
                    success_count: 0,
                    success_rate: 0.0,
                    total_cost_sats: 0,
                    total_duration_ms: 0,
                    avg_duration_ms: 0.0,
                });
                entry.total_queries += 1;
                if success {
                    entry.success_count += 1;
                }
                entry.total_cost_sats += cost;
                entry.total_duration_ms += duration;
            }
            _ => {}
        }
    }

    let mut response: Vec<ProviderStatResponse> = stats
        .into_values()
        .map(|mut stat| {
            if stat.total_queries > 0 {
                stat.success_rate = stat.success_count as f64 / stat.total_queries as f64;
                stat.avg_duration_ms = stat.total_duration_ms as f64 / stat.total_queries as f64;
            }
            stat
        })
        .collect();

    response.sort_by(|a, b| b.total_cost_sats.cmp(&a.total_cost_sats));
    Response::from_json(&response)
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
