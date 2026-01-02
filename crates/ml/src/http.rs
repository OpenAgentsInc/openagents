use crate::error::{MlError, Result};

#[cfg(target_arch = "wasm32")]
pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let mut opts = web_sys::RequestInit::new();
    opts.set_method("GET");

    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|e| MlError::Network(format!("request init failed: {e:?}")))?;
    let window = web_sys::window().ok_or_else(|| MlError::Network("no window".to_string()))?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| MlError::Network(format!("fetch failed: {e:?}")))?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|e| MlError::Network(format!("response cast failed: {e:?}")))?;

    if !resp.ok() {
        return Err(MlError::Network(format!(
            "http error: {}",
            resp.status()
        )));
    }

    let buffer = JsFuture::from(resp.array_buffer().map_err(|e| {
        MlError::Network(format!("array buffer failed: {e:?}"))
    })?)
    .await
    .map_err(|e| MlError::Network(format!("array buffer await failed: {e:?}")))?;

    let array = js_sys::Uint8Array::new(&buffer);
    Ok(array.to_vec())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
pub async fn fetch_bytes(url: &str) -> Result<Vec<u8>> {
    let resp = reqwest::get(url).await?;
    if !resp.status().is_success() {
        return Err(MlError::Network(format!(
            "http error: {}",
            resp.status()
        )));
    }
    Ok(resp.bytes().await?.to_vec())
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
pub async fn fetch_bytes(_url: &str) -> Result<Vec<u8>> {
    Err(MlError::Network(
        "native http fetch requires the native feature".to_string(),
    ))
}

#[cfg(target_arch = "wasm32")]
#[allow(dead_code)]
pub async fn fetch_range(url: &str, start: usize, end: usize) -> Result<Vec<u8>> {
    use wasm_bindgen::JsCast;
    use wasm_bindgen_futures::JsFuture;

    let mut opts = web_sys::RequestInit::new();
    opts.set_method("GET");

    let headers = web_sys::Headers::new()
        .map_err(|e| MlError::Network(format!("headers init failed: {e:?}")))?;
    headers
        .set("Range", &format!("bytes={}-{}", start, end.saturating_sub(1)))
        .map_err(|e| MlError::Network(format!("header set failed: {e:?}")))?;
    opts.set_headers(&headers);

    let request = web_sys::Request::new_with_str_and_init(url, &opts)
        .map_err(|e| MlError::Network(format!("request init failed: {e:?}")))?;
    let window = web_sys::window().ok_or_else(|| MlError::Network("no window".to_string()))?;
    let resp_value = JsFuture::from(window.fetch_with_request(&request))
        .await
        .map_err(|e| MlError::Network(format!("fetch failed: {e:?}")))?;
    let resp: web_sys::Response = resp_value
        .dyn_into()
        .map_err(|e| MlError::Network(format!("response cast failed: {e:?}")))?;

    if !resp.ok() {
        return Err(MlError::Network(format!(
            "http error: {}",
            resp.status()
        )));
    }

    let buffer = JsFuture::from(resp.array_buffer().map_err(|e| {
        MlError::Network(format!("array buffer failed: {e:?}"))
    })?)
    .await
    .map_err(|e| MlError::Network(format!("array buffer await failed: {e:?}")))?;

    let array = js_sys::Uint8Array::new(&buffer);
    Ok(array.to_vec())
}

#[cfg(all(not(target_arch = "wasm32"), feature = "native"))]
#[allow(dead_code)]
pub async fn fetch_range(url: &str, start: usize, end: usize) -> Result<Vec<u8>> {
    let client = reqwest::Client::new();
    let resp = client
        .get(url)
        .header("Range", format!("bytes={}-{}", start, end.saturating_sub(1)))
        .send()
        .await?;
    if !resp.status().is_success() {
        return Err(MlError::Network(format!(
            "http error: {}",
            resp.status()
        )));
    }
    Ok(resp.bytes().await?.to_vec())
}

#[cfg(all(not(target_arch = "wasm32"), not(feature = "native")))]
#[allow(dead_code)]
pub async fn fetch_range(_url: &str, _start: usize, _end: usize) -> Result<Vec<u8>> {
    Err(MlError::Network(
        "native http range fetch requires the native feature".to_string(),
    ))
}
