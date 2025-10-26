//! Filesystem watchers and Convex sync routines for Projects, Skills, and Sessions.
//!
//! This module encapsulates notify-based watchers and Convex sync functions so
//! the bridge can mirror local FS changes into Convex and keep UIs current.

use std::path::{Path, PathBuf};
use std::sync::Arc;

use notify::{EventKind, RecommendedWatcher, RecursiveMode, Watcher};
use tracing::{error, info, warn};

use crate::convex_write::stream_upsert_or_append;
use crate::state::AppState;
use crate::util::now_ms;

/// Full one-shot sync of Projects from disk into Convex.
///
/// - Reads `~/.openagents/projects/**/PROJECT.md` (and legacy single-file variants)
/// - Validates frontmatter, maps fields, and upserts rows via `projects:upsertFromFs`.
/// - Returns the number of successful upserts.
pub async fn sync_projects_to_convex(state: Arc<AppState>) -> anyhow::Result<usize> {
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let url = format!("http://127.0.0.1:{}", state.opts.convex_port);
    let mut client = ConvexClient::new(&url).await?;
    let items = crate::projects::list_projects().unwrap_or_default();
    let mut ok = 0usize;
    for p in items.iter() {
        let now = now_ms() as f64;
        let mut args: BTreeMap<String, Value> = BTreeMap::new();
        args.insert("id".into(), Value::from(p.id.clone()));
        args.insert("name".into(), Value::from(p.name.clone()));
        args.insert("workingDir".into(), Value::from(p.working_dir.clone()));
        if let Some(repo) = &p.repo {
            let mut robj: BTreeMap<String, Value> = BTreeMap::new();
            if let Some(v) = &repo.provider { robj.insert("provider".into(), Value::from(v.clone())); }
            if let Some(v) = &repo.remote { robj.insert("remote".into(), Value::from(v.clone())); }
            if let Some(v) = &repo.url { robj.insert("url".into(), Value::from(v.clone())); }
            if let Some(v) = &repo.branch { robj.insert("branch".into(), Value::from(v.clone())); }
            args.insert("repo".into(), Value::Object(robj));
        }
        if let Some(v) = &p.agent_file { args.insert("agentFile".into(), Value::from(v.clone())); }
        if let Some(v) = &p.instructions { args.insert("instructions".into(), Value::from(v.clone())); }
        args.insert("createdAt".into(), Value::from(now));
        args.insert("updatedAt".into(), Value::from(now));
        match client.mutation("projects:upsertFromFs", args).await {
            Ok(_) => ok += 1,
            Err(e) => warn!(?e, id=%p.id, "projects:upsertFromFs failed"),
        }
    }
    Ok(ok)
}

/// Full one-shot sync of Skills from disk into Convex.
///
/// Sources:
/// - User skills under `~/.openagents/skills/**/SKILL.md`
/// - Registry skills under `<repo>/skills/**/SKILL.md`
pub async fn sync_skills_to_convex(state: Arc<AppState>) -> anyhow::Result<usize> {
    use convex::{ConvexClient, Value};
    use std::collections::BTreeMap;
    let url = format!("http://127.0.0.1:{}", state.opts.convex_port);
    let mut client = ConvexClient::new(&url).await?;
    let items = crate::skills::list_skills().unwrap_or_default();
    let mut ok = 0usize;
    for s in items.iter() {
        let mut args: BTreeMap<String, Value> = BTreeMap::new();
        args.insert("id".into(), Value::from(s.id.clone()));
        args.insert("name".into(), Value::from(s.name.clone()));
        args.insert("description".into(), Value::from(s.description.clone()));
        if let Some(v) = s.source.as_deref() { args.insert("source".into(), Value::from(v)); }
        if let Some(v) = s.meta.license.as_deref() { args.insert("license".into(), Value::from(v)); }
        if let Some(v) = s.meta.allowed_tools.as_ref() {
            let arr: Vec<Value> = v.iter().cloned().map(Value::from).collect();
            args.insert("allowedTools".into(), Value::from(arr));
        }
        args.insert("createdAt".into(), Value::from(now_ms() as f64));
        args.insert("updatedAt".into(), Value::from(now_ms() as f64));
        match client.mutation("skills:upsertFromFs", args).await {
            Ok(_) => ok += 1,
            Err(e) => warn!(?e, id=%s.id, "skills:upsertFromFs failed"),
        }
    }
    Ok(ok)
}

/// Placeholder for project-scoped skills sync (e.g., `<project>/skills`).
/// Currently unused; kept for future expansion.
pub async fn sync_project_scoped_skills(_state: Arc<AppState>, _client: &mut convex::ConvexClient) -> anyhow::Result<()> {
    Ok(())
}

