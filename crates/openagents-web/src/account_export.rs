//! `/settings/export` (#11134, promise A8): everything on the signed-in
//! account in one file, to keep or to take elsewhere.
//!
//! Settings has a Download button for it. The file is plain JSON that opens
//! without OpenAgents: every chat (archived ones too) with its messages and
//! a Markdown copy to read, the account's projects, its uploaded traces
//! (each ATIF document and its agents), its signed-in computers, and its
//! settings (plan choices, the names of API keys, which kind of Claude
//! credential is saved).
//!
//! Never in it: a key, token, password, or credential of any kind, nor the
//! ids that would act for the account (session ids, key ids, request
//! tickets). An API key is listed by name and date only; a saved Claude
//! credential by its kind only.
//!
//! Isolation: the file holds only what the signed-in account owns. Chats
//! and traces are read under the account's own owner value
//! ([`crate::chat_store::account_owner`]), and each record is checked
//! against it again here. Workspace settings come from the same workspace
//! the Settings pages use (API keys from the account's own workspace, the
//! Claude credential from the selected one), and the file says which.
//! Another member's chats, traces, or keys never reach it.
//!
//! Trace documents can be large, so the file carries at most
//! [`MAX_TRACE_BYTES`] of them; a trace past that is listed with a link to
//! download it on its own.

use std::collections::BTreeMap;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::{Value, json};

use crate::App;
use crate::chat_store::{Conversation, Error, Message, Role, Store, account_owner, now_unix};
use crate::cloud::protect;
use crate::cloud::session::{CloudSession, Viewer, now};

/// The download.
pub(crate) const PATH: &str = "/settings/export";
/// What the file says it is, so a reader can tell versions apart.
const SCHEMA: &str = "openagents.account-export.v1";
/// The most trace bytes (documents and agents together) one file carries.
pub(crate) const MAX_TRACE_BYTES: usize = 64 * 1024 * 1024;

pub(crate) fn routes() -> Router<App> {
    Router::new().route(PATH, get(download))
}

async fn download(State(app): State<App>, headers: HeaderMap) -> Response {
    let (service, viewer) = match crate::settings::viewer(&app, &headers, PATH).await {
        Ok(value) => value,
        Err(response) => return response,
    };
    let owner = account_owner(&viewer.account_id);
    let origin = service.origin().trim_end_matches('/').to_owned();
    let store: &Store = &app.config.chat_store;
    let stored = match Stored::read(store, &owner, &origin, MAX_TRACE_BYTES).await {
        Ok(value) => value,
        Err(error) => {
            eprintln!("openagents-web: account export: {error}");
            return protect(crate::layout::problem(
                StatusCode::SERVICE_UNAVAILABLE,
                "Export",
                "Your export couldn't be made right now. Try again in a minute.",
                (crate::settings::PAGE, "Settings"),
            ));
        }
    };
    let account = account_section(&app, service, &headers, &viewer, &owner).await;
    let generated = now_unix();
    let document = assemble(&stored, &account, &origin, generated);
    let bytes = match serde_json::to_vec_pretty(&document) {
        Ok(bytes) => bytes,
        Err(_) => {
            return protect(crate::layout::problem(
                StatusCode::INTERNAL_SERVER_ERROR,
                "Export",
                "Your export couldn't be made right now. Try again in a minute.",
                (crate::settings::PAGE, "Settings"),
            ));
        }
    };
    let mut response = (StatusCode::OK, bytes).into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::CONTENT_TYPE,
        HeaderValue::from_static("application/json; charset=utf-8"),
    );
    if let Ok(value) = HeaderValue::from_str(&format!(
        "attachment; filename=\"{}\"",
        file_name(generated)
    )) {
        headers.insert(header::CONTENT_DISPOSITION, value);
    }
    protect(response)
}

/// `openagents-export-2026-10-09.json`.
pub(crate) fn file_name(unix: u64) -> String {
    format!("openagents-export-{}.json", day(unix))
}

