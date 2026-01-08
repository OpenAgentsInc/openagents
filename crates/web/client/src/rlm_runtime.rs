use std::cell::RefCell;
use std::rc::Rc;

use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;
use web_sys::{Event, MessageEvent, WebSocket};

use crate::state::{AppState, RlmRunSummary, RlmTraceEventRecord, RlmTraceEventView};

#[derive(serde::Deserialize)]
struct TraceResponse {
    run_id: String,
    events: Vec<RlmTraceEventRecord>,
}

pub(crate) fn init_rlm_list_runtime(state: Rc<RefCell<AppState>>) {
    {
        let mut state = state.borrow_mut();
        state.rlm_list.loading = true;
        state.rlm_list.error = None;
        state.rlm_list.scroll_offset = 0.0;
        state.rlm_list.runs.clear();
        state.rlm_list.row_bounds.clear();
    }

    wasm_bindgen_futures::spawn_local(async move {
        if let Err(err) = fetch_runs(&state).await {
            if let Ok(mut state) = state.try_borrow_mut() {
                state.rlm_list.loading = false;
                state.rlm_list.error = Some(format_js_error("Failed to load runs", err));
            }
        }
    });
}

pub(crate) fn init_rlm_detail_runtime(state: Rc<RefCell<AppState>>, run_id: String) {
    {
        let mut state = state.borrow_mut();
        state.rlm_detail.loading = true;
        state.rlm_detail.trace_loading = true;
        state.rlm_detail.error = None;
        state.rlm_detail.live_error = None;
        state.rlm_detail.scroll_offset = 0.0;
        state.rlm_detail.trace_scroll = 0.0;
        state.rlm_detail.trace_events.clear();
        state.rlm_detail.run = None;
        state.rlm_detail.run_id = Some(run_id.clone());
        state.rlm_detail.live_connected = false;
    }

    wasm_bindgen_futures::spawn_local(async move {
        let run_ok = fetch_run(&state, &run_id).await.is_ok();
        let trace_ok = fetch_trace(&state, &run_id).await.is_ok();
        if run_ok && trace_ok {
            connect_rlm_ws(state.clone(), run_id);
        }
    });
}

async fn fetch_runs(state: &Rc<RefCell<AppState>>) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window not available"))?;
    let url = "/api/rlm/runs?limit=50";
    let resp_value = JsFuture::from(window.fetch_with_str(url)).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;
    if !resp.ok() {
        return Err(JsValue::from_str(&format!(
            "rlm list fetch failed: {}",
            resp.status()
        )));
    }

    let json = JsFuture::from(resp.json()?).await?;
    let runs: Vec<RlmRunSummary> = serde_wasm_bindgen::from_value(json)
        .map_err(|e| JsValue::from_str(&format!("rlm list decode failed: {}", e)))?;

    if let Ok(mut state) = state.try_borrow_mut() {
        state.rlm_list.loading = false;
        state.rlm_list.runs = runs;
    }

    Ok(())
}

async fn fetch_run(state: &Rc<RefCell<AppState>>, run_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window not available"))?;
    let url = format!("/api/rlm/runs/{}", run_id);
    let resp_value = JsFuture::from(window.fetch_with_str(&url)).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;
    if !resp.ok() {
        let status = resp.status();
        if let Ok(mut state) = state.try_borrow_mut() {
            state.rlm_detail.loading = false;
            state.rlm_detail.error = Some(format!("Run fetch failed (status {}).", status));
        }
        return Err(JsValue::from_str("rlm run fetch failed"));
    }

    let json = JsFuture::from(resp.json()?).await?;
    let run: RlmRunSummary = serde_wasm_bindgen::from_value(json)
        .map_err(|e| JsValue::from_str(&format!("rlm run decode failed: {}", e)))?;

    if let Ok(mut state) = state.try_borrow_mut() {
        state.rlm_detail.loading = false;
        state.rlm_detail.run = Some(run);
    }

    Ok(())
}

async fn fetch_trace(state: &Rc<RefCell<AppState>>, run_id: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or_else(|| JsValue::from_str("window not available"))?;
    let url = format!("/api/rlm/runs/{}/trace", run_id);
    let resp_value = JsFuture::from(window.fetch_with_str(&url)).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;
    if !resp.ok() {
        let status = resp.status();
        if let Ok(mut state) = state.try_borrow_mut() {
            state.rlm_detail.trace_loading = false;
            state.rlm_detail.error = Some(format!("Trace fetch failed (status {}).", status));
        }
        return Err(JsValue::from_str("rlm trace fetch failed"));
    }

    let json = JsFuture::from(resp.json()?).await?;
    let payload: TraceResponse = serde_wasm_bindgen::from_value(json)
        .map_err(|e| JsValue::from_str(&format!("rlm trace decode failed: {}", e)))?;

    let mut events: Vec<RlmTraceEventView> = payload
        .events
        .into_iter()
        .map(build_trace_view)
        .collect();

    events.sort_by_key(|event| event.seq);

    if let Ok(mut state) = state.try_borrow_mut() {
        state.rlm_detail.trace_loading = false;
        state.rlm_detail.trace_events = events;
    }

    Ok(())
}

