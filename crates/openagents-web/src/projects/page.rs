//! One project's page, `/projects/{id}`: its name and instructions, and
//! its repositories with a Primary mark, add, remove, and make primary.
//! A project needs no repository; when it has some, exactly one is the
//! primary (a chat started in the project works in it), and removing the
//! primary makes the first one left the primary.

use axum::Router;
use axum::extract::{Form, Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use maud::{Markup, html};
use oa_auth::repos::{Project, ProjectChange, RepoError, Status};
use openagents_ui::actions::{
    Alert, Badge, Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize,
};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::forms::{Input, Textarea};
use serde::Deserialize;

use super::{CSRF_SCOPE, PAGE, PageQuery, checked, failed, github_section, htmx_head, viewer};
use crate::App;
use crate::cloud::protect;
use crate::cloud::session::github::RepoCallError;
use crate::ui_page::UiPage;

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/projects/{id}", get(show))
        .route("/projects/{id}/edit", post(edit))
        .route("/projects/{id}/repositories", post(add_repository))
        .route(
            "/projects/{id}/repositories/remove",
            post(remove_repository),
        )
        .route("/projects/{id}/repositories/primary", post(make_primary))
}

/// The project's page.
pub(crate) fn href(id: &str) -> String {
    format!("{PAGE}/{id}")
}

/// Where Add puts a repository in the project.
pub(crate) fn repositories_href(id: &str) -> String {
    format!("{PAGE}/{id}/repositories")
}

/// The project's repositories in a few words: the primary, and how many
/// more, or that it has none.
pub(crate) fn repositories_line(project: &Project) -> String {
    match (project.repository(), project.repositories.len()) {
        (None, _) => "No repository".to_owned(),
        (Some(primary), 1) => primary.to_owned(),
        (Some(primary), n) => format!("{primary} and {} more", n - 1),
    }
}

/// The New project form: a name, and instructions if you like.
pub(crate) fn new_form(csrf: &str) -> Markup {
    html! {
        form method="post" action=(PAGE) {
            input type="hidden" name="csrf" value=(csrf);
            (Input::new("name").placeholder("Name").aria_label("Project name").maxlength(100).required(true))
            (Textarea::new("instructions").rows(3).aria_label("Instructions").placeholder("Instructions for chats in this project (optional)"))
            (Button::new("Create project").kind(ButtonType::Submit).size(ControlSize::Sm))
        }
    }
}