fn day(unix: u64) -> String {
    atif::iso(unix.saturating_mul(1000))[..10].to_owned()
}

fn when(unix: u64) -> String {
    atif::iso(unix.saturating_mul(1000))
}

/// What the chat store holds for one account: its chats and its traces.
pub(crate) struct Stored {
    pub chats: Vec<Conversation>,
    pub traces: Vec<Value>,
}

impl Stored {
    /// Every chat and trace of `owner`, and nothing of anyone else's. Trace
    /// documents fill at most `budget` bytes; the rest are linked.
    pub(crate) async fn read(
        store: &Store,
        owner: &str,
        origin: &str,
        budget: usize,
    ) -> Result<Self, Error> {
        let mut chats = store.list(owner).await?;
        // The store reads one owner's folder; check every record anyway.
        chats.retain(|chat| chat.owner == owner);
        chats.sort_by(|a, b| {
            b.updated_unix
                .cmp(&a.updated_unix)
                .then_with(|| a.id.cmp(&b.id))
        });
        let traces = traces(store, owner, origin, budget).await?;
        Ok(Self { chats, traces })
    }
}

/// The account's traces, newest first, each with its document and agents
/// while they fit in `budget`.
async fn traces(
    store: &Store,
    owner: &str,
    origin: &str,
    budget: usize,
) -> Result<Vec<Value>, Error> {
    let mut left = budget;
    let mut out = Vec::new();
    for trace in crate::traces::list(store, owner).await? {
        let download = format!("{origin}{}/{}", crate::traces::API, trace.id);
        let mut entry = json!({
            "id": trace.id,
            "title": trace.title,
            "agent": trace.agent,
            "model": trace.model,
            "steps": trace.steps,
            "bytes": trace.bytes,
            "uploaded": when(trace.uploaded_unix),
            "shared": trace.shared,
            "url": format!("{origin}{}/{}", crate::traces::PAGE, trace.id),
        });
        if trace.shared {
            entry["share_url"] = json!(format!("{origin}{}/{}", crate::traces::PUBLIC, trace.id));
        }
        if take(&mut left, trace.bytes) {
            entry["document"] = match crate::traces::load(store, owner, &trace.id).await? {
                Some((_, document)) => document,
                None => Value::Null,
            };
        } else {
            entry["document"] = Value::Null;
            entry["note"] = json!(too_large(&download));
        }
        let mut agents = Vec::new();
        for agent in crate::traces::agents(store, owner, &trace.id).await? {
            let download = format!(
                "{origin}{}/{}/agents/{}",
                crate::traces::API,
                trace.id,
                agent.id
            );
            let mut row = json!({
                "id": agent.id,
                "parent": agent.parent,
                "title": agent.title,
                "agent": agent.agent,
                "model": agent.model,
                "steps": agent.steps,
                "bytes": agent.bytes,
                "uploaded": when(agent.uploaded_unix),
            });
            if take(&mut left, agent.bytes) {
                row["document"] =
                    match crate::traces::load_agent(store, owner, &trace.id, &agent.id).await? {
                        Some((_, document)) => document,
                        None => Value::Null,
                    };
            } else {
                row["document"] = Value::Null;
                row["note"] = json!(too_large(&download));
            }
            agents.push(row);
        }
        if !agents.is_empty() {
            entry["agents"] = Value::Array(agents);
        }
        out.push(entry);
    }
    Ok(out)
}

/// Take `bytes` from what is `left`, when they fit.
fn take(left: &mut usize, bytes: usize) -> bool {
    if bytes <= *left {
        *left -= bytes;
        true
    } else {
        false
    }
}

fn too_large(download: &str) -> String {
    format!("Too large to fit in this file. Download it on its own from {download}")
}

