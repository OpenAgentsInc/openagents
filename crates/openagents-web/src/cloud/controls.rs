//! Explicit browser enrollment and reviewed native operations. The resident
//! owner retains tasks, execution policy, queue state, reviews, and publication.

use super::effects::{Snapshot, State as RequestState};
use super::hosts::{Binding, Hosts};
use super::session::{CloudSession, SessionError, Viewer, now};
use super::{colors, protect, refused, service, ui, work, workspace_shell};
use crate::App;
use axum::Router;
use axum::extract::rejection::FormRejection;
use axum::extract::{DefaultBodyLimit, Form, Path, State};
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use coder_access::Right;
use coder_access::protocol::{
    CommandAction, Operation, Outcome, QueueEdit, TaskCommand, TaskCreate, random_id,
};
use coder_access::task_read::{ListQuery, Page, PageQuery};
use coder_ui::control;
use maud::{Markup, PreEscaped, Render, html};
use openagents_ui::forms::{Checkbox, Field, Input, Select, Textarea};
use serde::Deserialize;
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

pub(super) fn routes() -> Router<App> {
    Router::new()
        .route("/cloud/app/computers", get(computers))
        .route("/cloud/app/hosts/{binding}", get(computer))
        .route("/cloud/app/hosts/{binding}/enroll", post(enroll))
        .route(
            "/cloud/app/hosts/{binding}/new-task",
            get(new_task).post(stage_create),
        )
        .route(
            "/cloud/app/hosts/{binding}/tasks/{task}/actions",
            get(actions).post(stage_command),
        )
        .route(
            "/cloud/app/hosts/{binding}/tasks/{task}/queue",
            get(queue).post(stage_queue),
        )
        .route(
            "/cloud/app/hosts/{binding}/tasks/{task}/review",
            get(review).post(stage_publication),
        )
        .route(
            "/cloud/app/hosts/{binding}/requests/{request}",
            get(request),
        )
        .route(
            "/cloud/app/hosts/{binding}/requests/{request}/confirm",
            post(confirm),
        )
        .layer(DefaultBodyLimit::max(64 * 1024))
}

pub(super) struct Context<'a> {
    pub(super) app: &'a App,
    pub(super) service: &'a CloudSession,
    pub(super) viewer: Viewer,
    pub(super) hosts: &'a Hosts,
    pub(super) binding: &'a Binding,
    pub(super) scope: Value,
}

pub(super) async fn admitted<'a>(
    app: &'a App,
    headers: &HeaderMap,
    id: &str,
) -> Result<Context<'a>, Response> {
    let service = service(app)?;
    let viewer = service.authenticate(headers).await.map_err(refused)?;
    let hosts = app
        .config
        .cloud_hosts
        .as_deref()
        .ok_or_else(|| refused(SessionError::Unavailable))?;
    let binding = hosts.get(&viewer, id).map_err(refused)?;
    let scope = hosts.control_scope(&viewer, binding);
    Ok(Context {
        app,
        service,
        viewer,
        hosts,
        binding,
        scope,
    })
}

