use wasm_bindgen::prelude::JsValue;
use wasm_bindgen_futures::JsFuture;

pub(crate) fn js_optional_string(value: &JsValue) -> Option<String> {
    if value.is_null() || value.is_undefined() {
        None
    } else {
        value.as_string()
    }
}

pub(crate) fn track_funnel_event(event: &'static str, repo: Option<String>) {
    wasm_bindgen_futures::spawn_local(async move {
        let window = match web_sys::window() {
            Some(window) => window,
            None => return,
        };

        let body = serde_json::json!({
            "event": event,
            "repo": repo,
        });

        let opts = web_sys::RequestInit::new();
        opts.set_method("POST");
        opts.set_body(&JsValue::from_str(&body.to_string()));

        let headers = match web_sys::Headers::new() {
            Ok(headers) => headers,
            Err(_) => return,
        };
        if headers.set("Content-Type", "application/json").is_err() {
            return;
        }
        opts.set_headers(&headers);

        let _ = JsFuture::from(window.fetch_with_str_and_init(
            "/api/analytics/event",
            &opts,
        ))
        .await;
    });
}

pub(crate) fn copy_to_clipboard(text: String) {
    let Some(window) = web_sys::window() else {
        return;
    };
    let promise = window.navigator().clipboard().write_text(&text);
    wasm_bindgen_futures::spawn_local(async move {
        let _ = JsFuture::from(promise).await;
    });
}

pub(crate) async fn read_clipboard_text() -> Result<String, String> {
    let window = web_sys::window().ok_or_else(|| "Missing window".to_string())?;
    let promise = window.navigator().clipboard().read_text();
    let text = JsFuture::from(promise)
        .await
        .map_err(|_| "Clipboard read failed".to_string())?;
    text.as_string()
        .ok_or_else(|| "Clipboard read failed".to_string())
}
