//! The signed-in person's own screens: Settings and its pages, projects,
//! environments, purchases, tasks, agents, profiles, and analytics, each
//! from its live page function over sample data.

use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use coder_environment_operator::activity::{CheckLine, Entry, Record, Stage};
use coder_environment_operator::agent::Phase;
use coder_environment_operator::studio::claude::{Run, RunState};
use coder_environment_operator::studio::{Candidate, Status, Summary, Version, View, sign_in};
use maud::{Markup, html};
use serde_json::json;

use super::auth::{COMPUTER, NAME, signed_in};
use super::{Group, Screen, state};
use crate::environments::view::Claude;

/// October 9, 2026, 12:00 UTC.
const NOW: u64 = 1_791_547_200;
const DAY: u64 = 86_400;
const CSRF: &str = "sample-ticket";
const TICKET: (&str, &str) = ("sample-ticket", "sample-request");
const REPO: &str = "sample-org/sample-app";
const ENV: &str = "env-sample-1";
const RUN: &str = "claude-env-sample-1-1";

pub(crate) fn screens() -> Vec<Screen> {
    vec![
        Screen {
            id: "settings",
            title: "Settings",
            group: Group::Page,
            routes: &["/settings", "/settings/memory", "/settings/schedules"],
            states: vec![
                state("default", "Logged in", settings_default),
                state("plan-free", "Plan: free", settings_free),
                state("plan-paid", "Plan: paid", settings_paid),
                state("plan-problem", "Plan: billing problem", settings_problem),
                state("chats-none", "Chats: none saved", settings_no_chats),
                state("memory-empty", "Memory: empty", memory_empty),
                state("memory-notes", "Memory: with notes", memory_notes),
                state("schedules-empty", "Schedules: empty", schedules_empty),
                state("schedules-rows", "Schedules: with rows", schedules_rows),
            ],
        },
        Screen {
            id: "sign-in-keys",
            title: "Login keys",
            group: Group::Page,
            routes: &["/settings/sign-in-keys"],
            states: vec![
                state("none", "No keys", sign_in_keys_none),
                state("keys", "With keys", sign_in_keys_some),
                state("made", "Key just made", sign_in_keys_made),
                state("signed-out", "Logged out", signed_out),
                state(
                    "unreachable",
                    "Service unreachable",
                    sign_in_keys_unreachable,
                ),
            ],
        },
        Screen {
            id: "api-keys",
            title: "API keys",
            group: Group::Page,
            routes: &["/settings/api-keys"],
            states: vec![
                state("none", "No keys", api_keys_none),
                state("keys", "With keys", api_keys_some),
                state("made", "Key just made", api_keys_made),
                state("own-saved", "Own keys saved", api_keys_own_saved),
                state("own-unsaved", "Own keys not saved", api_keys_own_unsaved),
                state("signed-out", "Logged out", signed_out),
                state("unreachable", "Service unreachable", api_keys_unreachable),
            ],
        },
        Screen {
            id: "agent-accounts",
            title: "Agent accounts",
            group: Group::Page,
            routes: &[crate::agent_accounts::PAGE],
            states: vec![
                state("none", "No agents", agent_accounts_none),
                state("mine", "With agents", agent_accounts_mine),
                state("admin", "Admin: every agent", agent_accounts_admin),
                state("signed-out", "Logged out", signed_out),
                state(
                    "unreachable",
                    "Service unreachable",
                    agent_accounts_unreachable,
                ),
            ],
        },
        Screen {
            id: "computers",
            title: "Computers",
            group: Group::Organism,
            routes: &["/settings"],
            states: vec![
                state("none", "None", computers_none),
                state("one", "One logged in", computers_one),
                state("several", "Several", computers_several),
            ],
        },
        Screen {
            id: "vault",
            title: "Vault",
            group: Group::Page,
            routes: &["/settings/vault", "/projects/{id}/vault"],
            states: vec![
                state("opening", "Opening", vault_opening),
                state("set-up", "Set up", vault_set_up),
                state("locked", "Locked", vault_locked),
                state("open", "Open, no files yet", vault_open),
                state("project", "A project's vault, open", vault_project),
            ],
        },
        Screen {
            id: "claude-key",
            title: "Claude key",
            group: Group::Page,
            routes: &["/settings/claude"],
            states: vec![
                state("none", "Nothing saved", claude_key_none),
                state("saved", "A key saved", claude_key_saved),
                state(
                    "subscription",
                    "With a subscription offer",
                    claude_key_subscription,
                ),
            ],
        },
        Screen {
            id: "mac-jobs",
            title: "Mac jobs",
            group: Group::Page,
            routes: &["/settings/mac-jobs", "/settings/mac-jobs/{id}"],
            states: vec![
                state("list-empty", "List: no Mac yet", mac_jobs_empty),
                state("list", "List: a Mac and its jobs", mac_jobs_list),
                state("queued", "Queued", mac_job_queued),
                state("asking", "Running, question pending", mac_job_asking),
                state("done", "Done with files", mac_job_done),
                state("stopped", "Stopped", mac_job_stopped),
                state("failed", "Failed", mac_job_failed),
            ],
        },
        Screen {
            id: "projects",
            title: "Projects",
            group: Group::Page,
            routes: &[
                "/projects",
                "/projects/{id}",
                "/projects/{id}/sources",
                "/projects/join/{code}",
            ],
            states: vec![
                state("no-github", "No GitHub connection", projects_no_github),
                state(
                    "connected-empty",
                    "Connected, no repositories",
                    projects_connected_empty,
                ),
                state("search", "Repositories with search", projects_search),
                state("sources", "Project with sources", projects_sources),
                state(
                    "project-bare",
                    "A project with no repository",
                    project_no_repository,
                ),
                state(
                    "project-two",
                    "A project with two repositories",
                    project_two_repositories,
                ),
                state("install", "Install the app", projects_install),
                state("choose", "App installed, nothing picked", projects_choose),
                state("disconnected", "Disconnected", projects_disconnected),
                state("reconnect", "GitHub access ended", projects_reconnect),
                state("failed", "Repository call failed", projects_failed),
                state("drive", "Sources: attached and My Drive", sources_drive),
                state(
                    "drive-search",
                    "Sources: search with no files",
                    sources_search,
                ),
                state(
                    "drive-connect",
                    "Sources: connect Google first",
                    sources_connect,
                ),
                state(
                    "drive-reconnect",
                    "Sources: Google access ended",
                    sources_reconnect,
                ),
            ],
        },
        Screen {
            id: "environments",
            title: "Environments",
            group: Group::Page,
            routes: &[
                "/environments",
                "/environments/new",
                "/environments/{id}",
                "/environments/{id}/runs/{run}",
            ],
            states: vec![
                state("list-empty", "List: empty", environments_empty),
                state("list", "List: populated", environments_list),
                state("new", "New: pick a repository", environments_new),
                state("new-error", "New: repository error", environments_new_error),
                state("new-branch", "New: pick a branch", environments_new_branch),
                state("idle", "Detail: idle", environment_idle),
                state(
                    "running",
                    "Detail: running with events",
                    environment_running,
                ),
                state(
                    "question",
                    "Detail: asking a question",
                    environment_question,
                ),
                state("review", "Detail: ready to save", environment_review),
                state(
                    "needs-sign-in",
                    "Detail: needs Claude login",
                    environment_needs_sign_in,
                ),
                state("failed", "Detail: retry after failure", environment_failed),
                state("run-starting", "Run: starting", run_starting),
                state("run-running", "Run: running", run_running),
                state("run-paused", "Run: choose how to go on", run_paused),
                state("run-done", "Run: done", run_done),
                state("run-stopped", "Run: stopped", run_stopped),
            ],
        },
        Screen {
            id: "claude-code",
            title: "Claude Code",
            group: Group::Page,
            routes: &[
                "/environments/{id}/claude",
                "/environments/{id}/sign-in",
                "/environments/{id}/sign-in/screen",
                "/environments/{id}/sign-in/input",
            ],
            states: vec![
                state("run", "Run Claude Code", claude_run),
                state(
                    "run-signed-in",
                    "Run Claude Code, logged in",
                    claude_run_signed_in,
                ),
                state("sign-in", "Log in to Claude", claude_sign_in),
                state(
                    "sign-in-starting",
                    "Login starting",
                    claude_sign_in_starting,
                ),
                state("sign-in-screen", "Login screen", claude_sign_in_open),
                state("sign-in-input", "Login input error", claude_sign_in_input),
                state("signed-in", "Logged in to Claude", claude_signed_in),
                state("sign-in-failed", "Login failed", claude_sign_in_failed),
            ],
        },
        Screen {
            id: "purchases",
            title: "Plugin purchases",
            group: Group::Page,
            routes: &["/app/purchases", "/app/purchases/{id}"],
            states: vec![
                state("none", "None", purchases_none),
                state("list", "List", purchases_list),
                state("paid", "Paid", purchase_paid),
                state("pending", "Pending", purchase_pending),
                state("failed", "Failed", purchase_failed),
            ],
        },
        Screen {
            id: "tasks",
            title: "Task",
            group: Group::Page,
            routes: &["/app", "/app/tasks/{id}"],
            states: vec![
                state("list-empty", "List: no tasks", task_list_empty),
                state("list", "List: one task", task_list),
                state("queued", "Queued", task_queued),
                state("running", "Running", task_running),
                state("cancel-requested", "Stop asked", task_cancel_requested),
                state("cancelled", "Cancelled", task_cancelled),
                state("finished", "Finished", task_finished),
                state("unknown", "Unknown", task_unknown),
            ],
        },
        Screen {
            id: "agents",
            title: "Agents",
            group: Group::Page,
            routes: &["/settings/agents", "/settings/agents/message"],
            states: vec![
                state("none", "No computers", agents_none),
                state("machines", "Computers with agents", agents_machines),
                state("message", "Message an agent", agents_message),
                state("stopping", "Asked to stop", agents_stopping),
            ],
        },
        Screen {
            id: "profile",
            title: "Profile",
            group: Group::Page,
            routes: &["/u/{login}"],
            states: vec![
                state("own", "Your own", profile_own),
                state("other", "Someone else's", profile_other),
                state("missing", "Missing", profile_missing),
            ],
        },
        Screen {
            id: "analytics",
            title: "Analytics",
            group: Group::Page,
            routes: &["/admin/analytics"],
            states: vec![state("admin", "As an admin sees it", analytics)],
        },
        Screen {
            id: "drive-access",
            title: "Google Drive access",
            group: Group::Page,
            routes: &[crate::connections::access::PAGE],
            states: vec![state("admin", "As an admin sees it", drive_access)],
        },
    ]
}

