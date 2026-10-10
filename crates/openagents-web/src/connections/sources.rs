//! A project's sources (#11238): the Google Drive folders and files the
//! chat in that project reads from.
//!
//! `/projects/{id}/sources` lists them with Remove, adds one from a pasted
//! Drive link, and has a picker: browse My Drive folder by folder, or
//! search it, and Attach a folder or file. Attaching checks the item with
//! Drive first, so only something the connection can read is kept.

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use oa_auth::repos::Project;
use oa_connections::google::drive::{self, File};
use openagents_ui::actions::{
    Alert, Badge, Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize,
};
use openagents_ui::content::MarkdownRoot;
use openagents_ui::forms::Input;
use serde::Deserialize;

use super::{CONNECT, Unready, live};
use crate::App;
use crate::chat_store::account_owner;
use crate::cloud::connections::Source;
use crate::cloud::session::{CloudSession, SessionError, Viewer, now};
use crate::cloud::{protect, refused};

const CSRF_SCOPE: &str = "project-sources";

pub(crate) fn routes() -> Router<App> {
    Router::new()
        .route("/projects/{id}/sources", get(page).post(attach))
        .route("/projects/{id}/sources/remove", post(remove))
}

/// The sources page of a project.
pub(crate) fn href(project: &str) -> String {
    format!("/projects/{project}/sources")
}

fn target(viewer: &Viewer, project: &str) -> String {
    format!("{}:{project}", viewer.account_id)
}

/// The viewer and their project `id`, or the answer to give instead.
async fn project<'a>(
    app: &'a App,
    headers: &HeaderMap,
    id: &str,
) -> Result<(&'a CloudSession, Viewer, Project), Response> {
    let (service, viewer) = crate::settings::viewer(app, headers, &href(id)).await?;
    let status = service.github_status(headers).await.map_err(|_| {
        protect(crate::layout::problem(
            StatusCode::SERVICE_UNAVAILABLE,
            "Sources",
            "Your projects couldn't be read right now. Try again in a minute.",
            (crate::projects::PAGE, "Projects"),
        ))
    })?;
    let project = status
        .projects
        .into_iter()
        .find(|p| p.id == id)
        .ok_or_else(|| {
            protect(crate::layout::problem(
                StatusCode::NOT_FOUND,
                "Sources",
                "That project isn't one of yours.",
                (crate::projects::PAGE, "Projects"),
            ))
        })?;
    Ok((service, viewer, project))
}

#[derive(Deserialize, Default)]
struct PageQuery {
    #[serde(default)]
    folder: Option<String>,
    #[serde(default)]
    q: Option<String>,
    #[serde(default)]
    problem: Option<String>,
}

async fn page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let (service, viewer, project) = match project(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let csrf = service
        .csrf(&headers, &viewer, CSRF_SCOPE, &target(&viewer, &id))
        .unwrap_or_default();
    let attached: Vec<Source> = app
        .config
        .connections
        .as_deref()
        .and_then(|store| store.load(&owner).ok())
        .map(|account| account.sources_of(&id).into_iter().cloned().collect())
        .unwrap_or_default();
    let live = live(&app, &owner).await;
    // The picker: a search, or one folder (My Drive by default).
    let browse = match &live {
        Ok(live) => {
            let q = query.q.as_deref().map(str::trim).filter(|q| !q.is_empty());
            let listed = match q {
                Some(q) => live
                    .drive
                    .search(q, None, 30)
                    .await
                    .map(|files| (files, None)),
                None => {
                    let folder = query
                        .folder
                        .as_deref()
                        .filter(|f| drive::valid_id(f))
                        .unwrap_or("root");
                    live.drive
                        .list_folder(folder, None)
                        .await
                        .map(|(files, _)| (files, (folder != "root").then(|| folder.to_owned())))
                }
            };
            Some(listed.map_err(|error| error.to_string()))
        }
        Err(_) => None,
    };
    let body = view(
        &project,
        &csrf,
        &attached,
        live.as_ref().err().copied(),
        browse,
        query.q.as_deref().unwrap_or_default(),
        query.problem.as_deref(),
    );
    crate::settings::page(
        &headers,
        service,
        &viewer,
        &format!("Sources · {}", project.name),
        crate::projects::PAGE,
        body,
    )
}