impl Context<'_> {
    pub(super) async fn current(&self, headers: &HeaderMap) -> Result<(), Response> {
        let current = self.service.authenticate(headers).await.map_err(refused)?;
        let binding = self
            .hosts
            .get(&current, self.binding.id())
            .map_err(refused)?;
        if self.hosts.control_scope(&current, binding) != self.scope {
            return Err(refused(SessionError::Conflict));
        }
        Ok(())
    }

    pub(super) async fn probe(&self, headers: &HeaderMap) -> Result<(), Response> {
        let query = ListQuery {
            workspace: self.binding.workspace().into(),
            cursor: None,
            limit: 1,
        };
        let reply = self
            .binding
            .read(
                &self.viewer,
                Operation::ListTasks {
                    query: query.clone(),
                },
            )
            .await
            .map_err(refused)?;
        if !matches!(reply, Outcome::Tasks { tasks } if tasks.answers(&query)) {
            return Err(refused(SessionError::Conflict));
        }
        self.current(headers).await
    }

    pub(super) fn book(&self) -> Result<&super::effects::Effects, Response> {
        self.hosts
            .effects(&self.viewer, self.binding.id())
            .map_err(refused)
    }

    pub(super) fn enrolled(&self) -> Result<(), Response> {
        if !self
            .book()?
            .enrolled(&self.scope, self.binding.identity())
            .map_err(refused)?
        {
            return Err(refused(SessionError::Forbidden));
        }
        Ok(())
    }

    pub(super) fn csrf(
        &self,
        headers: &HeaderMap,
        action: &str,
        target: &str,
    ) -> Result<String, Response> {
        self.service
            .csrf(
                headers,
                &self.viewer,
                action,
                &digest(&json!({"scope":self.scope,"target":target})),
            )
            .map_err(refused)
    }

    pub(super) fn verify(
        &self,
        headers: &HeaderMap,
        token: &str,
        action: &str,
        target: &str,
    ) -> Result<(), Response> {
        self.service
            .verify_csrf(
                headers,
                Some(&self.viewer),
                action,
                &digest(&json!({"scope":self.scope,"target":target})),
                token,
            )
            .map_err(refused)
    }

    pub(super) fn page(&self, headers: &HeaderMap, content: Markup, resource: Value) -> Response {
        workspace_shell(
            self.app,
            headers,
            self.service,
            &self.viewer,
            "computers",
            Some(content),
            Some(resource),
        )
    }

    pub(super) fn resource(
        &self,
        snapshot: Option<&Snapshot>,
        page: Option<&Page>,
        queue: Option<&str>,
        review: Option<&coder_access::review::TaskReview>,
    ) -> Result<Value, SessionError> {
        if self.hosts.effects(&self.viewer, self.binding.id()).is_ok() {
            work::control_resource(
                self.binding,
                &self.viewer,
                self.hosts,
                snapshot,
                page,
                queue,
                review,
            )
        } else if let Some(page) = page {
            work::task_resource(self.binding, page, &self.viewer)
        } else {
            work::list_resource(self.binding, &self.viewer)
        }
    }

    pub(super) async fn task(&self, headers: &HeaderMap, id: &str) -> Result<Page, Response> {
        let query = PageQuery {
            workspace: self.binding.workspace().into(),
            task: id.into(),
            revision: None,
            cursor: None,
            limit: 1,
        };
        query
            .validate()
            .map_err(|_| refused(SessionError::InvalidRequest))?;
        let reply = self
            .binding
            .read(
                &self.viewer,
                Operation::ReadTask {
                    query: query.clone(),
                },
            )
            .await
            .map_err(refused)?;
        self.current(headers).await?;
        match reply {
            Outcome::Task { task } if task.answers(&query) => Ok(*task),
            _ => Err(refused(SessionError::Conflict)),
        }
    }

    fn target<'a>(&'a self, page: Option<&'a Page>) -> control::Target<'a> {
        control::Target {
            host: self.binding.host(),
            generation: self.binding.generation(),
            workspace: self.binding.workspace(),
            task: page.map(|page| page.scope.task.as_str()),
            revision: page.map(|page| page.scope.revision),
            attempt: page.and_then(|page| page.scope.attempt),
        }
    }
}

pub(super) fn digest(value: &Value) -> String {
    format!(
        "sha256:{}",
        Sha256::digest(value.to_string().as_bytes())
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect::<String>()
    )
}

pub(super) fn show<I: serde::Serialize + Clone>(
    view: &rust_native::View<I>,
) -> Result<String, Response> {
    rust_native_web::render_view(view).map_err(|_| refused(SessionError::Conflict))
}

pub(super) fn submit(key: &str, label: &str, enabled: bool) -> Result<String, Response> {
    // This validated view contains one application-owned submit intent. Native
    // form submission provides the transport; no browser effect handler exists.
    Ok(show(&control::submit(
        key,
        label,
        control::Gate {
            enabled,
            reason: None,
        },
        colors(),
    ))?
    .replace("type=\"button\"", "type=\"submit\""))
}

pub(super) fn hidden(name: &str, value: &str) -> String {
    ui::hidden(name, value).into_string()
}

pub(super) fn link(binding: &Binding) -> String {
    links(binding).into_string()
}

/// The computer connection and resident task links of one binding.
fn links(binding: &Binding) -> Markup {
    let computer = format!("/cloud/app/hosts/{}", binding.id());
    let tasks = format!("{computer}/tasks");
    ui::links([
        (computer.as_str(), "Computer connection"),
        (tasks.as_str(), "Resident tasks"),
    ])
}

/// A labelled form control: `control` receives the field's ARIA wiring.
fn field<C: Render>(
    id: &str,
    label: &str,
    required: bool,
    control: impl FnOnce(openagents_ui::forms::FieldAria) -> C,
) -> Markup {
    let field = Field::new(id, label).required(required);
    let aria = field.aria();
    field.control(control(aria)).render()
}

