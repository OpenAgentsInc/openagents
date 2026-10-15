//! Imported model bytes kept with the version and restored into an engine cache.
use super::*;
const MAX: usize = 2 * 1024 * 1024;
#[derive(Clone, Deserialize)]
#[serde(deny_unknown_fields)]
struct Asset {
    sha256: String,
    format: String,
    size_bytes: u64,
}
impl Asset {
    fn valid(&self) -> bool {
        self.sha256.len() == 64
            && self
                .sha256
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c))
            && matches!(self.format.as_str(), "step" | "stl")
            && self.size_bytes <= MAX as u64
    }
    fn name(&self) -> String {
        format!("asset-{}.{}", self.sha256, self.format)
    }
    fn check(&self, bytes: &[u8]) -> bool {
        self.valid()
            && bytes.len() as u64 == self.size_bytes
            && format!("{:x}", Sha256::digest(bytes)) == self.sha256
    }
}
fn manifest(bytes: &[u8]) -> Result<Vec<Asset>, String> {
    let assets: Vec<Asset> = serde_json::from_slice(bytes)
        .map_err(|_| "This design's imported file list couldn't be read.")?;
    if assets.len() > 128
        || assets.iter().any(|a| !a.valid())
        || assets.iter().map(|a| a.size_bytes).sum::<u64>() > 8 * 1024 * 1024
    {
        return Err("This design's imported files exceed their limits.".into());
    }
    Ok(assets)
}
pub(super) async fn persist(
    store: &Store,
    scope: &Scope,
    doc: &str,
    n: u64,
    at: &Path,
) -> Result<(), String> {
    let Ok(bytes) = tokio::fs::read(at.join("assets.json")).await else {
        return Ok(());
    };
    for asset in manifest(&bytes)? {
        let data = tokio::fs::read(at.join("out").join(asset.name()))
            .await
            .map_err(|_| "An imported model's source file is missing.")?;
        if !asset.check(&data) {
            return Err("An imported model failed its SHA-256 check.".into());
        }
        let key = scope
            .key(&format!("{doc}/v{n}/out/{}", asset.name()))
            .map_err(|e| e.to_string())?;
        put(store, &key, data).await?;
    }
    Ok(())
}
pub(super) async fn restore(
    store: &Store,
    scope: &Scope,
    dir: &Path,
    index: &Index,
    args: &Value,
) -> Result<(), String> {
    fn named(value: &Value, index: &Index, out: &mut BTreeMap<(String, u64), ()>) {
        match value {
            Value::Object(map) => {
                if let Some(doc) = map.get("doc").and_then(Value::as_str) {
                    if let Some(entry) = index.docs.get(doc) {
                        let n = map
                            .get("version")
                            .and_then(Value::as_u64)
                            .unwrap_or(entry.versions);
                        if n > 0 && n <= entry.versions {
                            out.insert((doc.into(), n), ());
                        }
                    }
                }
                for v in map.values() {
                    named(v, index, out);
                }
            }
            Value::Array(items) => {
                for v in items {
                    named(v, index, out);
                }
            }
            Value::String(text) if text.trim_start().starts_with(['{', '[']) => {
                if let Ok(value) = serde_json::from_str::<Value>(text) {
                    named(&value, index, out);
                }
            }
            _ => (),
        }
    }
    let mut needed = BTreeMap::new();
    named(args, index, &mut needed);
    for ((doc, n), ()) in needed {
        let at = dir.join(&doc).join(format!("v{n}"));
        let bytes = match tokio::fs::read(at.join("assets.json")).await {
            Ok(bytes) => bytes,
            Err(_) => {
                let key = scope
                    .key(&format!("{doc}/v{n}/assets.json"))
                    .map_err(|e| e.to_string())?;
                let bytes = store
                    .read_key(&key)
                    .await
                    .map_err(|_| "An imported model couldn't be read.")?
                    .map(|v| v.0)
                    .unwrap_or_else(|| b"[]".to_vec());
                tokio::fs::write(at.join("assets.json"), &bytes)
                    .await
                    .map_err(|_| "The CAD engine has no room for this model.")?;
                bytes
            }
        };
        for asset in manifest(&bytes)? {
            let path = dir
                .join(".parts/files")
                .join(format!("{}.{}", asset.sha256, asset.format));
            if tokio::fs::read(&path).await.is_ok_and(|b| asset.check(&b)) {
                continue;
            }
            let key = scope
                .key(&format!("{doc}/v{n}/out/{}", asset.name()))
                .map_err(|e| e.to_string())?;
            let bytes = store
                .read_key(&key)
                .await
                .map_err(|_| "An imported model couldn't be read.")?
                .ok_or("An imported model's source file is missing. Import the file again.")?
                .0;
            if !asset.check(&bytes) {
                return Err("An imported model failed its SHA-256 check.".into());
            }
            tokio::fs::create_dir_all(path.parent().unwrap())
                .await
                .map_err(|_| "The CAD engine has no room for this model.")?;
            tokio::fs::write(&path, &bytes)
                .await
                .map_err(|_| "The CAD engine has no room for this model.")?;
        }
    }
    Ok(())
}