/// Watch the Projects directory and trigger a best-effort sync on changes.
///
/// Debounces bursts by draining a few queued events each iteration.
pub async fn watch_projects_and_sync(state: Arc<AppState>) {
    let proj_dir = crate::projects::projects_dir();
    if let Err(e) = std::fs::create_dir_all(&proj_dir) { error!(?e, "projects mkdir failed"); }
    if !proj_dir.is_dir() { return; }
    let (txev, rcev) = std::sync::mpsc::channel();
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| { let _ = txev.send(res); }) { Ok(w) => w, Err(e) => { error!(?e, "projects watcher create failed"); return; } };
    if let Err(e) = watcher.watch(&proj_dir, RecursiveMode::Recursive) { error!(dir=%proj_dir.display(), ?e, "projects watcher watch failed"); return; }
    info!(dir=%proj_dir.display(), msg="projects watcher started");
    loop { match rcev.recv() { Ok(_evt) => { let _ = rcev.try_recv(); let _ = rcev.try_recv(); if let Err(e) = sync_projects_to_convex(state.clone()).await { warn!(?e, "projects convex sync failed on change"); } }, Err(_disconnected) => break } }
}

/// Watch user and registry Skills directories and broadcast a fresh skills list
/// to connected clients. Also mirrors to Convex on change.
pub async fn watch_skills_and_broadcast(state: Arc<AppState>) {
    let user_dir = crate::skills::skills_dir();
    let registry_dirs = crate::skills::registry_skills_dirs();
    let mut watched: Vec<PathBuf> = Vec::new();
    if let Err(e) = std::fs::create_dir_all(&user_dir) { error!(?e, "skills mkdir failed"); }
    if user_dir.is_dir() { watched.push(user_dir.clone()); }
    for d in registry_dirs { if d.is_dir() { watched.push(d); } }
    if watched.is_empty() { return; }
    let (txev, rcev) = std::sync::mpsc::channel();
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| { let _ = txev.send(res); }) { Ok(w) => w, Err(e) => { error!(?e, "skills watcher create failed"); return; } };
    for d in &watched { if let Err(e) = watcher.watch(d, RecursiveMode::Recursive) { error!(dir=%d.display(), ?e, "skills watcher watch failed"); } else { info!(dir=%d.display(), msg="skills watcher started"); } }
    loop { match rcev.recv() { Ok(_evt) => { let _ = rcev.try_recv(); let _ = rcev.try_recv(); match crate::skills::list_skills() { Ok(items) => { let line = serde_json::json!({"type":"bridge.skills","items": items}).to_string(); let _ = state.tx.send(line); if let Err(e) = sync_skills_to_convex(state.clone()).await { warn!(?e, "skills convex sync failed on change"); } }, Err(e) => { error!(?e, "skills list failed on change"); } } }, Err(_disconnected) => break } }
}

fn sessions_base_dir() -> String {
    std::env::var("CODEXD_HISTORY_DIR").ok().unwrap_or_else(|| std::env::var("HOME").map(|h| format!("{}/.codex/sessions", h)).unwrap_or_else(|_| ".".into()))
}

pub async fn enqueue_historical_on_start(state: Arc<AppState>) {
    let base = sessions_base_dir();
    let base_path_owned = PathBuf::from(&base);
    let base_path = base_path_owned.as_path();
    let initial = match crate::history::scan_history(base_path, 10) { Ok(v) => v, Err(e) => { warn!(?e, "initial history scan failed"); Vec::new() } };
    let mut ok = 0usize;
    for h in &initial { if let Err(e) = enqueue_single_thread(&state, h).await { warn!(?e, id=%h.id, "enqueue thread failed") } else { ok += 1; } }
    info!(count = ok, base=%base, msg="initial history import queued");
    let state2 = state.clone();
    let base_path2 = base_path_owned.clone();
    tokio::spawn(async move {
        let rest = match crate::history::scan_history(base_path2.as_path(), 2000) { Ok(mut all) => { if all.len() > 10 { all.drain(0..10); all } else { Vec::new() } }, Err(_e) => Vec::new(), };
        let mut cnt = 0usize;
        for h in rest { let _ = enqueue_single_thread(&state2, &h).await.map(|_| { cnt += 1; }); tokio::time::sleep(std::time::Duration::from_millis(5)).await; }
        if cnt > 0 { info!(count = cnt, msg = "import remaining history"); }
    });
}

/// Watch the Codex sessions directory (`~/.codex/sessions`) and tail JSONL files
/// to mirror assistant/reason text into Convex so external runs appear live.
pub async fn watch_sessions_and_tail(state: Arc<AppState>) {
    let base = PathBuf::from(sessions_base_dir());
    if !base.is_dir() { return; }
    let (txev, rcev) = std::sync::mpsc::channel();
    let mut watcher: RecommendedWatcher = match notify::recommended_watcher(move |res: notify::Result<notify::Event>| { let _ = txev.send(res); }) { Ok(w) => w, Err(e) => { error!(?e, "sessions watcher create failed"); return; } };
    if let Err(e) = watcher.watch(&base, RecursiveMode::Recursive) { error!(dir=%base.display(), ?e, "sessions watcher watch failed"); return; }
    info!(dir=%base.display(), msg="sessions watcher started");
    loop { match rcev.recv() { Ok(Ok(evt)) => { let is_change = matches!(evt.kind, EventKind::Modify(_) | EventKind::Create(_)); if !is_change { continue; } for path in evt.paths.into_iter() { if path.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; } if let Err(e) = mirror_session_tail_to_convex(state.clone(), &path).await { warn!(?e, "sessions mirror failed"); } } }, Ok(Err(e)) => { warn!(?e, "sessions watcher event error"); }, Err(_disconnected) => break } }
}