fn settings_page(headers: &HeaderMap, title: &str, path: &str, body: Markup) -> Response {
    crate::settings::page_as(headers, signed_in(), title, path, body)
}

fn signed_out(headers: &HeaderMap) -> Response {
    crate::auth::sign_in_page(headers, false, false, crate::sign_in_keys::PAGE)
}

// Settings.

fn plans() -> crate::plan::Plans {
    crate::plan::Plans::with_meter(
        Some(retail_cloud::journal::Journal::in_memory().unwrap()),
        Some("https://checkout.example/sample".to_owned()),
    )
}

fn paid_plans() -> crate::plan::Plans {
    let mut journal = retail_cloud::journal::Journal::in_memory().unwrap();
    retail_cloud::environment::record_period(
        &mut journal,
        &retail_cloud::environment::Period {
            account: "acct-sample".into(),
            plan: retail_cloud::environment::plan().version,
            start: (NOW - 8 * DAY) as i64,
            end: (NOW + 22 * DAY) as i64,
        },
    )
    .unwrap();
    crate::plan::Plans::with_meter(Some(journal), None)
}

fn plan_markup(view: &crate::plan::View) -> Markup {
    crate::plan::section(view, Some(TICKET), Some(TICKET))
}

fn settings_with(headers: &HeaderMap, chats: Option<usize>, plan: Markup) -> Response {
    let sessions = sessions();
    let body = html! {
        (crate::settings::settings_content(NAME, None, chats, plan))
        (crate::device::computers_markup(&rows(&sessions[..2])))
        (crate::vault::settings_row())
    };
    settings_page(headers, "Settings", crate::settings::PAGE, body)
}

fn settings_default(headers: &HeaderMap) -> Response {
    let view = paid_plans().view("acct-sample", NOW as i64, false);
    settings_with(headers, Some(42), plan_markup(&view))
}

fn settings_free(headers: &HeaderMap) -> Response {
    let view = plans().view("acct-sample", NOW as i64, false);
    settings_with(headers, Some(42), plan_markup(&view))
}

fn settings_paid(headers: &HeaderMap) -> Response {
    let view = paid_plans().view("acct-sample", NOW as i64, true);
    settings_with(headers, Some(42), plan_markup(&view))
}

fn settings_problem(headers: &HeaderMap) -> Response {
    let mut view = paid_plans().view("acct-sample", NOW as i64, false);
    if let Some(Some(summary)) = &mut view.summary {
        summary.notice = Some(retail_cloud::environment::Notice::Dispute);
    }
    settings_with(headers, Some(42), plan_markup(&view))
}

fn settings_no_chats(headers: &HeaderMap) -> Response {
    let view = plans().view("acct-sample", NOW as i64, false);
    settings_with(headers, Some(0), plan_markup(&view))
}

fn note(
    id: &str,
    kind: &str,
    name: &str,
    body: &str,
    project: Option<&str>,
) -> crate::account_memory::Note {
    crate::account_memory::Note {
        id: id.into(),
        scope: if project.is_some() {
            "project"
        } else {
            "account"
        }
        .into(),
        project: project.map(|_| "prj_0123456789abcdef".to_owned()),
        project_name: project.map(str::to_owned),
        kind: Some(kind.into()),
        name: Some(name.into()),
        description: None,
        body: Some(body.into()),
        updated: NOW - 3 * DAY,
        deleted: false,
    }
}

fn memory(headers: &HeaderMap, notes: &[crate::account_memory::Note]) -> Response {
    let notes: Vec<_> = notes.iter().collect();
    let body = crate::account_memory::memory_content(&notes, TICKET, TICKET);
    settings_page(headers, "Memory", crate::account_memory::PAGE, body)
}

fn memory_empty(headers: &HeaderMap) -> Response {
    memory(headers, &[])
}

fn memory_notes(headers: &HeaderMap) -> Response {
    memory(
        headers,
        &[
            note(
                "note-sample-1",
                "user",
                "Sample timezone",
                "I work from Austin, on Central time.",
                None,
            ),
            note(
                "note-sample-2",
                "feedback",
                "Short answers",
                "Keep replies short and lead with the answer.",
                None,
            ),
            note(
                "note-sample-3",
                "project",
                "Test command",
                "Run the tests with make check.",
                Some("sample-app"),
            ),
        ],
    )
}