pub(super) fn tool() -> Value {
    json!({"name":"cad_import_file","description":"Import an attached STEP or STL file into a CAD design, up to 2 MiB. Use the attachment's chat and file IDs. New doc names create a design. Existing assemblies can add a new part. STL stays a closed triangle mesh in its own part, in millimeters.","parameters":{"type":"object","required":["doc","part","feature","chat","file"],"properties":{"doc":{"type":"string"},"part":{"type":"string"},"feature":{"type":"string"},"chat":{"type":"string"},"file":{"type":"string"},"summary":{"type":"string"}}}})
}
pub(super) async fn from_chat(store: &Store, scope: &Scope, args: &Value) -> Result<Value, String> {
    use base64::Engine as _;
    let chat = args["chat"]
        .as_str()
        .filter(|v| crate::chat_store::valid_id(v))
        .ok_or("Name the attached file's chat.")?;
    let file = args["file"]
        .as_str()
        .filter(|v| crate::chat_files::valid_file_id(v))
        .ok_or("Name the attached file.")?;
    let (info, bytes) = crate::chat_files::read(store, &scope.actor, chat, file)
        .await
        .map_err(|_| "The attached model couldn't be read.")?
        .ok_or("This file isn't available here. Upload it from the design's page.")?;
    let format = match info.kind {
        crate::chat_files::Kind::Step => "step",
        crate::chat_files::Kind::Stl => "stl",
        _ => return Err("Choose an attached STEP or STL file.".into()),
    };
    if bytes.len() > MAX {
        return Err("CAD imports must be 2 MiB or smaller.".into());
    }
    Ok(
        json!({"doc":args["doc"],"part":args["part"],"feature":args["feature"],"summary":args["summary"],"format":format,"data":base64::engine::general_purpose::STANDARD.encode(bytes)}),
    )
}