fn attach_form(project: &str, csrf: &str, id: &str, label: &str) -> Markup {
    html! {
        form method="post" action=(href(project)) {
            input type="hidden" name="csrf" value=(csrf);
            input type="hidden" name="id" value=(id);
            (Button::new(label)
                .kind(ButtonType::Submit)
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Soft)
                .color(Color::Secondary))
        }
    }
}

fn view(
    project: &Project,
    csrf: &str,
    attached: &[Source],
    unready: Option<Unready>,
    browse: Option<Result<(Vec<File>, Option<String>), String>>,
    q: &str,
    problem: Option<&str>,
) -> Markup {
    let here = href(&project.id);
    let attached_ids: Vec<&str> = attached.iter().map(|s| s.id.as_str()).collect();
    html! {
        (MarkdownRoot::new(html! {
            h1 { "Sources" }
            p {
                "Google Drive folders and files the chat in " strong { (project.name) }
                " reads from to answer. Read only."
            }
            p { (super::chat::FAST) }
        }))
        @if let Some(problem) = problem {
            (Alert::new().color(Color::Danger).description(problem))
        }
        @match unready {
            Some(Unready::NotConnected) => {
                (Alert::new()
                    .title("Connect Google first")
                    .description("Then attach the Drive folders and files this project's chat should read.")
                    .actions(ButtonLink::new("Connect Google", format!("{CONNECT}?return_to={here}"))))
            }
            Some(Unready::Reconnect) => {
                (Alert::new()
                    .color(Color::Warning)
                    .title("Google access ended")
                    .description("Your sources are still here. Connect Google again to use them.")
                    .actions(ButtonLink::new("Connect again", format!("{CONNECT}?return_to={here}"))))
            }
            Some(other) => { (Alert::new().description(other.words())) }
            None => {}
        }
        section aria-labelledby="sources-attached" {
            (MarkdownRoot::new(html! { h2 #sources-attached { "Attached" } }))
            @if attached.is_empty() {
                (MarkdownRoot::new(html! { p { "Nothing yet." } }))
            } @else {
                ul.oa-chat-archive-list role="list" {
                    @for source in attached {
                        li.oa-chat-archive-row {
                            span {
                                a href=(source.link()) rel="noopener" target="_blank" { (source.name) }
                                " " (Badge::new(source.kind.clone()))
                            }
                            span.oa-page-actions {
                                form method="post" action=(format!("{here}/remove")) {
                                    input type="hidden" name="csrf" value=(csrf);
                                    input type="hidden" name="id" value=(source.id);
                                    (Button::new("Remove")
                                        .kind(ButtonType::Submit)
                                        .size(ControlSize::Sm)
                                        .variant(ButtonVariant::Ghost)
                                        .color(Color::Secondary))
                                }
                            }
                        }
                    }
                }
            }
        }
        @if let Some(browse) = browse {
            section aria-labelledby="sources-add" {
                (MarkdownRoot::new(html! { h2 #sources-add { "Add from Google Drive" } }))
                form method="post" action=(here) class="oa-page-actions" {
                    input type="hidden" name="csrf" value=(csrf);
                    (Input::new("link").placeholder("Paste a Drive folder or file link").aria_label("Drive link"))
                    (Button::new("Attach").kind(ButtonType::Submit))
                }
                form method="get" action=(here) class="oa-page-actions" {
                    (Input::new("q").value(q).placeholder("Search your Drive").aria_label("Search your Drive"))
                    (Button::new("Search").kind(ButtonType::Submit).variant(ButtonVariant::Soft).color(Color::Secondary))
                }
                @match browse {
                    Ok((files, parent)) => {
                        @if parent.is_some() || !q.is_empty() {
                            (MarkdownRoot::new(html! { p { a href=(here) { "Back to My Drive" } } }))
                        }
                        @if files.is_empty() {
                            (MarkdownRoot::new(html! { p { "No files here." } }))
                        } @else {
                            ul.oa-chat-archive-list role="list" {
                                @for file in &files {
                                    li.oa-chat-archive-row {
                                        span {
                                            @if file.is_folder() {
                                                a href=(format!("{here}?folder={}", file.id)) { (file.name) }
                                            } @else {
                                                (file.name)
                                            }
                                            " " (Badge::new(file.kind()))
                                        }
                                        span.oa-page-actions {
                                            @if attached_ids.contains(&file.id.as_str()) {
                                                "Attached"
                                            } @else {
                                                (attach_form(&project.id, csrf, &file.id, "Attach"))
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                    Err(problem) => { (Alert::new().color(Color::Danger).description(problem)) }
                }
            }
        }
        (MarkdownRoot::new(html! {
            p { a href=(crate::projects::PAGE) { "Back to Projects" } }
        }))
    }
}

#[derive(Deserialize)]
struct AttachForm {
    csrf: String,
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    link: Option<String>,
}

fn back(project: &str, problem: Option<&str>) -> Response {
    let mut to = href(project);
    if let Some(problem) = problem {
        to.push_str("?problem=");
        to.push_str(&url::form_urlencoded::byte_serialize(problem.as_bytes()).collect::<String>());
    }
    protect(Redirect::to(&to).into_response())
}

async fn attach(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<AttachForm>,
) -> Response {
    let (service, viewer, project) = match project(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        CSRF_SCOPE,
        &target(&viewer, &id),
        &form.csrf,
    ) {
        return refused(error);
    }
    let Some(store) = app.config.connections.as_deref() else {
        return refused(SessionError::Unavailable);
    };
    let owner = account_owner(&viewer.account_id);
    let item = match (form.id.as_deref(), form.link.as_deref()) {
        (Some(item), _) if drive::valid_id(item) && item != "root" => item.to_owned(),
        (_, Some(link)) => match drive::parse_link(link) {
            Some(found) => found.id,
            None => {
                return back(
                    &project.id,
                    Some("That isn't a Google Drive folder or file link."),
                );
            }
        },
        _ => {
            return back(
                &project.id,
                Some("Pick a folder or file, or paste its link."),
            );
        }
    };
    let live = match live(&app, &owner).await {
        Ok(live) => live,
        Err(unready) => return back(&project.id, Some(unready.words())),
    };
    let file = match live.drive.file(&item).await {
        Ok(file) => file,
        Err(error) => return back(&project.id, Some(&error.to_string())),
    };
    let source = Source {
        project: project.id.clone(),
        integration: oa_connections::google::SLUG.into(),
        id: file.id.clone(),
        name: if file.name.is_empty() {
            file.id.clone()
        } else {
            file.name.clone()
        },
        kind: file.kind().into(),
        added_at: now(),
    };
    match store.update(&owner, |account| account.attach(source)) {
        Ok(Ok(())) => back(&project.id, None),
        Ok(Err(why)) => back(&project.id, Some(why)),
        Err(_) => back(
            &project.id,
            Some("That couldn't be saved. Try again later."),
        ),
    }
}

#[derive(Deserialize)]
struct RemoveForm {
    csrf: String,
    id: String,
}

async fn remove(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<RemoveForm>,
) -> Response {
    let (service, viewer, project) = match project(&app, &headers, &id).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if let Err(error) = service.verify_csrf(
        &headers,
        Some(&viewer),
        CSRF_SCOPE,
        &target(&viewer, &id),
        &form.csrf,
    ) {
        return refused(error);
    }
    let Some(store) = app.config.connections.as_deref() else {
        return refused(SessionError::Unavailable);
    };
    let owner = account_owner(&viewer.account_id);
    match store.update(&owner, |account| {
        account
            .sources
            .retain(|s| !(s.project == project.id && s.id == form.id));
    }) {
        Ok(()) => back(&project.id, None),
        Err(_) => back(
            &project.id,
            Some("That couldn't be saved. Try again later."),
        ),
    }
}