fn schedules(headers: &HeaderMap, rows: &[crate::account_schedules::Schedule]) -> Response {
    let chats = [crate::account_schedules::Chat {
        session: "chat-sample-1".into(),
        computer: COMPUTER.into(),
        title: "Morning check".into(),
    }];
    let body = crate::account_schedules::schedules_content(
        rows,
        &[COMPUTER.to_owned()],
        &chats,
        [TICKET, TICKET, TICKET],
    );
    settings_page(
        headers,
        "Scheduled prompts",
        crate::account_schedules::PAGE,
        body,
    )
}

fn schedules_empty(headers: &HeaderMap) -> Response {
    schedules(headers, &[])
}

fn schedules_rows(headers: &HeaderMap) -> Response {
    let daily = crate::account_schedules::Schedule {
        id: "sch-sample-1".into(),
        computer: COMPUTER.into(),
        prompt: Some("Summarize yesterday's failing tests.".into()),
        days: vec![1, 2, 3, 4, 5],
        time: Some("09:00".into()),
        every_secs: None,
        chat: Some("chat-sample-1".into()),
        chat_title: Some("Morning check".into()),
        agent: None,
        workspace: Some("~/code/sample-app".into()),
        paused: false,
        updated: NOW - DAY,
        deleted: false,
    };
    let hourly = crate::account_schedules::Schedule {
        id: "sch-sample-2".into(),
        prompt: Some("Check the sample site is up.".into()),
        days: vec![],
        time: None,
        every_secs: Some(3_600),
        chat: None,
        chat_title: None,
        workspace: None,
        paused: true,
        ..daily.clone()
    };
    schedules(headers, &[daily, hourly])
}

// Sign-in keys.

fn sign_in_key(
    id: &str,
    name: &str,
    current: bool,
    used: Option<u64>,
) -> crate::cloud::session::device::SignInKey {
    crate::cloud::session::device::SignInKey {
        id: id.into(),
        name: name.into(),
        created_at: NOW - 30 * DAY,
        last_used_at: used,
        expires_at: Some(NOW + 60 * DAY),
        current,
    }
}

fn sign_in_keys(
    headers: &HeaderMap,
    keys: &[crate::cloud::session::device::SignInKey],
) -> Response {
    let body = crate::sign_in_keys::keys_content(keys, TICKET, TICKET);
    settings_page(headers, "Login keys", crate::sign_in_keys::PAGE, body)
}

fn sign_in_keys_none(headers: &HeaderMap) -> Response {
    sign_in_keys(headers, &[])
}

fn sign_in_keys_some(headers: &HeaderMap) -> Response {
    sign_in_keys(
        headers,
        &[
            sign_in_key("key-sample-1", "Sample laptop", true, Some(NOW - 3_600)),
            sign_in_key("key-sample-2", "Sample build box", false, None),
        ],
    )
}

fn sign_in_keys_made(headers: &HeaderMap) -> Response {
    let made = crate::cloud::session::device::MadeKey {
        key: sign_in_key("key-sample-3", "Sample phone", false, None),
        token: "oas_sample.not-a-real-key".into(),
    };
    let body = crate::sign_in_keys::made_content("https://openagents.com", &made);
    settings_page(
        headers,
        "Your new login key",
        crate::sign_in_keys::PAGE,
        body,
    )
}

fn sign_in_keys_unreachable(headers: &HeaderMap) -> Response {
    crate::ui_page::problem(
        headers,
        StatusCode::BAD_GATEWAY,
        "Login keys",
        crate::sign_in_keys::UNREACHABLE,
        (crate::sign_in_keys::PAGE, "Login keys"),
    )
}

// API keys.

const API_KEYS: &str = "/settings/api-keys";

fn api_key(id: &str, name: &str, active: bool, can: &str) -> crate::api_keys::Row {
    crate::api_keys::Row {
        id: id.into(),
        name: name.into(),
        created: "2026-09-12".into(),
        active,
        can: can.into(),
    }
}

fn api_keys(headers: &HeaderMap, rows: &[crate::api_keys::Row], own: Option<&[bool]>) -> Response {
    let body = html! {
        (crate::api_keys::keys_content(rows, TICKET, TICKET))
        @if let Some(saved) = own { (crate::api_keys::own_content(saved, TICKET)) }
    };
    settings_page(headers, "API keys", API_KEYS, body)
}

fn sample_api_keys() -> Vec<crate::api_keys::Row> {
    vec![
        api_key(
            "key_sample_1",
            "Sample script",
            true,
            "Call models, List models and prices",
        ),
        api_key("key_sample_2", "Old sample laptop", false, ""),
    ]
}

fn api_keys_none(headers: &HeaderMap) -> Response {
    api_keys(headers, &[], None)
}

fn api_keys_some(headers: &HeaderMap) -> Response {
    api_keys(headers, &sample_api_keys(), None)
}

fn api_keys_made(headers: &HeaderMap) -> Response {
    let body = crate::api_keys::made_content("oak_sample.not-a-real-key", "Sample script");
    settings_page(headers, "Your new key", API_KEYS, body)
}

fn api_keys_own_saved(headers: &HeaderMap) -> Response {
    api_keys(headers, &sample_api_keys(), Some(&[true, false]))
}

fn api_keys_own_unsaved(headers: &HeaderMap) -> Response {
    api_keys(headers, &sample_api_keys(), Some(&[false, false]))
}

fn api_keys_unreachable(headers: &HeaderMap) -> Response {
    crate::ui_page::problem(
        headers,
        StatusCode::BAD_GATEWAY,
        "API keys",
        crate::api_keys::UNREACHABLE,
        (API_KEYS, "API keys"),
    )
}

// Agent accounts.

fn sample_agents() -> serde_json::Value {
    json!({"agents": [
        {"id": "agt_sample_1", "name": "Sample research agent",
         "registered": "2026-10-01T09:00:00Z", "claimed": "2026-10-02T10:00:00Z",
         "revoked": null, "balance_usd": "1.25"},
        {"id": "agt_sample_2", "name": "Sample helper",
         "registered": "2026-09-20T09:00:00Z", "claimed": "2026-09-21T10:00:00Z",
         "revoked": "2026-10-05T12:00:00Z"},
    ]})
}

fn agent_accounts(headers: &HeaderMap, mine: serde_json::Value, admin: bool) -> Response {
    let mine = crate::agent_accounts::rows(&mine);
    let all = admin.then(|| {
        let mut every = sample_agents();
        every["agents"].as_array_mut().unwrap().push(json!({
            "id": "agt_sample_3", "name": "Sample unclaimed agent",
            "registered": "2026-10-10T09:00:00Z", "claimed": null, "revoked": null,
        }));
        crate::agent_accounts::rows(&every)
    });
    let body = crate::agent_accounts::content(&mine, all.as_deref(), TICKET, TICKET);
    settings_page(headers, "Agents", crate::agent_accounts::PAGE, body)
}

fn agent_accounts_none(headers: &HeaderMap) -> Response {
    agent_accounts(headers, json!({"agents": []}), false)
}

fn agent_accounts_mine(headers: &HeaderMap) -> Response {
    agent_accounts(headers, sample_agents(), false)
}

fn agent_accounts_admin(headers: &HeaderMap) -> Response {
    agent_accounts(headers, sample_agents(), true)
}

