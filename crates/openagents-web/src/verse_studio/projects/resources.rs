//! Bounded creator PNGs in the existing private project-file folder.
use super::*;
use aethyrforge_host::{ProjectResource, project_hash};
use axum::{body::Bytes, response::IntoResponse};
use std::{collections::BTreeMap, io::Cursor};
const MAX_BYTES: usize = 4 * 1024 * 1024;
pub(super) fn routes() -> Router<App> {
    Router::new().route(
        "/v1/verse/studio/projects/{id}/{namespace}/{branch}/resources/{sha}",
        get(get_resource)
            .post(put_resource)
            .layer(DefaultBodyLimit::max(MAX_BYTES)),
    )
}
fn hash_ok(s: &str) -> bool {
    s.len() == 64
        && s.bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
}
fn resource_key(a: &Access, ns: WorldNamespace, branch: &str, name: &str) -> Result<String, Error> {
    Store::owner_key(
        &a.owner(),
        &format!(
            "project-files/projects/{}/verse/{}/{branch}/resources/{name}",
            a.project.id,
            ns.key()
        ),
    )
}
fn png(bytes: &[u8]) -> bool {
    if bytes.len() > MAX_BYTES || bytes.len() < 24 || bytes[..8] != *b"\x89PNG\r\n\x1a\n" {
        return false;
    }
    let mut reader = image::ImageReader::with_format(Cursor::new(bytes), image::ImageFormat::Png);
    let mut limits = image::Limits::default();
    limits.max_image_width = Some(2048);
    limits.max_image_height = Some(2048);
    limits.max_alloc = Some(32 * 1024 * 1024);
    reader.limits(limits);
    reader.decode().is_ok()
}
type Index = BTreeMap<String, ProjectResource>;
pub(super) async fn present(
    app: &App,
    a: &Access,
    ns: WorldNamespace,
    branch: &str,
    document: &ProjectDocument,
) -> Result<(), Response> {
    if document.layers.is_empty() {
        return Ok(());
    }
    let k = resource_key(a, ns, branch, "index.json").map_err(|e| stored(&e))?;
    let index = read::<Index>(&app.config.chat_store, &k)
        .await
        .map_err(|e| stored(&e))?
        .map(|(i, _)| i)
        .unwrap_or_default();
    for layer in &document.layers {
        if index.get(&layer.texture.sha256) != Some(&layer.texture) {
            return Err(refused(
                StatusCode::BAD_REQUEST,
                "missing_image",
                "Upload the material image before saving this world.",
            ));
        }
        let k = resource_key(a, ns, branch, &layer.texture.sha256).map_err(|e| stored(&e))?;
        let blob = app
            .config
            .chat_store
            .read_key(&k)
            .await
            .map_err(|e| stored(&e))?;
        if !blob.is_some_and(|(b, _)| {
            b.len() as u64 == layer.texture.bytes && project_hash(&b) == layer.texture.sha256
        }) {
            return Err(refused(
                StatusCode::CONFLICT,
                "missing_image",
                "The material image did not finish saving. Upload it again.",
            ));
        }
    }
    Ok(())
}
type AtResource = Path<(String, String, String, String)>;
async fn put_resource(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch, sha)): AtResource,
    bytes: Bytes,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    if !hash_ok(&sha) || project_hash(&bytes) != sha || !png(&bytes) {
        return invalid();
    }
    let a = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    if let Err(r) = a.edits() {
        return r;
    }
    let token = headers
        .get("x-verse-csrf")
        .and_then(|h| h.to_str().ok())
        .unwrap_or("");
    if !csrf(&app, &headers, &a.actor, token) {
        return denied();
    }
    let image = ProjectResource {
        sha256: sha.clone(),
        bytes: bytes.len() as u64,
        media_type: "image/png".into(),
    };
    let index_key = match resource_key(&a, ns, &branch, "index.json") {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    let mut reserved = false;
    for _ in 0..4 {
        let (mut index, generation) = match read::<Index>(&app.config.chat_store, &index_key).await
        {
            Ok(Some((i, g))) => (i, Some(g)),
            Ok(None) => (BTreeMap::new(), None),
            Err(e) => return stored(&e),
        };
        if index.get(&sha) == Some(&image) {
            reserved = true;
            break;
        }
        if index.len() >= 32 {
            return fault(ProjectFault::Full);
        }
        index.insert(sha.clone(), image.clone());
        match app
            .config
            .chat_store
            .write_key(
                &index_key,
                serde_json::to_vec(&index).unwrap(),
                generation.as_deref(),
            )
            .await
        {
            Ok(_) => {
                reserved = true;
                break;
            }
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    if !reserved {
        return fault(ProjectFault::Stale);
    }
    let k = match resource_key(&a, ns, &branch, &sha) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    match app
        .config
        .chat_store
        .write_key(&k, bytes.to_vec(), None)
        .await
    {
        Ok(_) => answer(StatusCode::OK, json!({"resource":image})),
        Err(Error::Conflict) => match app.config.chat_store.read_key(&k).await {
            Ok(Some((kept, _))) if kept.as_slice() == bytes.as_ref() => {
                answer(StatusCode::OK, json!({"resource":image}))
            }
            Ok(_) => fault(ProjectFault::Invalid),
            Err(e) => stored(&e),
        },
        Err(e) => stored(&e),
    }
}
async fn get_resource(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, namespace, branch, sha)): AtResource,
) -> Response {
    let ns = match location(&namespace, &branch) {
        Ok(n) => n,
        Err(r) => return r,
    };
    if !hash_ok(&sha) {
        return invalid();
    }
    let a = match access(&app, &headers, &id).await {
        Ok(a) => a,
        Err(r) => return r,
    };
    let k = match resource_key(&a, ns, &branch, &sha) {
        Ok(k) => k,
        Err(e) => return stored(&e),
    };
    match app.config.chat_store.read_key(&k).await {
        Ok(Some((bytes, _))) if project_hash(&bytes) == sha && png(&bytes) => (
            [
                ("content-type", "image/png"),
                ("cache-control", "private, no-store"),
                ("x-content-type-options", "nosniff"),
            ],
            bytes,
        )
            .into_response(),
        Ok(_) => no_project(),
        Err(e) => stored(&e),
    }
}