async fn computers(State(app): State<App>, headers: HeaderMap) -> Response {
    let service = match service(&app) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let viewer = match service.authenticate(&headers).await {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    let bindings = app
        .config
        .cloud_hosts
        .as_ref()
        .map_or_else(Vec::new, |hosts| hosts.current(&viewer));
    let content = html! {
        h2 { "Computers" }
        p { "These are explicitly configured account connections. Open one to verify its current native grant and resident generation. Account membership enrolls no computer." }
        ul {
            @for binding in &bindings {
                li {
                    a href=(format!("/cloud/app/hosts/{}", binding.id())) {
                        "Open connection " (binding.id())
                    }
                }
            }
        }
        p { "Device keys remain with the separately configured server adapter. Host invitations, owner keys, and automatic discovery are unavailable in this browser." }
    };
    workspace_shell(
        &app,
        &headers,
        service,
        &viewer,
        "computers",
        Some(content),
        None,
    )
}

async fn computer(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = context.probe(&headers).await {
        return r;
    }
    let native = &context.binding.access().grant;
    let rights = Right::ALL
        .into_iter()
        .filter(|right| native.rights.contains(*right))
        .map(Right::as_str)
        .collect::<Vec<_>>();
    let grant = control::Grant {
        key: "resident-grant",
        host: context.binding.host(),
        generation: context.binding.generation(),
        workspace: context.binding.workspace(),
        device: &native.device,
        id: &native.grant,
        epoch: native.epoch,
        expires_at: native.expires_at,
        rights: &rights,
        scope: "Exact configured account, selected workspace, membership epoch, and resident alias.",
        custody: "The separately admitted server adapter holds this device key. The browser receives no key.",
        consent: "Enrollment acknowledges this existing scoped device binding; it grants no additional native rights.",
    };
    let available = context.book().is_ok();
    let enrolled = available && context.enrolled().is_ok();
    let host = control::Host {
        key: "resident-computer",
        host: context.binding.host(),
        generation: context.binding.generation(),
        workspace: context.binding.workspace(),
        route: "Verified resident direct channel",
        version: None,
        capabilities: &[],
        capacity: control::Capacity::Unknown,
        providers: &[],
        observed_at: Some(now()),
    };
    let computer = match show(&control::computer(&host, colors())) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let selection = context
        .viewer
        .workspace
        .as_ref()
        .expect("admission requires a selected workspace");
    let display = control::EnrollmentReview {
        key: "browser-enrollment",
        grant,
        account: &context.viewer.account_id,
        account_workspace: &selection.id,
        disclosure: "Canonical resident tasks, original evidence, and explicitly reviewed operations under this grant. World, sales, and spending remain separately granted.",
        confirm: control::Gate {
            enabled: false,
            reason: Some("Use the exact server form below to confirm this connection."),
        },
    };
    let review = match show(&control::enrollment_review(&display, colors())) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let standing = if enrolled {
        html! {
            p { "This native connection is enrolled for the current browser session and exact account projection." }
            @if native.rights.contains(Right::Operate) {
                p { a href=(format!("/cloud/app/hosts/{id}/new-task")) { "Create a resident task" } }
            } @else {
                p { "This connection is read-only. Its native grant admits no task effects." }
            }
        }
    } else if available {
        let csrf = match context.csrf(&headers, "enroll", context.binding.identity()) {
            Ok(v) => v,
            Err(r) => return r,
        };
        let button = match submit("enrollment-submit", "Enroll this browser connection", true) {
            Ok(v) => v,
            Err(r) => return r,
        };
        ui::BoundForm::new(format!("/cloud/app/hosts/{id}/enroll"))
            .csrf(&csrf)
            .body(
                Checkbox::new(
                    "custody",
                    "I reviewed this exact account, workspace, host, device grant, expiry, and server custody.",
                )
                .id("enroll-custody")
                .value("yes")
                .required(true),
            )
            .submit_with(PreEscaped(button))
            .render()
    } else {
        ui::unavailable(
            "Browser control custody is unavailable",
            "The operator has not separately admitted this binding and its protected request journal. Canonical observation remains read-only.",
        )
    };
    let content = html! {
        (ui::native(&computer))
        (links(context.binding))
        (ui::native(&review))
        (standing)
    };
    let resource = match context.resource(None, None, None, None) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    context.page(&headers, content, resource)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct EnrollForm {
    csrf: String,
    custody: Option<String>,
}

async fn enroll(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<EnrollForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if form.custody.as_deref() != Some("yes") {
        return refused(SessionError::InvalidRequest);
    }
    if let Err(r) = context.verify(&headers, &form.csrf, "enroll", context.binding.identity()) {
        return r;
    }
    if let Err(r) = context.probe(&headers).await {
        return r;
    }
    let book = match context.book() {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(e) = book.accept_enrollment(&context.scope, context.binding.identity()) {
        return refused(e);
    }
    protect(Redirect::to(&format!("/cloud/app/hosts/{id}")).into_response())
}

fn form_basis(page: &Page, id: &str, issued: u64) -> String {
    digest(&json!({"request":id,"issued_at":issued,"task":page.scope}))
}

pub(super) fn request_url(binding: &Binding, request: &str) -> String {
    format!("/cloud/app/hosts/{}/requests/{request}", binding.id())
}

pub(super) async fn staged(
    context: &Context<'_>,
    headers: &HeaderMap,
    id: &str,
    operation: Operation,
) -> Response {
    if let Err(r) = context.enrolled() {
        return r;
    }
    if let Err(r) = context.probe(headers).await {
        return r;
    }
    if operation.validate().is_err() {
        return refused(SessionError::InvalidRequest);
    }
    let book = match context.book() {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(e) = book.stage(&context.scope, id, &operation, || {
        context
            .binding
            .prepare(&context.viewer, operation.clone(), id.into())
    }) {
        return refused(e);
    }
    protect(Redirect::to(&request_url(context.binding, id)).into_response())
}

async fn new_task(State(app): State<App>, headers: HeaderMap, Path(id): Path<String>) -> Response {
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = context.enrolled() {
        return r;
    }
    if !context
        .binding
        .access()
        .grant
        .rights
        .contains(Right::Operate)
    {
        return refused(SessionError::Forbidden);
    }
    if let Err(r) = context.probe(&headers).await {
        return r;
    }
    let request = random_id();
    let csrf = match context.csrf(&headers, "create", &request) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let composer = control::Composer {
        key: "task",
        target: context.target(None),
        engine: None,
        engine_readiness: "Unknown. The resident owner admits the explicitly requested engine under its current policy.",
        max_bytes: 16 * 1024,
        enabled: true,
        reason: None,
    };
    let composer = match show(&control::composer(&composer, colors())) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let button = match submit("create-review", "Review task submission", true) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let form = ui::BoundForm::here()
        .csrf(&csrf)
        .bind("request", &request)
        .body(html! {
            (field("task-title", "Title", true, |aria| {
                Input::new("title").maxlength(200).aria(aria)
            }))
            (ui::native(&composer))
            (field("task-engine", "Requested engine", false, |aria| {
                Select::new("engine")
                    .option("", "Resident policy")
                    .option("codex", "Codex")
                    .option("claude_code", "Claude Code")
                    .option("devin", "Devin")
                    .option("opencode", "OpenCode")
                    .option("grok_build", "Grok Build")
                    .aria(aria)
            }))
        })
        .submit_with(PreEscaped(button));
    let content = html! {
        (links(context.binding))
        h2 { "New resident task" }
        p { "Task submission records intent. The resident owner's current auto-start policy determines whether it starts. Capacity and execution are unknown until reported by the owner." }
        (form)
    };
    let resource = match context.resource(None, None, None, None) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    context.page(&headers, content, resource)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CreateForm {
    csrf: String,
    request: String,
    title: String,
    #[serde(rename = "task:prompt")]
    prompt: String,
    engine: String,
}

async fn stage_create(
    State(app): State<App>,
    headers: HeaderMap,
    Path(id): Path<String>,
    form: Result<Form<CreateForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = context.verify(&headers, &form.csrf, "create", &form.request) {
        return r;
    }
    let engine = if form.engine.is_empty() {
        None
    } else {
        match serde_json::from_value(json!(form.engine)) {
            Ok(v) => Some(v),
            Err(_) => return refused(SessionError::InvalidRequest),
        }
    };
    let operation = Operation::CreateTask {
        task: TaskCreate {
            title: form.title,
            prompt: form.prompt,
            workspace: context.binding.workspace().into(),
            images: Vec::new(),
            engine,
        },
    };
    staged(&context, &headers, &form.request, operation).await
}

async fn actions(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
) -> Response {
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = context.enrolled() {
        return r;
    }
    let page = match context.task(&headers, &task).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let enabled = context
        .binding
        .access()
        .grant
        .rights
        .contains(Right::Operate);
    let request = random_id();
    let issued = now();
    let csrf = match context.csrf(&headers, "command", &form_basis(&page, &request, issued)) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let composer = control::Composer {
        key: "task",
        target: context.target(Some(&page)),
        engine: None,
        engine_readiness: "The existing task retains its resident engine configuration.",
        max_bytes: 16 * 1024,
        enabled,
        reason: (!enabled).then_some("The native grant is read-only."),
    };
    let composer = match show(&control::composer(&composer, colors())) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let button = match submit("command-review", "Review exact task command", enabled) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let form = ui::BoundForm::here()
        .csrf(&csrf)
        .bind("request", &request)
        .bind("issued_at", &issued.to_string())
        .bind("revision", &page.scope.revision.to_string())
        .body(html! {
            (field("command-action", "Operation", false, |aria| {
                Select::new("action")
                    .option("queue", "Queue after this turn")
                    .option("send", "Send as a new turn")
                    .option("steer", "Request native steering")
                    .option("interrupt", "Request stop")
                    .option("answer", "Answer the pending question")
                    .aria(aria)
            }))
            (ui::native(&composer))
            (Checkbox::new("emulate", "Use emulated steering (steer only)")
                .id("command-emulate")
                .value("yes"))
        })
        .submit_with(PreEscaped(button));
    let task_url = format!("/cloud/app/hosts/{id}/tasks/{task}");
    let queue_url = format!("{task_url}/queue");
    let review_url = format!("{task_url}/review");
    let content = html! {
        (links(context.binding))
        h2 { "Exact task command" }
        p { "Displayed revision " (page.scope.revision) ". The owner atomically refuses a command based on another revision. Queue, send, steer, stop, and answering a pending question retain distinct native semantics. A stop request does not prove executor termination or cleanup." }
        (form)
        (ui::links([
            (queue_url.as_str(), "Inspect and edit the native queue"),
            (review_url.as_str(), "Read exact candidate review"),
        ]))
    };
    let resource = match context.resource(None, Some(&page), None, None) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    context.page(&headers, content, resource)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct CommandForm {
    csrf: String,
    request: String,
    issued_at: u64,
    revision: u64,
    action: String,
    #[serde(rename = "task:prompt")]
    text: String,
    emulate: Option<String>,
}

async fn stage_command(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
    form: Result<Form<CommandForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let page = match context.task(&headers, &task).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if page.scope.revision != form.revision {
        return refused(SessionError::Conflict);
    }
    if let Err(r) = context.verify(
        &headers,
        &form.csrf,
        "command",
        &form_basis(&page, &form.request, form.issued_at),
    ) {
        return r;
    }
    let action = match serde_json::from_value::<CommandAction>(json!(form.action)) {
        Ok(v) => v,
        Err(_) => return refused(SessionError::InvalidRequest),
    };
    if form.emulate.as_deref().is_some_and(|v| v != "yes") {
        return refused(SessionError::InvalidRequest);
    }
    let operation = Operation::CommandTaskAtRevision {
        revision: form.revision,
        command: TaskCommand {
            command: form.request.clone(),
            task,
            action,
            based_on: form.revision,
            text: form.text,
            emulate: form.emulate.is_some(),
            issued_at: form.issued_at,
        },
    };
    staged(&context, &headers, &form.request, operation).await
}

async fn native_queue(
    context: &Context<'_>,
    headers: &HeaderMap,
    page: &Page,
) -> Result<(coder_access::protocol::TaskQueue, String), Response> {
    let operation = Operation::QueueTaskAtRevision {
        task: page.scope.task.clone(),
        revision: page.scope.revision,
        edit: QueueEdit::List {},
        queue_digest: None,
    };
    let reply = context
        .binding
        .read_queue(&context.viewer, operation.clone())
        .await
        .map_err(refused)?;
    context.current(headers).await?;
    if !reply.answers(&operation) {
        return Err(refused(SessionError::Conflict));
    }
    match reply {
        Outcome::QueueAtRevision {
            queue,
            revision,
            queue_digest,
        } if revision == page.scope.revision => Ok((queue, queue_digest)),
        _ => Err(refused(SessionError::Conflict)),
    }
}

async fn queue(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
) -> Response {
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = context.enrolled() {
        return r;
    }
    let page = match context.task(&headers, &task).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let (queue, queue_digest) = match native_queue(&context, &headers, &page).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let request = random_id();
    let basis = digest(&json!({"request":request,"scope":page.scope,"queue":queue_digest}));
    let csrf = match context.csrf(&headers, "queue", &basis) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let record = match serde_json::to_string_pretty(&queue) {
        Ok(v) => v,
        Err(_) => return refused(SessionError::Conflict),
    };
    let button = match submit("queue-review", "Review exact queue edit", true) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let form = ui::BoundForm::here()
        .csrf(&csrf)
        .bind("request", &request)
        .bind("revision", &page.scope.revision.to_string())
        .bind("queue_digest", &queue_digest)
        .body(html! {
            (field("queue-action", "Queue operation", false, |aria| {
                Select::new("action")
                    .option("lease", "Take edit lease")
                    .option("release", "Release edit lease")
                    .option("edit", "Edit my held command")
                    .option("remove", "Remove my held command")
                    .option("reorder", "Set the exact order")
                    .option("send_now", "Send my held command now")
                    .aria(aria)
            }))
            (field("queue-command", "Command ID", false, |aria| {
                Input::new("command").maxlength(64).aria(aria)
            }))
            (field("queue-text", "Replacement text", false, |aria| {
                Textarea::new("text").maxlength(16384).aria(aria)
            }))
            (field("queue-order", "Exact ordered command IDs (one per line)", false, |aria| {
                Textarea::new("order").maxlength(8192).aria(aria)
            }))
        })
        .submit_with(PreEscaped(button));
    let content = html! {
        (links(context.binding))
        h2 { "Native queue" }
        p {
            "Task revision " (page.scope.revision) " \u{b7} queue snapshot "
            code { (queue_digest) }
            ". Native edit leases, original command ownership, and this exact queue digest fence edits. Lease or release does not renew itself on lost-reply recovery."
        }
        pre { (record) }
        (form)
    };
    let resource = match context.resource(None, Some(&page), Some(&queue_digest), None) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    context.page(&headers, content, resource)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct QueueForm {
    csrf: String,
    request: String,
    revision: u64,
    queue_digest: String,
    action: String,
    command: String,
    text: String,
    order: String,
}

async fn stage_queue(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
    form: Result<Form<QueueForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let page = match context.task(&headers, &task).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if page.scope.revision != form.revision {
        return refused(SessionError::Conflict);
    }
    let basis =
        digest(&json!({"request":form.request,"scope":page.scope,"queue":form.queue_digest}));
    if let Err(r) = context.verify(&headers, &form.csrf, "queue", &basis) {
        return r;
    }
    let edit = match form.action.as_str() {
        "lease" if form.command.is_empty() && form.text.is_empty() && form.order.is_empty() => {
            QueueEdit::Lease {}
        }
        "release" if form.command.is_empty() && form.text.is_empty() && form.order.is_empty() => {
            QueueEdit::Release {}
        }
        "edit" if form.order.is_empty() => QueueEdit::Edit {
            command: form.command,
            text: form.text,
        },
        "remove" if form.text.is_empty() && form.order.is_empty() => QueueEdit::Remove {
            command: form.command,
        },
        "reorder" if form.command.is_empty() && form.text.is_empty() => QueueEdit::Reorder {
            commands: form
                .order
                .lines()
                .map(str::trim)
                .filter(|v| !v.is_empty())
                .map(str::to_owned)
                .collect(),
        },
        "send_now" if form.text.is_empty() && form.order.is_empty() => QueueEdit::SendNow {
            command: form.command,
        },
        _ => return refused(SessionError::InvalidRequest),
    };
    let operation = Operation::QueueTaskAtRevision {
        task,
        revision: form.revision,
        edit,
        queue_digest: Some(form.queue_digest),
    };
    staged(&context, &headers, &form.request, operation).await
}

async fn review(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
) -> Response {
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let page = match context.task(&headers, &task).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let operation = Operation::ReviewTask { task: task.clone() };
    let result = match context
        .binding
        .read(&context.viewer, operation.clone())
        .await
    {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    if let Err(r) = context.current(&headers).await {
        return r;
    }
    if !result.answers(&operation) {
        return refused(SessionError::Conflict);
    }
    let Outcome::Review { review } = result else {
        return refused(SessionError::Conflict);
    };
    let request = random_id();
    let basis = digest(
        &json!({"request":request,"task":page.scope,"base":review.base,"head_commit":review.head_commit,"head":review.head}),
    );
    let csrf = match context.csrf(&headers, "publish", &basis) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let allowed = context.enrolled().is_ok()
        && context
            .binding
            .access()
            .grant
            .rights
            .contains(Right::Operate);
    let button = match submit("publish-review", "Review exact publication", allowed) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let original = match serde_json::to_string_pretty(&review) {
        Ok(v) => v,
        Err(_) => return refused(SessionError::Conflict),
    };
    let form = ui::BoundForm::here()
        .csrf(&csrf)
        .bind("request", &request)
        .bind("revision", &page.scope.revision.to_string())
        .bind("base", &review.base)
        .bind("head_commit", &review.head_commit)
        .bind("head", &review.head)
        .submit_with(PreEscaped(button));
    let content = html! {
        (links(context.binding))
        h2 { "Original candidate review" }
        p { "Publication commits the exact reviewed tree and pushes only under the resident repository policy. This is distinct from local integration, checks, deployment, executor completion, and cleanup. An unsupported resident publication lane refuses; a reviewed page does not activate one." }
        pre { (original) }
        (form)
    };
    let resource = match context.resource(None, Some(&page), None, Some(&review)) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    context.page(&headers, content, resource)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct PublicationForm {
    csrf: String,
    request: String,
    revision: u64,
    base: String,
    head_commit: String,
    head: String,
}

async fn stage_publication(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, task)): Path<(String, String)>,
    form: Result<Form<PublicationForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    let page = match context.task(&headers, &task).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if page.scope.revision != form.revision {
        return refused(SessionError::Conflict);
    }
    let basis = digest(
        &json!({"request":form.request,"task":page.scope,"base":form.base,"head_commit":form.head_commit,"head":form.head}),
    );
    if let Err(r) = context.verify(&headers, &form.csrf, "publish", &basis) {
        return r;
    }
    let operation = Operation::PublishTask {
        task,
        base: form.base,
        head_commit: form.head_commit,
        head: form.head,
    };
    staged(&context, &headers, &form.request, operation).await
}

pub(super) async fn request(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, request)): Path<(String, String)>,
) -> Response {
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = context.enrolled() {
        return r;
    }
    if let Err(r) = context.probe(&headers).await {
        return r;
    }
    let book = match context.book() {
        Ok(v) => v,
        Err(r) => return r,
    };
    let snapshot = match book.lookup(&context.scope, &request) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    let snapshot = match recover_request(&context, &headers, snapshot).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    receipt_page(&context, &headers, &snapshot)
}

pub(super) async fn recover_request(
    context: &Context<'_>,
    headers: &HeaderMap,
    snapshot: Snapshot,
) -> Result<Snapshot, Response> {
    super::operator::admit_action(context.binding, &context.viewer, &snapshot.action)
        .await
        .map_err(refused)?;
    let book = context.book()?;
    let recovery = Operation::RequestOperation {
        request: snapshot.id.clone(),
        request_event: snapshot.packet_digest.clone(),
    };
    let reply = match context
        .binding
        .read(&context.viewer, recovery.clone())
        .await
    {
        Ok(v) => v,
        Err(e) => return Err(refused(e)),
    };
    if !reply.answers(&recovery) {
        return Err(refused(SessionError::Conflict));
    }
    context.current(headers).await?;
    match reply {
        Outcome::RequestOperation {
            result: Some(result),
            ..
        } => match book.reconcile_reply(&context.scope, &snapshot.id, *result) {
            Ok(v) => Ok(v),
            Err(e) => Err(refused(e)),
        },
        Outcome::RequestOperation { result: None, .. }
            if matches!(
                snapshot.state,
                RequestState::Answered | RequestState::Refused
            ) =>
        {
            Err(refused(SessionError::Unavailable))
        }
        Outcome::RequestOperation { result: None, .. } => Ok(snapshot),
        _ => Err(refused(SessionError::Conflict)),
    }
}

fn confirmation(snapshot: &Snapshot) -> String {
    digest(
        &json!({"request":snapshot.id,"packet":snapshot.packet_digest,"action":snapshot.action,"expires_at":snapshot.expires_at}),
    )
}

fn action_task(action: &Operation) -> Option<&str> {
    match action {
        Operation::CommandTaskAtRevision { command, .. } => Some(&command.task),
        Operation::QueueTaskAtRevision { task, .. }
        | Operation::PublishTask { task, .. }
        | Operation::SteerTask { task, .. }
        | Operation::CancelTask { task, .. } => Some(task),
        _ => None,
    }
}

fn receipt_page(context: &Context<'_>, headers: &HeaderMap, snapshot: &Snapshot) -> Response {
    let outcome = snapshot
        .outcome
        .as_ref()
        .and_then(|outcome| serde_json::to_value(outcome).ok());
    let mut target = context.target(None);
    target.task = action_task(&snapshot.action);
    target.revision = match &snapshot.action {
        Operation::CommandTaskAtRevision { revision, .. }
        | Operation::QueueTaskAtRevision { revision, .. } => Some(*revision),
        _ => None,
    };
    let value = control::Receipt {
        key: "resident-receipt",
        request: &snapshot.id,
        request_digest: &snapshot.packet_digest,
        principal: &context.binding.access().grant.device,
        target,
        state: match snapshot.state {
            RequestState::Prepared => control::ReceiptState::Prepared,
            RequestState::Unknown => control::ReceiptState::Unknown,
            RequestState::Answered => control::ReceiptState::Answered,
            RequestState::Refused => control::ReceiptState::Refused,
        },
        operation: snapshot.action.name(),
        outcome: outcome.as_ref(),
    };
    let receipt = match show(&control::receipt(&value, colors())) {
        Ok(v) => v,
        Err(r) => return r,
    };
    let action = match serde_json::to_string_pretty(&snapshot.action) {
        Ok(v) => v,
        Err(_) => return refused(SessionError::Conflict),
    };
    let pending = matches!(
        snapshot.state,
        RequestState::Prepared | RequestState::Unknown
    );
    let unknown = matches!(snapshot.state, RequestState::Unknown);
    // Only the original request is ever confirmed again; an expired or
    // refused original admits no redispatch.
    let next = if pending && snapshot.expires_at > now() && snapshot.refusal.is_none() {
        let csrf = match context.csrf(headers, "confirm", &confirmation(snapshot)) {
            Ok(v) => v,
            Err(r) => return r,
        };
        let button = match submit("confirm-native", "Confirm this exact native request", true) {
            Ok(v) => v,
            Err(r) => return r,
        };
        let form = ui::BoundForm::new(format!(
            "{}/confirm",
            request_url(context.binding, &snapshot.id)
        ))
        .csrf(&csrf)
        .submit_with(PreEscaped(button));
        if unknown {
            ui::outcome_unknown(
                "The native reply to this original request was lost. Confirming sends this same original request again; this page never generates a replacement request.",
                Some(form),
            )
        } else {
            form.render()
        }
    } else if pending {
        let reason = if snapshot.expires_at <= now() {
            "The original packet expired."
        } else {
            "The native owner retained an uncertain result."
        };
        let text = format!(
            "{reason} No redispatch is admitted. Reconcile this original request through the resident task owner before reviewing any separate action."
        );
        if unknown {
            ui::outcome_unknown(text.as_str(), None)
        } else {
            html! { p { (text) } }
        }
    } else {
        html! {}
    };
    let binding = context.binding.id();
    let mut follow: Vec<(String, &str)> = Vec::new();
    if let Some(Outcome::CloudAccepted { accepted }) = &snapshot.outcome {
        follow.push((
            super::operator::job_url(binding, &accepted.scope),
            "Inspect the canonical Cloud job and cleanup evidence",
        ));
    } else if let Operation::CloudSubmit { intent } = &snapshot.action {
        follow.push((
            format!(
                "/cloud/app/hosts/{binding}/cloud/{}/jobs/{}",
                intent.project, snapshot.id
            ),
            "Inspect the original Cloud creation identity",
        ));
    } else {
        let scope = match &snapshot.action {
            Operation::CloudContinue { intent } => Some(&intent.scope),
            Operation::CloudCancel { intent } => Some(&intent.scope),
            Operation::CloudFollow { intent } => Some(&intent.scope),
            _ => None,
        };
        if let Some(scope) = scope {
            follow.push((
                super::operator::job_url(binding, scope),
                "Inspect the original Cloud job",
            ));
        }
        if let Some(url) = super::environment::action_url(binding, &snapshot.action) {
            follow.push((url, "Return to the project environment"));
        }
    }
    if let Some(task) = action_task(&snapshot.action) {
        follow.push((
            format!("/cloud/app/hosts/{binding}/tasks/{task}"),
            "Inspect the canonical task",
        ));
    } else if matches!(snapshot.action, Operation::CreateTask { .. }) {
        follow.push((
            format!("/cloud/app/hosts/{binding}/tasks/{}", snapshot.id),
            "Inspect the original creation identity",
        ));
    }
    let content = html! {
        (ui::native(&receipt))
        h3 { "Exact reviewed operation" }
        pre { (action) }
        p { "Original request expires at Unix second " (snapshot.expires_at) ". Closing this page dispatches nothing. A lost or expired reply remains unknown; this page never generates a replacement request or replays an operation automatically." }
        @if let Some(error) = snapshot.failure {
            p { "Last native exchange: " (error.to_string()) ". Inspect canonical resident state; this is not evidence of completed work or cleanup." }
        }
        @if let Some(refusal) = &snapshot.refusal {
            p {
                "Original native refusal: " code { (format!("{:?}", refusal.code)) }
                ". No successful operation is reported. An unavailable result can follow an effect; inspect canonical task, stop, publication, and cleanup evidence."
            }
        }
        (next)
        (links(context.binding))
        @for (href, label) in &follow {
            p { a href=(href) { (label) } }
        }
    };
    let resource = match context.resource(Some(snapshot), None, None, None) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    context.page(headers, content, resource)
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ConfirmForm {
    csrf: String,
}

async fn confirm(
    State(app): State<App>,
    headers: HeaderMap,
    Path((id, request)): Path<(String, String)>,
    form: Result<Form<ConfirmForm>, FormRejection>,
) -> Response {
    let Ok(Form(form)) = form else {
        return refused(SessionError::InvalidRequest);
    };
    let context = match admitted(&app, &headers, &id).await {
        Ok(v) => v,
        Err(r) => return r,
    };
    if let Err(r) = context.enrolled() {
        return r;
    }
    let book = match context.book() {
        Ok(v) => v,
        Err(r) => return r,
    };
    let snapshot = match book.lookup(&context.scope, &request) {
        Ok(v) => v,
        Err(e) => return refused(e),
    };
    if let Err(r) = context.verify(&headers, &form.csrf, "confirm", &confirmation(&snapshot)) {
        return r;
    }
    if let Err(r) = context.probe(&headers).await {
        return r;
    }
    let context_ref = &context;
    if let Err(error) =
        super::operator::admit_action(context.binding, &context.viewer, &snapshot.action).await
    {
        return refused(error);
    }
    let request_headers = &headers;
    let binding_id = &id;
    let result = book
        .dispatch(&context.scope, &request, |pending| async move {
            // Check account standing immediately before the native effect. The
            // resident independently checks current grants and exact domain state.
            let current = context_ref.service.authenticate(request_headers).await?;
            let binding = context_ref.hosts.get(&current, binding_id)?;
            if context_ref.hosts.control_scope(&current, binding) != context_ref.scope {
                return Err(SessionError::Conflict);
            }
            super::operator::admit_action(binding, &current, &pending.request.op).await?;
            // The user's own Claude credential, for the one turn this effect
            // starts (BYO-05). It travels apart from the staged packet.
            super::byo::release_turn(
                context_ref.app,
                binding,
                &current,
                &pending.request.request,
                &pending.request.op,
            )
            .await?;
            let answer = binding.send(&current, &pending).await?;
            Ok(answer)
        })
        .await;
    if let Err(r) = context.current(&headers).await {
        return r;
    }
    match result {
        Ok(snapshot) => match recover_request(&context, &headers, snapshot).await {
            Ok(snapshot) => receipt_page(&context, &headers, &snapshot),
            Err(response) => response,
        },
        Err(error) => refused(error),
    }
}