fn agent_accounts_unreachable(headers: &HeaderMap) -> Response {
    crate::ui_page::problem(
        headers,
        StatusCode::BAD_GATEWAY,
        "Agents",
        crate::agent_accounts::UNREACHABLE,
        (crate::agent_accounts::PAGE, "Agents"),
    )
}

// Computers.

fn sessions() -> Vec<crate::cloud::session::device::AppSession> {
    [COMPUTER, "Sample Studio Mac", "sample-linux-box"]
        .iter()
        .enumerate()
        .map(|(i, computer)| crate::cloud::session::device::AppSession {
            id: format!("ses-sample-{i}"),
            app: "Coder".into(),
            computer: (*computer).into(),
            created_at: NOW - (i as u64 + 1) * 9 * DAY,
            expires_at: NOW + 60 * DAY,
        })
        .collect()
}

fn rows(sessions: &[crate::cloud::session::device::AppSession]) -> Vec<crate::device::Row<'_>> {
    let choices = [
        Some(crate::chat_store::SyncChoice::All),
        Some(crate::chat_store::SyncChoice::Local),
        None,
    ];
    sessions
        .iter()
        .zip(choices)
        .map(|(session, choice)| crate::device::Row {
            session,
            csrf: CSRF.into(),
            sync: CSRF.into(),
            choice,
        })
        .collect()
}

fn computers(headers: &HeaderMap, count: usize) -> Response {
    let sessions = sessions();
    let body = crate::device::computers_markup(&rows(&sessions[..count]));
    settings_page(headers, "Settings", crate::settings::PAGE, body)
}

fn computers_none(headers: &HeaderMap) -> Response {
    computers(headers, 0)
}

fn computers_one(headers: &HeaderMap) -> Response {
    computers(headers, 1)
}

fn computers_several(headers: &HeaderMap) -> Response {
    computers(headers, 3)
}

// Mac jobs.

fn mac_job(state: &str, extra: serde_json::Value) -> crate::mac_jobs::Job {
    let mut job = json!({
        "schema": "openagents.web.mac-jobs.job.v1",
        "id": "job-sample-1",
        "computer": COMPUTER,
        "spec": {"repo": REPO, "ref": "main", "recipe": "ios-release-gate", "args": []},
        "created_unix": NOW - 900,
        "updated_unix": NOW - 60,
        "state": state,
        "lines": ["Cloning sample-org/sample-app", "Building the app", "Starting the simulator"],
    });
    if let (Some(job), Some(extra)) = (job.as_object_mut(), extra.as_object()) {
        job.extend(extra.clone());
    }
    serde_json::from_value(job).unwrap()
}

fn mac_job_page(headers: &HeaderMap, job: &crate::mac_jobs::Job) -> Response {
    let body = crate::mac_jobs_page::job_body(job, CSRF, NOW);
    settings_page(
        headers,
        "Mac job",
        &format!("{}/{}", crate::mac_jobs_page::PAGE, job.id),
        body,
    )
}

fn mac_jobs_page(
    headers: &HeaderMap,
    macs: &[(String, crate::mac_jobs::Reported, bool)],
    jobs: &[crate::mac_jobs::Job],
) -> Response {
    let body = crate::mac_jobs_page::list_body(macs, jobs, NOW);
    settings_page(headers, "Mac jobs", crate::mac_jobs_page::PAGE, body)
}

fn mac_jobs_empty(headers: &HeaderMap) -> Response {
    mac_jobs_page(headers, &[], &[])
}

fn mac_jobs_list(headers: &HeaderMap) -> Response {
    let reported: crate::mac_jobs::Reported = serde_json::from_value(json!({
        "reported_unix": NOW - 30,
        "capabilities": {
            "macos": "26.4 (25E246)",
            "xcode": "26.6 (17F113)",
            "signing_identities": ["Apple Distribution: Sample Company (SAMPLE1234)"],
            "simulators": [{"name": "iPhone 17", "runtime": "iOS 26.5", "udid": "sample-udid", "booted": true}],
            "asc_key": true,
            "free_disk_gb": 212,
            "busy": true
        }
    }))
    .unwrap();
    let jobs = [
        mac_job("running", json!({})),
        mac_job(
            "done",
            json!({"id": "job-sample-2", "summary": "All 48 UI tests passed.", "finished_unix": NOW - 7_200}),
        ),
    ];
    mac_jobs_page(headers, &[(COMPUTER.to_owned(), reported, true)], &jobs)
}

fn claude_key(
    headers: &HeaderMap,
    saved: Option<(crate::cloud::custody::Material, &str)>,
    subscribe: bool,
) -> Response {
    let body = crate::settings::claude_content(saved, subscribe.then_some(TICKET), TICKET, TICKET);
    settings_page(headers, "Claude credential", crate::settings::CLAUDE, body)
}

fn claude_key_none(headers: &HeaderMap) -> Response {
    claude_key(headers, None, false)
}

fn claude_key_saved(headers: &HeaderMap) -> Response {
    claude_key(
        headers,
        Some((
            crate::cloud::custody::Material::AnthropicApiKey,
            "SHA-256 5a5a5a5a5a5a…",
        )),
        false,
    )
}

fn claude_key_subscription(headers: &HeaderMap) -> Response {
    claude_key(headers, None, true)
}

fn mac_job_queued(headers: &HeaderMap) -> Response {
    mac_job_page(headers, &mac_job("waiting", json!({"lines": []})))
}

fn mac_job_asking(headers: &HeaderMap) -> Response {
    let job = mac_job(
        "asking",
        json!({"question": {
            "id": "q-sample-1",
            "text": "Upload build 412 of sample-app to TestFlight?",
            "subject": "testflight"
        }}),
    );
    mac_job_page(headers, &job)
}

fn mac_job_done(headers: &HeaderMap) -> Response {
    let job = mac_job(
        "done",
        json!({
            "commit": "0123456789abcdef0123456789abcdef01234567",
            "summary": "All 48 UI tests passed.",
            "artifacts": [
                {"name": "results.xcresult.zip", "size": 18_400_000, "parts": 3, "done": true},
                {"name": "screens.zip", "size": 2_100_000, "parts": 1, "done": true}
            ],
            "finished_unix": NOW - 120
        }),
    );
    mac_job_page(headers, &job)
}

fn mac_job_stopped(headers: &HeaderMap) -> Response {
    let job = mac_job(
        "cancelled",
        json!({"cancel": true, "why": "You stopped it.", "finished_unix": NOW - 300}),
    );
    mac_job_page(headers, &job)
}

fn mac_job_failed(headers: &HeaderMap) -> Response {
    let job = mac_job(
        "failed",
        json!({"why": "The simulator didn't start. Try again.", "finished_unix": NOW - 300}),
    );
    mac_job_page(headers, &job)
}

// Projects.

fn project() -> oa_auth::repos::Project {
    oa_auth::repos::Project {
        id: "prj_0123456789abcdef".into(),
        name: "sample-app".into(),
        instructions: Some("Answer in plain words.".into()),
        created_unix: NOW - 20 * DAY,
        sharing: None,
        repositories: vec![oa_auth::repos::ProjectRepository {
            repository_id: 101,
            repository: REPO.into(),
            default_branch: "main".into(),
            private: true,
            primary: true,
            installation_id: None,
        }],
    }
}

