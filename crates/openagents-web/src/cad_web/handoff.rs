//! Account-scoped delivery of immutable CAD versions to Verse and Robot Lab.
use super::*;
use axum::Json;
use base64::Engine as _;
const WORLD: &str = "world/index.json";
const MAX_PLACEMENTS: usize = 20;

fn personal(scope: &Scope) -> Result<Scope, String> {
    Scope::personal(&scope.actor).ok_or("Log in to place a model in your world.".into())
}
fn project(scope: &Scope) -> &str {
    scope.project.as_deref().unwrap_or("me")
}
fn robot_base(scope: &Scope, doc: &str, n: u64) -> String {
    format!("/cad/{doc}/robot/{n}/{}", project(scope))
}
fn world_file(scope: &Scope, doc: &str, n: u64) -> String {
    let hash = Sha256::digest(format!("{}:{}:{doc}:{n}", scope.home, scope.folder).as_bytes());
    format!(
        "world/{}.glb",
        hash.iter().map(|b| format!("{b:02x}")).collect::<String>()
    )
}
fn zone(args: &Value) -> Result<&'static str, String> {
    match args["zone"]
        .as_str()
        .unwrap_or("everglade")
        .to_ascii_lowercase()
        .replace([' ', '_'], "-")
        .trim_start_matches("verse-")
    {
        "everglade" => Ok("verse-everglade"),
        "meteor-showcase" => Ok("verse-meteor-showcase"),
        "meteor-stress-test" => Ok("verse-meteor-stress-test"),
        _ => Err("Choose Everglade, Meteor Showcase, or Meteor Stress Test.".into()),
    }
}
pub(super) fn place_settings(args: &Value) -> Result<Value, String> {
    let zone = zone(args)?;
    let at = match args.get("at") {
        None | Some(Value::Null) => Value::Null,
        Some(value) => {
            let value = if let Some(s) = value.as_str() {
                serde_json::from_str(s).map_err(|_| "Choose a position in meters.")?
            } else {
                value.clone()
            };
            let values: Vec<f64> =
                serde_json::from_value(value).map_err(|_| "Choose a position in meters.")?;
            let at = match values.as_slice() {
                [x, z] => [*x, 0., *z],
                [x, y, z] => [*x, *y, *z],
                _ => return Err("Use [x,z] or [x,y,z] in meters.".into()),
            };
            if at.iter().any(|v| !v.is_finite() || v.abs() > 5000.) {
                return Err("Place the model within 5 km of the center.".into());
            }
            json!(at)
        }
    };
    let heading = args
        .get("heading")
        .map_or(Some(0.), Value::as_f64)
        .ok_or("Choose a numeric heading.")?;
    let scale = args
        .get("scale")
        .map_or(Some(1.), Value::as_f64)
        .ok_or("Choose a numeric scale.")?;
    if !heading.is_finite() || !(0.01..=100.).contains(&scale) {
        return Err("Choose a finite heading and a scale between 0.01 and 100.".into());
    }
    Ok(json!({"zone":zone,"at":at,"heading_deg":heading,"scale":scale}))
}
pub(super) async fn place(
    store: &Store,
    scope: &Scope,
    args: &Value,
    doc: &str,
    n: u64,
    bytes: Option<Vec<u8>>,
) -> Result<Value, String> {
    let settings = place_settings(args)?;
    let me = personal(scope)?;
    let key = me.key(WORLD).map_err(|e| e.to_string())?;
    let file = world_file(scope, doc, n);
    let byte_count = bytes.as_ref().map_or(0, Vec::len);
    let hash = bytes.as_ref().map(|b| format!("{:x}", Sha256::digest(b)));
    if let Some(bytes) = bytes {
        if bytes.len() > MAX_FILE_BYTES {
            return Err("This model is too large for your world.".into());
        }
        let key = me.key(&file).map_err(|e| e.to_string())?;
        match store.write_key(&key, bytes.clone(), None).await {
            Ok(_) => {}
            Err(StoreError::Conflict) => {
                if store
                    .read_key(&key)
                    .await
                    .map_err(|e| e.to_string())?
                    .map(|p| p.0)
                    != Some(bytes)
                {
                    return Err(
                        "This saved model changed. Save a new version before placing it.".into(),
                    );
                }
            }
            Err(e) => return Err(e.to_string()),
        }
    }
    let copy = args["placement"].as_str().unwrap_or(doc);
    if copy.is_empty()
        || copy.len() > 80
        || !copy
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || "-_.".contains(c))
    {
        return Err("Choose a short name for the placed copy.".into());
    }
    let id = format!(
        "{}:{copy}:{}",
        scope.local(),
        settings["zone"].as_str().unwrap_or_default()
    );
    for _ in 0..5 {
        let old = store.read_key(&key).await.map_err(|e| e.to_string())?;
        let mut entries: Vec<Value> = old
            .as_ref()
            .map(|(b, _)| serde_json::from_slice(b))
            .transpose()
            .map_err(|_| "Your placed models couldn't be opened.")?
            .unwrap_or_default();
        entries.retain(|p| p["id"] != id);
        if args["remove"] != true {
            if entries.len() >= MAX_PLACEMENTS {
                return Err(
                    "Your world has 20 CAD models. Remove one before adding another.".into(),
                );
            }
            if entries
                .iter()
                .filter_map(|p| p["bytes"].as_u64())
                .sum::<u64>()
                + byte_count as u64
                > 32 << 20
            {
                return Err(
                    "Your CAD models fill this world. Remove one before adding another.".into(),
                );
            }
            let mut item = settings.clone();
            item["bytes"] = json!(byte_count);
            item["sha256"] = json!(hash);
            item["id"] = json!(id);
            item["doc"] = json!(doc);
            item["version"] = json!(n);
            item["generation"] = json!(format!("{:032x}", secp256k1::rand::random::<u128>()));
            item["file"] = json!(file);
            entries.push(item);
        }
        match store
            .write_key(
                &key,
                serde_json::to_vec(&entries).map_err(|e| e.to_string())?,
                old.as_ref().map(|(_, g)| g.as_str()),
            )
            .await
        {
            Ok(_) => {
                return Ok(
                    json!({"doc":doc,"version":n,"summary":if args["remove"]==true{format!("Took {doc} out of your world.")}else{format!("{doc} v{n} is in your world.")},"verse":{"href":format!("/everglade?zone={}",settings["zone"].as_str().unwrap_or("verse-everglade").trim_start_matches("verse-"))}}),
                );
            }
            Err(StoreError::Conflict) => continue,
            Err(e) => return Err(e.to_string()),
        }
    }
    Err("Your world changed in another window. Try again.".into())
}
pub(super) async fn robot_saved(
    store: &Store,
    scope: &Scope,
    dir: &Path,
    result: &mut Value,
) -> Result<(), String> {
    let doc = result["doc"].as_str().ok_or("The robot has no design.")?;
    let n = result["version"]
        .as_u64()
        .ok_or("The robot has no version.")?;
    let file = format!("{doc}-v{n}.robot.json");
    let bytes = tokio::fs::read(dir.join(doc).join(format!("v{n}/out/{file}")))
        .await
        .map_err(|_| "The robot files couldn't be read.")?;
    if bytes.len() > MAX_FILE_BYTES {
        return Err("This robot is too large. Send a smaller assembly.".into());
    }
    let value: Value =
        serde_json::from_slice(&bytes).map_err(|_| "The robot files couldn't be read.")?;
    let mut bundle: robot_lab::view::Bundle = serde_json::from_value(value["bundle"].clone())
        .map_err(|_| "The robot scene couldn't be read.")?;
    bundle.validate()?;
    let key = scope
        .key(&format!("{doc}/v{n}/out/{file}"))
        .map_err(|e| e.to_string())?;
    match store.write_key(&key, bytes.clone(), None).await {
        Ok(_) => {}
        Err(StoreError::Conflict) => {
            let old = store
                .read_key(&key)
                .await
                .map_err(|e| e.to_string())?
                .ok_or("The saved robot couldn't be read.")?
                .0;
            let old: Value =
                serde_json::from_slice(&old).map_err(|_| "The saved robot couldn't be read.")?;
            if old["source"] != value["source"] || old["revision"] != value["revision"] {
                return Err("The saved robot belongs to a different CAD version.".into());
            }
            bundle = serde_json::from_value(old["bundle"].clone())
                .map_err(|_| "The saved robot couldn't be read.")?;
            bundle.validate()?;
        }
        Err(e) => return Err(e.to_string()),
    }
    let href = format!(
        "{}/play?document={}&version={}&readonly=1",
        robot_base(scope, doc, n),
        bundle.subject.document,
        bundle.subject.version
    );
    result["robot"]["reference"] = json!(bundle.subject);
    result["card"]["robot"]["reference"] = json!(bundle.subject);
    result["robot"]["href"] = json!(href);
    result["card"]["robot"]["href"] = json!(href);
    Ok(())
}