/// What the account service and this server say about the account, beside
/// the chat store. A part that can't be read is `null` and named in
/// `unavailable`.
#[derive(Default)]
pub(crate) struct AccountParts {
    pub profile: Value,
    pub projects: Option<Value>,
    pub computers: Option<Value>,
    pub settings: Value,
    pub unavailable: Vec<&'static str>,
}

async fn account_section(
    app: &App,
    service: &CloudSession,
    headers: &HeaderMap,
    viewer: &Viewer,
    owner: &str,
) -> AccountParts {
    let mut parts = AccountParts {
        profile: profile(viewer),
        ..AccountParts::default()
    };
    match service.github_status(headers).await {
        Ok(status) => parts.projects = Some(projects(&status)),
        Err(_) => parts.unavailable.push("projects"),
    }
    match service.app_sessions(headers).await {
        Ok(sessions) => {
            let choices = app
                .config
                .chat_store
                .computers(owner)
                .await
                .map(|computers| computers.sync)
                .unwrap_or_default();
            let rows: Vec<Value> = sessions
                .iter()
                .map(|session| {
                    let name = crate::coder_sync::line(&session.computer, 64);
                    json!({
                        "computer": name,
                        "app": session.app,
                        "signed_in": when(session.created_at),
                        "expires": when(session.expires_at),
                        "chats_live": choices.get(&name).map(|choice| choice.as_str()),
                    })
                })
                .collect();
            parts.computers = Some(Value::Array(rows));
        }
        Err(_) => parts.unavailable.push("computers"),
    }
    let mut settings = serde_json::Map::new();
    // API keys: the account's own workspace, as Settings > API keys.
    if let Some(workspace) = viewer.workspaces.iter().find(|w| w.role == "owner") {
        match viewer.client().account().keys(&workspace.id).await {
            Ok(keys) => {
                let rows: Vec<Value> = keys
                    .into_iter()
                    .filter(|key| key.status.as_deref() != Some("revoked"))
                    .map(|key| {
                        json!({
                            "name": key.name.unwrap_or_else(|| "Unnamed key".to_owned()),
                            "created": key.created,
                            "status": key.status.unwrap_or_else(|| "active".to_owned()),
                        })
                    })
                    .collect();
                settings.insert("api_keys".into(), Value::Array(rows));
            }
            Err(_) => parts.unavailable.push("api_keys"),
        }
        if let Ok(saved) = viewer.client().account().provider_keys(&workspace.id).await {
            let rows: Vec<Value> = saved
                .into_iter()
                .map(|key| json!({"provider": key.provider, "added": when(key.added_at)}))
                .collect();
            settings.insert("own_provider_keys".into(), Value::Array(rows));
        }
    }
    // The Claude credential: its kind only, from the selected workspace.
    if let Some(computers) = app.config.cloud_byo.as_deref()
        && let Ok(owner) = crate::cloud::byo::Owner::from_viewer(viewer)
    {
        match computers.status(&owner, now()) {
            Ok(Some(status)) => {
                settings.insert(
                    "claude_credential".into(),
                    json!({
                        "saved": crate::settings::material_label(status.material),
                        "since": when(status.stored_at),
                    }),
                );
            }
            Ok(None) => {
                settings.insert("claude_credential".into(), Value::Null);
            }
            Err(_) => parts.unavailable.push("claude_credential"),
        }
    }
    if let Some(plans) = app.config.plan.as_deref() {
        let view = plans.view(&viewer.account_id, now() as i64, false);
        match view.summary {
            Some(Some(summary)) => {
                settings.insert(
                    "plan".into(),
                    json!({
                        "hours_used_this_month": summary.used_seconds as f64 / 3600.0,
                        "hours_included": summary.included_seconds as f64 / 3600.0,
                        "extra_hours": summary.extra.enabled,
                        "extra_hours_monthly_cap_usd": summary.extra.cap_usd_micros as f64 / 1_000_000.0,
                    }),
                );
            }
            Some(None) => parts.unavailable.push("plan"),
            None => {}
        }
    }
    parts.settings = Value::Object(settings);
    parts
}

