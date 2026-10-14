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
use oa_auth::repos::{Project, ProjectChange, RepoError, Role, Status};
use openagents_ui::actions::CopyButton;
use openagents_ui::actions::{
    Alert, Badge, Button, ButtonLink, ButtonType, ButtonVariant, Color, ControlSize,
};
use openagents_ui::content::{MarkdownRoot, PageColumn};
use openagents_ui::forms::{Input, Textarea};
use reqwest::Method;
use serde::Deserialize;
use serde_json::{Value, json};

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
        .route("/projects/{id}/invite", post(invite))
        .route("/projects/{id}/members/{account}/role", post(set_role))
        .route(
            "/projects/{id}/members/{account}/remove",
            post(remove_member),
        )
        .route("/projects/{id}/leave", post(leave))
        .route("/projects/join/{code}", get(join_page).post(join))
}

/// The plain mark for a shared project: "Shared", with your role when it
/// isn't yours.
pub(crate) fn shared_label(role: Role) -> &'static str {
    match role {
        Role::Owner => "Shared",
        Role::Editor => "Shared · editor",
        Role::Viewer => "Shared · viewer",
    }
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
    let people = people(&app, service, &headers, &status, &id, &csrf, None).await;
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
        people,
    )
}

/// What the Members section shows, and the notes about repositories the
/// viewer's own GitHub access doesn't reach (#11587, #11589).
pub(crate) struct People {
    pub(crate) members: Value,
    /// The project's files open only on its owner's devices: no invites.
    pub(crate) only_me: bool,
    /// Repositories the viewer can't reach on GitHub, with why.
    pub(crate) unreachable: Vec<(String, String)>,
    /// A link just made: shown once.
    pub(crate) invited: Option<(String, String)>,
}

async fn people(
    app: &App,
    service: &crate::cloud::session::CloudSession,
    headers: &HeaderMap,
    status: &Status,
    id: &str,
    _csrf: &str,
    invited: Option<(String, String)>,
) -> Option<People> {
    let project = status.projects.iter().find(|p| p.id == id)?;
    let members = service
        .project_call(
            headers,
            Method::GET,
            &format!("/v1/projects/{id}/members"),
            None,
        )
        .await
        .unwrap_or(Value::Null);
    let home = project
        .sharing
        .as_ref()
        .and_then(|s| s.home.as_deref())
        .map(crate::chat_store::account_owner);
    let owner = match home {
        Some(home) => home,
        None => crate::pages::chat::reader(app, headers)
            .await
            .unwrap_or_default(),
    };
    let only_me = matches!(
        super::files::protection(&app.config.chat_store, &owner, id).await,
        Ok(super::files::Protection::OnlyMe)
    );
    let mut unreachable = Vec::new();
    if super::api::shared(project) && !project.repositories.is_empty() {
        if let Ok(reach) = service
            .project_call(
                headers,
                Method::GET,
                &format!("/v1/projects/{id}/access"),
                None,
            )
            .await
        {
            for row in reach["repositories"].as_array().into_iter().flatten() {
                if row["access"] == false {
                    unreachable.push((
                        row["repository"].as_str().unwrap_or_default().to_owned(),
                        row["message"]
                            .as_str()
                            .unwrap_or("You don't have access to this repository on GitHub.")
                            .to_owned(),
                    ));
                }
            }
        }
    }
    Some(People {
        members,
        only_me,
        unreachable,
        invited,
    })
}

/// "an editor" or "a viewer".
fn a_role(role: &str) -> &'static str {
    match role {
        "owner" => "the owner",
        "viewer" => "a viewer",
        _ => "an editor",
    }
}