#[derive(Deserialize)]
struct Send {
    csrf: String,
    #[serde(default)]
    project: String,
    version: u64,
    target: String,
}
/// Where a design can go. Without Pro (#11693), only out of Everglade.
pub(super) fn form(scope: &Scope, doc: &str, n: u64, csrf: &str, pro: bool) -> Markup {
    html! {form method="post" action=(format!("/cad/{doc}/send")) {
        input type="hidden" name="csrf" value=(csrf);input type="hidden" name="project" value=(scope.project.as_deref().unwrap_or(""));input type="hidden" name="version" value=(n);
        div.oa-cad-files {
            @for (target,label) in [("verse","Put in Everglade"),("remove","Remove from Everglade"),("robot","Open in Robot Lab"),("urdf","URDF and meshes")].into_iter().filter(|(target,_)|pro || *target=="remove") {
                (Button::new(label).kind(ButtonType::Submit).variant(ButtonVariant::Outline).attr("name","target").attr("value",target))
            }
        }
    }}
}
async fn send(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Form(form): Form<Send>,
) -> Response {
    if crate::pages::chat::validate_form(&app, &headers, &form.csrf)
        .await
        .is_err()
    {
        return StatusCode::FORBIDDEN.into_response();
    }
    // Taking a placed part out of a world needs no plan; everything else
    // here makes something from the design (#11693).
    if form.target != "remove" && !crate::cad_gate::plan(&app, &headers).await.pro() {
        return crate::cad_gate::refused();
    }
    let Some(scope) = page_scope(&app, &headers, Some(&form.project))
        .await
        .filter(|s| s.edits)
    else {
        return crate::not_found().await;
    };
    let Some(engine) = engine() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "The CAD engine isn't available.",
        )
            .into_response();
    };
    let (name, args) = match form.target.as_str() {
        "verse" | "remove" => (
            "cad_place",
            json!({"doc":doc,"version":form.version,"zone":"everglade","remove":form.target=="remove"}),
        ),
        "robot" => ("cad_robot", json!({"doc":doc,"version":form.version})),
        "urdf" => (
            "cad_export",
            json!({"doc":doc,"version":form.version,"formats":["urdf"]}),
        ),
        _ => return StatusCode::BAD_REQUEST.into_response(),
    };
    let result = engine
        .run(&app.config.chat_store, &scope, name, &args)
        .await;
    if let Some(e) = result["error"].as_str() {
        return (StatusCode::CONFLICT, e.to_owned()).into_response();
    }
    if let Some(href) = result["robot"]["href"]
        .as_str()
        .or_else(|| result["verse"]["href"].as_str())
    {
        return Redirect::to(href).into_response();
    }
    Redirect::to(&download_url(
        &scope,
        &doc,
        form.version,
        &format!("{doc}-v{}.urdf.zip", form.version),
    ))
    .into_response()
}
async fn world(State(app): State<App>, headers: HeaderMap) -> Response {
    let mut response = world_list(&app, &headers).await;
    response.headers_mut().insert(
        header::CACHE_CONTROL,
        HeaderValue::from_static("private, no-store"),
    );
    response
}
async fn world_list(app: &App, headers: &HeaderMap) -> Response {
    let Some(scope) = page_scope(&app, &headers, None).await else {
        return Json(json!([])).into_response();
    };
    let entries = app
        .config
        .chat_store
        .read_key(&scope.key(WORLD).unwrap())
        .await
        .ok()
        .flatten()
        .map(|(b, _)| serde_json::from_slice::<Vec<Value>>(&b).unwrap_or_default())
        .unwrap_or_default();
    Json(
        entries
            .into_iter()
            .map(|mut p| {
                p["glb"] = json!(format!(
                    "/cad-world/{}",
                    p["file"]
                        .as_str()
                        .unwrap_or("")
                        .trim_start_matches("world/")
                ));
                p.as_object_mut().unwrap().remove("file");
                p
            })
            .collect::<Vec<_>>(),
    )
    .into_response()
}
async fn world_asset(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(file): UrlPath<String>,
) -> Response {
    if !valid_out(&file) {
        return crate::not_found().await;
    }
    let Some(scope) = page_scope(&app, &headers, None).await else {
        return crate::not_found().await;
    };
    match app
        .config
        .chat_store
        .read_key(&scope.key(&format!("world/{file}")).unwrap())
        .await
        .ok()
        .flatten()
    {
        Some((b, _)) => file_response(&file, b, None),
        None => crate::not_found().await,
    }
}
async fn saved(
    app: &App,
    headers: &HeaderMap,
    doc: &str,
    n: u64,
    project: &str,
) -> Result<Value, Response> {
    let scope = page_scope(app, headers, (project != "me").then_some(project))
        .await
        .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    let bytes = read_file(
        &app.config.chat_store,
        &scope,
        doc,
        n,
        &format!("{doc}-v{n}.robot.json"),
    )
    .await
    .ok_or_else(|| StatusCode::NOT_FOUND.into_response())?;
    serde_json::from_slice(&bytes).map_err(|_| StatusCode::NOT_FOUND.into_response())
}
async fn play(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath((doc, n, project)): UrlPath<(String, u64, String)>,
) -> Response {
    if let Err(r) = saved(&app, &headers, &doc, n, &project).await {
        return r;
    }
    let mut response =
        axum::response::Html(include_str!("../../../robot-workbench/src/player.html"))
            .into_response();
    response.headers_mut().insert(header::CONTENT_SECURITY_POLICY,HeaderValue::from_static("default-src 'self'; script-src 'self' 'wasm-unsafe-eval'; style-src 'self' 'unsafe-inline'; img-src 'self' blob:; connect-src 'self'; frame-ancestors 'self'; base-uri 'none'; form-action 'self'"));
    response
}
async fn robot_view(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath((doc, n, project)): UrlPath<(String, u64, String)>,
) -> Response {
    match saved(&app, &headers, &doc, n, &project).await {
        Ok(v) => Json(v["bundle"].clone()).into_response(),
        Err(r) => r,
    }
}
async fn robot_asset(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath((doc, n, project, hash)): UrlPath<(String, u64, String, String)>,
) -> Response {
    if hash.len() != 64 || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return crate::not_found().await;
    }
    let value = match saved(&app, &headers, &doc, n, &project).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match value["assets"][&hash]
        .as_str()
        .and_then(|s| base64::engine::general_purpose::STANDARD.decode(s).ok())
    {
        Some(b) => file_response("asset.bin", b, None),
        None => crate::not_found().await,
    }
}
async fn robot_poster(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath((doc, n, project)): UrlPath<(String, u64, String)>,
) -> Response {
    if let Err(r) = saved(&app, &headers, &doc, n, &project).await {
        return r;
    }
    let Some(scope) = page_scope(
        &app,
        &headers,
        (project != "me").then_some(project.as_str()),
    )
    .await
    else {
        return crate::not_found().await;
    };
    match read_file(&app.config.chat_store, &scope, &doc, n, "hero.png").await {
        Some(b) => file_response("hero.png", b, None),
        None => crate::not_found().await,
    }
}
async fn robot_build(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath((doc, n, project, file)): UrlPath<(String, u64, String, String)>,
) -> Response {
    if let Err(r) = saved(&app, &headers, &doc, n, &project).await {
        return r;
    }
    let kind = match file.as_str() {
        "start.js" | "robot_web.js" => "text/javascript",
        "robot_web_bg.wasm" => "application/wasm",
        _ => return crate::not_found().await,
    };
    let Some(dir) = std::env::var_os("OPENAGENTS_ROBOT_BUILD") else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "The Robot Lab player isn't available.",
        )
            .into_response();
    };
    match tokio::fs::read(PathBuf::from(dir).join(&file)).await {
        Ok(b) => ([(header::CONTENT_TYPE, kind)], b).into_response(),
        Err(_) => crate::not_found().await,
    }
}
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cad/{file}/send", post(send))
        .merge(super::world_physics::routes())
        .route("/cad-world", get(world))
        .route("/cad-world/{asset}", get(world_asset))
        .route("/cad/{file}/robot/{n}/{project}/play", get(play))
        .route("/cad/{file}/robot/{n}/{project}/api/view", get(robot_view))
        .route(
            "/cad/{file}/robot/{n}/{project}/api/poster",
            get(robot_poster),
        )
        .route(
            "/cad/{file}/robot/{n}/{project}/api/asset/{hash}",
            get(robot_asset),
        )
        .route(
            "/cad/{file}/robot/{n}/{project}/build/{asset}",
            get(robot_build),
        )
}
