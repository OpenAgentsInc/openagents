use worker::*;

const ASSET_BINDING: &str = "ASSETS";

#[event(fetch)]
async fn fetch(req: Request, env: Env, _ctx: Context) -> Result<Response> {
    console_error_panic_hook::set_once();

    let url = req.url()?;
    let path = url.path();

    let mut response = env
        .assets(ASSET_BINDING)?
        .fetch_request(req)
        .await?
        .cloned()?;
    let headers = response.headers_mut();

    headers.set("Cross-Origin-Opener-Policy", "same-origin")?;
    headers.set("Cross-Origin-Embedder-Policy", "require-corp")?;
    headers.set("Cross-Origin-Resource-Policy", "same-origin")?;

    if path.ends_with(".wasm") {
        headers.set("Content-Type", "application/wasm")?;
    } else if path.ends_with(".js") {
        headers.set("Content-Type", "application/javascript; charset=utf-8")?;
    }

    if path.starts_with("/pkg/") {
        headers.set("Cache-Control", "public, max-age=31536000, immutable")?;
    } else if path == "/" || path.ends_with(".html") {
        headers.set("Cache-Control", "no-store")?;
    }

    Ok(response)
}
