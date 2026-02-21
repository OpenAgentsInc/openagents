//! Browser entry point for Autopilot.
//!
//! This module provides WASM-compatible bindings for spawning Autopilot agents
//! in browser environments. The agent uses the /compute mount to submit NIP-90
//! jobs rather than spawning local processes.

#[cfg(target_arch = "wasm32")]
use wasm_bindgen::prelude::*;

#[cfg(target_arch = "wasm32")]
use js_sys::Math;

#[cfg(target_arch = "wasm32")]
use wasm_bindgen_futures::spawn_local;

use crate::agent::{AutopilotAgent, AutopilotConfig};

#[cfg(target_arch = "wasm32")]
use runtime::{AgentId, BrowserRuntime, BrowserRuntimeConfig};

#[cfg(target_arch = "wasm32")]
use std::sync::OnceLock;

/// Global runtime instance for browser environment.
#[cfg(target_arch = "wasm32")]
static RUNTIME: OnceLock<BrowserRuntime> = OnceLock::new();

/// Generate a random agent ID.
#[cfg(target_arch = "wasm32")]
fn generate_agent_id() -> AgentId {
    let random = (Math::random() * 1_000_000_000.0) as u64;
    let timestamp = js_sys::Date::now() as u64;
    AgentId::new(format!("autopilot-{}-{}", timestamp, random))
}

/// Initialize the global browser runtime.
///
/// This must be called before spawning any agents.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn init_runtime(api_base_url: &str) -> Result<(), JsValue> {
    console_error_panic_hook::set_once();

    let config = BrowserRuntimeConfig::new(api_base_url);
    let runtime = BrowserRuntime::new(config);

    RUNTIME
        .set(runtime)
        .map_err(|_| JsValue::from_str("Runtime already initialized"))?;

    Ok(())
}

/// Spawn an Autopilot agent in the browser.
///
/// This function creates a new AutopilotAgent and registers it with the
/// BrowserRuntime. The agent will use the /compute mount to submit NIP-90
/// jobs for planning, execution, and code review.
///
/// # Arguments
/// * `repo_url` - The repository URL to work on
/// * `issue_description` - The issue description to solve
///
/// # Returns
/// The agent ID as a string, which can be used to track progress via /hud.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn spawn_autopilot(repo_url: &str, issue_description: &str) -> Result<String, JsValue> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| JsValue::from_str("Runtime not initialized. Call init_runtime first."))?;

    let repo = repo_url.to_string();
    let issue = issue_description.to_string();
    let agent_id = generate_agent_id();
    let agent_id_str = agent_id.to_string();

    runtime
        .register_agent_with(agent_id, |env| AutopilotAgent::new(env, repo, issue))
        .map_err(|e| JsValue::from_str(&format!("Failed to register agent: {}", e)))?;

    Ok(agent_id_str)
}

/// Spawn an Autopilot agent with custom configuration.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn spawn_autopilot_with_config(
    repo_url: &str,
    issue_description: &str,
    model: &str,
    max_cost_per_tick_usd: u64,
    max_cost_per_day_usd: u64,
) -> Result<String, JsValue> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| JsValue::from_str("Runtime not initialized. Call init_runtime first."))?;

    let repo = repo_url.to_string();
    let issue = issue_description.to_string();
    let config = AutopilotConfig {
        model: model.to_string(),
        max_cost_per_tick_usd,
        max_cost_per_day_usd,
    };
    let agent_id = generate_agent_id();
    let agent_id_str = agent_id.to_string();

    runtime
        .register_agent_with(agent_id, |env| {
            AutopilotAgent::with_config(env, repo, issue, config)
        })
        .map_err(|e| JsValue::from_str(&format!("Failed to register agent: {}", e)))?;

    Ok(agent_id_str)
}

/// Trigger a manual tick for an agent (async via spawn_local).
///
/// The tick runs asynchronously. Check agent status via list_agents or
/// read from the /hud mount to observe progress.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn tick_agent(agent_id: String) {
    spawn_local(async move {
        if let Some(runtime) = RUNTIME.get() {
            let id = AgentId::new(&agent_id);
            if let Err(e) = runtime.tick_manual(&id).await {
                web_sys::console::error_1(&JsValue::from_str(&format!("Tick failed: {}", e)));
            }
        }
    });
}

/// List all registered agent IDs.
#[cfg(target_arch = "wasm32")]
#[wasm_bindgen]
pub fn list_agents() -> Result<Vec<JsValue>, JsValue> {
    let runtime = RUNTIME
        .get()
        .ok_or_else(|| JsValue::from_str("Runtime not initialized"))?;

    let ids = runtime.list_agents();
    Ok(ids
        .into_iter()
        .map(|id| JsValue::from_str(id.as_str()))
        .collect())
}

// Non-WASM stubs for native builds (allows compiling with native feature)
#[cfg(not(target_arch = "wasm32"))]
pub fn init_runtime(_api_base_url: &str) -> Result<(), String> {
    Err("init_runtime is only available in WASM builds".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_autopilot(_repo_url: &str, _issue_description: &str) -> Result<String, String> {
    Err("spawn_autopilot is only available in WASM builds".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn spawn_autopilot_with_config(
    _repo_url: &str,
    _issue_description: &str,
    _model: &str,
    _max_cost_per_tick_usd: u64,
    _max_cost_per_day_usd: u64,
) -> Result<String, String> {
    Err("spawn_autopilot_with_config is only available in WASM builds".to_string())
}

#[cfg(not(target_arch = "wasm32"))]
pub fn tick_agent(_agent_id: String) {
    // No-op in native builds
}

#[cfg(not(target_arch = "wasm32"))]
pub fn list_agents() -> Result<Vec<String>, String> {
    Err("list_agents is only available in WASM builds".to_string())
}