/// Who the account is, and the workspace this export was made in.
fn profile(viewer: &Viewer) -> Value {
    json!({
        "name": viewer.account_label,
        "email": viewer.email,
        "workspaces": viewer
            .workspaces
            .iter()
            .map(|w| json!({"name": w.name, "role": w.role}))
            .collect::<Vec<_>>(),
        "workspace": viewer.workspace.as_ref().map(|w| json!({"name": w.name, "role": w.role})),
    })
}

/// The account's GitHub connection and projects; never its token.
pub(crate) fn projects(status: &oa_auth::repos::Status) -> Value {
    use oa_auth::repos::Access;
    let github = match &status.access {
        Access::None => json!({"connected": false}),
        Access::Connected { login, private } => {
            json!({"connected": true, "login": login, "private_repositories": private})
        }
        Access::Reconnect { login } => {
            json!({"connected": false, "login": login, "needs_reconnect": true})
        }
        Access::Installed { login, .. } => json!({"connected": true, "login": login}),
    };
    let projects: Vec<Value> = status
        .projects
        .iter()
        .map(|project| {
            json!({
                "id": project.id,
                "name": project.name,
                "repository": project.repository,
                "default_branch": project.default_branch,
                "private": project.private,
                "added": when(project.created_unix),
            })
        })
        .collect();
    json!({"github": github, "projects": projects})
}

/// One chat: its record as a person reads it, and a Markdown copy.
pub(crate) fn chat(chat: &Conversation, projects: &BTreeMap<String, String>) -> Value {
    let messages: Vec<&Message> = all_messages(chat);
    let project = chat.project.as_ref().map(|id| {
        json!({
            "id": id,
            "repository": projects.get(id),
        })
    });
    let tasks: Vec<Value> = chat
        .tasks
        .iter()
        .map(|task| {
            json!({
                "title": task.title,
                "kind": task.kind,
                "state": task.state,
                "started": when(task.started_unix),
                "finished": task.finished_unix.map(when),
            })
        })
        .collect();
    json!({
        "id": chat.id,
        "title": chat.title,
        "updated": when(chat.updated_unix),
        "pinned": chat.pinned_unix.is_some(),
        "archived": chat.archived_unix.is_some(),
        "project": project,
        "branch": chat.branch,
        "environment": chat.environment.as_ref().map(|e| json!({"repository": e.repository})),
        "computer": chat.terminal.as_ref().map(|t| t.computer.clone()),
        "tasks": tasks,
        "messages": messages
            .iter()
            .map(|m| json!({"role": role(m.role), "text": m.text}))
            .collect::<Vec<_>>(),
        "markdown": markdown(chat, &messages, projects),
    })
}

/// The chat's messages, as the chat page shows them. A Coder chat's
/// messages already hold the ones added on the website
/// (`coder_sync::with_continued`).
fn all_messages(chat: &Conversation) -> Vec<&Message> {
    chat.messages.iter().collect()
}

fn role(role: Role) -> &'static str {
    match role {
        Role::User => "you",
        Role::Assistant => "openagents",
        Role::Tool => "tool",
    }
}

fn speaker(role: Role) -> &'static str {
    match role {
        Role::User => "You",
        Role::Assistant => "OpenAgents",
        Role::Tool => "Tool",
    }
}

/// A chat as Markdown: its title, when, where, and each message.
pub(crate) fn markdown(
    chat: &Conversation,
    messages: &[&Message],
    projects: &BTreeMap<String, String>,
) -> String {
    let title = if chat.title.trim().is_empty() {
        "Untitled chat"
    } else {
        chat.title.trim()
    };
    let mut out = format!("# {title}\n\n- Updated: {}\n", when(chat.updated_unix));
    if let Some(repository) = chat.project.as_ref().and_then(|id| projects.get(id)) {
        out.push_str(&format!("- Project: {repository}\n"));
    }
    if let Some(terminal) = &chat.terminal {
        out.push_str(&format!("- From Coder on: {}\n", terminal.computer));
    }
    if chat.archived_unix.is_some() {
        out.push_str("- Archived\n");
    }
    for message in messages {
        out.push_str(&format!(
            "\n**{}**\n\n{}\n",
            speaker(message.role),
            message.text.trim_end()
        ));
    }
    out
}