pub(super) const SCRIPT: &str = "/chat/cad/import.js";
pub(super) fn routes() -> Router<App> {
    Router::new()
        .route(
            "/cad/{file}/import",
            axum::routing::post(upload).layer(axum::extract::DefaultBodyLimit::max(MAX)),
        )
        .route(
            SCRIPT,
            axum::routing::get(|| async {
                (
                    [(header::CONTENT_TYPE, "text/javascript; charset=utf-8")],
                    include_str!("../../static/cad-import.js"),
                )
            }),
        )
}
#[derive(Default, Deserialize)]
struct Target {
    #[serde(default)]
    design: String,
    #[serde(default)]
    project: String,
    part: String,
    feature: String,
}
async fn upload(
    State(app): State<App>,
    headers: HeaderMap,
    UrlPath(doc): UrlPath<String>,
    Query(target): Query<Target>,
    body: Result<axum::body::Bytes, axum::extract::rejection::BytesRejection>,
) -> Response {
    use base64::Engine as _;
    let csrf = headers
        .get("x-csrf-token")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    if crate::pages::chat::validate_form(&app, &headers, csrf)
        .await
        .is_err()
    {
        return (StatusCode::FORBIDDEN, "Reload the page and try again.").into_response();
    }
    if !crate::cad_gate::plan(&app, &headers).await.pro() {
        return crate::cad_gate::refused();
    }
    let Some(scope) = page_scope(&app, &headers, Some(&target.project)).await else {
        return crate::not_found_for(&headers);
    };
    if !scope.edits {
        return (
            StatusCode::FORBIDDEN,
            "Only this project's editors can add models.",
        )
            .into_response();
    }
    let Ok(bytes) = body else {
        return (
            StatusCode::PAYLOAD_TOO_LARGE,
            "CAD imports must be 2 MiB or smaller.",
        )
            .into_response();
    };
    let kind = match crate::chat_files::check(&bytes) {
        Ok(kind) => kind,
        Err((status, error)) => return (status, error).into_response(),
    };
    let format = match kind {
        crate::chat_files::Kind::Step => "step",
        crate::chat_files::Kind::Stl => "stl",
        _ => return (StatusCode::BAD_REQUEST, "Choose a STEP or STL model.").into_response(),
    };
    let Some(engine) = engine() else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            "The CAD engine isn't available. Try again later.",
        )
            .into_response();
    };
    let doc = if target.design.trim().is_empty() {
        doc
    } else {
        target.design.trim().to_owned()
    };
    if !valid_doc(&doc) {
        return (
            StatusCode::BAD_REQUEST,
            "Choose a short design name with letters, numbers, and hyphens.",
        )
            .into_response();
    }
    let result=engine.run(&app.config.chat_store,&scope,"cad_import_model",&json!({"doc":doc,"part":target.part,"feature":target.feature,"format":format,"data":base64::engine::general_purpose::STANDARD.encode(bytes)})).await;
    if let Some(error) = result["error"].as_str() {
        return (StatusCode::BAD_REQUEST, error.to_owned()).into_response();
    }
    if result["saved"] != true {
        return (StatusCode::CONFLICT, "This model couldn't be saved.").into_response();
    }
    axum::Json(json!({"url":scope.page(&doc,result["version"].as_u64())})).into_response()
}

pub(super) fn form(scope: &Scope, doc: &str, csrf: &str, record: &Value) -> Markup {
    let part = record["report"]["parts"][0]["id"].as_str().unwrap_or(doc);
    html! {
        details {
            summary {"Add a STEP or STL model"}
            form.oa-cad-pull data-cad-import-form=(format!("/cad/{doc}/import")) data-cad-import-csrf=(csrf) data-cad-import-project=(scope.project.as_deref().unwrap_or("")) {
                label {"Model file " input type="file" accept=".step,.stp,.stl" required data-cad-import-file="";}
                label {"Design name " input value=(doc) required maxlength="48" data-cad-import-design="";}
                label {"Part " input value=(part) required maxlength="48" data-cad-import-part="";}
                label {"Feature " input value="imported" required maxlength="48" data-cad-import-feature="";}
                p {"Up to 2 MiB. Use a new design name for a separate model. STL meshes must be closed, in millimeters, and in their own part."}
                (Button::new("Add model").kind(ButtonType::Submit).disabled(true))
                p data-cad-import-status="" aria-live="polite" {}
            }
        }
    }
}

pub(super) async fn bundle(
    store: &Store,
    scope: &Scope,
    doc: &str,
    n: u64,
) -> Result<Vec<Value>, String> {
    use base64::Engine as _;
    let Some(bytes) = read_file(store, scope, doc, n, "assets.json").await else {
        return Ok(Vec::new());
    };
    let mut result = Vec::new();
    for asset in manifest(&bytes)? {
        let data = read_file(store, scope, doc, n, &format!("out/{}", asset.name()))
            .await
            .ok_or("An imported model's source file is missing.")?;
        if !asset.check(&data) {
            return Err("An imported model failed its SHA-256 check.".into());
        }
        result.push(json!({"sha256":asset.sha256,"format":asset.format,"bytes_base64":base64::engine::general_purpose::STANDARD.encode(data)}));
    }
    Ok(result)
}
