//! Progressive GPUI entry for memory and schedules. The HTML forms stay
//! usable without JavaScript or a WebGPU adapter; chat is never loaded here.
use crate::{App, cloud::session::Viewer};
use axum::http::{HeaderMap, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use maud::{Markup, html};

const SCRIPT: &str = "/settings/account-prompts.js";

pub(crate) fn routes() -> axum::Router<App> {
    axum::Router::new().route(
        SCRIPT,
        get(|| async {
            (
                [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                include_str!("../static/gpui-memory-schedules.js"),
            )
                .into_response()
        }),
    )
}

pub(crate) fn entry(
    app: &App,
    headers: &HeaderMap,
    query: Option<&str>,
    viewer: &Viewer,
    page: &str,
) -> Markup {
    if !crate::pages::gpui_chat::choice(app.config.features, headers, query).serves(app, headers) {
        return html! {};
    }
    let Some(directory) = app
        .config
        .gpui_chat
        .as_deref()
        .filter(|d| d.join("openagents_web_settings.js").is_file())
    else {
        return html! {};
    };
    let url = |file| crate::pages::gpui_build::url(Some(directory), "/gpui/build", file);
    let fonts: Vec<_> = [
        "Geist.ttf",
        "Geist-Italic.ttf",
        "Geist-Medium.ttf",
        "Geist-SemiBold.ttf",
        "Geist-SemiBoldItalic.ttf",
        "Geist-Bold.ttf",
        "Geist-BoldItalic.ttf",
        "GeistMono.ttf",
        "GeistMono-Medium.ttf",
        "GeistMono-SemiBold.ttf",
    ]
    .iter()
    .map(|f| format!("/gpui/build/fonts/{f}"))
    .collect();
    let owner = crate::chat_store::account_owner(&viewer.account_id);
    let module = "openagents_web_settings.js";
    let wasm = "openagents_web_settings_bg.wasm";
    html! {
        meta name="openagents-csrf" content=(crate::pages::chat::csrf(app, &owner));
        meta name="openagents-features" content=(app.config.features.meta());
        script #gpui-account-prompts-entry type="module" src=(SCRIPT)
            data-page=(page) data-module=(url(module))
            data-wasm=(url(wasm)) data-fonts=(serde_json::to_string(&fonts).unwrap_or_default()) {}
    }
}

pub(crate) fn finish(
    app: &App,
    headers: &HeaderMap,
    query: Option<&str>,
    mut response: Response,
) -> Response {
    crate::pages::gpui_chat::choice(app.config.features, headers, query)
        .remember(app, &mut response);
    if let Some(policy) = response
        .headers()
        .get(header::CONTENT_SECURITY_POLICY)
        .and_then(|v| v.to_str().ok())
    {
        let policy = policy.replace("script-src 'self'", "script-src 'self' 'wasm-unsafe-eval'");
        if let Ok(policy) = policy.parse() {
            response
                .headers_mut()
                .insert(header::CONTENT_SECURITY_POLICY, policy);
        }
    }
    response
}
