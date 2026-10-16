//! Public sand drawings beside the site's other durable objects. Updates use
//! storage generation checks, including across Cloud Run replicas.
use crate::{
    App, ask,
    chat_store::{Error, Store},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, HeaderValue, StatusCode, header},
    response::{IntoResponse, Response},
    routing::{get, post},
};
use sand_garden::{Creation, Drawing, Gallery};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use std::collections::BTreeSet;

const PATH: &str = "/studios/blue-rush/garden/api";
const KEY: &str = "sand-garden/gallery.json";
const LIMIT: usize = 100;
const PAGE: usize = 12;

#[derive(Default, Serialize, Deserialize)]
struct Board {
    entries: Vec<Entry>,
    #[serde(default)]
    deleted: BTreeSet<String>,
}
#[derive(Serialize, Deserialize)]
struct Entry {
    id: String,
    owner: String,
    title: String,
    drawing: Drawing,
    voters: BTreeSet<String>,
    created: u64,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Upload {
    title: String,
    drawing: Drawing,
    request_id: String,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Vote {
    voted: bool,
}
#[derive(Default, Deserialize)]
struct Page {
    #[serde(default)]
    offset: usize,
}

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route(PATH, get(list).post(upload))
        .route(&format!("{PATH}/{{id}}/vote"), post(vote))
        .route(&format!("{PATH}/{{id}}/delete"), post(delete))
        .layer(DefaultBodyLimit::max(180_000))
}
fn error(status: StatusCode, message: &str) -> Response {
    protect((status, Json(serde_json::json!({"error":message}))).into_response())
}
fn protect(mut r: Response) -> Response {
    r.headers_mut()
        .insert(header::CACHE_CONTROL, HeaderValue::from_static("no-store"));
    r.headers_mut().insert(
        header::X_CONTENT_TYPE_OPTIONS,
        HeaderValue::from_static("nosniff"),
    );
    r
}
fn identity(_app: &App, visitor: &str) -> String {
    format!(
        "{:x}",
        Sha256::new()
            .chain_update(b"sand-garden-v1")
            .chain_update(visitor)
            .finalize()
    )
}
fn write_visitor(app: &App, h: &HeaderMap) -> Result<String, Response> {
    let host = h
        .get(header::HOST)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let origin = h
        .get(header::ORIGIN)
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();
    let local = host.starts_with("127.0.0.1:") || host.starts_with("localhost:");
    if h.get("x-sand-garden").and_then(|v| v.to_str().ok()) != Some("1")
        || !(origin == format!("https://{host}") || (local && origin == format!("http://{host}")))
    {
        return Err(error(
            StatusCode::FORBIDDEN,
            "Reload the garden and try again.",
        ));
    }
    ask::visitor(h).map(|v| identity(app, &v)).ok_or_else(|| {
        error(
            StatusCode::UNAUTHORIZED,
            "Reload the garden before sharing.",
        )
    })
}
async fn admin(app: &App, h: &HeaderMap) -> bool {
    if let Some(service) = app.config.cloud.as_deref() {
        service.authenticate(h).await.is_ok_and(|v| v.admin)
    } else {
        false
    }
}
async fn read(store: &Store) -> Result<(Board, Option<String>), Error> {
    match store.read_key(KEY).await? {
        Some((bytes, generation)) => Ok((
            serde_json::from_slice(&bytes)
                .map_err(|_| Error::Corrupt("The gallery could not be read."))?,
            Some(generation),
        )),
        None => Ok((Board::default(), None)),
    }
}
async fn update<F>(store: &Store, apply: F) -> Result<(), Response>
where
    F: Fn(&mut Board) -> Result<(), Response>,
{
    for _ in 0..8 {
        let (mut board, generation) = read(store).await.map_err(|_| {
            error(
                StatusCode::SERVICE_UNAVAILABLE,
                "The gallery couldn't open. Try again.",
            )
        })?;
        apply(&mut board)?;
        let bytes = serde_json::to_vec(&board).map_err(|_| {
            error(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Your changes couldn't be saved.",
            )
        })?;
        if bytes.len() > 6 * 1024 * 1024 {
            return Err(error(
                StatusCode::INSUFFICIENT_STORAGE,
                "The gallery is full. Try again later.",
            ));
        }
        match store.write_key(KEY, bytes, generation.as_deref()).await {
            Ok(_) => return Ok(()),
            Err(Error::Conflict) => continue,
            Err(_) => {
                return Err(error(
                    StatusCode::SERVICE_UNAVAILABLE,
                    "Your changes couldn't be saved. Try again.",
                ));
            }
        }
    }
    Err(error(
        StatusCode::CONFLICT,
        "The gallery is busy. Try again.",
    ))
}
async fn list(State(app): State<App>, h: HeaderMap, Query(page): Query<Page>) -> Response {
    let visitor = ask::visitor(&h);
    let fresh = visitor.is_none();
    let visitor = visitor.unwrap_or_else(ask::new_visitor);
    let owner = identity(&app, &visitor);
    let is_admin = admin(&app, &h).await;
    let (mut board, _) = match read(&app.config.chat_store).await {
        Ok(v) => v,
        Err(_) => {
            return error(
                StatusCode::SERVICE_UNAVAILABLE,
                "The gallery couldn't open. Try again.",
            );
        }
    };
    // Older entries win ties, so a brand-new zero-vote entry goes last.
    board.entries.sort_by(|a, b| {
        b.voters
            .len()
            .cmp(&a.voters.len())
            .then(a.created.cmp(&b.created))
            .then(a.id.cmp(&b.id))
    });
    let total = board.entries.len();
    let offset = page.offset.min(total);
    let items = board
        .entries
        .into_iter()
        .skip(offset)
        .take(PAGE)
        .map(|e| Creation {
            voted: e.voters.contains(&owner),
            can_delete: is_admin || e.owner == owner,
            votes: e.voters.len(),
            id: e.id,
            title: e.title,
            drawing: e.drawing,
        })
        .collect();
    let mut r = protect(
        Json(Gallery {
            items,
            total,
            offset,
            admin: is_admin,
        })
        .into_response(),
    );
    if fresh {
        let secure = if app.config.secure_cookies {
            "; Secure"
        } else {
            ""
        };
        if let Ok(c) = HeaderValue::from_str(&format!(
            "{}={visitor}; Path=/; HttpOnly; SameSite=Lax; Max-Age=31536000{secure}",
            ask::COOKIE
        )) {
            r.headers_mut().append(header::SET_COOKIE, c);
        }
    }
    r
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}
async fn upload(State(app): State<App>, h: HeaderMap, Json(body): Json<Upload>) -> Response {
    let owner = match write_visitor(&app, &h) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let title = body.title.trim();
    if title.is_empty()
        || title.chars().count() > 48
        || title.chars().any(char::is_control)
        || !body.drawing.valid()
        || body.request_id.len() != 32
        || !body.request_id.bytes().all(|b| b.is_ascii_hexdigit())
    {
        return error(
            StatusCode::BAD_REQUEST,
            "Give your design a short name and add something to the sand.",
        );
    }
    let id = format!(
        "{:x}",
        Sha256::new()
            .chain_update(&owner)
            .chain_update(&body.request_id)
            .finalize()
    );
    let created = now();
    let result = update(&app.config.chat_store, |b| {
        if b.deleted.contains(&id) {
            return Err(error(StatusCode::GONE, "This creation was deleted."));
        }
        if b.entries.iter().any(|e| e.id == id) {
            return Ok(());
        }
        if b.entries.len() >= LIMIT || b.entries.iter().filter(|e| e.owner == owner).count() >= 12 {
            return Err(error(
                StatusCode::TOO_MANY_REQUESTS,
                "There's no room for another creation. Delete one before adding more.",
            ));
        }
        let created = created.max(
            b.entries
                .iter()
                .map(|e| e.created)
                .max()
                .unwrap_or_default()
                .saturating_add(1),
        );
        b.entries.push(Entry {
            id: id.clone(),
            owner: owner.clone(),
            title: title.into(),
            drawing: body.drawing.clone(),
            voters: BTreeSet::new(),
            created,
        });
        Ok(())
    })
    .await;
    match result {
        Ok(()) => {
            protect((StatusCode::CREATED, Json(serde_json::json!({"id":id}))).into_response())
        }
        Err(r) => r,
    }
}
async fn vote(
    State(app): State<App>,
    h: HeaderMap,
    Path(id): Path<String>,
    Json(body): Json<Vote>,
) -> Response {
    let owner = match write_visitor(&app, &h) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let result = update(&app.config.chat_store, |b| {
        let e = b
            .entries
            .iter_mut()
            .find(|e| e.id == id)
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "This creation was deleted."))?;
        if body.voted {
            if e.voters.len() >= 10_000 {
                return Err(error(
                    StatusCode::TOO_MANY_REQUESTS,
                    "This creation can't take more votes.",
                ));
            }
            e.voters.insert(owner.clone());
        } else {
            e.voters.remove(&owner);
        }
        Ok(())
    })
    .await;
    match result {
        Ok(()) => protect(Json(serde_json::json!({"ok":true})).into_response()),
        Err(r) => r,
    }
}
async fn delete(State(app): State<App>, h: HeaderMap, Path(id): Path<String>) -> Response {
    let owner = match write_visitor(&app, &h) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let is_admin = admin(&app, &h).await;
    let result = update(&app.config.chat_store, |b| {
        if b.deleted.contains(&id) {
            return Ok(());
        }
        let e = b
            .entries
            .iter()
            .find(|e| e.id == id)
            .ok_or_else(|| error(StatusCode::NOT_FOUND, "This creation was deleted."))?;
        if !is_admin && e.owner != owner {
            return Err(error(
                StatusCode::FORBIDDEN,
                "You can only delete your own creations.",
            ));
        }
        if b.deleted.len() >= 10_000 {
            return Err(error(
                StatusCode::SERVICE_UNAVAILABLE,
                "This creation couldn't be deleted. Try again later.",
            ));
        }
        b.entries.retain(|e| e.id != id);
        b.deleted.insert(id.clone());
        Ok(())
    })
    .await;
    match result {
        Ok(()) => protect(Json(serde_json::json!({"ok":true})).into_response()),
        Err(r) => r,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::{
        body::{Body, to_bytes},
        http::Request,
    };
    use serde_json::{Value, json};
    use tower::ServiceExt;
    const HOST: &str = "127.0.0.1:4300";
    fn site(dir: &std::path::Path) -> Router {
        crate::router(crate::Config::development(dir.join("tasks")))
    }
    async fn call(
        site: Router,
        method: &str,
        path: &str,
        visitor: Option<&str>,
        body: Option<Value>,
    ) -> (StatusCode, HeaderMap, Value) {
        let mut r = Request::builder()
            .method(method)
            .uri(path)
            .header(header::HOST, HOST)
            .header(header::ORIGIN, format!("http://{HOST}"))
            .header("x-sand-garden", "1")
            .header(header::CONTENT_TYPE, "application/json");
        if let Some(v) = visitor {
            r = r.header(header::COOKIE, format!("{}={v}", ask::COOKIE));
        }
        let response = site
            .oneshot(
                r.body(body.map_or_else(Body::empty, |b| Body::from(b.to_string())))
                    .unwrap(),
            )
            .await
            .unwrap();
        let status = response.status();
        let headers = response.headers().clone();
        let bytes = to_bytes(response.into_body(), 2 * 1024 * 1024)
            .await
            .unwrap();
        (
            status,
            headers,
            serde_json::from_slice(&bytes)
                .unwrap_or_else(|_| json!({"body":String::from_utf8_lossy(&bytes)})),
        )
    }
    fn design(title: &str, request: &str) -> Value {
        json!({"title":title,"request_id":request,"drawing":{"actions":[{"tool":"conch","size":1.,"angle":0.,"points":[{"x":200.,"y":200.}]}]}})
    }
    #[tokio::test]
    async fn gallery_is_durable_votes_are_idempotent_and_only_the_creator_can_delete() {
        let root = tempfile::tempdir().unwrap();
        let config = crate::Config::development(root.path().join("tasks"));
        let site = crate::router(config.clone());
        let (_, h, _) = call(site.clone(), "GET", PATH, None, None).await;
        assert!(
            h[header::SET_COOKIE]
                .to_str()
                .unwrap()
                .contains("HttpOnly; SameSite=Lax")
        );
        let one = "11111111111111111111111111111111";
        let two = "22222222222222222222222222222222";
        let mut body = design("Sea glass", one);
        body["drawing"]["light"] = json!("shade");
        body["drawing"]["actions"][0]["color"] = json!("#52d6ca");
        let (status, _, a) = call(site.clone(), "POST", PATH, Some(one), Some(body.clone())).await;
        assert_eq!(status, StatusCode::CREATED);
        let id = a["id"].as_str().unwrap();
        let (_, _, again) = call(site.clone(), "POST", PATH, Some(one), Some(body)).await;
        assert_eq!(a, again);
        let (_, _, b) = call(
            site.clone(),
            "POST",
            PATH,
            Some(two),
            Some(design("Conch", two)),
        )
        .await;
        let other = b["id"].as_str().unwrap();
        let (_, _, list) = call(site.clone(), "GET", PATH, Some(one), None).await;
        assert_eq!(list["total"], 2);
        assert_eq!(list["items"][0]["drawing"]["light"], "shade");
        assert_eq!(
            list["items"][0]["drawing"]["actions"][0]["color"],
            "#52d6ca"
        );
        assert_eq!(list["items"][0]["id"], id);
        assert_eq!(list["items"][1]["id"], other);
        for _ in 0..2 {
            let (status, _, _) = call(
                site.clone(),
                "POST",
                &format!("{PATH}/{other}/vote"),
                Some(one),
                Some(json!({"voted":true})),
            )
            .await;
            assert_eq!(status, StatusCode::OK);
        }
        let mut restarted = config;
        restarted.ask_salt = [3; 32];
        let (_, _, list) = call(crate::router(restarted), "GET", PATH, Some(one), None).await;
        assert_eq!(list["items"][0]["id"], other);
        assert_eq!(list["items"][0]["votes"], 1);
        assert_eq!(list["items"][0]["voted"], true);
        assert_eq!(list["items"][0]["can_delete"], false);
        assert_eq!(list["items"][1]["can_delete"], true);
        assert!(list["items"][0].get("owner").is_none());
        assert!(list["items"][0].get("voters").is_none());
        assert_eq!(
            call(
                site.clone(),
                "POST",
                &format!("{PATH}/{other}/delete"),
                Some(one),
                Some(json!({}))
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
        assert_eq!(
            call(
                site.clone(),
                "POST",
                &format!("{PATH}/{id}/delete"),
                Some(one),
                Some(json!({}))
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(
                site.clone(),
                "POST",
                &format!("{PATH}/{id}/delete"),
                Some(one),
                Some(json!({}))
            )
            .await
            .0,
            StatusCode::OK
        );
        assert_eq!(
            call(
                site,
                "POST",
                PATH,
                Some(one),
                Some(design("Sea glass", one))
            )
            .await
            .0,
            StatusCode::GONE
        );
    }
    #[tokio::test]
    async fn simultaneous_votes_from_separate_instances_are_not_lost() {
        let root = tempfile::tempdir().unwrap();
        let config = crate::Config::development(root.path().join("tasks"));
        let s = crate::router(config.clone());
        let owner = "11111111111111111111111111111111";
        let (_, _, v) = call(
            s.clone(),
            "POST",
            PATH,
            Some(owner),
            Some(design("A beach", owner)),
        )
        .await;
        let id = v["id"].as_str().unwrap();
        let path = format!("{PATH}/{id}/vote");
        let mut tasks = Vec::new();
        for i in 2..8 {
            let mut c = config.clone();
            c.chat_store = std::sync::Arc::new(Store::local(root.path().join("web-chats")));
            let site = crate::router(c);
            let path = path.clone();
            tasks.push(tokio::spawn(async move {
                call(
                    site,
                    "POST",
                    &path,
                    Some(&format!("{i:032x}")),
                    Some(json!({"voted":true})),
                )
                .await
                .0
            }));
        }
        for t in tasks {
            assert_eq!(t.await.unwrap(), StatusCode::OK);
        }
        let (_, _, list) = call(s, "GET", PATH, Some(owner), None).await;
        assert_eq!(list["items"][0]["votes"], 6);
    }
    #[tokio::test]
    async fn public_writes_reject_cross_site_requests_and_unbounded_drawings() {
        let root = tempfile::tempdir().unwrap();
        let s = site(root.path());
        let owner = "11111111111111111111111111111111";
        let r = Request::builder()
            .method("POST")
            .uri(PATH)
            .header(header::HOST, HOST)
            .header(header::CONTENT_TYPE, "application/json")
            .header(header::ORIGIN, "https://unrelated.example")
            .header("x-sand-garden", "1")
            .header(header::COOKIE, format!("{}={owner}", ask::COOKIE))
            .body(Body::from(design("Beach", owner).to_string()))
            .unwrap();
        assert_eq!(
            s.clone().oneshot(r).await.unwrap().status(),
            StatusCode::FORBIDDEN
        );
        let mut body = design("Beach", owner);
        body["drawing"]["actions"][0]["points"][0]["x"] = json!(100_000);
        assert_eq!(
            call(s.clone(), "POST", PATH, Some(owner), Some(body))
                .await
                .0,
            StatusCode::BAD_REQUEST
        );
        assert_eq!(
            call(s, "POST", PATH, None, Some(design("Beach", owner)))
                .await
                .0,
            StatusCode::UNAUTHORIZED
        );
    }
    #[tokio::test]
    async fn studio_domain_maps_the_gallery_api() {
        let root = tempfile::tempdir().unwrap();
        let r = Request::builder()
            .uri("/garden/api")
            .header(header::HOST, "bluerushstudios.com")
            .body(Body::empty())
            .unwrap();
        assert_eq!(
            site(root.path()).oneshot(r).await.unwrap().status(),
            StatusCode::OK
        );
    }
}
