use leptos::prelude::*;
use serde::Deserialize;
use wasm_bindgen::prelude::*;
use wasm_bindgen_futures::JsFuture;
use wasm_bindgen::JsValue;

#[wasm_bindgen]
extern "C" {
    #[wasm_bindgen(js_namespace = ["__TAURI__", "tauri"], js_name = invoke)]
    fn tauri_invoke(cmd: &str, args: JsValue) -> js_sys::Promise;
}

#[derive(Clone, Debug, Default, Deserialize)]
struct UiAuthStatus {
    method: Option<String>,
    email: Option<String>,
    plan: Option<String>,
}

async fn fetch_auth_status() -> UiAuthStatus {
    let args = js_sys::Object::new();
    let promise = tauri_invoke("get_auth_status", JsValue::from(args));
    match JsFuture::from(promise).await {
        Ok(val) => serde_wasm_bindgen::from_value::<UiAuthStatus>(val).unwrap_or_default(),
        Err(_) => UiAuthStatus::default(),
    }
}

#[component]
pub fn App() -> impl IntoView {
    // Load auth status on mount
    let status = create_resource(|| (), |_| async { fetch_auth_status().await });

    view! {
        <div class="container">
            <div class="title">"OpenAgents"</div>
            <div style="margin-top: 0.75rem; font-size: 0.95rem; opacity: 0.9;">
                {move || match status.get() {
                    Some(s) => {
                        let method = s.method.unwrap_or_else(|| "Not logged in".to_string());
                        let email = s.email.unwrap_or_default();
                        let plan = s.plan.unwrap_or_default();
                        let extra = match (email.is_empty(), plan.is_empty()) {
                            (true, true) => String::new(),
                            (false, true) => format!(" · {}", email),
                            (true, false) => format!(" · {}", plan),
                            (false, false) => format!(" · {} · {}", email, plan),
                        };
                        format!("Auth: {}{}", method, extra)
                    }
                    None => "Auth: Loading…".to_string()
                }}
            </div>
        </div>
    }
}