/// The Members section: who is in the project and their roles; the owner
/// changes roles, removes people, and makes invitation links; everyone
/// else can leave (#11589).
pub(crate) fn members_section(project: &Project, people: &People, csrf: &str) -> Markup {
    let role = super::api::role_of(project);
    let rows = people.members["members"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    let base = href(&project.id);
    html! {
        section #members aria-labelledby="project-members" {
            (MarkdownRoot::new(html! { h2 #project-members { "Members" } }))
            @if rows.is_empty() {
                p { "Only you are in this project." }
            } @else {
                ul.oa-chat-archive-list role="list" {
                    @for row in &rows {
                        @let account = row["account"].as_str().unwrap_or_default();
                        @let their = row["role"].as_str().unwrap_or("viewer");
                        @let you = row["you"] == true;
                        li.oa-chat-archive-row {
                            span {
                                (row["name"].as_str().unwrap_or("Someone"))
                                @if let Some(login) = row["login"].as_str() {
                                    @if Some(login) != row["name"].as_str() { " " span.oa-page-meta { "@" (login) } }
                                }
                                @if you { " " span.oa-page-meta { "(you)" } }
                                " " (Badge::new(match their { "owner" => "Owner", "editor" => "Editor", _ => "Viewer" }))
                            }
                            @if role.owns() && !you {
                                span.oa-page-actions {
                                    form method="post" action=(format!("{base}/members/{account}/role")) {
                                        input type="hidden" name="csrf" value=(csrf);
                                        input type="hidden" name="role" value=(if their == "editor" { "viewer" } else { "editor" });
                                        (Button::new(if their == "editor" { "Make viewer" } else { "Make editor" })
                                            .kind(ButtonType::Submit)
                                            .size(ControlSize::Sm)
                                            .variant(ButtonVariant::Soft)
                                            .color(Color::Secondary))
                                    }
                                    form method="post" action=(format!("{base}/members/{account}/role")) {
                                        input type="hidden" name="csrf" value=(csrf);
                                        input type="hidden" name="role" value="owner";
                                        (Button::new("Make owner")
                                            .kind(ButtonType::Submit)
                                            .size(ControlSize::Sm)
                                            .variant(ButtonVariant::Ghost)
                                            .color(Color::Secondary))
                                    }
                                    form method="post" action=(format!("{base}/members/{account}/remove")) {
                                        input type="hidden" name="csrf" value=(csrf);
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
            @if let Some((link, their)) = &people.invited {
                (Alert::new()
                    .color(Color::Success)
                    .title("Invitation link")
                    .description(format!("Anyone with this link can join once as {} within 7 days. It's shown only now.", a_role(their))))
                div class="oa-terminal-command" {
                    pre { code { (link) } }
                    (CopyButton::new(link.as_str())
                        .label("Copy")
                        .copied_label("Copied")
                        .variant(ButtonVariant::Soft)
                        .color(Color::Secondary))
                }
            }
            @if role.owns() {
                @if people.only_me {
                    p.oa-page-meta { (super::members::ONLY_ME_REFUSAL) }
                } @else {
                    form.oa-page-actions method="post" action=(format!("{base}/invite")) {
                        input type="hidden" name="csrf" value=(csrf);
                        (Button::new("Invite an editor")
                            .kind(ButtonType::Submit)
                            .size(ControlSize::Sm)
                            .name("role")
                            .value("editor"))
                        (Button::new("Invite a viewer")
                            .kind(ButtonType::Submit)
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Soft)
                            .color(Color::Secondary)
                            .name("role")
                            .value("viewer"))
                    }
                }
            } @else {
                form method="post" action=(format!("{base}/leave")) {
                    input type="hidden" name="csrf" value=(csrf);
                    (Button::new("Leave this project")
                        .kind(ButtonType::Submit)
                        .size(ControlSize::Sm)
                        .variant(ButtonVariant::Ghost)
                        .color(Color::Secondary))
                }
            }
        }
    }
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
    // A shared project's chats are everyone's (#11586).
    if let Some(sidebar) = super::sidebar(app).await
        && let Some(project) = sidebar.project(id)
        && super::api::shared(project)
    {
        chats = super::shared_chats::chats_in(app, project, &owner, &chats)
            .await
            .ok()?;
        let names = super::shared_chats::starters(app, headers, project).await;
        for chat in chats.iter_mut().filter(|chat| chat.owner != owner) {
            // Shown on the row: who started someone else's chat.
            chat.title = format!(
                "{} · started by {}",
                if chat.title.trim().is_empty() {
                    "New chat"
                } else {
                    chat.title.as_str()
                },
                super::shared_chats::started_by(chat, &owner, &names)
            );
        }
        chats.sort_by(|a, b| b.updated_unix.cmp(&a.updated_unix));
    }
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
    people: Option<People>,
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
                @if let Some(people) = &people {
                    @for (repository, why) in &people.unreachable {
                        (Alert::new().color(Color::Warning).title(repository.as_str()).description(why.as_str()))
                    }
                }
                (view(status, project, csrf, q, problem, install, None, sources))
                @if let Some(people) = &people { (members_section(project, people, csrf)) }
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
    let role = super::api::role_of(project);
    // Sources and the vault belong to the account whose project it is.
    let home = project.sharing.as_ref().is_none_or(|s| s.home.is_none());
    html! {
        (MarkdownRoot::new(html! {
            p { a href=(PAGE) { "Projects" } }
            h1 { (project.name) }
        }))
        @if let Some(problem) = problem {
            (Alert::new().color(Color::Danger).description(problem))
        }
        @if super::api::shared(project) {
            p.oa-page-meta { (shared_label(role)) }
        }
        div.oa-page-actions {
            @if role.edits() {
                (ButtonLink::new("New chat", format!("/?project={}", project.id)).size(ControlSize::Sm))
            }
            @if sources && home {
                (ButtonLink::new("Sources", crate::connections::sources::href(&project.id))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .color(Color::Secondary))
            }
            @if home {
                (ButtonLink::new("Vault", crate::vault::project_href(&project.id))
                    .size(ControlSize::Sm)
                    .variant(ButtonVariant::Ghost)
                    .color(Color::Secondary))
            }
        }
        section aria-labelledby="project-about" {
            @if role.edits() {
                (MarkdownRoot::new(html! { h2 #project-about { @if role.owns() { "Name and instructions" } @else { "Instructions" } } }))
                form method="post" action=(format!("{}/edit", href(&project.id))) {
                    input type="hidden" name="csrf" value=(csrf);
                    @if role.owns() {
                        (Input::new("name").value(&project.name).aria_label("Project name").maxlength(100).required(true))
                    } @else {
                        input type="hidden" name="name" value=(project.name);
                    }
                    (Textarea::new("instructions")
                        .rows(4)
                        .value(project.instructions.as_deref().unwrap_or_default())
                        .aria_label("Instructions")
                        .placeholder("Instructions for chats in this project (optional)"))
                    (Button::new("Save").kind(ButtonType::Submit).size(ControlSize::Sm))
                }
            } @else {
                (MarkdownRoot::new(html! { h2 #project-about { "Instructions" } }))
                @match project.instructions.as_deref().filter(|t| !t.trim().is_empty()) {
                    Some(text) => p style="white-space: pre-wrap" { (text) },
                    None => p.oa-page-meta { "No instructions." },
                }
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
                            @if role.edits() {
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
                }
                p.oa-page-meta { "Chats you start in this project work in the primary repository." }
            }
        }
        @if role.edits() {
            section aria-labelledby="project-add" {
                (MarkdownRoot::new(html! { h2 #project-add { "Add a repository" } }))
                (github_section(status, csrf, q, install, list, Some(project)))
            }
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

#[derive(Deserialize)]
struct RoleForm {
    csrf: String,
    #[serde(default)]
    role: String,
}

#[derive(Deserialize)]
struct CsrfForm {
    csrf: String,
}

/// The page again with `problem`, or with a link just made.
async fn again(
    app: &App,
    headers: &HeaderMap,
    id: &str,
    problem: Option<String>,
    invited: Option<(String, String)>,
    code: StatusCode,
) -> Response {
    let (service, viewer) = match viewer(app, headers).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let status = match service.github_status(headers).await {
        Ok(status) => status,
        Err(error) => return failed(headers, &error),
    };
    let csrf = service
        .csrf(headers, &viewer, CSRF_SCOPE, "")
        .unwrap_or_default();
    let chats = project_chats(app, headers, id).await;
    let people = people(app, service, headers, &status, id, &csrf, invited).await;
    respond(
        headers,
        &status,
        id,
        &csrf,
        "",
        problem.as_deref(),
        code,
        app.config.github_install.is_some(),
        crate::connections::access::permitted(app, &viewer),
        chats.as_deref(),
        people,
    )
}

fn problem_of(error: &RepoCallError) -> (String, StatusCode) {
    match error {
        RepoCallError::Repo(error) => (
            error.to_string(),
            StatusCode::from_u16(error.status()).unwrap_or(StatusCode::BAD_REQUEST),
        ),
        RepoCallError::Session(_) => (
            "That didn't work. Try again in a minute.".to_owned(),
            StatusCode::SERVICE_UNAVAILABLE,
        ),
    }
}

/// Owner: make a single-use invitation link for an editor or a viewer.
async fn invite(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<RoleForm>,
) -> Response {
    let (service, _) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let role = if form.role == "viewer" {
        "viewer"
    } else {
        "editor"
    };
    if !oa_auth::repos::project_id(&id) {
        return again(&app, &headers, &id, None, None, StatusCode::NOT_FOUND).await;
    }
    // Files that open only on the owner's devices: not shareable (#11588).
    let owner = crate::pages::chat::reader(&app, &headers)
        .await
        .unwrap_or_default();
    if matches!(
        super::files::protection(&app.config.chat_store, &owner, &id).await,
        Ok(super::files::Protection::OnlyMe)
    ) {
        return again(
            &app,
            &headers,
            &id,
            Some(super::members::ONLY_ME_REFUSAL.to_owned()),
            None,
            StatusCode::CONFLICT,
        )
        .await;
    }
    match service
        .project_call(
            &headers,
            Method::POST,
            &format!("/v1/projects/{id}/invitations"),
            Some(json!({"role": role, "seconds": 0})),
        )
        .await
    {
        Ok(made) => {
            let code = made["code"].as_str().unwrap_or_default();
            let link = format!(
                "{}{}/{code}",
                crate::traces::origin(&app, &headers),
                super::members::JOIN
            );
            again(
                &app,
                &headers,
                &id,
                None,
                Some((link, role.to_owned())),
                StatusCode::OK,
            )
            .await
        }
        Err(error) => {
            let (problem, code) = problem_of(&error);
            again(&app, &headers, &id, Some(problem), None, code).await
        }
    }
}

fn valid_account(account: &str) -> bool {
    !account.is_empty()
        && account.len() <= 128
        && account
            .bytes()
            .all(|b| b.is_ascii_alphanumeric() || matches!(b, b'_' | b'-' | b'.' | b':'))
}

/// Owner: change a member's role, or give them the project.
async fn set_role(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, account)): Path<(String, String)>,
    Form(form): Form<RoleForm>,
) -> Response {
    let (service, _) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if !valid_account(&account) || Role::from_word(&form.role).is_none() {
        return again(&app, &headers, &id, None, None, StatusCode::BAD_REQUEST).await;
    }
    match service
        .project_call(
            &headers,
            Method::PATCH,
            &format!("/v1/projects/{id}/members/{account}"),
            Some(json!({"role": form.role})),
        )
        .await
    {
        Ok(_) => protect(Redirect::to(&format!("{}#members", href(&id))).into_response()),
        Err(error) => {
            let (problem, code) = problem_of(&error);
            again(&app, &headers, &id, Some(problem), None, code).await
        }
    }
}

/// Owner: take a member out.
async fn remove_member(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, account)): Path<(String, String)>,
    Form(form): Form<CsrfForm>,
) -> Response {
    let (service, _) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    if !valid_account(&account) {
        return again(&app, &headers, &id, None, None, StatusCode::BAD_REQUEST).await;
    }
    match service
        .project_call(
            &headers,
            Method::DELETE,
            &format!("/v1/projects/{id}/members/{account}"),
            None,
        )
        .await
    {
        Ok(_) => protect(Redirect::to(&format!("{}#members", href(&id))).into_response()),
        Err(error) => {
            let (problem, code) = problem_of(&error);
            again(&app, &headers, &id, Some(problem), None, code).await
        }
    }
}

/// Anyone but the owner: leave the project.
async fn leave(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Response {
    let (service, _) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    match service
        .project_call(
            &headers,
            Method::POST,
            &format!("/v1/projects/{id}/leave"),
            None,
        )
        .await
    {
        Ok(_) => protect(Redirect::to(PAGE).into_response()),
        Err(error) => {
            let (problem, code) = problem_of(&error);
            again(&app, &headers, &id, Some(problem), None, code).await
        }
    }
}

/// The signed-in viewer for an invitation link: signed out, through
/// sign-in and back here.
async fn join_viewer<'a>(
    app: &'a App,
    headers: &HeaderMap,
    code: &str,
) -> Result<
    (
        &'a crate::cloud::session::CloudSession,
        crate::cloud::session::Viewer,
    ),
    Response,
> {
    let service = crate::cloud::service(app)?;
    match service.authenticate(headers).await {
        Ok(viewer) => Ok((service, viewer)),
        Err(crate::cloud::session::SessionError::Unauthenticated) => Err(protect(
            Redirect::to(&crate::auth::login_href(
                &format!("{}/{code}", super::members::JOIN),
                false,
            ))
            .into_response(),
        )),
        Err(error) => Err(crate::cloud::refused(error)),
    }
}

fn join_problem(headers: &HeaderMap, text: &str, code: StatusCode) -> Response {
    protect(crate::ui_page::problem(
        headers,
        code,
        "Join a project",
        text,
        (PAGE, "Your projects"),
    ))
}

/// An invitation link: the project's name, who invited you, and the role,
/// with Join and Decline.
async fn join_page(
    State(app): State<App>,
    headers: HeaderMap,
    Path(code): Path<String>,
) -> Response {
    let Some(code) = super::members::code_in(&code) else {
        return join_problem(
            &headers,
            &RepoError::InvitationGone.to_string(),
            StatusCode::NOT_FOUND,
        );
    };
    let (service, viewer) = match join_viewer(&app, &headers, &code).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    let preview = match service
        .project_call(
            &headers,
            Method::POST,
            "/v1/projects/invitations/preview",
            Some(json!({"code": code})),
        )
        .await
    {
        Ok(preview) => preview,
        Err(error) => {
            let (problem, status) = problem_of(&error);
            return join_problem(&headers, &problem, status);
        }
    };
    let csrf = service
        .csrf(&headers, &viewer, CSRF_SCOPE, "")
        .unwrap_or_default();
    let name = preview["project"]["name"].as_str().unwrap_or("A project");
    let id = preview["project"]["id"].as_str().unwrap_or_default();
    let inviter = preview["invited_by"].as_str().unwrap_or("Someone");
    let role = preview["role"].as_str().unwrap_or("viewer");
    let already = preview["member"].is_string();
    protect(
        UiPage::new(format!("Join {name}"))
            .path(format!("{}/{code}", super::members::JOIN))
            .section(PAGE)
            .content(PageColumn::new(html! {
                (MarkdownRoot::new(html! {
                    h1 { (name) }
                    @if already {
                        p { "You're already in this project." }
                    } @else {
                        p { (inviter) " invited you to join as " (a_role(role)) "." }
                        p.oa-page-meta { @if role == "viewer" { "You'll read its chats, files, notes, and instructions." } @else { "You'll read and add chats, files, and notes, and edit its instructions and repositories." } " Messages you send are paid from your own account." }
                    }
                }))
                div.oa-page-actions {
                    @if already {
                        (ButtonLink::new("Open the project", href(id)).size(ControlSize::Sm))
                    } @else {
                        form method="post" action=(format!("{}/{code}", super::members::JOIN)) {
                            input type="hidden" name="csrf" value=(csrf);
                            (Button::new("Join").kind(ButtonType::Submit).size(ControlSize::Sm))
                        }
                        (ButtonLink::new("Decline", PAGE)
                            .size(ControlSize::Sm)
                            .variant(ButtonVariant::Ghost)
                            .color(Color::Secondary))
                    }
                }
            }))
            .respond(&headers),
    )
}

/// Join with an invitation link, then open the project.
async fn join(
    State(app): State<App>,
    headers: HeaderMap,
    Path(code): Path<String>,
    Form(form): Form<CsrfForm>,
) -> Response {
    let Some(code) = super::members::code_in(&code) else {
        return join_problem(
            &headers,
            &RepoError::InvitationGone.to_string(),
            StatusCode::NOT_FOUND,
        );
    };
    let (service, _) = match checked(&app, &headers, &form.csrf).await {
        Ok(found) => found,
        Err(response) => return response,
    };
    match service
        .project_call(
            &headers,
            Method::POST,
            "/v1/projects/invitations/accept",
            Some(json!({"code": code})),
        )
        .await
    {
        Ok(body) => {
            let id = body["project"]["id"]
                .as_str()
                .unwrap_or_default()
                .to_owned();
            // The chats already in the project move in with it (#11586).
            if let Ok(project) = serde_json::from_value::<Project>(body["project"].clone()) {
                let home = super::api::home_of(&project, "");
                if !home.is_empty() {
                    let home = crate::chat_store::account_owner(home);
                    if let Err(error) =
                        super::shared_chats::take_in(&app, &project.id, &home, &home).await
                    {
                        eprintln!("openagents-web: shared project chats: {error}");
                    }
                }
            }
            protect(Redirect::to(&href(&id)).into_response())
        }
        Err(error) => {
            let (problem, status) = problem_of(&error);
            join_problem(&headers, &problem, status)
        }
    }
}
