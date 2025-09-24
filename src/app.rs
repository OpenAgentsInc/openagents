use leptos::prelude::*;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::{JsFuture, spawn_local};
use wasm_bindgen::JsValue;

#[wasm_bindgen]
extern "C" {
    // Tauri v2 global API
    #[wasm_bindgen(js_namespace = ["__TAURI__", "core"], js_name = invoke)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> js_sys::Promise;
}

#[derive(Clone, Debug, Default, Deserialize)]
struct WorkspaceStatus { path: Option<String>, approval_mode: Option<String>, sandbox: Option<String>, agents_files: Vec<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct AccountStatus { signed_in_with: Option<String>, login: Option<String>, plan: Option<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct ModelStatus { name: Option<String>, provider: Option<String>, reasoning_effort: Option<String>, reasoning_summaries: Option<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct ClientStatus { cli_version: Option<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct TokenUsageStatus { session_id: Option<String>, input: Option<u64>, output: Option<u64>, total: Option<u64> }
#[derive(Clone, Debug, Default, Deserialize)]
struct UsageLimitsStatus { note: Option<String> }
#[derive(Clone, Debug, Default, Deserialize)]
struct FullStatus { workspace: WorkspaceStatus, account: AccountStatus, model: ModelStatus, client: ClientStatus, token_usage: TokenUsageStatus, usage_limits: UsageLimitsStatus }

async fn fetch_full_status() -> FullStatus {
    let args = js_sys::Object::new();
    let promise = tauri_invoke("get_full_status", JsValue::from(args));
    match JsFuture::from(promise).await {
        Ok(val) => serde_wasm_bindgen::from_value::<FullStatus>(val).unwrap_or_default(),
        Err(_) => FullStatus::default(),
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Load auth status on mount
    let full: RwSignal<FullStatus> = RwSignal::new(Default::default());
    let full_setter = full.write_only();
    spawn_local(async move {
        let f = fetch_full_status().await;
        full_setter.set(f);
    });

    view! {
        <div class="h-screen w-full flex items-center justify-center">
            <div class="text-2xl font-normal">"OpenAgents"</div>

            <div class="fixed top-3 right-3 bottom-3 w-96 overflow-auto p-3 border border-white rounded-none bg-white/5 text-white text-[0.95rem] leading-6">
                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"📂 Workspace"</div>
                    <div class="ml-2 opacity-90">{move || format!("• Path: {}", full.get().workspace.path.unwrap_or_else(|| "(unknown)".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Approval Mode: {}", full.get().workspace.approval_mode.unwrap_or_else(|| "(default)".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Sandbox: {}", full.get().workspace.sandbox.unwrap_or_else(|| "(default)".into()))}</div>
                    <div class="ml-2 opacity-90">{move || {
                        let files = full.get().workspace.agents_files;
                        if files.is_empty() { "• AGENTS files: none".to_string() } else { format!("• AGENTS files: {}", files.join(", ")) }
                    }}</div>
                </div>

                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"👤 Account"</div>
                    <div class="ml-2 opacity-90">{move || format!("• Signed in with {}", full.get().account.signed_in_with.unwrap_or_else(|| "Not logged in".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Login: {}", full.get().account.login.unwrap_or_default())}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Plan: {}", full.get().account.plan.unwrap_or_default())}</div>
                </div>

                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"🧠 Model"</div>
                    <div class="ml-2 opacity-90">{move || format!("• Name: {}", full.get().model.name.unwrap_or_else(|| "gpt-5".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Provider: {}", full.get().model.provider.unwrap_or_else(|| "OpenAI".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Reasoning Effort: {}", full.get().model.reasoning_effort.unwrap_or_else(|| "Medium".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Reasoning Summaries: {}", full.get().model.reasoning_summaries.unwrap_or_else(|| "Auto".into()))}</div>
                </div>

                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"💻 Client"</div>
                    <div class="ml-2 opacity-90">{move || format!("• CLI Version: {}", full.get().client.cli_version.unwrap_or_else(|| "0.0.0".into()))}</div>
                </div>

                <div class="space-y-1.5 mb-3">
                    <div class="font-semibold mb-1 opacity-95">"📊 Token Usage"</div>
                    <div class="ml-2 opacity-90">{move || format!("• Session ID: {}", full.get().token_usage.session_id.unwrap_or_else(|| "(not started)".into()))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Input: {}", full.get().token_usage.input.unwrap_or(0))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Output: {}", full.get().token_usage.output.unwrap_or(0))}</div>
                    <div class="ml-2 opacity-90">{move || format!("• Total: {}", full.get().token_usage.total.unwrap_or(0))}</div>
                </div>

                <div class="space-y-1.5 mb-1">
                    <div class="font-semibold mb-1 opacity-95">"⏱️ Usage Limits"</div>
                    <div class="ml-2 opacity-90">{move || format!("• {}", full.get().usage_limits.note.unwrap_or_else(|| "Rate limit data not available yet.".into()))}</div>
                </div>
            </div>
        </div>
    }
}
