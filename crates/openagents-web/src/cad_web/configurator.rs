//! Public, revocable snapshots; visitor edits run only in their browser.
use super::*;
use openagents_ui::forms::Slider;

const SCRIPT: &str = "/chat/cad/configurator.js";
#[derive(Serialize, Deserialize)]
struct Shared {
    home: String,
    folder: String,
    doc: String,
    data: Value,
}
fn valid(token: &str) -> bool {
    token.len() == 48
        && token
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn key(token: &str) -> String {
    format!("cad-configurators/{token}/snapshot.json")
}
fn list_key(scope: &Scope, doc: &str, token: &str) -> Result<String, String> {
    scope
        .key(&format!("{doc}/configurators/{token}.json"))
        .map_err(|_| "The link couldn't be saved.".into())
}
async fn tokens(store: &Store, scope: &Scope, doc: &str) -> Result<Vec<String>, String> {
    let folder = scope
        .key(&format!("{doc}/configurators"))
        .map_err(|_| "This design couldn't be read.")?;
    Ok(store
        .folder_keys(&folder)
        .await
        .map_err(|_| "The links couldn't be read.")?
        .iter()
        .filter_map(|key| {
            key.rsplit('/')
                .next()?
                .strip_suffix(".json")
                .filter(|s| valid(s))
                .map(str::to_owned)
        })
        .collect())
}
async fn remove(store: &Store, scope: &Scope, doc: &str, token: &str) -> Result<(), String> {
    if !valid(token) {
        return Err("This link couldn't be found.".into());
    }
    if let Some((bytes, _)) = store
        .read_key(&key(token))
        .await
        .map_err(|_| "The link couldn't be read.")?
    {
        let saved: Shared =
            serde_json::from_slice(&bytes).map_err(|_| "The link couldn't be read.")?;
        if saved.home != scope.home || saved.folder != scope.folder || saved.doc != doc {
            return Err("This link belongs to another design.".into());
        }
        store
            .remove_folder(&format!("cad-configurators/{token}"))
            .await
            .map_err(|_| "The link couldn't be removed.")?;
    }
    let list = list_key(scope, doc, token)?;
    if let Some((_, generation)) = store
        .read_key(&list)
        .await
        .map_err(|_| "The link couldn't be read.")?
    {
        store
            .delete_key(&list, &generation)
            .await
            .map_err(|_| "The link couldn't be removed.")?;
    }
    Ok(())
}
pub(super) async fn purge(store: &Store, scope: &Scope, doc: &str) -> Result<(), String> {
    for token in tokens(store, scope, doc).await? {
        remove(store, scope, doc, &token).await?;
    }
    Ok(())
}

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cad/{file}/configurator", get(settings).post(publish))
        .route("/cad/{file}/configurator/revoke", post(revoke))
        .route("/cad-config/{token}", get(public))
        .route("/cad-config/{token}/snapshot", get(snapshot))
        .route(
            SCRIPT,
            get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../static/cad-configurator.js"),
                )
            }),
        )
}
fn private(scope: &Scope) -> bool {
    scope.edits && scope.actor == scope.home
}
async fn settings(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let Some(scope) = page_scope(&app, &headers, query.project.as_deref())
        .await
        .filter(private)
    else {
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
    let Some(engine) = engine().filter(|_| app.config.cad.is_some()) else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "The CAD engine isn't available. Try again later.",
        )
            .into_response();
    };
    let result = engine
        .run(
            &app.config.chat_store,
            &scope,
            "cad_config_options",
            &json!({"doc":doc,"version":version}),
        )
        .await;
    let Some(options) = result["options"].as_array() else {
        return (
            StatusCode::CONFLICT,
            "This design's dimensions couldn't be read.",
        )
            .into_response();
    };
    let csrf = crate::pages::chat::csrf(&app, &scope.actor);
    let links = tokens(&app.config.chat_store, &scope, &doc)
        .await
        .unwrap_or_default();
    let offers = super::sales::listings(&app.config.chat_store, &scope, &doc)
        .await
        .unwrap_or_default();
    let action = format!("/cad/{doc}/configurator");
    let mut response = crate::ui_page::UiPage::new("Share a configurable design").head(html!{link rel="stylesheet" href=(crate::cad_lab::ASSET_CSS);script type="module" src=(SCRIPT) {}}).content(PageColumn::new(html!{
        h1 {"Share " (&entry.title)}
        p {"Choose up to eight sliders and their limits. Create a free link or list this version for sale."}
        form method="post" action=(&action) hx-boost="false" data-cad-config-form="" {
            input type="hidden" name="csrf" value=(&csrf);input type="hidden" name="project" value=(scope.project.as_deref().unwrap_or(""));input type="hidden" name="version" value=(version);input type="hidden" name="controls";
            @for option in options {
                @let value=option["value"].as_f64().unwrap_or_default();
                fieldset.oa-cad-config-option data-cad-config-option=(option["path"].as_str().unwrap_or("")) {
                    label { input type="checkbox" data-config-chosen=""; (option["label"].as_str().unwrap_or("Dimension")) }
                    label {"Name " input data-config-label="" value=(option["label"].as_str().unwrap_or("Dimension")) maxlength="80";}
                    label {"Minimum " input type="number" step="any" data-config-min="" value=(value.min(value*0.5)-1.0);}
                    label {"Maximum " input type="number" step="any" data-config-max="" value=(value.max(value*1.5)+1.0);}
                    label {"Step " input type="number" step="any" min="0.000001" data-config-step="" value="0.1";}
                }
            }
            (Button::new("Create link").kind(ButtonType::Submit).attr("name","mode").attr("value","free").disabled(true))
            details {
                summary {"Sell this version"}
                label {"Price (sats) " input type="number" name="sats" min="1" max="10000000" step="1";}
                label {"Your Lightning address " input type="text" name="lightning" maxlength="256" placeholder="you@your-wallet.com";}
                p {"Payments go directly to this address. Your wallet must support payment checks."}
                (Button::new("List for sale").kind(ButtonType::Submit).attr("name","mode").attr("value","sell").disabled(true))
            }
        }
        @if !offers.is_empty(){h2 {"For sale"}}
        @for (id,title,sats) in offers {div.oa-cad-config-option {
            a href=(format!("/cad-shop/{id}")) {(title) " · " (sats) " sats"}
            form method="post" action=(format!("/cad/{doc}/unlist")) {input type="hidden" name="csrf" value=(&csrf);input type="hidden" name="project" value=(scope.project.as_deref().unwrap_or(""));input type="hidden" name="token" value=(id);(Button::new("Stop selling").kind(ButtonType::Submit).variant(ButtonVariant::Outline))}
        }}
        @if !links.is_empty() {h2 {"Shared links"}}
        @for token in links {
            div.oa-cad-config-option {
                a href=(format!("/cad-config/{token}")) {"Open shared design"}
                form method="post" action=(format!("{action}/revoke")) {
                    input type="hidden" name="csrf" value=(&csrf);input type="hidden" name="project" value=(scope.project.as_deref().unwrap_or(""));input type="hidden" name="token" value=(token);
                    (Button::new("Stop sharing").variant(ButtonVariant::Outline).kind(ButtonType::Submit))
                }
            }
        }
    })).respond(&headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(crate::pages::cad::CAD_POLICY),
    );
    response
}
#[derive(Deserialize)]
struct Publish {
    csrf: String,
    #[serde(default)]
    project: String,
    version: u64,
    controls: String,
    #[serde(default)]
    mode: String,
    #[serde(default)]
    sats: String,
    #[serde(default)]
    lightning: String,
}
async fn publish(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Form(form): Form<Publish>,
) -> Response {
    if crate::pages::chat::validate_form(&app, &headers, &form.csrf)
        .await
        .is_err()
    {
        return (StatusCode::FORBIDDEN, "Reload the page and try again.").into_response();
    }
    let Some(scope) = page_scope(&app, &headers, Some(&form.project))
        .await
        .filter(private)
    else {
        return crate::not_found_for(&headers);
    };
    if form.controls.len() > 8192 {
        return (StatusCode::BAD_REQUEST, "Choose up to eight sliders.").into_response();
    }
    let Ok(controls) = serde_json::from_str::<Value>(&form.controls) else {
        return (StatusCode::BAD_REQUEST, "Check the slider settings.").into_response();
    };
    let Some(engine) = engine() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "The CAD engine isn't available.",
        )
            .into_response();
    };
    let result = engine
        .run(
            &app.config.chat_store,
            &scope,
            "cad_configure",
            &json!({"doc":doc,"version":form.version,"controls":controls}),
        )
        .await;
    if let Some(error) = result["error"].as_str() {
        return (StatusCode::BAD_REQUEST, error.to_owned()).into_response();
    }
    let Some(view) = read_file(
        &app.config.chat_store,
        &scope,
        &doc,
        form.version,
        "view.json",
    )
    .await
    .and_then(|b| serde_json::from_slice::<Value>(&b).ok()) else {
        return crate::not_found_for(&headers);
    };
    let assets = match imports::bundle(&app.config.chat_store, &scope, &doc, form.version).await {
        Ok(assets) => assets,
        Err(error) => return (StatusCode::BAD_REQUEST, error).into_response(),
    };
    let data = json!({"title":result["document"]["title"].as_str().unwrap_or(&doc),"version":form.version,"document":result["document"],"controls":result["controls"],"values":result["values"],"assets":assets,"view":view});
    if form.mode == "sell" {
        let sats = match form.sats.parse::<u64>() {
            Ok(sats) => sats,
            Err(_) => return (StatusCode::BAD_REQUEST, "Enter a price in sats.").into_response(),
        };
        return match super::sales::create(
            &app.config.chat_store,
            &scope,
            &doc,
            data,
            sats,
            &form.lightning,
        )
        .await
        {
            Ok(id) => Redirect::to(&format!("/cad-shop/{id}")).into_response(),
            Err(error) => (StatusCode::BAD_REQUEST, error).into_response(),
        };
    }
    let saved = save(&app.config.chat_store, &scope, &doc, data).await;
    match saved {
        Ok(token) => Redirect::to(&format!("/cad-config/{token}")).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error).into_response(),
    }
}
async fn save(store: &Store, scope: &Scope, doc: &str, data: Value) -> Result<String, String> {
    if !private(scope) || !valid_doc(doc) {
        return Err("Only the design's owner can share it.".into());
    }
    if tokens(store, scope, doc).await?.len() >= 20 {
        return Err(
            "This design has 20 shared links. Stop sharing one before making another.".into(),
        );
    }
    let bytes = serde_json::to_vec(&data).map_err(|_| "The design couldn't be shared.")?;
    if bytes.len() > 8 * 1024 * 1024 {
        return Err("This design is too large for a shared configurator.".into());
    }
    let token: String = secp256k1::rand::random::<[u8; 24]>()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect();
    let snapshot = Shared {
        home: scope.home.clone(),
        folder: scope.folder.clone(),
        doc: doc.into(),
        data,
    };
    let bytes = serde_json::to_vec(&snapshot).map_err(|_| "The design couldn't be shared.")?;
    store
        .write_key(&key(&token), bytes, None)
        .await
        .map_err(|_| "The design couldn't be shared.")?;
    if store
        .write_key(&list_key(scope, doc, &token)?, b"{}".to_vec(), None)
        .await
        .is_err()
    {
        let _ = store
            .remove_folder(&format!("cad-configurators/{token}"))
            .await;
        return Err("The design couldn't be shared.".into());
    }
    Ok(token)
}
#[derive(Deserialize)]
struct Revoke {
    csrf: String,
    #[serde(default)]
    project: String,
    token: String,
}
async fn revoke(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Form(form): Form<Revoke>,
) -> Response {
    if crate::pages::chat::validate_form(&app, &headers, &form.csrf)
        .await
        .is_err()
    {
        return (StatusCode::FORBIDDEN, "Reload the page and try again.").into_response();
    }
    let Some(scope) = page_scope(&app, &headers, Some(&form.project))
        .await
        .filter(private)
    else {
        return crate::not_found_for(&headers);
    };
    match remove(&app.config.chat_store, &scope, &doc, &form.token).await {
        Ok(()) => Redirect::to(&scope.page(&doc, None)).into_response(),
        Err(error) => (StatusCode::BAD_REQUEST, error).into_response(),
    }
}
async fn read(store: &Store, token: &str) -> Option<Value> {
    if !valid(token) {
        return None;
    }
    let (bytes, _) = store.read_key(&key(token)).await.ok()??;
    serde_json::from_slice::<Shared>(&bytes)
        .ok()
        .map(|s| s.data)
}
#[derive(Default, Deserialize)]
struct PublicQuery {
    view: Option<u8>,
}
async fn snapshot(
    State(app): State<App>,
    UrlPath(token): UrlPath<String>,
    Query(query): Query<PublicQuery>,
) -> Response {
    let Some(data) = read(&app.config.chat_store, &token).await else {
        return crate::not_found().await;
    };
    (
        [
            (header::CACHE_CONTROL, "no-store, private"),
            (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
        ],
        axum::Json(if query.view.is_some() {
            data["view"].clone()
        } else {
            data
        }),
    )
        .into_response()
}
async fn public(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(token): UrlPath<String>,
) -> Response {
    let Some(data) = read(&app.config.chat_store, &token).await else {
        return crate::not_found_for(&headers);
    };
    let title = data["title"].as_str().unwrap_or("Shared design");
    let mut response=crate::ui_page::UiPage::new(title).head(html!{link rel="stylesheet" href=(crate::cad_lab::ASSET_CSS);script src=(crate::cad_lab::ASSET_JS) defer {} script type="module" src=(SCRIPT) {}}).content(PageColumn::new(html!{
        section data-cad-config-public=(format!("/cad-config/{token}/snapshot")) {
            h1 {(title)} p {"Choose your dimensions. Your changes stay in this browser."}
            (crate::cad_lab::viewer(&format!("/cad-config/{token}/snapshot?view=1"),None,title,"shared",data["version"].as_u64().unwrap_or(1)))
            div.oa-cad-config-sliders {
                @for (i,c) in data["controls"].as_array().into_iter().flatten().enumerate() {
                    @let initial=data["values"][i].as_f64().unwrap_or(0.0);
                    (Slider::new(format!("dimension-{i}")).id(format!("dimension-{i}")).label(c["label"].as_str().unwrap_or("Dimension")).range(c["min"].as_f64().unwrap_or(0.),c["max"].as_f64().unwrap_or(1.)).step(c["step"].as_f64().unwrap_or(0.1)).value(initial))
                }
            }
            p data-config-status="" aria-live="polite" {"Loading the CAD engine…"}
            div.oa-cad-files {
                @for (format,label) in [("step","STEP"),("stl","STL"),("pdf","Drawing (PDF)"),("dxf","Drawing (DXF)")] {
                    (Button::new(label).variant(ButtonVariant::Outline).attr("data-config-download",format).disabled(true))
                }
            }
        }
    }).wide()).respond(&headers);
    response.headers_mut().insert(
        header::CONTENT_SECURITY_POLICY,
        HeaderValue::from_static(crate::pages::cad::CAD_POLICY),
    );
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("no-store, private"),
    );
    response
        .headers_mut()
        .insert("referrer-policy", HeaderValue::from_static("no-referrer"));
    response.headers_mut().insert(
        "x-robots-tag",
        HeaderValue::from_static("noindex, nofollow"),
    );
    response
}