fn project_page(headers: &HeaderMap, project: oa_auth::repos::Project) -> Response {
    let status = oa_auth::repos::Status {
        access: connected(),
        projects: vec![project.clone()],
    };
    let listed = listing(vec![repository(102, "sample-org/sample-site", false)]);
    let list = crate::projects::repos_page(Ok(listed), CSRF, "", 1, Some(&project));
    let body =
        crate::projects::page::view(&status, &project, CSRF, "", None, false, Some(list), true);
    crate::projects::page_for(headers, StatusCode::OK, body)
}

fn project_no_repository(headers: &HeaderMap) -> Response {
    let mut bare = project();
    bare.repositories.clear();
    project_page(headers, bare)
}

fn project_two_repositories(headers: &HeaderMap) -> Response {
    let mut two = project();
    two.repositories.push(oa_auth::repos::ProjectRepository {
        repository_id: 103,
        repository: "sample-org/sample-docs".into(),
        default_branch: "main".into(),
        private: false,
        primary: false,
        installation_id: None,
    });
    project_page(headers, two)
}

fn repository(id: u64, name: &str, private: bool) -> oa_auth::repos::Repository {
    oa_auth::repos::Repository {
        id,
        full_name: name.into(),
        default_branch: "main".into(),
        private,
        archived: false,
        installation: None,
    }
}

fn connected() -> oa_auth::repos::Access {
    oa_auth::repos::Access::Connected {
        login: "sample-person".into(),
        private: true,
    }
}

fn projects(
    headers: &HeaderMap,
    access: oa_auth::repos::Access,
    projects: Vec<oa_auth::repos::Project>,
    q: &str,
    install: bool,
    listed: Option<Result<oa_auth::repos::Listing, crate::cloud::session::github::RepoCallError>>,
) -> Response {
    let status = oa_auth::repos::Status { access, projects };
    let list = listed.map(|listed| crate::projects::repos_page(listed, CSRF, q, 1, None));
    let body = crate::projects::view_with(&status, CSRF, q, None, install, list, true, None);
    crate::projects::page_for(headers, StatusCode::OK, body)
}

fn listing(repositories: Vec<oa_auth::repos::Repository>) -> oa_auth::repos::Listing {
    oa_auth::repos::Listing {
        repositories,
        more: false,
        sso_hidden: false,
    }
}

fn projects_no_github(headers: &HeaderMap) -> Response {
    projects(
        headers,
        oa_auth::repos::Access::None,
        vec![],
        "",
        false,
        None,
    )
}

fn projects_connected_empty(headers: &HeaderMap) -> Response {
    projects(
        headers,
        connected(),
        vec![],
        "",
        false,
        Some(Ok(listing(vec![]))),
    )
}

fn projects_search(headers: &HeaderMap) -> Response {
    let listed = listing(vec![
        repository(102, "sample-org/sample-site", false),
        repository(103, "sample-org/sample-docs", true),
    ]);
    projects(
        headers,
        connected(),
        vec![],
        "sample",
        false,
        Some(Ok(listed)),
    )
}

fn projects_sources(headers: &HeaderMap) -> Response {
    let listed = listing(vec![repository(102, "sample-org/sample-site", false)]);
    projects(
        headers,
        connected(),
        vec![project()],
        "",
        false,
        Some(Ok(listed)),
    )
}

fn source(id: &str, name: &str, kind: &str) -> crate::cloud::connections::Source {
    crate::cloud::connections::Source {
        project: project().id,
        integration: "google".into(),
        id: id.into(),
        name: name.into(),
        kind: kind.into(),
        added_at: NOW - 2 * DAY,
    }
}

fn drive_file(id: &str, name: &str, mime_type: &str) -> oa_connections::google::drive::File {
    oa_connections::google::drive::File {
        id: id.into(),
        name: name.into(),
        mime_type: mime_type.into(),
        web_view_link: None,
        modified_time: None,
        size: None,
        video_media_metadata: None,
    }
}

fn sources(
    headers: &HeaderMap,
    attached: &[crate::cloud::connections::Source],
    unready: Option<crate::connections::Unready>,
    browse: Option<Result<(Vec<oa_connections::google::drive::File>, Option<String>), String>>,
    q: &str,
) -> Response {
    let project = project();
    let body =
        crate::connections::sources::view(&project, CSRF, attached, unready, browse, q, None);
    settings_page(
        headers,
        &format!("Sources · {}", project.name),
        crate::projects::PAGE,
        body,
    )
}

fn sources_drive(headers: &HeaderMap) -> Response {
    let attached = [source("sample-folder-1", "Sample team notes", "folder")];
    let files = vec![
        drive_file(
            "sample-folder-1",
            "Sample team notes",
            "application/vnd.google-apps.folder",
        ),
        drive_file(
            "sample-folder-2",
            "Sample designs",
            "application/vnd.google-apps.folder",
        ),
        drive_file(
            "sample-doc-1",
            "Sample launch plan",
            "application/vnd.google-apps.document",
        ),
        drive_file("sample-pdf-1", "Sample price list.pdf", "application/pdf"),
    ];
    sources(headers, &attached, None, Some(Ok((files, None))), "")
}

fn sources_search(headers: &HeaderMap) -> Response {
    sources(headers, &[], None, Some(Ok((Vec::new(), None))), "budget")
}

fn sources_connect(headers: &HeaderMap) -> Response {
    sources(
        headers,
        &[],
        Some(crate::connections::Unready::NotConnected),
        None,
        "",
    )
}

fn sources_reconnect(headers: &HeaderMap) -> Response {
    let attached = [source("sample-doc-1", "Sample launch plan", "document")];
    sources(
        headers,
        &attached,
        Some(crate::connections::Unready::Reconnect),
        None,
        "",
    )
}

fn projects_install(headers: &HeaderMap) -> Response {
    projects(
        headers,
        oa_auth::repos::Access::None,
        vec![],
        "",
        true,
        None,
    )
}

fn projects_choose(headers: &HeaderMap) -> Response {
    let access = oa_auth::repos::Access::Installed {
        login: "sample-person".into(),
        installations: vec![],
    };
    projects(headers, access, vec![], "", true, None)
}

fn projects_disconnected(headers: &HeaderMap) -> Response {
    projects(
        headers,
        oa_auth::repos::Access::None,
        vec![project()],
        "",
        false,
        None,
    )
}

fn projects_reconnect(headers: &HeaderMap) -> Response {
    let access = oa_auth::repos::Access::Reconnect {
        login: "sample-person".into(),
    };
    projects(headers, access, vec![project()], "", false, None)
}

fn projects_failed(headers: &HeaderMap) -> Response {
    let failed = Err(crate::cloud::session::github::RepoCallError::Repo(
        oa_auth::repos::RepoError::NotFound,
    ));
    projects(
        headers,
        connected(),
        vec![project()],
        "",
        false,
        Some(failed),
    )
}

// Environments.

fn summary(id: &str, repository: &str, status: Status, saved: Option<u64>) -> Summary {
    Summary {
        id: id.into(),
        repository: repository.into(),
        branch: "main".into(),
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        status,
        saved,
        updated_ms: (NOW - 3_600) * 1_000,
        account: None,
    }
}

fn summaries() -> Vec<Summary> {
    vec![
        summary(ENV, REPO, Status::Saved, Some(NOW * 1_000)),
        summary(
            "env-sample-2",
            "sample-org/sample-site",
            Status::Working,
            None,
        ),
        summary(
            "env-sample-3",
            "sample-org/sample-docs",
            Status::Failed,
            None,
        ),
    ]
}

