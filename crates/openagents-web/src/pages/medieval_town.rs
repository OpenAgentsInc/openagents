//! Public page shell for a separately deployed proprietary engine release.
use crate::App;
use aethyrforge_host::{Host, Project, endpoint};
use axum::{
    Router,
    http::{HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::get,
};
use maud::{DOCTYPE, PreEscaped, html};
use serde::{Deserialize, Serialize};
const PATH: &str = "/demo/medieval-town";
#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Release {
    engine_url: String,
    sdk_url: String,
    release: String,
    engine_abi: String,
    project_format: u16,
    project: Project,
}
impl Release {
    fn parse(text: &str) -> Option<Self> {
        let r: Self = serde_json::from_str(text).ok()?;
        Host::new(
            &r.engine_url,
            "configuration".into(),
            r.release.clone(),
            r.engine_abi.clone(),
            r.project_format,
            r.project.clone(),
        )
        .ok()?;
        let engine = endpoint(&r.engine_url).ok()?;
        let sdk = endpoint(&r.sdk_url).ok()?;
        (r.release != "unconfigured"
            && engine.origin() == sdk.origin()
            && sdk.path().ends_with("/aethyrforge_host.js"))
        .then_some(r)
    }
    fn origin(&self) -> String {
        endpoint(&self.engine_url)
            .expect("validated URL")
            .origin()
            .ascii_serialization()
    }
}
fn release() -> Option<Release> {
    let text = std::env::var("OPENAGENTS_MEDIEVAL_RELEASE")
        .unwrap_or_else(|_| include_str!("../../content/demos/medieval-town.json").into());
    Release::parse(&text)
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(page))
        .route("/demo/medieval-town/boot.js", get(boot))
        .route("/demo/medieval-town/style.css", get(style))
}
fn response(
    status: StatusCode,
    body: impl Into<String>,
    content_type: &'static str,
    origin: Option<&str>,
) -> Response {
    let mut r = (status, body.into()).into_response();
    let h = r.headers_mut();
    h.insert(header::CONTENT_TYPE, HeaderValue::from_static(content_type));
    h.insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    h.insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    h.insert(
        header::REFERRER_POLICY,
        HeaderValue::from_static("no-referrer"),
    );
    h.insert(
        "permissions-policy",
        HeaderValue::from_static("camera=(), microphone=(), geolocation=(), payment=()"),
    );
    let csp = format!(
        "default-src 'none'; script-src 'self' {} 'wasm-unsafe-eval'; style-src 'self'; connect-src 'self' {}; frame-src {}; frame-ancestors 'self'; base-uri 'none'; form-action 'none'; object-src 'none'",
        origin.unwrap_or("'none'"),
        origin.unwrap_or("'none'"),
        origin.unwrap_or("'none'")
    );
    h.insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_str(&csp).expect("validated origin"),
    );
    r
}
async fn page() -> Response {
    let Some(r) = release() else {
        return response(StatusCode::SERVICE_UNAVAILABLE,html!{(DOCTYPE)html lang="en"{head{meta charset="utf-8";title{"Medieval Town"}}body{h1{"Medieval Town"}p{"The world is unavailable right now."}a href=(PATH){"Try again"}" · " a href="/"{"Back to OpenAgents"}}}}.into_string(),"text/html; charset=utf-8",None);
    };
    let config = serde_json::json!({"release":r.release,"engine_abi":r.engine_abi,"project_format":r.project_format,"project":r.project});
    let text = serde_json::to_string(&config)
        .expect("configuration")
        .replace('<', "\\u003c");
    let body=html!{(DOCTYPE)html lang="en"{head{meta charset="utf-8";meta name="viewport" content="width=device-width,initial-scale=1";title{"Medieval Town · OpenAgents"}link rel="stylesheet" href="/demo/medieval-town/style.css";}body{header{a href="/"{"OpenAgents"}h1{"Medieval Town"}span{"Click the world · WASD to move · Mouse to look · Space / C to rise or descend"}a id="world-retry" href=(PATH){"Reload world"}button id="world-fullscreen" type="button"{"Full screen"}}main{iframe id="engine" title="Medieval Town" data-engine-url=(r.engine_url){}p id="world-notice" role="status" aria-live="polite"{"Opening Medieval Town… The first visit can take a little longer."}}script id="engine-config" type="application/json"{(PreEscaped(text))}script type="module" src="/demo/medieval-town/boot.js"{}}}}.into_string();
    response(
        StatusCode::OK,
        body,
        "text/html; charset=utf-8",
        Some(&r.origin()),
    )
}
async fn boot() -> Response {
    let Some(r) = release() else {
        return response(
            StatusCode::SERVICE_UNAVAILABLE,
            "",
            "text/javascript; charset=utf-8",
            None,
        );
    };
    let url = serde_json::to_string(&r.sdk_url).expect("SDK URL");
    response(
        StatusCode::OK,
        format!(
            "try{{const {{default:init,mount_demo}}=await import({url});await init();window.medievalDemo=mount_demo();}}catch{{const notice=document.getElementById('world-notice');notice.hidden=false;notice.dataset.state='error';notice.textContent='The world could not open. Reload the page to try again.';}}"
        ),
        "text/javascript; charset=utf-8",
        Some(&r.origin()),
    )
}
async fn style() -> Response {
    response(
        StatusCode::OK,
        "html,body{height:100%;margin:0;background:#0b1217;color:#e3eeee;font:14px system-ui}body{display:flex;flex-direction:column}header{display:flex;align-items:center;gap:18px;padding:12px 18px;background:#10202a}h1{font-size:16px;margin:0}header span{flex:1;color:#b5c5cb}a,button{color:inherit}button{background:#1b3641;border:1px solid #517381;padding:6px 10px;border-radius:4px}main{position:relative;flex:1;min-height:0}iframe{width:100%;height:100%;border:0;display:block}#world-notice{position:absolute;left:50%;top:45%;transform:translate(-50%,-50%);max-width:420px;padding:24px;background:#10202aee;border:1px solid #426171;border-radius:8px;line-height:1.5}#world-notice[hidden]{display:none}@media(max-width:800px){header{flex-wrap:wrap;gap:10px}header span{order:5;flex-basis:100%;font-size:12px}}",
        "text/css; charset=utf-8",
        None,
    )
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn release_configuration_requires_one_pinned_origin() {
        let base = serde_json::json!({"engine_url":"https://engine.example/","sdk_url":"https://engine.example/sdk/hash/aethyrforge_host.js","release":"release-1","engine_abi":"abi-1","project_format":1,"project":{"id":"world","branch":"main","revision":0}});
        assert!(Release::parse(&base.to_string()).is_some());
        for (field, value) in [
            ("sdk_url", "https://evil.example/aethyrforge_host.js"),
            ("engine_url", "http://engine.example/"),
            ("release", "unconfigured"),
        ] {
            let mut bad = base.clone();
            bad[field] = value.into();
            assert!(Release::parse(&bad.to_string()).is_none());
        }
    }
}
