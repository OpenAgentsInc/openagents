//! Browser/wasm HTTP helpers using fetch.

#![cfg(target_arch = "wasm32")]

use js_sys::{Promise, Uint8Array};
use serde::de::DeserializeOwned;
use wasm_bindgen::{JsCast, JsValue};
use wasm_bindgen_futures::JsFuture;
use web_sys::{Headers, Request, RequestInit, RequestMode, Response, Window, WorkerGlobalScope};

fn fetch_promise(request: &Request) -> Result<Promise, JsValue> {
    if let Some(window) = web_sys::window() {
        return Ok(Window::fetch_with_request(&window, request));
    }
    let global = js_sys::global();
    let scope: WorkerGlobalScope = global.dyn_into()?;
    Ok(scope.fetch_with_request(request))
}

pub async fn request_bytes(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<String>,
) -> Result<(u16, Vec<u8>), String> {
    let mut init = RequestInit::new();
    init.method(method);
    init.mode(RequestMode::Cors);
    if let Some(ref body) = body {
        init.body(Some(&JsValue::from_str(body)));
    }

    let headers = Headers::new().map_err(|err| format!("headers error: {err:?}"))?;
    headers
        .set("accept", "application/json")
        .map_err(|err| format!("headers error: {err:?}"))?;
    if body.is_some() {
        headers
            .set("content-type", "application/json")
            .map_err(|err| format!("headers error: {err:?}"))?;
    }
    if let Some(token) = token {
        headers
            .set("authorization", &format!("Bearer {}", token))
            .map_err(|err| format!("headers error: {err:?}"))?;
    }
    init.headers(&headers);

    let request = Request::new_with_str_and_init(url, &init)
        .map_err(|err| format!("request error: {err:?}"))?;
    let promise = fetch_promise(&request).map_err(|err| format!("fetch error: {err:?}"))?;
    let response_value = JsFuture::from(promise)
        .await
        .map_err(|err| format!("fetch error: {err:?}"))?;
    let response: Response = response_value
        .dyn_into()
        .map_err(|err| format!("response error: {err:?}"))?;
    let status = response.status() as u16;
    let buffer = JsFuture::from(
        response
            .array_buffer()
            .map_err(|err| format!("array_buffer error: {err:?}"))?,
    )
    .await
    .map_err(|err| format!("array_buffer error: {err:?}"))?;
    let array = Uint8Array::new(&buffer);
    let mut bytes = vec![0u8; array.length() as usize];
    array.copy_to(&mut bytes);
    Ok((status, bytes))
}

pub async fn request_json<T: DeserializeOwned>(
    method: &str,
    url: &str,
    token: Option<&str>,
    body: Option<serde_json::Value>,
) -> Result<(u16, T), String> {
    let body = match body {
        Some(value) => Some(
            serde_json::to_string(&value).map_err(|err| format!("json error: {err}"))?,
        ),
        None => None,
    };
    let (status, bytes) = request_bytes(method, url, token, body).await?;
    let parsed =
        serde_json::from_slice(&bytes).map_err(|err| format!("json error: {err}"))?;
    Ok((status, parsed))
}
