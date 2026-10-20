//! CAD page data and the viewer mounted over the GPUI canvas (#12334).
use super::*;
use axum::Json;

/// A CAD page opens only when its own lazy module is installed.
pub(crate) fn page_for(
    app: &App,
    headers: &HeaderMap,
    query: Option<&str>,
    path: &str,
) -> Option<Response> {
    let directory = app.config.gpui_chat.as_deref()?;
    if !["openagents_web_cad.js", "openagents_web_cad_bg.wasm"]
        .iter()
        .all(|name| directory.join(name).is_file())
    {
        return None;
    }
    crate::pages::gpui_chat::page_for(app, headers, query, path)
}

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/cad", get(list))
        .route(
            "/v1/cad/world/placements",
            get(|State(app): State<App>, headers: HeaderMap| async move {
                handoff::world_list(&app, &headers).await
            }),
        )
        .route("/v1/cad/{doc}", get(document))
        .route("/v1/cad/{doc}/pull", post(pull))
        .route(
            "/v1/cad/{doc}/configurator",
            get(configurator::data).post(publish),
        )
        .route("/v1/cad/{doc}/configurator/revoke", post(revoke))
        .route("/cad/{file}/viewer", get(viewer))
        .route(
            "/chat/cad/gpui-viewer.js",
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../static/cad-gpui-viewer.js"),
                )
            }),
        )
        .layer(axum::middleware::map_response(
            |mut response: Response| async move {
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("private, no-store"),
                );
                response
            },
        ))
}

async fn list(
    State(app): State<App>,
    headers: HeaderMap,
    Query(query): Query<PageQuery>,
) -> Response {
    let Some(scope) = page_scope(&app, &headers, query.project.as_deref()).await else {
        return crate::not_found_for(&headers);
    };
    match load_index(&app.config.chat_store, &scope).await {
        Ok(index) => {
            Json(json!({"docs":index.docs,"pro":crate::cad_gate::plan(&app,&headers).await.pro()}))
                .into_response()
        }
        Err(error) => (StatusCode::SERVICE_UNAVAILABLE, error).into_response(),
    }
}

async fn document(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let Some(scope) = page_scope(&app, &headers, query.project.as_deref()).await else {
        return crate::not_found_for(&headers);
    };
    let Ok(index) = load_index(&app.config.chat_store, &scope).await else {
        return crate::not_found_for(&headers);
    };
    let Some(entry) = index.docs.get(&doc) else {
        return crate::not_found_for(&headers);
    };
    let version = query
        .v
        .filter(|v| *v > 0 && *v <= entry.versions)
        .unwrap_or(entry.versions);
    let record = read_file(
        &app.config.chat_store,
        &scope,
        &doc,
        version,
        "version.json",
    )
    .await
    .and_then(|b| serde_json::from_slice::<Value>(&b).ok());
    let pro = crate::cad_gate::plan(&app, &headers).await.pro();
    Json(json!({"title":entry.title,"version":version,"history":entry.history,"record":record,
        "edits":scope.edits && pro && engine().is_some(), "share":configurator_owner(&scope),
        "pro":pro,"viewer":app.config.cad.is_some(),"glb":download_url(&scope,&doc,version,"model.glb")})).into_response()
}

fn configurator_owner(scope: &Scope) -> bool {
    scope.edits && scope.actor == scope.home
}

/// The bridge already checks same-origin and CSRF; the existing form handlers
/// check the supplied value again and enforce scope, plan, and version rules.
fn form_body(headers: &HeaderMap, mut body: Value) -> Option<Value> {
    let fields = body.as_object_mut()?;
    fields.insert(
        "csrf".into(),
        json!(
            headers
                .get(crate::cookie_api::CSRF_HEADER)
                .and_then(|h| h.to_str().ok())
                .unwrap_or("")
        ),
    );
    Some(body)
}

async fn pull(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Json(body): Json<Value>,
) -> Response {
    let Some(form) =
        form_body(&headers, body).and_then(|body| serde_json::from_value::<PullForm>(body).ok())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    pull_form(State(app), headers, UrlPath(doc), Form(form)).await
}

fn link_response(response: Response) -> Response {
    if response.status().is_redirection() {
        if let Some(url) = response
            .headers()
            .get(header::LOCATION)
            .and_then(|v| v.to_str().ok())
        {
            return Json(json!({"url":url})).into_response();
        }
    }
    response
}
async fn publish(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Json(body): Json<Value>,
) -> Response {
    let Some(form) = form_body(&headers, body)
        .and_then(|body| serde_json::from_value::<configurator::Publish>(body).ok())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    link_response(configurator::publish(State(app), headers, UrlPath(doc), Form(form)).await)
}
async fn revoke(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Json(body): Json<Value>,
) -> Response {
    let Some(form) = form_body(&headers, body)
        .and_then(|body| serde_json::from_value::<configurator::Revoke>(body).ok())
    else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    link_response(configurator::revoke(State(app), headers, UrlPath(doc), Form(form)).await)
}

async fn viewer(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let Some(scope) = page_scope(&app, &headers, query.project.as_deref()).await else {
        return crate::not_found_for(&headers);
    };
    let Ok(index) = load_index(&app.config.chat_store, &scope).await else {
        return crate::not_found_for(&headers);
    };
    let Some(entry) = index.docs.get(&doc) else {
        return crate::not_found_for(&headers);
    };
    let version = query
        .v
        .filter(|v| *v > 0 && *v <= entry.versions)
        .unwrap_or(entry.versions);
    let body = html! {
        (maud::DOCTYPE)
        html lang="en" {
            head {
                meta charset="utf-8";
                meta name="viewport" content="width=device-width,initial-scale=1";
                title {(entry.title)}
                link rel="stylesheet" href=(crate::cad_lab::ASSET_CSS);
                script src=(crate::cad_lab::ASSET_JS) defer {}
                script src="/chat/cad/gpui-viewer.js" defer {}
                @if app.config.cad.is_some() { script type="module" src=(DOC_JS) {} }
            }
            body.oa-cad-frame {
                section data-cad-doc-page="" data-cad-doc=(&doc) data-cad-version=(version) data-cad-document=(file_url(&scope,&doc,version,"doc.json")) {
                    (crate::cad_lab::viewer(&file_url(&scope,&doc,version,"view.json"),Some(&file_url(&scope,&doc,version,"hero.png")),&entry.title,&doc,version))
                    // The GPUI forms invoke these worker commands through the frame.
                    div hidden {
                        span id="cad-doc-title" {(entry.title)}
                        @if app.config.cad.is_some() {
                            @for format in ["step","stl","3mf","pdf","dxf"] {
                                button data-cad-download=(format) disabled {(format)}
                            }
                        }
                        p data-cad-download-status="" {}
                    }
                }
            }
        }
    };
    let mut response = crate::chat_html::protect(body.into_response());
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY, HeaderValue::from_static("default-src 'none'; style-src 'self'; img-src 'self' data:; script-src 'self' 'wasm-unsafe-eval'; connect-src 'self'; worker-src 'self'; frame-ancestors 'self'; base-uri 'none'"));
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_json_form_uses_the_header_ticket_and_rejects_non_objects() {
        let mut headers = HeaderMap::new();
        headers.insert(
            crate::cookie_api::CSRF_HEADER,
            HeaderValue::from_static("from-header"),
        );
        assert!(form_body(&headers, json!([])).is_none());
        assert!(form_body(&headers, Value::Null).is_none());
        assert_eq!(
            form_body(&headers, json!({"csrf":"from-body"})).unwrap()["csrf"],
            "from-header"
        );
    }
}
