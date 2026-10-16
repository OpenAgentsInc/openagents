//! Home reads and revisioned layouts. All private reads use the current account.
use crate::{
    App,
    chat_store::{Error, Store},
    coder_sync::{answer, refused, stored},
};
use axum::{
    Json, Router,
    extract::{DefaultBodyLimit, Path, Query, State},
    http::{HeaderMap, StatusCode},
    response::Response,
    routing::{get, post},
};
use openagents_sdk::dashboard::*;
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use std::collections::BTreeMap;

mod issues;
mod moderation;
use moderation::moderate;
#[cfg(test)]
mod tests;
const MAX_BOOK: usize = 16 * 1024 * 1024;
const DIRECTORY: &str = "home-templates/v1.json";
#[derive(Clone, Default, Serialize, Deserialize)]
struct Book {
    library: Library,
    templates: BTreeMap<String, Vec<SavedListing>>,
    receipts: BTreeMap<String, (String, Value)>,
    reports: BTreeMap<String, Vec<(String, String)>>,
}
#[derive(Clone, Serialize, Deserialize)]
struct SavedListing {
    owner: String,
    listing: Listing,
}
fn invalid(message: &str) -> Response {
    refused(StatusCode::BAD_REQUEST, "invalid_dashboard", message)
}
fn conflict() -> Response {
    refused(
        StatusCode::CONFLICT,
        "version_conflict",
        "This layout changed. Reload it or save a copy.",
    )
}
fn missing() -> Response {
    refused(
        StatusCode::NOT_FOUND,
        "not_found",
        "This layout is no longer available.",
    )
}
async fn load(store: &Store, key: &str) -> Result<(Book, Option<String>), Error> {
    match store.read_key(key).await? {
        None => {
            let mut book = Book::default();
            if key == DIRECTORY {
                for (number, title, kinds) in [
                    (1, "Project overview", vec!["issues", "my_issues", "agents"]),
                    (
                        2,
                        "Agent desk",
                        vec!["agents", "attention", "recent", "computers"],
                    ),
                ] {
                    let mut d = Dashboard::starter("starter");
                    d.widgets = kinds
                        .into_iter()
                        .enumerate()
                        .map(|(i, kind)| {
                            let mut w = Widget::new(format!("widget-{i}"), kind);
                            if matches!(kind, "agents" | "attention" | "recent") {
                                w.scope = Some(Scope::default());
                            }
                            w
                        })
                        .collect();
                    let package = d.export(title.into()).expect("built-in layout");
                    let id = format!("{number:064x}");
                    book.templates.insert(
                        id.clone(),
                        vec![SavedListing {
                            owner: "system".into(),
                            listing: Listing {
                                id,
                                version: 1,
                                author: "OpenAgents".into(),
                                author_id: "openagents".into(),
                                owned: false,
                                visibility: Visibility::Public,
                                package,
                                removed: false,
                            },
                        }],
                    );
                }
            }
            Ok((book, None))
        }
        Some((bytes, g)) if bytes.len() <= MAX_BOOK => Ok((
            serde_json::from_slice(&bytes).map_err(|_| Error::Corrupt("Couldn't open Home."))?,
            Some(g),
        )),
        _ => Err(Error::Corrupt("Couldn't open Home.")),
    }
}
async fn mutate<F>(
    store: &Store,
    key: &str,
    owner: &str,
    request: &str,
    fingerprint: &str,
    change: F,
) -> Response
where
    F: Fn(&mut Book) -> Result<Value, Response>,
{
    if !openagents_sdk::agents::valid_id(request) {
        return invalid("Try saving again.");
    }
    let receipt = format!("{owner}:{request}");
    for _ in 0..16 {
        let (mut book, generation) = match load(store, key).await {
            Ok(v) => v,
            Err(e) => return stored(&e),
        };
        if let Some((previous, result)) = book.receipts.get(&receipt) {
            return if previous == fingerprint {
                answer(StatusCode::OK, result.clone())
            } else {
                refused(
                    StatusCode::CONFLICT,
                    "idempotency_conflict",
                    "This request was used for another change.",
                )
            };
        }
        if book.receipts.len() >= 16_384
            || book
                .receipts
                .keys()
                .filter(|k| k.starts_with(&format!("{owner}:")))
                .count()
                >= 4096
        {
            return refused(
                StatusCode::TOO_MANY_REQUESTS,
                "quota_exhausted",
                "Your layout storage is full.",
            );
        }
        let result = match change(&mut book) {
            Ok(v) => v,
            Err(r) => return r,
        };
        book.receipts
            .insert(receipt.clone(), (fingerprint.into(), result.clone()));
        let bytes = match serde_json::to_vec(&book) {
            Ok(b) if b.len() <= MAX_BOOK => b,
            _ => {
                return refused(
                    StatusCode::TOO_MANY_REQUESTS,
                    "quota_exhausted",
                    "Your layout storage is full.",
                );
            }
        };
        match store.write_key(key, bytes, generation.as_deref()).await {
            Ok(_) => return answer(StatusCode::OK, result),
            Err(Error::Conflict) => continue,
            Err(e) => return stored(&e),
        }
    }
    conflict()
}
pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/v1/home/projects", get(projects))
        .route("/v1/home/issues", post(issues::summary))
        .route("/v1/home/work", get(work))
        .route("/v1/home/dashboards", get(library).put(save))
        .route("/v1/home/templates", get(directory))
        .route(
            "/v1/home/templates/{id}",
            get(detail).put(publish).delete(remove),
        )
        .route("/v1/home/templates/{id}/report", post(report))
        .route("/v1/home/templates/{id}/moderate", post(moderate))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}