fn connect_rlm_ws(state: Rc<RefCell<AppState>>, run_id: String) {
    let window = match web_sys::window() {
        Some(window) => window,
        None => return,
    };
    let location = window.location();
    let protocol = location.protocol().unwrap_or_else(|_| "https:".to_string());
    let host = location.host().unwrap_or_default();
    if host.is_empty() {
        return;
    }

    let ws_scheme = if protocol == "https:" { "wss" } else { "ws" };
    let ws_url = format!("{}://{}/api/rlm/ws/browser?run_id={}", ws_scheme, host, run_id);

    let ws = match WebSocket::new(&ws_url) {
        Ok(ws) => ws,
        Err(_) => return,
    };

    ws.set_binary_type(web_sys::BinaryType::Arraybuffer);

    {
        let state = state.clone();
        let onopen = Closure::wrap(Box::new(move |_event: Event| {
            if let Ok(mut state) = state.try_borrow_mut() {
                state.rlm_detail.live_connected = true;
            }
        }) as Box<dyn FnMut(_)>);
        ws.set_onopen(Some(onopen.as_ref().unchecked_ref()));
        onopen.forget();
    }

    {
        let state = state.clone();
        let onerror = Closure::wrap(Box::new(move |_event: Event| {
            if let Ok(mut state) = state.try_borrow_mut() {
                state.rlm_detail.live_connected = false;
                state.rlm_detail.live_error = Some("WebSocket error".to_string());
            }
        }) as Box<dyn FnMut(_)>);
        ws.set_onerror(Some(onerror.as_ref().unchecked_ref()));
        onerror.forget();
    }

    {
        let state = state.clone();
        let onclose = Closure::wrap(Box::new(move |_event: Event| {
            if let Ok(mut state) = state.try_borrow_mut() {
                state.rlm_detail.live_connected = false;
            }
        }) as Box<dyn FnMut(_)>);
        ws.set_onclose(Some(onclose.as_ref().unchecked_ref()));
        onclose.forget();
    }

    {
        let state = state.clone();
        let onmessage = Closure::wrap(Box::new(move |event: MessageEvent| {
            let data = event.data();
            let text = if let Some(text) = data.as_string() {
                text
            } else {
                return;
            };

            let value: serde_json::Value = match serde_json::from_str(&text) {
                Ok(value) => value,
                Err(_) => return,
            };

            let (event_type, timestamp_ms, event_json) = match extract_event_payload(&value) {
                Some(payload) => payload,
                None => return,
            };

            let view = RlmTraceEventView {
                seq: next_seq(&state),
                event_type: event_type.clone(),
                timestamp_ms,
                summary: summarize_event(&event_type, &event_json),
                event_json,
            };

            if let Ok(mut state) = state.try_borrow_mut() {
                state.rlm_detail.trace_events.push(view);
            }
        }) as Box<dyn FnMut(_)>);
        ws.set_onmessage(Some(onmessage.as_ref().unchecked_ref()));
        onmessage.forget();
    }

    if let Ok(mut state) = state.try_borrow_mut() {
        state.rlm_detail.ws = Some(ws);
    }
}

fn build_trace_view(record: RlmTraceEventRecord) -> RlmTraceEventView {
    let summary = summarize_event(&record.event_type, &record.event_json);
    RlmTraceEventView {
        seq: record.seq,
        event_type: record.event_type,
        timestamp_ms: record.timestamp_ms,
        summary,
        event_json: record.event_json,
    }
}

fn extract_event_payload(
    value: &serde_json::Value,
) -> Option<(String, i64, String)> {
    let mut event_value = value.clone();
    if let Some(event) = value.get("event") {
        event_value = event.clone();
    }

    let event_json = event_value.to_string();
    let event_type = value
        .get("event_type")
        .and_then(|v| v.as_str())
        .map(|s| s.to_string())
        .or_else(|| {
            event_value
                .get("type")
                .and_then(|v| v.as_str())
                .map(|s| to_snake_case(s))
        })?;

    let timestamp_ms = event_value
        .get("timestamp_ms")
        .and_then(|v| v.as_i64())
        .or_else(|| value.get("timestamp_ms").and_then(|v| v.as_i64()))
        .unwrap_or_else(|| (js_sys::Date::now() as i64));

    Some((event_type, timestamp_ms, event_json))
}