fn record(entry: Entry) -> Record {
    Record {
        at_ms: (NOW - 600) * 1_000,
        entry,
    }
}

fn started() -> Vec<Record> {
    vec![
        record(Entry::User {
            text: "Set up sample-org/sample-app on main so agents can build and test it.".into(),
        }),
        record(Entry::Starting),
        record(Entry::Source {
            ok: true,
            revision: "0123456789abcdef0123456789abcdef01234567".into(),
            output: String::new(),
        }),
        record(Entry::Agent {
            text: "I'll look at how this builds.".into(),
        }),
        record(Entry::Explored {
            command: "cat README.md".into(),
            exit: Some(0),
            output: "# sample-app\n".into(),
        }),
        record(Entry::Recipe {
            revision: 1,
            script: "make deps".into(),
            previous: None,
        }),
    ]
}

fn finished() -> Vec<Record> {
    let mut records = started();
    records.extend([
        record(Entry::Install {
            revision: 1,
            exit: Some(0),
            output: "done\n".into(),
        }),
        record(Entry::Checks {
            checks: vec![CheckLine {
                name: "build".into(),
                command: "make".into(),
            }],
        }),
        record(Entry::Build {
            stage: Stage::Passed,
            detail: "Image saved".into(),
        }),
        record(Entry::Verify {
            stage: Stage::Passed,
            detail: "1 check passed".into(),
        }),
        record(Entry::Ready {
            summary: "Installs and builds with make.".into(),
        }),
    ]);
    records
}

fn run(state: RunState, events: Vec<serde_json::Value>) -> Run {
    Run {
        id: RUN.into(),
        environment: ENV.into(),
        prompt: "Fix the failing login test.".into(),
        version: Some(1),
        state,
        events,
        reply: None,
        error: None,
        created_ms: (NOW - 900) * 1_000,
        updated_ms: (NOW - 60) * 1_000,
        cost_usd: None,
        paused_until: None,
        openagents: false,
    }
}

fn environment(status: Status, phase: Phase, records: Vec<Record>) -> View {
    let saved = matches!(status, Status::Saved);
    View {
        summary: summary(ENV, REPO, status, saved.then_some(NOW * 1_000)),
        records,
        phase,
        question: None,
        candidate: None,
        versions: if saved {
            vec![Version {
                number: 1,
                selected: true,
                image: "sample-image-1".into(),
                created_ms: (NOW - DAY) * 1_000,
            }]
        } else {
            vec![]
        },
        recipe: Some("make deps".into()),
        runs: vec![],
    }
}

fn saved() -> View {
    let mut view = environment(Status::Saved, Phase::Saved { number: 1 }, finished());
    view.runs = vec![run(RunState::Done, vec![])];
    view
}

fn detail(headers: &HeaderMap, view: &View, claude: Claude) -> Response {
    crate::environments::detail_page(headers, ENV, view, &summaries(), claude, None)
}

fn environments_empty(headers: &HeaderMap) -> Response {
    crate::environments::index_page(headers, &[])
}

fn environments_list(headers: &HeaderMap) -> Response {
    crate::environments::index_page(headers, &summaries())
}

fn environments_new(headers: &HeaderMap) -> Response {
    crate::environments::pick_page(false, headers, "", None)
}

fn environments_new_error(headers: &HeaderMap) -> Response {
    crate::environments::pick_page(
        false,
        headers,
        "sample-app",
        Some("Enter a GitHub repository as owner/name or its github.com address."),
    )
}

fn environments_new_branch(headers: &HeaderMap) -> Response {
    let branches = [
        "main".to_owned(),
        "release".to_owned(),
        "sample-feature".to_owned(),
    ];
    crate::environments::branch_page(headers, REPO, &branches, "main", None)
}

fn environment_idle(headers: &HeaderMap) -> Response {
    detail(headers, &saved(), Claude::Ready)
}

fn environment_running(headers: &HeaderMap) -> Response {
    let view = environment(Status::Working, Phase::Working, started());
    detail(headers, &view, Claude::Unavailable)
}

fn environment_question(headers: &HeaderMap) -> Response {
    let question = "May I add libsample to the install?".to_owned();
    let mut records = started();
    records.push(record(Entry::Question {
        text: question.clone(),
    }));
    let mut view = environment(
        Status::NeedsInput,
        Phase::Waiting {
            question: question.clone(),
        },
        records,
    );
    view.question = Some(question);
    detail(headers, &view, Claude::Unavailable)
}

fn environment_review(headers: &HeaderMap) -> Response {
    let mut view = environment(
        Status::ReadyToSave,
        Phase::Review {
            verification: "ver-sample-1".into(),
        },
        finished(),
    );
    view.candidate = Some(Candidate {
        digest: "c".repeat(64),
        recipe_revision: 1,
        commit: "0123456789abcdef0123456789abcdef01234567".into(),
        image: "sample-image-2".into(),
        checks: vec![CheckLine {
            name: "build".into(),
            command: "make".into(),
        }],
        summary: "Installs and builds with make.".into(),
    });
    detail(headers, &view, Claude::Unavailable)
}

fn environment_needs_sign_in(headers: &HeaderMap) -> Response {
    detail(headers, &saved(), Claude::AddKey)
}

fn environment_failed(headers: &HeaderMap) -> Response {
    let mut records = started();
    records.push(record(Entry::Install {
        revision: 1,
        exit: Some(2),
        output: "make: *** No rule to make target 'deps'.\n".into(),
    }));
    let view = environment(
        Status::Failed,
        Phase::Failed {
            reason: "The install didn't finish.".into(),
        },
        records,
    );
    detail(headers, &view, Claude::Unavailable)
}

fn run_page(headers: &HeaderMap, run: &Run, waiting: &[String]) -> Response {
    let offered = if run.state == RunState::Paused {
        vec![coder_cloud::claude_task::PauseChoice::KeepWaiting]
    } else {
        vec![]
    };
    crate::environments::run_view_page(
        headers,
        ENV,
        RUN,
        &saved(),
        run,
        &summaries(),
        &offered,
        waiting,
        None,
    )
}

fn run_starting(headers: &HeaderMap) -> Response {
    run_page(headers, &run(RunState::Starting, vec![]), &[])
}

fn run_events() -> Vec<serde_json::Value> {
    vec![
        json!({"event": "message", "text": "I'll find the failing test first."}),
        json!({"event": "message", "text": "The login test waits for a button that moved."}),
    ]
}

fn run_running(headers: &HeaderMap) -> Response {
    run_page(
        headers,
        &run(RunState::Running, run_events()),
        &["Also add a test for the logout button.".into()],
    )
}

fn run_paused(headers: &HeaderMap) -> Response {
    let mut paused = run(RunState::Paused, run_events());
    paused.paused_until = Some((NOW + 2 * 3_600) * 1_000);
    run_page(headers, &paused, &[])
}

fn run_done(headers: &HeaderMap) -> Response {
    let mut done = run(RunState::Done, run_events());
    done.reply = Some("Fixed: the test now waits for the moved button.".into());
    done.cost_usd = Some(0.42);
    run_page(headers, &done, &[])
}

fn run_stopped(headers: &HeaderMap) -> Response {
    run_page(headers, &run(RunState::Stopped, run_events()), &[])
}

// Claude Code.

fn claude_run(headers: &HeaderMap) -> Response {
    detail(headers, &saved(), Claude::Ready)
}

fn claude_run_signed_in(headers: &HeaderMap) -> Response {
    detail(headers, &saved(), Claude::SignedIn)
}

