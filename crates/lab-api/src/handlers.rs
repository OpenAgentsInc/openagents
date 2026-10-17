use crate::*;
use axum::{
    Json, Router,
    extract::{
        DefaultBodyLimit, Path, Query, State,
        rejection::{JsonRejection, QueryRejection},
    },
    http::{HeaderMap, StatusCode},
    response::{IntoResponse, Response, Sse, sse::Event},
    routing::{get, post},
};
use base64::Engine;
use lab_kit::*;
use lab_store::{ArtifactInput, LabStore, Store};
use serde_json::json;
use std::{convert::Infallible, sync::Arc, time::Duration};

type App = State<Arc<Api>>;
type Body<T> = Result<Json<T>, JsonRejection>;
type Params = Result<Query<Page>, QueryRejection>;
fn params(p: Params) -> Result<Page, ApiError> {
    p.map(|p| p.0)
        .map_err(|_| ApiError::invalid("Check the page and space settings."))
}
fn now() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |d| d.as_millis() as u64)
}
fn answer(status: StatusCode, value: impl serde::Serialize) -> Response {
    let mut r = (status, Json(value)).into_response();
    r.headers_mut().insert(
        "Cache-Control",
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    r
}
fn key(headers: &HeaderMap) -> Result<&str, ApiError> {
    if headers.get_all("Idempotency-Key").iter().count() != 1 {
        return Err(ApiError::invalid(
            "Supply one Idempotency-Key for this request.",
        ));
    }
    let key = headers
        .get("Idempotency-Key")
        .and_then(|h| h.to_str().ok())
        .filter(|k| !k.trim().is_empty() && k.len() <= 128)
        .ok_or_else(|| ApiError::invalid("Supply an Idempotency-Key of up to 128 characters."))?;
    Ok(key)
}
pub(crate) fn routes() -> Router<Arc<Api>> {
    Router::new()
        .route("/v1/labs/kinds", get(kinds))
        .route("/v1/labs/kinds/{kind}/tools", get(tools))
        .route("/v1/labs/kinds/{kind}/tools/{name}", post(tool))
        .route("/v1/labs/documents", get(documents).post(create))
        .route("/v1/labs/documents/{id}", get(document).delete(delete))
        .route("/v1/labs/documents/{id}/versions", get(versions).post(edit))
        .route("/v1/labs/documents/{id}/versions/{n}", get(version))
        .route("/v1/labs/documents/{id}/versions/{n}/card", get(card))
        .route("/v1/labs/documents/{id}/notes", get(notes).post(note))
        .route("/v1/labs/documents/{id}/events", get(events))
        .route("/v1/labs/jobs", get(jobs).post(job_create))
        .route("/v1/labs/jobs/{id}", get(job).delete(cancel))
        .route("/v1/labs/artifacts/{id}", get(artifact))
        .route("/v1/labs/artifacts/{id}/content", get(content))
        .route("/v1/labs/uploads", post(upload))
        .layer(DefaultBodyLimit::max(1024 * 1024))
}
async fn kinds(State(api): App, headers: HeaderMap, p: Params) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (a, _) = api.scoped(&headers, &p.space, Action::Read).await?;
    let data: Vec<_> = api
        .registry
        .0
        .iter()
        .filter(|(id, _)| api.registry.get(id, &a).is_ok())
        .map(|(_, r)| r.kind.manifest())
        .collect();
    Ok(answer(StatusCode::OK, Data { data }))
}
async fn tools(
    State(api): App,
    headers: HeaderMap,
    Path(kind): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (a, _) = api.scoped(&headers, &p.space, Action::Read).await?;
    let k = api.registry.get(&kind, &a)?;
    Ok(answer(
        StatusCode::OK,
        Data {
            data: k.manifest().tools,
        },
    ))
}
async fn documents(State(api): App, headers: HeaderMap, p: Params) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (a, s) = api.scoped(&headers, &p.space, Action::Read).await?;
    if let Some(k) = &p.kind {
        api.registry.get(k, &a)?;
    }
    let mut page = s.documents(p.kind.as_deref(), p.after, p.limit).await?;
    page.items.retain(|d| api.registry.get(&d.kind, &a).is_ok());
    Ok(answer(
        StatusCode::OK,
        List {
            data: page.items,
            next: page.next,
        },
    ))
}
async fn create(
    State(api): App,
    headers: HeaderMap,
    body: Body<CreateDocument>,
) -> Result<Response, ApiError> {
    let r = body?.0;
    let (a, s) = api.scoped(&headers, &r.space, Action::Edit).await?;
    let k = api.registry.get(&r.kind, &a)?;
    let d = k
        .initial(&s, &r.id, &r.schema, r.content, &r.summary)
        .await?;
    api.references(&headers, k.as_ref(), d.content()).await?;
    let version = s.create(&r.id, &r.title, &a.account, d, now()).await?;
    k.saved(&s, &version).await?;
    Ok(answer(StatusCode::CREATED, version))
}
async fn document(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s, _) = api.document(&headers, &p.space, Action::Read, &id).await?;
    Ok(answer(StatusCode::OK, s.document(&id).await?))
}
async fn delete(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s, _) = api
        .document(&headers, &p.space, Action::Delete, &id)
        .await?;
    s.delete(&id, now()).await?;
    Ok(answer(StatusCode::OK, Deleted { id, deleted: true }))
}
async fn versions(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s, k) = api.document(&headers, &p.space, Action::Read, &id).await?;
    let page = s.versions(&id, p.after, p.limit).await?;
    for v in &page.items {
        api.references(&headers, k.as_ref(), &v.content).await?;
    }
    Ok(answer(
        StatusCode::OK,
        List {
            data: page.items,
            next: page.next,
        },
    ))
}
async fn edit(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Body<EditDocument>,
) -> Result<Response, ApiError> {
    let r = body?.0;
    let (a, s, k) = api.document(&headers, &r.space, Action::Edit, &id).await?;
    let base = s.version(&id, r.base).await?;
    let d = k.change(&s, &base, &r.change, &r.summary).await?;
    api.references(&headers, k.as_ref(), d.content()).await?;
    let version = s.save(&id, &a.account, d, now()).await?;
    k.saved(&s, &version).await?;
    Ok(answer(StatusCode::CREATED, version))
}
async fn version(
    State(api): App,
    headers: HeaderMap,
    Path((id, n)): Path<(String, String)>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s, k) = api.document(&headers, &p.space, Action::Read, &id).await?;
    let n = n
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| ApiError::invalid("Choose a valid version number."))?;
    let v = s.version(&id, n).await?;
    api.references(&headers, k.as_ref(), &v.content).await?;
    Ok(answer(StatusCode::OK, v))
}
pub(crate) async fn card_value(
    api: &Api,
    headers: &HeaderMap,
    space: &str,
    store: &Store,
    kind: &dyn Kind,
    v: &Version,
) -> Result<Card, ApiError> {
    api.references(headers, kind, &v.content).await?;
    let mut jobs = vec![];
    let mut after = 0;
    loop {
        let page = store.version_jobs(&v.reference(), after, 100).await?;
        jobs.extend(page.items);
        if jobs.len() > 1000 {
            return Err(ApiError::new(
                StatusCode::PAYLOAD_TOO_LARGE,
                "card_limit",
                "This card has too many jobs. Open the job list instead.",
            ));
        }
        match page.next {
            Some(next) => after = next,
            None => break,
        }
    }
    let artifacts = store.version_artifacts(&v.reference()).await?;
    if artifacts.len() > 1000 {
        return Err(ApiError::new(
            StatusCode::PAYLOAD_TOO_LARGE,
            "card_limit",
            "This card has too many files. Open the design instead.",
        ));
    }
    let mut card = kind.card(v, &jobs, &artifacts);
    if let (Some(project), Some(page)) = (space.strip_prefix("project:"), card.page.as_mut()) {
        let encoded: String = project
            .bytes()
            .map(|b| {
                if b.is_ascii_alphanumeric() || b"-_.~".contains(&b) {
                    (b as char).to_string()
                } else {
                    format!("%{b:02X}")
                }
            })
            .collect();
        page.push_str(if page.contains('?') {
            "&project="
        } else {
            "?project="
        });
        page.push_str(&encoded);
    }
    Ok(card)
}
async fn card(
    State(api): App,
    headers: HeaderMap,
    Path((id, n)): Path<(String, String)>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s, k) = api.document(&headers, &p.space, Action::Read, &id).await?;
    let n = n
        .parse::<u64>()
        .ok()
        .filter(|n| *n > 0)
        .ok_or_else(|| ApiError::invalid("Choose a valid version number."))?;
    let v = s.version(&id, n).await?;
    Ok(answer(
        StatusCode::OK,
        card_value(&api, &headers, &p.space, &s, k.as_ref(), &v).await?,
    ))
}
async fn enqueue(api: &Api, headers: &HeaderMap, r: CreateJob) -> Result<JobState, ApiError> {
    let (access, s, k) = api
        .document(headers, &r.space, Action::Edit, &r.version.document)
        .await?;
    let v = s.version(&r.version.document, r.version.version).await?;
    api.references(headers, k.as_ref(), &v.content).await?;
    let specs = k.jobs(&v, &r.want);
    if specs.len() != 1 {
        return Err(ApiError::invalid("Choose one job this lab supports."));
    }
    let mut spec = specs.into_iter().next().ok_or_else(ApiError::missing)?;
    // Work that reads a private referenced file stays on an enrolled host.
    if crate::workers::referenced(api, &access, &s, &v)
        .await?
        .iter()
        .any(Version::has_private_local)
    {
        spec.requirements.private_local = true;
    }
    s.reserve_request(
        key(headers)?,
        &json!({"operation":"job","request":r,"spec":spec}),
    )
    .await?;
    let requirements = lab_jobs::EffectiveRequirements::for_job(&spec, &v, &k.manifest())
        .map_err(lab_store::Error::from)?;
    let mut job = s.create_job(spec, &k.manifest(), now()).await?;
    let pending = job.clone();
    if let Some(capabilities) = k
        .server_capabilities()
        .filter(|caps| requirements.eligible(caps))
    {
        if pending.status == JobStatus::Queued {
            let backend = api.backend.clone();
            let authorizer = api.authorizer.clone();
            let headers = headers.clone();
            let alias = r.space;
            tokio::spawn(async move {
                let _ = crate::server::run(
                    backend,
                    authorizer,
                    headers,
                    alias,
                    access.account,
                    access.space,
                    k,
                    capabilities,
                    s,
                    pending,
                )
                .await;
            });
        }
    } else {
        s.wait_for_worker(
            &job.id,
            if requirements.private_local {
                "No computer with these files is online."
            } else {
                "Waiting for a computer with the tools and space for this work."
            },
            now(),
        )
        .await?;
        job = s.job(&job.id).await?;
    }
    Ok(job)
}
async fn job_create(
    State(api): App,
    headers: HeaderMap,
    body: Body<CreateJob>,
) -> Result<Response, ApiError> {
    Ok(answer(
        StatusCode::CREATED,
        enqueue(&api, &headers, body?.0).await?,
    ))
}
async fn jobs(State(api): App, headers: HeaderMap, p: Params) -> Result<Response, ApiError> {
    let p = params(p)?;
    let id = p
        .document
        .as_deref()
        .ok_or_else(|| ApiError::invalid("Choose a document."))?;
    let (_, s, _) = api.document(&headers, &p.space, Action::Read, id).await?;
    let page = s.jobs(id, p.after, p.limit).await?;
    Ok(answer(
        StatusCode::OK,
        List {
            data: page.items,
            next: page.next,
        },
    ))
}
async fn job(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s) = api.scoped(&headers, &p.space, Action::Read).await?;
    let j = s.job(&id).await?;
    let (_, _, k) = api
        .document(&headers, &p.space, Action::Read, &j.spec.version.document)
        .await?;
    let v = s
        .version(&j.spec.version.document, j.spec.version.version)
        .await?;
    api.references(&headers, k.as_ref(), &v.content).await?;
    Ok(answer(StatusCode::OK, j))
}
async fn cancel(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s) = api.scoped(&headers, &p.space, Action::Edit).await?;
    let j = s.job(&id).await?;
    api.document(&headers, &p.space, Action::Edit, &j.spec.version.document)
        .await?;
    Ok(answer(StatusCode::OK, s.cancel(&id, now()).await?))
}
async fn upload(
    State(api): App,
    headers: HeaderMap,
    body: Body<Upload>,
) -> Result<Response, ApiError> {
    let r = body?.0;
    let (_, s, k) = api
        .document(&headers, &r.space, Action::Edit, &r.version.document)
        .await?;
    let v = s.version(&r.version.document, r.version.version).await?;
    api.references(&headers, k.as_ref(), &v.content).await?;
    let bytes = base64::engine::general_purpose::STANDARD
        .decode(r.content_base64)
        .map_err(|_| ApiError::invalid("Choose a valid base64 file."))?;
    let a = s
        .put_artifact(
            ArtifactInput {
                version: r.version,
                role: ArtifactRole::Source,
                media_type: r.media_type,
                class: r.class,
                no_training: r.no_training,
            },
            bytes,
            512 * 1024,
            now(),
        )
        .await?;
    Ok(answer(StatusCode::CREATED, a))
}
pub(crate) async fn allowed_artifact(
    api: &Api,
    headers: &HeaderMap,
    space: &str,
    id: &str,
) -> Result<(Store, Artifact), ApiError> {
    let (_, s) = api.scoped(headers, space, Action::Read).await?;
    let a = s.artifact(id).await?;
    let (_, _, k) = api
        .document(headers, space, Action::Read, &a.version.document)
        .await?;
    let v = s.version(&a.version.document, a.version.version).await?;
    api.references(headers, k.as_ref(), &v.content).await?;
    Ok((s, a))
}
async fn artifact(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (s, a) = allowed_artifact(&api, &headers, &p.space, &id).await?;
    let signed = s.artifact_url(&id, 60).await?;
    let expires_seconds = signed.as_ref().map(|_| 60);
    let space = percent_encoding::utf8_percent_encode(&p.space, percent_encoding::NON_ALPHANUMERIC);
    let url = signed.unwrap_or_else(|| format!("/v1/labs/artifacts/{id}/content?space={space}"));
    Ok(answer(
        StatusCode::OK,
        ArtifactLink {
            artifact: a,
            url,
            expires_seconds,
        },
    ))
}
async fn content(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (s, a) = allowed_artifact(&api, &headers, &p.space, &id).await?;
    let range = match headers.get("Range") {
        None => None,
        Some(h) => {
            let r = h
                .to_str()
                .ok()
                .and_then(|s| s.strip_prefix("bytes="))
                .and_then(|s| s.split_once('-'))
                .ok_or_else(|| {
                    ApiError::new(
                        StatusCode::RANGE_NOT_SATISFIABLE,
                        "invalid_range",
                        "Choose one byte range within the file.",
                    )
                })?;
            let start =
                r.0.parse::<u64>()
                    .map_err(|_| ApiError::invalid("Choose a valid byte range."))?;
            let end = if r.1.is_empty() {
                a.size
            } else {
                r.1.parse::<u64>()
                    .ok()
                    .and_then(|n| n.checked_add(1))
                    .ok_or_else(|| ApiError::invalid("Choose a valid byte range."))?
            };
            if start >= end || end > a.size {
                return Err(ApiError::new(
                    StatusCode::RANGE_NOT_SATISFIABLE,
                    "invalid_range",
                    "Choose a byte range within the file.",
                ));
            }
            Some(start..end)
        }
    };
    let bytes = s.artifact_bytes(&id, range.clone()).await?;
    let mut r = (
        if range.is_some() {
            StatusCode::PARTIAL_CONTENT
        } else {
            StatusCode::OK
        },
        bytes,
    )
        .into_response();
    r.headers_mut().insert(
        "Cache-Control",
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    r.headers_mut().insert(
        "Accept-Ranges",
        axum::http::HeaderValue::from_static("bytes"),
    );
    r.headers_mut().insert(
        "X-Content-Type-Options",
        axum::http::HeaderValue::from_static("nosniff"),
    );
    r.headers_mut().insert(
        "Content-Disposition",
        axum::http::HeaderValue::from_static("attachment"),
    );
    if let Ok(t) = a.media_type.parse() {
        r.headers_mut().insert("Content-Type", t);
    }
    if let Some(range) = range {
        r.headers_mut().insert(
            "Content-Range",
            format!("bytes {}-{}/{}", range.start, range.end - 1, a.size)
                .parse()
                .map_err(|_| ApiError::invalid("Choose a valid byte range."))?,
        );
    }
    Ok(r)
}
async fn note(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    body: Body<AddNote>,
) -> Result<Response, ApiError> {
    let r = body?.0;
    let (a, s, k) = api.document(&headers, &r.space, Action::Edit, &id).await?;
    let v = s.version(&id, r.version).await?;
    api.references(&headers, k.as_ref(), &v.content).await?;
    s.add_note(Note {
        subject: v.reference(),
        locator: r.locator,
        author: a.account,
        text: r.text,
        created_unix_ms: now(),
    })
    .await?;
    Ok(answer(StatusCode::CREATED, Saved { saved: true }))
}
async fn notes(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s, _) = api.document(&headers, &p.space, Action::Read, &id).await?;
    let page = s.notes(&id, p.after, p.limit).await?;
    Ok(answer(
        StatusCode::OK,
        List {
            data: page.items,
            next: page.next,
        },
    ))
}
async fn events(
    State(api): App,
    headers: HeaderMap,
    Path(id): Path<String>,
    p: Params,
) -> Result<Response, ApiError> {
    let p = params(p)?;
    let (_, s, _) = api.document(&headers, &p.space, Action::Read, &id).await?;
    let since = p
        .since
        .or_else(|| {
            headers
                .get("Last-Event-ID")
                .and_then(|h| h.to_str().ok())
                .and_then(|s| s.parse().ok())
        })
        .unwrap_or(0);
    if !headers
        .get("Accept")
        .and_then(|h| h.to_str().ok())
        .is_some_and(|a| a.split(',').any(|t| t.trim() == "text/event-stream"))
    {
        let page = s.events(&id, since, p.limit).await?;
        return Ok(answer(
            StatusCode::OK,
            List {
                data: page.items,
                next: page.next,
            },
        ));
    }
    let deadline = tokio::time::Instant::now() + Duration::from_secs(300);
    let stream = futures_util::stream::unfold(
        (api, headers, p.space, id, since, deadline, false),
        |(api, h, space, id, mut since, deadline, mut sent)| async move {
            if tokio::time::Instant::now() >= deadline {
                return None;
            }
            if sent {
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            let (_, store, _) = api.document(&h, &space, Action::Read, &id).await.ok()?;
            let page = store.events(&id, since, 100).await.ok()?;
            let event = if let Some(last) = page.items.last() {
                since = last.cursor;
                Event::default()
                    .id(since.to_string())
                    .event("changes")
                    .json_data(json!({"data":page.items}))
                    .ok()?
            } else {
                Event::default().comment("keep-alive")
            };
            sent = true;
            Some((
                Ok::<_, Infallible>(event),
                (api, h, space, id, since, deadline, sent),
            ))
        },
    );
    let mut r = Sse::new(stream).into_response();
    r.headers_mut().insert(
        "Cache-Control",
        axum::http::HeaderValue::from_static("private, no-store"),
    );
    Ok(r)
}
async fn tool(
    State(api): App,
    headers: HeaderMap,
    Path((kind, name)): Path<(String, String)>,
    body: Body<ToolCall>,
) -> Result<Response, ApiError> {
    let r = body?.0;
    let (a, s) = api.scoped(&headers, &r.space, Action::Read).await?;
    let k = api.registry.get(&kind, &a)?;
    let m = k.manifest();
    if !m.tools.iter().any(|t| t.name == name) {
        return Err(ApiError::missing());
    }
    if canonical_json(&r.arguments).len() as u64 > m.limits.max_args_bytes {
        return Err(ApiError::invalid("These tool arguments are too large."));
    }
    let operation = k.tool(&name, &r.arguments)?;
    let document = match &operation {
        Tool::Put { document, .. } => document,
        Tool::Read { document } | Tool::Change { document, .. } | Tool::Job { document, .. } => {
            document
        }
    };
    if !matches!(&operation, Tool::Put { .. }) && s.document(document).await?.kind != kind {
        return Err(ApiError::new(
            StatusCode::FORBIDDEN,
            "wrong_kind",
            "Choose a document from this lab.",
        ));
    }
    let result = match operation {
        Tool::Put {
            document,
            title,
            schema,
            content,
            summary,
        } => {
            let (a, s) = api.scoped(&headers, &r.space, Action::Edit).await?;
            let k = api.registry.get(&kind, &a)?;
            let draft = match s.document(&document).await {
                Ok(meta) => {
                    if meta.kind != kind {
                        return Err(ApiError::invalid("Choose a document from this lab."));
                    }
                    let base = r.base.ok_or_else(|| {
                        ApiError::invalid("Choose the document version this tool changes.")
                    })?;
                    let version = s.version(&document, base).await?;
                    let change = Change::Patch {
                        ops: vec![lab_kit::change::PatchOp::Replace {
                            path: String::new(),
                            value: content,
                        }],
                    };
                    k.change(&s, &version, &change, &summary).await?
                }
                Err(lab_store::Error::NotFound) => {
                    k.initial(&s, &document, &schema, content, &summary).await?
                }
                Err(error) => return Err(error.into()),
            };
            api.references(&headers, k.as_ref(), draft.content())
                .await?;
            let version = if s.knows_document(&document).await? {
                s.save(&document, &a.account, draft, now()).await?
            } else {
                s.create(&document, &title, &a.account, draft, now())
                    .await?
            };
            k.saved(&s, &version).await?;
            json!({"version":version,"card":card_value(&api,&headers,&r.space,&s,k.as_ref(),&version).await?})
        }
        Tool::Read { document } => {
            let d = s.document(&document).await?;
            let v = s.version(&document, d.latest).await?;
            api.references(&headers, k.as_ref(), &v.content).await?;
            json!({"version":v,"card":card_value(&api,&headers,&r.space,&s,k.as_ref(),&v).await?})
        }
        Tool::Change {
            document,
            change,
            summary,
        } => {
            let (a, s, _) = api
                .document(&headers, &r.space, Action::Edit, &document)
                .await?;
            let base = r.base.ok_or_else(|| {
                ApiError::invalid("Choose the document version this tool changes.")
            })?;
            let v = s.version(&document, base).await?;
            let draft = k.change(&s, &v, &change, &summary).await?;
            api.references(&headers, k.as_ref(), draft.content())
                .await?;
            let v = s.save(&document, &a.account, draft, now()).await?;
            k.saved(&s, &v).await?;
            json!({"version":v,"card":card_value(&api,&headers,&r.space,&s,k.as_ref(),&v).await?})
        }
        Tool::Job {
            document,
            version,
            want,
        } => serde_json::to_value(
            enqueue(
                &api,
                &headers,
                CreateJob {
                    space: r.space,
                    version: Reference { document, version },
                    want,
                },
            )
            .await?,
        )
        .map_err(|_| ApiError::invalid("This job could not be read."))?,
    };
    Ok(answer(StatusCode::OK, ToolResult { result }))
}