fn summarize_event(event_type: &str, event_json: &str) -> String {
    let parsed: serde_json::Value = match serde_json::from_str(event_json) {
        Ok(value) => value,
        Err(_) => return truncate_text(event_json, 160),
    };

    match event_type {
        "run_init" => {
            let fragments = parsed.get("fragment_count").and_then(|v| v.as_i64()).unwrap_or(0);
            let program = parsed
                .get("program")
                .and_then(|v| v.as_str())
                .map(|s| truncate_text(s, 120))
                .unwrap_or_else(|| "run init".to_string());
            format!("program={} fragments={}", program, fragments)
        }
        "run_done" => {
            let cost = parsed.get("total_cost_sats").and_then(|v| v.as_i64()).unwrap_or(0);
            let duration = parsed.get("total_duration_ms").and_then(|v| v.as_i64()).unwrap_or(0);
            let output = parsed
                .get("output")
                .and_then(|v| v.as_str())
                .map(|s| truncate_text(s, 120))
                .unwrap_or_else(|| "run done".to_string());
            format!("cost={} sats duration={}ms output={}", cost, duration, output)
        }
        "env_select_fragments" => {
            let count = parsed
                .get("fragment_ids")
                .and_then(|v| v.as_array())
                .map(|v| v.len())
                .unwrap_or(0);
            format!("selected {} fragments", count)
        }
        "subquery_submit" => {
            let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("?");
            let fragment_id = parsed
                .get("fragment_id")
                .and_then(|v| v.as_str())
                .unwrap_or("-");
            let prompt = parsed
                .get("prompt_preview")
                .and_then(|v| v.as_str())
                .map(|s| truncate_text(s, 120))
                .unwrap_or_else(|| "".to_string());
            format!("query={} fragment={} prompt={}", query_id, fragment_id, prompt)
        }
        "subquery_return" => {
            let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("?");
            let success = parsed.get("success").and_then(|v| v.as_bool()).unwrap_or(false);
            let cost = parsed.get("cost_sats").and_then(|v| v.as_i64()).unwrap_or(0);
            let duration = parsed.get("duration_ms").and_then(|v| v.as_i64()).unwrap_or(0);
            let result = parsed
                .get("result_preview")
                .and_then(|v| v.as_str())
                .map(|s| truncate_text(s, 120))
                .unwrap_or_else(|| "".to_string());
            format!(
                "query={} success={} cost={} sats duration={}ms result={}",
                query_id, success, cost, duration, result
            )
        }
        "subquery_timeout" => {
            let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("?");
            let elapsed = parsed.get("elapsed_ms").and_then(|v| v.as_i64()).unwrap_or(0);
            format!("query={} timeout after {}ms", query_id, elapsed)
        }
        "verify_redundant" => {
            let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("?");
            let agreement = parsed.get("agreement").and_then(|v| v.as_f64()).unwrap_or(0.0);
            let passed = parsed.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
            format!("query={} agreement={:.2} passed={}", query_id, agreement, passed)
        }
        "verify_objective" => {
            let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("?");
            let check = parsed.get("check_type").and_then(|v| v.as_str()).unwrap_or("-");
            let passed = parsed.get("passed").and_then(|v| v.as_bool()).unwrap_or(false);
            format!("query={} check={} passed={}", query_id, check, passed)
        }
        "budget_reserve" => {
            let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("?");
            let amount = parsed.get("amount_sats").and_then(|v| v.as_i64()).unwrap_or(0);
            let remaining = parsed.get("remaining_sats").and_then(|v| v.as_i64()).unwrap_or(0);
            format!("query={} reserve={} sats remaining={}", query_id, amount, remaining)
        }
        "budget_settle" => {
            let query_id = parsed.get("query_id").and_then(|v| v.as_str()).unwrap_or("?");
            let actual = parsed.get("actual_sats").and_then(|v| v.as_i64()).unwrap_or(0);
            let refund = parsed.get("refund_sats").and_then(|v| v.as_i64()).unwrap_or(0);
            format!("query={} actual={} sats refund={}", query_id, actual, refund)
        }
        "aggregate" => {
            let count = parsed.get("input_count").and_then(|v| v.as_i64()).unwrap_or(0);
            let output = parsed
                .get("output_preview")
                .and_then(|v| v.as_str())
                .map(|s| truncate_text(s, 120))
                .unwrap_or_else(|| "".to_string());
            format!("aggregate inputs={} output={}", count, output)
        }
        "fallback_local" => {
            let reason = parsed
                .get("reason")
                .and_then(|v| v.as_str())
                .map(|s| truncate_text(s, 120))
                .unwrap_or_else(|| "fallback".to_string());
            format!("fallback local reason={}", reason)
        }
        _ => truncate_text(event_json, 160),
    }
}

fn to_snake_case(value: &str) -> String {
    let mut out = String::new();
    for (idx, ch) in value.chars().enumerate() {
        if ch.is_uppercase() {
            if idx != 0 {
                out.push('_');
            }
            for lower in ch.to_lowercase() {
                out.push(lower);
            }
        } else {
            out.push(ch);
        }
    }
    out
}

fn truncate_text(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_string();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(3)).collect();
    out.push_str("...");
    out
}

fn next_seq(state: &Rc<RefCell<AppState>>) -> i64 {
    if let Ok(state) = state.try_borrow() {
        return state.rlm_detail.trace_events.len() as i64;
    }
    0
}

fn format_js_error(prefix: &str, err: JsValue) -> String {
    let detail = err.as_string().unwrap_or_else(|| "unknown".to_string());
    format!("{}: {}", prefix, detail)
}