fn sign_in_view(phase: sign_in::Phase) -> sign_in::View {
    sign_in::View {
        phase,
        screen: "Welcome to Claude Code\n\nBrowser didn't open? Use the url below to sign in:\n\nhttps://claude.example/sample-sign-in\n\nPaste code here if prompted >".into(),
        link: Some("https://claude.example/sample-sign-in".into()),
    }
}

fn claude_page(headers: &HeaderMap, panel: Markup) -> Response {
    crate::environments::sign_in_page_for(headers, ENV, REPO.into(), panel)
}

fn claude_sign_in(headers: &HeaderMap) -> Response {
    claude_page(
        headers,
        crate::environments::view::sign_in_panel(ENV, None, None),
    )
}

fn claude_with(headers: &HeaderMap, phase: sign_in::Phase) -> Response {
    let view = sign_in_view(phase);
    claude_page(
        headers,
        crate::environments::view::sign_in_panel(ENV, Some(&view), None),
    )
}

fn claude_sign_in_starting(headers: &HeaderMap) -> Response {
    let mut view = sign_in_view(sign_in::Phase::Starting);
    view.screen = String::new();
    view.link = None;
    claude_page(
        headers,
        crate::environments::view::sign_in_panel(ENV, Some(&view), None),
    )
}

fn claude_sign_in_open(headers: &HeaderMap) -> Response {
    claude_with(headers, sign_in::Phase::Open)
}

fn claude_sign_in_input(headers: &HeaderMap) -> Response {
    let view = sign_in_view(sign_in::Phase::Open);
    let panel = html! {
        div #claude-sign-in.oa-sign-in {
            (crate::environments::view::sign_in_screen(ENV, Some(&view), None))
            (crate::environments::view::sign_in_input(ENV, None, Some("Type the code Anthropic shows, then Send.")))
        }
    };
    claude_page(headers, panel)
}

fn claude_signed_in(headers: &HeaderMap) -> Response {
    claude_with(headers, sign_in::Phase::SignedIn)
}

fn claude_sign_in_failed(headers: &HeaderMap) -> Response {
    claude_with(
        headers,
        sign_in::Phase::Failed("The login closed before it finished.".into()),
    )
}

// Purchases.

fn purchase(id: &str, phase: coder::customer::plugins::Phase) -> coder::customer::plugins::Summary {
    use coder::customer::plugins::Phase;
    let paid = matches!(phase, Phase::Paid | Phase::Completed);
    coder::customer::plugins::Summary {
        id: id.into(),
        phase,
        account: "acct-sample".into(),
        workspace: "ws-sample".into(),
        plugin: Some("sample-weather".into()),
        release: Some("1.2.0".into()),
        url: "https://plugins.example/sample-weather".into(),
        quote_digest: "a".repeat(64),
        approval_digest: "b".repeat(64),
        price_msat: 21_000,
        max_fee_msat: 1_000,
        payer_node: "sample-wallet".into(),
        payer_network: "bitcoin".into(),
        created_at_ms: (NOW - DAY) * 1_000,
        expires_at_ms: (NOW + DAY) * 1_000,
        charge: paid.then(|| coder::customer::plugins::Charge {
            payment_hash: "d".repeat(64),
            amount_msat: 21_000,
            fee_msat: 12,
        }),
        settled: paid.then_some(true),
        transaction: paid.then(|| "tx-sample-1".to_owned()),
        delivery_status: matches!(phase, Phase::Completed).then_some(200),
        result_present: matches!(phase, Phase::Completed),
        unresolved_maximum_msat: None,
        recovery_present: false,
    }
}

fn purchases_none(headers: &HeaderMap) -> Response {
    crate::purchases::list_page(headers, &[])
}

fn purchases_list(headers: &HeaderMap) -> Response {
    use coder::customer::plugins::Phase;
    crate::purchases::list_page(
        headers,
        &[
            purchase("pur-sample-1", Phase::Completed),
            purchase("pur-sample-2", Phase::Paying),
            purchase("pur-sample-3", Phase::Failed),
        ],
    )
}

fn purchase_paid(headers: &HeaderMap) -> Response {
    let item = purchase("pur-sample-1", coder::customer::plugins::Phase::Completed);
    crate::purchases::one_page(headers, &item)
}

fn purchase_pending(headers: &HeaderMap) -> Response {
    let item = purchase("pur-sample-2", coder::customer::plugins::Phase::Paying);
    crate::purchases::one_page(headers, &item)
}

fn purchase_failed(headers: &HeaderMap) -> Response {
    let item = purchase("pur-sample-3", coder::customer::plugins::Phase::Failed);
    crate::purchases::one_page(headers, &item)
}

// Tasks.

/// A sample task, read back from a throwaway store.
fn task_view() -> coder::task::view::View {
    static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);
    let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
    let root = std::env::temp_dir().join(format!("oa-screens-task-{}-{n}", std::process::id()));
    let store = root.join("tasks");
    let command = json!({
        "schema": coder::task::COMMAND_SCHEMA,
        "command_id": "sample-submit-1",
        "task_id": "sample-task-1",
        "expected_revision": null,
        "action": {"type": "submit", "intent": {
            "title": "Fix the failing login test",
            "prompt": "The login test in sample-app fails since the button moved. Fix it.",
            "workspace": {"path": "/workspace/sample-app", "source_revision": null},
            "configuration": {"adapter": "coder", "model": null}
        }}
    });
    coder::task::Store::open(&store)
        .unwrap()
        .apply(command.to_string().as_bytes())
        .unwrap();
    let view = coder::task::view::read(&store, "sample-task-1", None, 50).unwrap();
    let _ = std::fs::remove_dir_all(&root);
    view
}

fn task_list_empty(headers: &HeaderMap) -> Response {
    crate::tasks::list_page(headers, &[])
}

fn task_list(headers: &HeaderMap) -> Response {
    let mut view = task_view();
    view.task.status = coder::task::Status::Running;
    view.task.execution = coder::task::Execution::Running;
    crate::tasks::list_page(headers, &[view.task])
}

fn task(
    headers: &HeaderMap,
    status: coder::task::Status,
    execution: coder::task::Execution,
) -> Response {
    let mut view = task_view();
    view.task.status = status;
    view.task.execution = execution;
    crate::tasks::task_page(headers, &view)
}

fn task_queued(headers: &HeaderMap) -> Response {
    task(
        headers,
        coder::task::Status::Queued,
        coder::task::Execution::NotStarted,
    )
}

fn task_running(headers: &HeaderMap) -> Response {
    task(
        headers,
        coder::task::Status::Running,
        coder::task::Execution::Running,
    )
}

fn task_cancel_requested(headers: &HeaderMap) -> Response {
    task(
        headers,
        coder::task::Status::CancelRequested,
        coder::task::Execution::Running,
    )
}

fn task_cancelled(headers: &HeaderMap) -> Response {
    task(
        headers,
        coder::task::Status::Cancelled,
        coder::task::Execution::Stopped,
    )
}

fn task_finished(headers: &HeaderMap) -> Response {
    task(
        headers,
        coder::task::Status::Finished,
        coder::task::Execution::Finished,
    )
}

fn task_unknown(headers: &HeaderMap) -> Response {
    task(
        headers,
        coder::task::Status::Unknown,
        coder::task::Execution::Unknown,
    )
}

// Agents.

