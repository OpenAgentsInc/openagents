//! FM Bridge runtime - connects to the local FM Bridge server

use std::cell::RefCell;
use std::rc::Rc;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use wasm_bindgen_futures::JsFuture;

use crate::state::{AppState, FmConnectionStatus, FmStreamStatus};

/// Initialize the FM Bridge runtime
pub(crate) fn init_fm_runtime(state: Rc<RefCell<AppState>>) {
    let state_clone = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        // Check FM Bridge health
        match check_health().await {
            Ok((available, latency)) => {
                let mut s = state_clone.borrow_mut();
                s.fm_viz.connection_status = FmConnectionStatus::Connected;
                s.fm_viz.model_available = available;
                s.fm_viz.ping_latency_ms = Some(latency);
            }
            Err(e) => {
                web_sys::console::log_1(&format!("FM Bridge health check failed: {:?}", e).into());
                let mut s = state_clone.borrow_mut();
                s.fm_viz.connection_status = FmConnectionStatus::Disconnected;
            }
        }
    });

    // Start a demo generation after connection
    let state_clone2 = state.clone();
    wasm_bindgen_futures::spawn_local(async move {
        // Wait a bit for UI to render
        sleep_ms(1500).await;

        // Check if connected
        let connected = {
            let s = state_clone2.borrow();
            matches!(s.fm_viz.connection_status, FmConnectionStatus::Connected)
        };

        if connected {
            if let Err(e) = stream_completion(
                state_clone2,
                "Explain what Apple Foundation Models can do in 2 sentences."
            ).await {
                web_sys::console::error_1(&format!("Stream error: {:?}", e).into());
            }
        }
    });
}

async fn sleep_ms(ms: i32) {
    let promise = js_sys::Promise::new(&mut |resolve, _| {
        let window = web_sys::window().unwrap();
        let _ = window.set_timeout_with_callback_and_timeout_and_arguments_0(&resolve, ms);
    });
    let _ = JsFuture::from(promise).await;
}

async fn check_health() -> Result<(bool, u32), JsValue> {
    let window = web_sys::window().ok_or("no window")?;
    let start = js_sys::Date::now();

    let opts = web_sys::RequestInit::new();
    opts.set_method("GET");
    opts.set_mode(web_sys::RequestMode::Cors);

    let url = "http://localhost:11435/health";
    let request = web_sys::Request::new_with_str_and_init(url, &opts)?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;

    let latency = (js_sys::Date::now() - start) as u32;

    let json = JsFuture::from(resp.json()?).await?;
    let model_available = js_sys::Reflect::get(&json, &"model_available".into())?
        .as_bool()
        .unwrap_or(false);

    Ok((model_available, latency))
}

async fn stream_completion(state: Rc<RefCell<AppState>>, prompt: &str) -> Result<(), JsValue> {
    let window = web_sys::window().ok_or("no window")?;

    // Set streaming status
    {
        let mut s = state.borrow_mut();
        s.fm_viz.stream_status = FmStreamStatus::Streaming;
        s.fm_viz.token_stream.clear();
        s.fm_viz.token_count = 0;
        s.fm_viz.ttft_ms = None;
    }

    let start = js_sys::Date::now();

    // Build request body
    let body = format!(
        r#"{{"messages":[{{"role":"user","content":"{}"}}],"stream":false}}"#,
        prompt.replace('"', r#"\""#)
    );

    let opts = web_sys::RequestInit::new();
    opts.set_method("POST");
    opts.set_body(&JsValue::from_str(&body));
    opts.set_mode(web_sys::RequestMode::Cors);

    let headers = web_sys::Headers::new()?;
    headers.set("Content-Type", "application/json")?;
    opts.set_headers(&headers);

    let url = "http://localhost:11435/v1/chat/completions";
    let request = web_sys::Request::new_with_str_and_init(url, &opts)?;

    let resp_value = JsFuture::from(window.fetch_with_request(&request)).await?;
    let resp: web_sys::Response = resp_value.dyn_into()?;

    // Record TTFT
    let ttft = (js_sys::Date::now() - start) as u64;
    {
        let mut s = state.borrow_mut();
        s.fm_viz.ttft_ms = Some(ttft);
    }

    if !resp.ok() {
        let mut s = state.borrow_mut();
        s.fm_viz.stream_status = FmStreamStatus::Error;
        return Err("Request failed".into());
    }

    let json = JsFuture::from(resp.json()?).await?;

    // Extract content from response
    let choices = js_sys::Reflect::get(&json, &"choices".into())?;
    let choices_arr: js_sys::Array = choices.dyn_into()?;
    if choices_arr.length() > 0 {
        let choice = choices_arr.get(0);
        let message = js_sys::Reflect::get(&choice, &"message".into())?;
        let content = js_sys::Reflect::get(&message, &"content".into())?
            .as_string()
            .unwrap_or_default();

        let total_chars = content.len();
        let stream_start = js_sys::Date::now();

        // Simulate streaming by adding characters one at a time
        for (i, c) in content.chars().enumerate() {
            {
                let mut s = state.borrow_mut();
                s.fm_viz.token_stream.push(c);
                s.fm_viz.token_count = i + 1;

                // Calculate tokens per second
                let elapsed_secs = (js_sys::Date::now() - stream_start) / 1000.0;
                if elapsed_secs > 0.0 {
                    s.fm_viz.tokens_per_sec = (i + 1) as f32 / elapsed_secs as f32;
                }
            }

            // Small delay between characters (simulates streaming)
            sleep_ms(15).await;
        }

        // Final throughput
        {
            let mut s = state.borrow_mut();
            let elapsed_secs = (js_sys::Date::now() - stream_start) / 1000.0;
            if elapsed_secs > 0.0 {
                s.fm_viz.tokens_per_sec = total_chars as f32 / elapsed_secs as f32;
            }
        }
    }

    // Mark complete
    {
        let mut s = state.borrow_mut();
        s.fm_viz.stream_status = FmStreamStatus::Complete;
    }

    Ok(())
}