/// Enqueue a single historical thread (by parsed HistoryItem) into Convex.
async fn enqueue_single_thread(state: &Arc<AppState>, h: &crate::history::HistoryItem) -> anyhow::Result<()> {
    let path = Path::new(&h.path);
    let th = crate::history::parse_thread(path)?;
    let resume_id = th.resume_id.clone().unwrap_or(h.id.clone());
    let title = th.title.clone();
    let started_ms = th.started_ts.map(|t| t * 1000).unwrap_or_else(now_ms);
    use convex::{ConvexClient, Value}; use std::collections::BTreeMap;
    let url = format!("http://127.0.0.1:{}", state.opts.convex_port);
    let mut client = ConvexClient::new(&url).await?;
    let mut targs: BTreeMap<String, Value> = BTreeMap::new();
    targs.insert("threadId".into(), Value::from(resume_id.clone()));
    targs.insert("resumeId".into(), Value::from(resume_id.clone()));
    targs.insert("title".into(), Value::from(title.clone()));
    targs.insert("createdAt".into(), Value::from(started_ms as f64));
    targs.insert("updatedAt".into(), Value::from(started_ms as f64));
    let _ = client.mutation("threads:upsertFromStream", targs).await;
    for it in th.items {
        if it.kind == "message" {
            let role = it.role.as_deref().unwrap_or("assistant");
            let text = it.text;
            let mut margs: BTreeMap<String, Value> = BTreeMap::new();
            margs.insert("threadId".into(), Value::from(resume_id.clone()));
            margs.insert("role".into(), Value::from(role));
            margs.insert("kind".into(), Value::from("message"));
            margs.insert("text".into(), Value::from(text));
            margs.insert("ts".into(), Value::from(now_ms() as f64));
            let _ = client.mutation("messages:create", margs).await;
        }
    }
    Ok(())
}

/// Parse a session JSONL file and upsert the latest assistant/reason text for streaming parity.
async fn mirror_session_tail_to_convex(state: Arc<AppState>, path: &Path) -> anyhow::Result<()> {
    let th = match crate::history::parse_thread(path) { Ok(v) => v, Err(_) => return Ok(()) };
    if let Some(resume_id) = th.resume_id.clone() {
        use convex::{ConvexClient, Value}; use std::collections::BTreeMap;
        let url = format!("http://127.0.0.1:{}", state.opts.convex_port);
        if let Ok(mut client) = ConvexClient::new(&url).await {
            let mut targs: BTreeMap<String, Value> = BTreeMap::new();
            targs.insert("threadId".into(), Value::from(resume_id.clone()));
            targs.insert("resumeId".into(), Value::from(resume_id.clone()));
            targs.insert("title".into(), Value::from(th.title.clone()));
            targs.insert("createdAt".into(), Value::from(now_ms() as f64));
            targs.insert("updatedAt".into(), Value::from(now_ms() as f64));
            let _ = client.mutation("threads:upsertFromStream", targs).await;
        }
        let mut last_assistant: Option<String> = None;
        let mut last_reason: Option<String> = None;
        for it in th.items.iter() { if it.kind == "message" && it.role.as_deref() == Some("assistant") { last_assistant = Some(it.text.clone()); } if it.kind == "reason" { last_reason = Some(it.text.clone()); } }
        if let Some(txt) = last_assistant { stream_upsert_or_append(&state, &resume_id, "assistant", &txt).await; }
        if let Some(txt) = last_reason { stream_upsert_or_append(&state, &resume_id, "reason", &txt).await; }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picks_env_override_for_sessions_dir() {
        unsafe { std::env::set_var("CODEXD_HISTORY_DIR", "/tmp/sessions_override") };
        assert_eq!(sessions_base_dir(), "/tmp/sessions_override");
        unsafe { std::env::remove_var("CODEXD_HISTORY_DIR") };
    }

    #[test]
    fn falls_back_to_home_sessions_dir() {
        // Ensure env override not set
        unsafe { std::env::remove_var("CODEXD_HISTORY_DIR") };
        let home = std::env::var("HOME").unwrap_or_else(|_| ".".into());
        let expect = if home == "." { ".".to_string() } else { format!("{home}/.codex/sessions") };
        assert_eq!(sessions_base_dir(), expect);
    }
}