#[cfg(test)]
mod tests {
    use super::*;
    #[tokio::test]
    async fn anonymous_reads_get_the_shared_data_and_no_edit_route() {
        use axum::body::{Body, to_bytes};
        use axum::http::Request;
        use tower::ServiceExt;
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().join("store"));
        let owner = Scope::personal(&crate::chat_store::account_owner("acct_owner")).unwrap();
        let data = json!({"title":"Bracket","version":1,"document":{"id":"bracket"},"controls":[],"values":[],"view":{"kind":"cad-view.v1","solids":[]}});
        let token = save(&store, &owner, "bracket", data.clone()).await.unwrap();
        let mut config = crate::Config::development(root.path().join("tasks"));
        config.chat_store = Arc::new(store.clone());
        config.public_hosts = vec!["openagents.test".into()];
        let site = crate::router(config);
        let page = site
            .clone()
            .oneshot(
                Request::get(format!("/cad-config/{token}"))
                    .header(header::HOST, "openagents.test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(page.status(), StatusCode::OK);
        let html = String::from_utf8(
            to_bytes(page.into_body(), 1024 * 1024)
                .await
                .unwrap()
                .to_vec(),
        )
        .unwrap();
        assert!(html.contains("Your changes stay in this browser"));
        assert!(!html.contains(&owner.home));
        assert!(oa_copy::violations(&html, &[]).is_empty());
        let view = site
            .clone()
            .oneshot(
                Request::get(format!("/cad-config/{token}/snapshot?view=1"))
                    .header(header::HOST, "openagents.test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let value: Value =
            serde_json::from_slice(&to_bytes(view.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert_eq!(value["kind"], "cad-view.v1");
        assert!(value.get("home").is_none());
        remove(&store, &owner, "bracket", &token).await.unwrap();
        let gone = site
            .oneshot(
                Request::get(format!("/cad-config/{token}"))
                    .header(header::HOST, "openagents.test")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(gone.status(), StatusCode::NOT_FOUND);
    }

    #[tokio::test]
    async fn snapshots_are_private_to_their_link_and_revocation_removes_them() {
        let root = tempfile::tempdir().unwrap();
        let store = Store::local(root.path().into());
        let owner = Scope::personal(&crate::chat_store::account_owner("acct_owner")).unwrap();
        let data = json!({"document":{"id":"bracket"},"controls":[],"view":{"kind":"cad-view.v1"}});
        let token = save(&store, &owner, "bracket", data.clone()).await.unwrap();
        assert_eq!(read(&store, &token).await, Some(data));
        assert!(read(&store, "wrong").await.is_none());
        let other = Scope::personal(&crate::chat_store::account_owner("acct_other")).unwrap();
        assert!(remove(&store, &other, "bracket", &token).await.is_err());
        purge(&store, &owner, "bracket").await.unwrap();
        assert!(read(&store, &token).await.is_none());
    }
}