async fn show(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Query(query): Query<PageQuery>,
) -> Response {
    let (service, viewer) = match viewer(&app, &headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match service.github_status(&headers).await {
        Ok(status) => status,
        Err(error) => return failed(&headers, &error),
    };
    let csrf = service
        .csrf(&headers, &viewer, CSRF_SCOPE, "")
        .unwrap_or_default();
    let chats = project_chats(&app, &headers, &id).await;
    respond(
        &headers,
        &status,
        &id,
        &csrf,
        &query.q,
        None,
        StatusCode::OK,
        app.config.github_install.is_some(),
        crate::connections::access::permitted(&app, &viewer),
        chats.as_deref(),
    )
}

/// The project's chats, newest first (#11598); `None` when the list can't
/// be read, so the page still shows.
async fn project_chats(
    app: &App,
    headers: &HeaderMap,
    id: &str,
) -> Option<Vec<crate::chat_store::Conversation>> {
    let owner = crate::pages::chat::reader(app, headers).await?;
    let mut chats = app.config.chat_store.list(&owner).await.ok()?;
    chats.retain(|chat| chat.project.as_deref() == Some(id) && chat.archived_unix.is_none());
    Some(chats)
}

/// The Chats section of a project's page: each chat with its second line,
/// newest first.
pub(crate) fn chats_section(chats: &[crate::chat_store::Conversation]) -> Markup {
    html! {
        section aria-labelledby="project-chats" {
            (MarkdownRoot::new(html! { h2 #project-chats { "Chats" } }))
            @if chats.is_empty() {
                p { "No chats in this project yet." }
            } @else {
                ul.oa-chat-archive-list role="list" {
                    @for chat in chats {
                        li.oa-chat-archive-row {
                            a href=(format!("/chat/{}", chat.id)) {
                                @if chat.title.trim().is_empty() { "New chat" } @else { (chat.title) }
                            }
                            @if let Some(line) = crate::pages::chat::line_two(chat, false) {
                                span.oa-page-meta { (line) }
                            }
                        }
                    }
                }
            }
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn respond(
    headers: &HeaderMap,
    status: &Status,
    id: &str,
    csrf: &str,
    q: &str,
    problem: Option<&str>,
    code: StatusCode,
    install: bool,
    sources: bool,
    chats: Option<&[crate::chat_store::Conversation]>,
) -> Response {
    let Some(project) = status.projects.iter().find(|p| p.id == id) else {
        return protect(crate::ui_page::problem(
            headers,
            StatusCode::NOT_FOUND,
            "Projects",
            "That isn't one of your projects.",
            (PAGE, "Back to projects"),
        ));
    };
    protect(
        UiPage::new(project.name.clone())
            .path(href(&project.id))
            .section(PAGE)
            .status(code)
            .head(htmx_head())
            .content(PageColumn::new(html! {
                (view(status, project, csrf, q, problem, install, None, sources))
                @if let Some(chats) = chats { (chats_section(chats)) }
            }))
            .respond(headers),
    )
}

/// The page body (separate from I/O for tests and the screens catalog).
pub(crate) fn view(
    status: &Status,
    project: &Project,
    csrf: &str,
    q: &str,
    problem: Option<&str>,
    install: bool,
    list: Option<Markup>,
    sources: bool,
) -> Markup {
    let action = |what: &str| format!("{}/{what}", repositories_href(&project.id));
    html! {
        (MarkdownRoot::new(html! {
            p { a href=(PAGE) { "Projects" } }
            h1 { (project.name) }
        }))
        @if let Some(problem) = problem {
            (Alert::new().color(Color::Danger).description(problem))
        }
        div.oa-page-actions {
            (ButtonLink::new("New chat", format!("/?project={}", project.id)).size(ControlSize::Sm))
            @if sources {
                (ButtonLink::new("Sources", crate::connections::sources::href(&project.id))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .color(Color::Secondary))
            }
            (ButtonLink::new("Vault", crate::vault::project_href(&project.id))
                .size(ControlSize::Sm)
                .variant(ButtonVariant::Ghost)
                .color(Color::Secondary))
        }
        section aria-labelledby="project-about" {
            (MarkdownRoot::new(html! { h2 #project-about { "Name and instructions" } }))
            form method="post" action=(format!("{}/edit", href(&project.id))) {
                input type="hidden" name="csrf" value=(csrf);
                (Input::new("name").value(&project.name).aria_label("Project name").maxlength(100).required(true))
                (Textarea::new("instructions")
                    .rows(4)
                    .value(project.instructions.as_deref().unwrap_or_default())
                    .aria_label("Instructions")
                    .placeholder("Instructions for chats in this project (optional)"))
                (Button::new("Save").kind(ButtonType::Submit).size(ControlSize::Sm))
            }
        }
        section aria-labelledby="project-repositories" {
            (MarkdownRoot::new(html! { h2 #project-repositories { "Repositories" } }))
            @if project.repositories.is_empty() {
                p { "No repository yet. Chats in this project are plain chats with its instructions and files." }
            } @else {
                ul.oa-chat-archive-list role="list" {
                    @for repository in &project.repositories {
                        li.oa-chat-archive-row {
                            span {
                                (repository.repository)
                                @if repository.primary { " " (Badge::new("Primary")) }
                                @if repository.private { " " (Badge::new("Private")) }
                            }
                            span.oa-page-actions {
                                @if !repository.primary {
                                    form method="post" action=(action("primary")) {
                                        input type="hidden" name="csrf" value=(csrf);
                                        input type="hidden" name="repository" value=(repository.repository);
                                        (Button::new("Make primary")
                                            .kind(ButtonType::Submit)
                                            .size(ControlSize::Sm)
                                            .variant(ButtonVariant::Soft)
                                            .color(Color::Secondary))
                                    }
                                }
                                form method="post" action=(action("remove")) {
                                    input type="hidden" name="csrf" value=(csrf);
                                    input type="hidden" name="repository" value=(repository.repository);
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
                p.oa-page-meta { "Chats you start in this project work in the primary repository." }
            }
        }
        section aria-labelledby="project-add" {
            (MarkdownRoot::new(html! { h2 #project-add { "Add a repository" } }))
            (github_section(status, csrf, q, install, list, Some(project)))
        }
    }
}

#[derive(Deserialize)]
struct EditForm {
    csrf: String,
    name: String,
    #[serde(default)]
    instructions: String,
}

#[derive(Deserialize)]
struct RepositoryForm {
    csrf: String,
    repository: String,
}

/// Apply `change` and go back to the project's page, or show it again with
/// what went wrong.
async fn apply(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    csrf: &str,
    change: ProjectChange,
) -> Response {
    let (service, viewer) = match checked(app, headers, csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    match service.edit_project(headers, id, &change).await {
        Ok(project) => protect(Redirect::to(&href(&project.id)).into_response()),
        Err(RepoCallError::Repo(error)) => {
            let status = match service.github_status(headers).await {
                Ok(status) => status,
                Err(other) => return failed(headers, &other),
            };
            let problem = match error {
                RepoError::NotFound if change.add_repository.is_none() => {
                    "That repository isn't in this project anymore.".to_owned()
                }
                RepoError::Invalid if change.name.is_some() => {
                    "Give the project a name of up to 100 characters.".to_owned()
                }
                _ => error.to_string(),
            };
            let token = service
                .csrf(headers, &viewer, CSRF_SCOPE, "")
                .unwrap_or_default();
            respond(
                headers,
                &status,
                id,
                &token,
                "",
                Some(&problem),
                StatusCode::from_u16(error.status()).unwrap_or(StatusCode::BAD_REQUEST),
                app.config.github_install.is_some(),
                crate::connections::access::permitted(app, &viewer),
                None,
            )
        }
        Err(error) => failed(headers, &error),
    }
}

async fn edit(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<EditForm>,
) -> Response {
    let instructions = form.instructions.trim().to_owned();
    let change = ProjectChange {
        name: Some(form.name.trim().to_owned()),
        instructions: Some((!instructions.is_empty()).then_some(instructions)),
        ..ProjectChange::default()
    };
    apply(&app, &headers, &id, &form.csrf, change).await
}

async fn add_repository(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<RepositoryForm>,
) -> Response {
    let change = ProjectChange {
        add_repository: Some(form.repository.trim().to_owned()),
        ..ProjectChange::default()
    };
    apply(&app, &headers, &id, &form.csrf, change).await
}

async fn remove_repository(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<RepositoryForm>,
) -> Response {
    let change = ProjectChange {
        remove_repository: Some(form.repository.trim().to_owned()),
        ..ProjectChange::default()
    };
    apply(&app, &headers, &id, &form.csrf, change).await
}

async fn make_primary(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<RepositoryForm>,
) -> Response {
    let change = ProjectChange {
        primary_repository: Some(form.repository.trim().to_owned()),
        ..ProjectChange::default()
    };
    apply(&app, &headers, &id, &form.csrf, change).await
}