/// The whole file.
pub(crate) fn assemble(
    stored: &Stored,
    account: &AccountParts,
    origin: &str,
    generated_unix: u64,
) -> Value {
    let names: BTreeMap<String, String> = account
        .projects
        .as_ref()
        .and_then(|projects| projects["projects"].as_array())
        .map(|rows| {
            rows.iter()
                .filter_map(|row| {
                    Some((
                        row["id"].as_str()?.to_owned(),
                        row["repository"].as_str()?.to_owned(),
                    ))
                })
                .collect()
        })
        .unwrap_or_default();
    let chats: Vec<Value> = stored.chats.iter().map(|c| chat(c, &names)).collect();
    json!({
        "schema": SCHEMA,
        "made": when(generated_unix),
        "from": origin,
        "about": "Everything on your OpenAgents account: chats (each with a Markdown copy), projects, traces, computers, and settings. Keys, tokens, and passwords are never included.",
        "account": account.profile,
        "chats": chats,
        "projects": account.projects,
        "traces": stored.traces,
        "computers": account.computers,
        "settings": account.settings,
        "unavailable": account.unavailable,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    const ONE: &str = "11111111-1111-4111-8111-111111111111";
    const TWO: &str = "22222222-2222-4222-8222-222222222222";

    fn conversation(owner: &str, id: &str, title: &str, text: &str) -> Conversation {
        serde_json::from_value(json!({
            "id": id,
            "owner": owner,
            "revision": 1,
            "title": title,
            "messages": [
                {"role": "user", "text": text},
                {"role": "assistant", "text": "Here you go."},
            ],
            "pending": null,
            "requests": [],
            "updated_unix": 1_790_000_000u64,
        }))
        .unwrap()
    }

    fn trace(text: &str) -> Vec<u8> {
        serde_json::to_vec(&json!({
            "schema_version": "ATIF-v1.7",
            "session_id": "s1",
            "agent": {"name": "openagents-coder", "version": "1", "model_name": "gpt-test"},
            "steps": [
                {"step_id": 1, "source": "user", "message": text},
                {"step_id": 2, "source": "agent", "message": "Done.", "model_name": "gpt-test"}
            ]
        }))
        .unwrap()
    }

    #[tokio::test]
    async fn an_export_holds_only_the_accounts_own_chats_and_traces() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let me = account_owner("acct_me");
        let them = account_owner("acct_them");
        store
            .create(&conversation(&me, ONE, "Mine", "my question"))
            .await
            .unwrap();
        store
            .create(&conversation(&them, TWO, "Theirs", "their secret plan"))
            .await
            .unwrap();
        crate::traces::upload(&store, &me, &trace("Fix my build"), false)
            .await
            .unwrap();
        crate::traces::upload(&store, &them, &trace("Their private run"), true)
            .await
            .unwrap();

        let stored = Stored::read(&store, &me, "https://openagents.com", MAX_TRACE_BYTES)
            .await
            .unwrap();
        let file = assemble(
            &stored,
            &AccountParts::default(),
            "https://openagents.com",
            1_790_000_000,
        );
        let text = serde_json::to_string(&file).unwrap();
        assert!(text.contains("my question"), "{text}");
        assert!(text.contains("Fix my build"), "{text}");
        assert!(!text.contains("their secret plan"), "{text}");
        assert!(!text.contains("Their private run"), "{text}");
        assert!(!text.contains("Theirs"), "{text}");
        assert_eq!(file["chats"].as_array().unwrap().len(), 1);
        assert_eq!(file["traces"].as_array().unwrap().len(), 1);
        assert_eq!(file["schema"], SCHEMA);
        // The trace document travels whole.
        assert_eq!(
            file["traces"][0]["document"]["steps"]
                .as_array()
                .map(Vec::len),
            Some(2)
        );
        // The owner value (a digest of the account id) is never written.
        assert!(!text.contains(&me), "{text}");
        assert!(!text.contains("acct_me"), "{text}");

        // And the other account's export is theirs alone.
        let stored = Stored::read(&store, &them, "https://openagents.com", MAX_TRACE_BYTES)
            .await
            .unwrap();
        let text = serde_json::to_string(&assemble(
            &stored,
            &AccountParts::default(),
            "https://openagents.com",
            1_790_000_000,
        ))
        .unwrap();
        assert!(text.contains("their secret plan"));
        assert!(!text.contains("my question"));
    }

    #[tokio::test]
    async fn a_trace_past_the_budget_is_linked_not_carried() {
        let dir = tempfile::tempdir().unwrap();
        let store = Store::local(dir.path().to_path_buf());
        let me = account_owner("acct_me");
        crate::traces::upload(&store, &me, &trace("Big run"), false)
            .await
            .unwrap();
        let stored = Stored::read(&store, &me, "https://openagents.com", 1)
            .await
            .unwrap();
        let trace = &stored.traces[0];
        assert!(trace["document"].is_null());
        let note = trace["note"].as_str().unwrap();
        assert!(
            note.starts_with("Too large to fit in this file. Download it on its own from https://openagents.com/api/traces/"),
            "{note}"
        );
        assert!(trace["url"].as_str().unwrap().contains("/settings/traces/"));
    }

    #[test]
    fn a_chat_reads_as_markdown_with_its_project() {
        let mut record = conversation(&account_owner("acct_me"), ONE, "Fix CI", "Why is CI red?");
        record.project = Some("prj_0123456789abcdef".into());
        record.archived_unix = Some(1);
        let projects =
            BTreeMap::from([("prj_0123456789abcdef".to_owned(), "ada/engine".to_owned())]);
        let value = chat(&record, &projects);
        let text = value["markdown"].as_str().unwrap();
        assert!(text.starts_with("# Fix CI\n\n- Updated: "), "{text}");
        assert!(text.contains("- Project: ada/engine\n"), "{text}");
        assert!(text.contains("- Archived\n"), "{text}");
        assert!(
            text.contains("\n**You**\n\nWhy is CI red?\n\n**OpenAgents**\n\nHere you go.\n"),
            "{text}"
        );
        assert_eq!(value["project"]["repository"], "ada/engine");
        assert_eq!(value["archived"], true);
        assert_eq!(value["messages"][0]["role"], "you");
        // Request bookkeeping and selections never reach the file.
        for absent in ["requests", "selection", "pending", "owner", "revision"] {
            assert!(value.get(absent).is_none(), "{absent}");
        }
    }

    #[test]
    fn projects_never_carry_the_github_token() {
        let status = oa_auth::repos::Status {
            access: oa_auth::repos::Access::Connected {
                login: "ada".into(),
                private: true,
            },
            projects: Vec::new(),
        };
        let value = projects(&status);
        assert_eq!(value["github"]["login"], "ada");
        assert_eq!(value["github"]["connected"], true);
        assert!(value["projects"].as_array().unwrap().is_empty());
    }

    #[test]
    fn the_file_is_named_for_its_day() {
        assert_eq!(file_name(0), "openagents-export-1970-01-01.json");
        assert_eq!(file_name(86_400 * 365), "openagents-export-1971-01-01.json");
    }

    #[test]
    fn take_spends_the_budget_once() {
        let mut left = 10;
        assert!(take(&mut left, 6));
        assert!(!take(&mut left, 6));
        assert!(take(&mut left, 4));
        assert_eq!(left, 0);
    }
}