async fn projects(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, token, _) = match crate::projects::api::caller(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    match crate::projects::api::status(service, &token).await {
        Ok(status) => answer(
            StatusCode::OK,
            json!(
                status
                    .projects
                    .into_iter()
                    .map(|p| Project {
                        id: p.id,
                        name: p.name,
                        repositories: p.repositories.into_iter().map(|r| r.repository).collect()
                    })
                    .collect::<Vec<_>>()
            ),
        ),
        Err(r) => r,
    }
}
async fn library(State(app): State<App>, headers: HeaderMap) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let key = match Store::owner_key(&owner, "home.json") {
        Ok(v) => v,
        Err(e) => return stored(&e),
    };
    match load(&app.config.chat_store, &key).await {
        Ok((b, _)) => answer(StatusCode::OK, json!(b.library)),
        Err(e) => stored(&e),
    }
}
async fn save(
    State(app): State<App>,
    headers: HeaderMap,
    body: Result<Json<SaveLibrary>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(Json(r)) = body else {
        return invalid("Check the dashboard and try again.");
    };
    if let Err(e) = r.library.validate() {
        return invalid(e);
    }
    let key = match Store::owner_key(&owner, "home.json") {
        Ok(v) => v,
        Err(e) => return stored(&e),
    };
    let fingerprint = crate::chat_store::digest(&serde_json::to_vec(&r).unwrap_or_default());
    mutate(
        &app.config.chat_store,
        &key,
        &owner,
        &r.request_id,
        &fingerprint,
        |book| {
            if book.library.revision != r.expected_revision {
                return Err(conflict());
            }
            book.library = r.library.clone();
            book.library.revision = r.expected_revision + 1;
            Ok(json!(book.library))
        },
    )
    .await
}
#[derive(Default, Deserialize)]
#[serde(deny_unknown_fields)]
struct Page {
    after: Option<String>,
    #[serde(default)]
    q: String,
    #[serde(default)]
    mine: bool,
    version: Option<u64>,
    author: Option<String>,
}
async fn directory(State(app): State<App>, headers: HeaderMap, Query(q): Query<Page>) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if q.q.len() > 100
        || q.after
            .as_ref()
            .is_some_and(|v| !openagents_sdk::agents::valid_id(v))
    {
        return invalid("Try a shorter search.");
    }
    let (book, _) = match load(&app.config.chat_store, DIRECTORY).await {
        Ok(v) => v,
        Err(e) => return stored(&e),
    };
    let search = q.q.to_lowercase();
    let mut data: Vec<_> = book
        .templates
        .iter()
        .filter(|(id, _)| q.after.as_ref().is_none_or(|a| *id > a))
        .filter_map(|(_, versions)| versions.last())
        .filter(|s| {
            !s.listing.removed
                && if q.mine {
                    s.owner == owner
                } else {
                    s.listing.visibility == Visibility::Public
                }
        })
        .filter(|s| q.author.as_ref().is_none_or(|a| &s.listing.author_id == a))
        .filter(|s| {
            format!(
                "{} {} {}",
                s.listing.package.title,
                s.listing.package.tags.join(" "),
                s.listing.author
            )
            .to_lowercase()
            .contains(&search)
        })
        .take(51)
        .map(|s| {
            let mut l = s.listing.clone();
            l.owned = s.owner == owner;
            l
        })
        .collect();
    let next = (data.len() > 50).then(|| data[49].id.clone());
    data.truncate(50);
    answer(StatusCode::OK, json!(Directory { data, next }))
}
async fn detail(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(q): Query<Page>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (book, _) = match load(&app.config.chat_store, DIRECTORY).await {
        Ok(v) => v,
        Err(e) => return stored(&e),
    };
    let Some(versions) = book.templates.get(&id) else {
        return missing();
    };
    let Some(latest) = versions.last() else {
        return missing();
    };
    if latest.listing.removed
        || (latest.listing.visibility == Visibility::Private && latest.owner != owner)
    {
        return missing();
    }
    let found = if let Some(v) = q.version {
        versions.iter().find(|s| s.listing.version == v)
    } else {
        Some(latest)
    };
    match found {
        Some(s) if s.listing.visibility != Visibility::Private || s.owner == owner => {
            let mut l = s.listing.clone();
            l.owned = s.owner == owner;
            answer(StatusCode::OK, json!(l))
        }
        _ => missing(),
    }
}
async fn publish(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<Publish>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (service, credential, _) = match crate::projects::api::caller(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let viewer = match service.app_viewer(&credential).await {
        Ok(v) => v,
        Err(_) => {
            return refused(
                StatusCode::SERVICE_UNAVAILABLE,
                "unavailable",
                "Couldn't check your account. Try again.",
            );
        }
    };
    let author = viewer
        .github
        .map(|(_, login)| login)
        .unwrap_or(viewer.account_label);
    let Ok(Json(r)) = body else {
        return invalid("Check the layout file and try again.");
    };
    if !openagents_sdk::agents::valid_id(&id) {
        return missing();
    }
    if let Err(e) = r.package.validate() {
        return invalid(e);
    }
    let fingerprint =
        crate::chat_store::digest(&serde_json::to_vec(&("publish", &id, &r)).unwrap_or_default());
    mutate(
        &app.config.chat_store,
        DIRECTORY,
        &owner,
        &r.request_id,
        &fingerprint,
        |b| {
            if b.reports.get(&id).is_some_and(|r| r.len() >= 3) {
                return Err(invalid("This layout is being reviewed."));
            }
            let versions = b.templates.entry(id.clone()).or_default();
            let version = match versions.last() {
                Some(old) if old.owner == owner => old.listing.version,
                Some(_) => return Err(missing()),
                None => 0,
            };
            if version != r.expected_revision {
                return Err(conflict());
            }
            if versions.len() >= 64 {
                return Err(invalid("Save a new template instead."));
            }
            let listing = Listing {
                id: id.clone(),
                version: version + 1,
                author: author.clone(),
                author_id: crate::chat_store::digest(owner.as_bytes()),
                owned: true,
                visibility: r.visibility,
                package: r.package.clone(),
                removed: false,
            };
            versions.push(SavedListing {
                owner: owner.clone(),
                listing: listing.clone(),
            });
            Ok(json!(listing))
        },
    )
    .await
}
async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<Remove>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(Json(r)) = body else {
        return invalid("Try removing the template again.");
    };
    let fingerprint =
        crate::chat_store::digest(&serde_json::to_vec(&("remove", &id, &r)).unwrap_or_default());
    mutate(
        &app.config.chat_store,
        DIRECTORY,
        &owner,
        &r.request_id,
        &fingerprint,
        |b| {
            let versions = b.templates.get_mut(&id).ok_or_else(missing)?;
            let old = versions
                .last()
                .filter(|s| s.owner == owner)
                .ok_or_else(missing)?;
            if old.listing.version != r.expected_revision {
                return Err(conflict());
            }
            let mut next = old.clone();
            next.listing.version += 1;
            next.listing.removed = true;
            let result = json!(next.listing);
            versions.push(next);
            Ok(result)
        },
    )
    .await
}
async fn report(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Result<Json<Report>, axum::extract::rejection::JsonRejection>,
) -> Response {
    let owner = match crate::coder_sync::owner(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let Ok(Json(r)) = body else {
        return invalid("Add a short reason.");
    };
    if !short(&r.reason, 512) {
        return invalid("Add a short reason.");
    }
    let fingerprint =
        crate::chat_store::digest(&serde_json::to_vec(&("report", &id, &r)).unwrap_or_default());
    mutate(
        &app.config.chat_store,
        DIRECTORY,
        &owner,
        &r.request_id,
        &fingerprint,
        |b| {
            let latest = b
                .templates
                .get(&id)
                .and_then(|v| v.last())
                .filter(|s| {
                    !s.listing.removed
                        && (s.owner == owner || s.listing.visibility != Visibility::Private)
                })
                .ok_or_else(missing)?;
            let _ = latest;
            let reports = b.reports.entry(id.clone()).or_default();
            if !reports.iter().any(|(o, _)| o == &owner) {
                reports.push((owner.clone(), r.reason.clone()));
            }
            // Three independent reports hide discovery pending operator review.
            if reports.len() >= 3 {
                if let Some(s) = b.templates.get_mut(&id).and_then(|v| v.last_mut()) {
                    s.listing.visibility = Visibility::Unlisted;
                }
            }
            Ok(json!({"reported":true}))
        },
    )
    .await
}
async fn work(State(app): State<App>, headers: HeaderMap, Query(q): Query<Page>) -> Response {
    let account = match crate::coder_sync::account(&app, &headers).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let owner = crate::chat_store::account_owner(&account);
    let agents = match crate::phone_api::read_agents(&app.config.chat_store, &owner).await {
        Ok(v) => v,
        Err(e) => return stored(&e),
    };
    let computers = match app.config.chat_store.computers(&owner).await {
        Ok(v) => v,
        Err(e) => return stored(&e),
    };
    let mut out = WorkPage {
        complete: true,
        ..WorkPage::default()
    };
    let mut jobs = crate::mac_jobs_actor::board_items(&app, &account).await;
    let mut targets: BTreeMap<_, _> = jobs
        .iter()
        .map(|(host, item)| {
            (
                (host.clone(), item.id.clone()),
                WorkTarget::MacJob {
                    id: item.id.clone(),
                },
            )
        })
        .collect();
    let runs = crate::work_runs::board_items(&app.config.chat_store, &owner).await;
    targets.extend(runs.iter().map(|(host, item)| {
        (
            (host.clone(), item.id.clone()),
            WorkTarget::Work {
                id: item.id.clone(),
            },
        )
    }));
    jobs.extend(runs);
    for (name, board) in crate::phone_api::with_mac_jobs(agents.boards, jobs) {
        let online = computers
            .seen
            .get(&name)
            .is_some_and(|seen| crate::chat_store::now_unix().saturating_sub(*seen) < 90);
        out.computers.push(Computer {
            name: name.clone(),
            online,
        });
        for item in board.items {
            out.data.push(Work {
                id: format!(
                    "{}:{}",
                    &crate::chat_store::digest(name.as_bytes())[..12],
                    item.id
                ),
                destination: Some(
                    targets
                        .get(&(name.clone(), item.id.clone()))
                        .cloned()
                        .unwrap_or(WorkTarget::AgentBoard),
                ),
                session: item.session,
                parent: None,
                agent: item.engine.unwrap_or_else(|| "Agent".into()),
                title: item.title,
                project: None,
                host: name.clone(),
                state: item.status,
                online,
                observed_at: board.updated_unix,
            });
        }
    }
    out.data = work_rows(out.data, None);
    out.data.sort_by(|a, b| a.id.cmp(&b.id));
    out.data
        .retain(|w| q.after.as_ref().is_none_or(|a| w.id > *a));
    out.next = (out.data.len() > 100).then(|| out.data[99].id.clone());
    out.data.truncate(100);
    answer(StatusCode::OK, json!(out))
}