fn agent(id: &str, title: &str, status: &str) -> crate::phone_api::Item {
    crate::phone_api::Item {
        id: id.into(),
        kind: "agent".into(),
        title: title.into(),
        engine: Some("claude".into()),
        status: status.into(),
        started_unix: NOW - 1_800,
        finished_unix: None,
        cost_usd: Some(0.42),
        tokens: Some(18_000),
        session: None,
        question: (status == "asking").then(|| crate::phone_api::Question {
            id: "q-sample-1".into(),
            text: "May I delete the old sample fixtures?".into(),
        }),
        line: Some("Running the tests again".into()),
        choices: vec![],
    }
}

fn machines() -> Vec<crate::agents_page::Machine> {
    vec![
        crate::agents_page::Machine {
            name: COMPUTER.into(),
            online: true,
            updated_unix: NOW - 30,
            agents: vec![
                agent("item-sample-1", "Fix the failing login test", "working"),
                agent("item-sample-2", "Tidy the sample fixtures", "asking"),
            ],
        },
        crate::agents_page::Machine {
            name: "Sample Studio Mac".into(),
            online: false,
            updated_unix: NOW - 3 * 3_600,
            agents: vec![],
        },
    ]
}

fn agents(headers: &HeaderMap, machines: &[crate::agents_page::Machine], said: &str) -> Response {
    let notice = crate::agents_page::notice_words(said);
    let body = crate::agents_page::list_markup(machines, CSRF, NOW, notice);
    settings_page(headers, "Agents", crate::agents_page::PAGE, body)
}

fn agents_none(headers: &HeaderMap) -> Response {
    agents(headers, &[], "")
}

fn agents_machines(headers: &HeaderMap) -> Response {
    agents(headers, &machines(), "")
}

fn agents_stopping(headers: &HeaderMap) -> Response {
    agents(headers, &machines(), "stopping")
}

fn agents_message(headers: &HeaderMap) -> Response {
    let item = agent("item-sample-1", "Fix the failing login test", "working");
    let body = crate::agents_page::message_markup(&item, COMPUTER, CSRF);
    settings_page(headers, "Message an agent", crate::agents_page::PAGE, body)
}

// Profiles.

fn profile(headers: &HeaderMap, login: &str, name: Option<&str>) -> Response {
    let profile = crate::backend::Profile {
        login: login.into(),
        name: name.map(str::to_owned),
        joined: "September 2026".into(),
    };
    crate::pages::profile::profile_page(headers, format!("/u/{login}"), &profile)
}

fn profile_own(headers: &HeaderMap) -> Response {
    profile(headers, "christopher-sample", Some(NAME))
}

fn profile_other(headers: &HeaderMap) -> Response {
    profile(headers, "another-sample", None)
}

fn profile_missing(headers: &HeaderMap) -> Response {
    crate::pages::profile::missing(headers)
}

// Analytics.

fn drive_access(headers: &HeaderMap) -> Response {
    let access = crate::cloud::connections::Access::seeded();
    settings_page(
        headers,
        "Google Drive access",
        crate::connections::access::PAGE,
        crate::connections::access::view(&access, CSRF),
    )
}

fn analytics(headers: &HeaderMap) -> Response {
    use crate::analytics::{Counts, Key, Series};
    let hour = NOW / 3_600;
    let mut counts = Counts::new();
    let rows: [(u64, Series, &str, &str, u64); 9] = [
        (0, Series::View, "/", "human", 120),
        (0, Series::View, "/", "agent", 31),
        (1, Series::View, "/chat/{id}", "human", 64),
        (2, Series::View, "/settings", "human", 12),
        (0, Series::Event, "chat_sent", "new", 18),
        (1, Series::Event, "answer_shown", "", 15),
        (3, Series::Event, "chat_sent", "follow_up", 9),
        (0, Series::Status, "/chat/{id}", "500", 1),
        (0, Series::Latency, "/", ">3s", 2),
    ];
    for (ago, series, name, detail, count) in rows {
        counts.insert(
            Key {
                hour: hour - ago,
                series,
                name: name.into(),
                detail: detail.into(),
            },
            count,
        );
    }
    crate::analytics::dashboard::report_page(headers, &counts, hour, true)
}

#[cfg(test)]
mod tests {
    use axum::body::to_bytes;
    use axum::http::HeaderMap;

    async fn text(render: super::super::Render) -> String {
        let body = to_bytes(render(&HeaderMap::new()).into_body(), usize::MAX)
            .await
            .unwrap();
        oa_copy::visible_text(&String::from_utf8(body.to_vec()).unwrap())
    }

    #[tokio::test]
    async fn the_samples_show_what_each_state_is_about() {
        for (render, needle) in [
            (
                super::mac_job_asking as super::super::Render,
                "waiting for your approval",
            ),
            (super::mac_job_done, "results.xcresult.zip"),
            (super::projects_search, "sample-org/sample-docs"),
            (super::sign_in_keys_some, "Sample laptop"),
            (super::agents_stopping, "Asked it to stop."),
            (super::claude_run, "Run Claude Code"),
            (super::task_finished, "Fix the failing login test"),
            (super::profile_own, "Christopher D."),
        ] {
            let text = text(render).await;
            assert!(text.contains(needle), "{needle}: {text}");
        }
    }
}

// The vault. Its page holds every step hidden until its script shows one;
// a sample shows the step by hand, with no script.

fn vault(headers: &HeaderMap, project: Option<(&str, &str)>, step: Option<&str>) -> Response {
    let body =
        crate::vault::page::content(CSRF, project, Some(("sample", "sample")), NAME).into_string();
    let body = match step {
        Some(step) => body
            .replace(&format!("id=\"{step}\" hidden"), &format!("id=\"{step}\""))
            .replacen(
                &format!(
                    "id=\"{step}\" class=\"oa-vault-step\" aria-labelledby=\"{step}-title\" hidden"
                ),
                &format!("id=\"{step}\" class=\"oa-vault-step\" aria-labelledby=\"{step}-title\""),
                1,
            )
            .replacen(
                &format!("id=\"{step}\" aria-label=\"Your vault\" hidden"),
                &format!("id=\"{step}\" aria-label=\"Your vault\""),
                1,
            )
            .replacen("Opening your vault…", "", 1),
        None => body,
    };
    let (title, path) = match project {
        Some((id, name)) => (format!("Vault · {name}"), crate::vault::project_href(id)),
        None => ("Vault (only you)".to_owned(), crate::vault::PAGE.to_owned()),
    };
    crate::cloud::protect(
        crate::ui_page::UiPage::new(title)
            .path(path)
            .section(crate::settings::PAGE)
            .account(signed_in())
            .scriptless()
            .head(html! { link rel="stylesheet" href=(crate::vault::STYLE); })
            .content(openagents_ui::content::PageColumn::new(maud::PreEscaped(
                body,
            )))
            .respond(headers),
    )
}

fn vault_opening(headers: &HeaderMap) -> Response {
    vault(headers, None, None)
}

fn vault_set_up(headers: &HeaderMap) -> Response {
    vault(headers, None, Some("vault-new"))
}

fn vault_locked(headers: &HeaderMap) -> Response {
    vault(headers, None, Some("vault-locked"))
}

fn vault_open(headers: &HeaderMap) -> Response {
    vault(headers, None, Some("vault-open"))
}

fn vault_project(headers: &HeaderMap) -> Response {
    let project = project();
    vault(
        headers,
        Some((&project.id, &project.name)),
        Some("vault-open"),
    )
}
