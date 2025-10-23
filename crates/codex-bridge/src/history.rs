use anyhow::*;
use serde::Serialize;
use serde_json::Value as JsonValue;
use std::{
    fs,
    io::{BufRead, BufReader},
    path::Path,
    time::SystemTime,
};
use std::result::Result as StdResult;
use tracing::info;

#[derive(Debug, Serialize, Clone)]
pub struct HistoryItem {
    pub id: String,
    pub path: String,
    pub mtime: u64,
    pub title: String,
    pub snippet: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub has_instructions: Option<bool>,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThreadItem {
    pub ts: u64,
    pub kind: String, // message | reason | cmd
    #[serde(skip_serializing_if = "Option::is_none")]
    pub role: Option<String>, // assistant | user
    pub text: String,
}

#[derive(Debug, Serialize, Clone)]
pub struct ThreadResponse {
    pub title: String,
    pub items: Vec<ThreadItem>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub instructions: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resume_id: Option<String>,
}

// HTTP handlers removed: the bridge now serves history and thread content
// exclusively via WebSocket control messages.

pub fn scan_history(base: &Path, limit: usize) -> Result<Vec<HistoryItem>> {
    let mut items: Vec<HistoryItem> = vec![];
    let mut stack = vec![base.to_path_buf()];
    let mut scanned_dirs: usize = 0;
    let mut scanned_files: usize = 0;
    let mut scanned_jsonl: usize = 0;
    let mut skipped_old: usize = 0;
    while let Some(dir) = stack.pop() {
        scanned_dirs += 1;
        if let StdResult::Ok(rd) = fs::read_dir(&dir) {
            for ent in rd.flatten() {
                let p = ent.path();
                scanned_files += 1;
                if p.is_dir() { stack.push(p); continue; }
                if p.extension().and_then(|e| e.to_str()) != Some("jsonl") { continue; }
                scanned_jsonl += 1;
                if !file_is_new_format(&p) { skipped_old += 1; continue; }
                let md = match ent.metadata() { StdResult::Ok(m) => m, StdResult::Err(_) => continue };
                let mtime = md
                    .modified()
                    .ok()
                    .and_then(|t| t.duration_since(SystemTime::UNIX_EPOCH).ok())
                    .map(|d| d.as_secs())
                    .unwrap_or(0);
                let (title, snippet, has_instr) = extract_title_and_snippet_ext(&p).unwrap_or_else(|| ("Session".into(), "".into(), false));
                let id = p.file_name().and_then(|n| n.to_str()).unwrap_or("").to_string();
                items.push(HistoryItem { id, path: p.to_string_lossy().to_string(), mtime, title, snippet, has_instructions: Some(has_instr) });
            }
        }
    }
    items.sort_by(|a, b| b.mtime.cmp(&a.mtime));
    items.truncate(limit);
    info!(
        base=%base.display(),
        scanned_dirs,
        scanned_files,
        scanned_jsonl,
        skipped_old,
        returned=items.len(),
        limit,
        msg="history scan summary"
    );
    Ok(items)
}

fn file_is_new_format(p: &Path) -> bool {
    if let StdResult::Ok(f) = fs::File::open(p) {
        let r = BufReader::new(f);
        for line in r.lines().filter_map(Result::ok).take(50) {
            if let StdResult::Ok(v) = serde_json::from_str::<JsonValue>(&line) {
                if v.get("type").and_then(|x| x.as_str()).is_some() {
                    return true;
                }
            }
        }
    }
    false
}

fn extract_title_and_snippet_ext(p: &Path) -> Option<(String, String, bool)> {
    let mut last_assistant: Option<String> = None;
    let mut last_user: Option<String> = None;
    let mut last_reasoning: Option<String> = None;
    let mut has_instructions = false;
    let f = fs::File::open(p).ok()?;
    let r = BufReader::new(f);
    for line in r.lines().filter_map(Result::ok) {
        if let StdResult::Ok(v) = serde_json::from_str::<JsonValue>(&line) {
            if v.get("type").and_then(|x| x.as_str()) == Some("session_meta") {
                if v.get("payload").and_then(|m| m.get("instructions")).and_then(|x| x.as_str()).is_some() {
                    has_instructions = true;
                }
            }
            if v.get("type").and_then(|x| x.as_str()) == Some("item.completed") {
                if let Some(item) = v.get("item") {
                    let kind = item.get("type").and_then(|x| x.as_str());
                    if kind == Some("agent_message") {
                        if let Some(text) = item.get("text").and_then(|x| x.as_str()) {
                            last_assistant = Some(text.to_string());
                        }
                    } else if kind == Some("user_message") {
                        if let Some(text) = item.get("text").and_then(|x| x.as_str()) {
                            last_user = Some(text.to_string());
                        }
                    } else if kind == Some("reasoning") {
                        if let Some(text) = item.get("text").and_then(|x| x.as_str()) {
                            last_reasoning = Some(text.to_string());
                        }
                    }
                }
            }
        }
    }
    let snippet = last_assistant.clone()
        .or(last_user.clone())
        .or(last_reasoning.clone())
        .unwrap_or_else(|| "(no messages)".into());
    // Prefer a more semantic title source before falling back
    let title_source = last_assistant.or(last_reasoning).or(last_user).unwrap_or_else(|| "Thread".into());
    let title = infer_title(&title_source);
    Some((title, snippet, has_instructions))
}

fn infer_title(s: &str) -> String {
    if let Some(start) = s.find("**") {
        if let Some(end_rel) = s[start + 2..].find("**") {
            return s[start + 2..start + 2 + end_rel].trim().to_string();
        }
    }
    s.split_whitespace().take(6).collect::<Vec<_>>().join(" ")
}

pub fn resolve_session_path(base: &Path, id: Option<&str>, hint: Option<&str>) -> Option<String> {
    if let Some(h) = hint {
        let ph = Path::new(h);
        if ph.exists() && h.starts_with(&*base.to_string_lossy()) { return Some(h.to_string()); }
    }
    let target = id?;
    let mut stack = vec![base.to_path_buf()];
    while let Some(dir) = stack.pop() {
        if let StdResult::Ok(rd) = fs::read_dir(&dir) {
            for ent in rd.flatten() {
                let p = ent.path();
                if p.is_dir() { stack.push(p); continue; }
                if p.file_name().and_then(|n| n.to_str()) == Some(target) { return Some(p.to_string_lossy().to_string()); }
            }
        }
    }
    None
}

pub fn parse_thread(path: &Path) -> Result<ThreadResponse> {
    let f = fs::File::open(path).context("open session file")?;
    let r = BufReader::new(f);
    let mut items: Vec<ThreadItem> = vec![];
    let mut first_assistant: Option<String> = None;
    let mut instructions: Option<String> = None;
    let mut resume_id: Option<String> = None;
    for line in r.lines().filter_map(Result::ok) {
        let v: JsonValue = match serde_json::from_str(&line) { StdResult::Ok(v) => v, StdResult::Err(_) => continue };
        match v.get("type").and_then(|x| x.as_str()) {
            Some("thread.started") => {
                if let Some(id) = v.get("thread_id").and_then(|x| x.as_str()) {
                    resume_id = Some(id.to_string());
                }
            }
            Some("session_meta") => {
                if let Some(instr) = v.get("payload").and_then(|m| m.get("instructions")).and_then(|x| x.as_str()) {
                    instructions = Some(instr.to_string());
                }
            }
            // Newer JSONL shapes (response_item / event_msg)
            Some("response_item") => {
                if let Some(payload) = v.get("payload") {
                    match payload.get("type").and_then(|x| x.as_str()) {
                        Some("message") => {
                            let role = payload.get("role").and_then(|x| x.as_str()).unwrap_or("");
                            // Extract text from content array
                            let mut txt = String::new();
                            if let Some(arr) = payload.get("content").and_then(|x| x.as_array()) {
                                for part in arr {
                                    if let Some(t) = part.get("text").and_then(|x| x.as_str()) {
                                        if !txt.is_empty() { txt.push_str("\n"); }
                                        txt.push_str(t);
                                    }
                                }
                            }
                            if !txt.trim().is_empty() {
                                let role_s = if role == "assistant" { Some("assistant".to_string()) } else if role == "user" { Some("user".to_string()) } else { None };
                                if role_s.as_deref() == Some("assistant") && first_assistant.is_none() { first_assistant = Some(txt.clone()); }
                                items.push(ThreadItem { ts: now_ts(), kind: "message".into(), role: role_s, text: txt });
                            }
                        }
                        Some("reasoning") => {
                            // Prefer summary text if available
                            if let Some(summary) = payload.get("summary").and_then(|x| x.as_array()) {
                                let mut txt = String::new();
                                for s in summary {
                                    if s.get("type").and_then(|x| x.as_str()) == Some("summary_text") {
                                        if let Some(t) = s.get("text").and_then(|x| x.as_str()) {
                                            if !txt.is_empty() { txt.push_str("\n"); }
                                            txt.push_str(t);
                                        }
                                    }
                                }
                                if !txt.trim().is_empty() {
                                    items.push(ThreadItem { ts: now_ts(), kind: "reason".into(), role: None, text: txt });
                                }
                            }
                        }
                        _ => {}
                    }
                }
            }
            Some("event_msg") => {
                if let Some(p) = v.get("payload") {
                    if p.get("type").and_then(|x| x.as_str()) == Some("agent_reasoning") {
                        if let Some(text) = p.get("text").and_then(|x| x.as_str()) {
                            items.push(ThreadItem { ts: now_ts(), kind: "reason".into(), role: None, text: text.to_string() });
                        }
                    }
                }
            }
            Some("item.started") => {
                if let Some(item) = v.get("item") {
                    if item.get("type").and_then(|x| x.as_str()) == Some("command_execution") {
                        let cmd = item.get("command").and_then(|x| x.as_str()).unwrap_or("");
                        // Start entry: in_progress with empty sample
                        let payload = serde_json::json!({
                            "command": cmd,
                            "status": item.get("status").and_then(|x| x.as_str()).unwrap_or("in_progress"),
                            "exit_code": serde_json::Value::Null,
                            "sample": "",
                            "output_len": 0
                        });
                        items.push(ThreadItem { ts: now_ts(), kind: "cmd".into(), role: None, text: payload.to_string() });
                    }
                }
            }
            Some("item.completed") => {
                if let Some(item) = v.get("item") {
                    match item.get("type").and_then(|x| x.as_str()) {
                        Some("agent_message") => {
                            if let Some(text) = item.get("text").and_then(|x| x.as_str()) {
                                if first_assistant.is_none() { first_assistant = Some(text.to_string()); }
                                items.push(ThreadItem { ts: now_ts(), kind: "message".into(), role: Some("assistant".into()), text: text.to_string() });
                            }
                        }
                        Some("user_message") => {
                            if let Some(text) = item.get("text").and_then(|x| x.as_str()) {
                                items.push(ThreadItem { ts: now_ts(), kind: "message".into(), role: Some("user".into()), text: text.to_string() });
                            }
                        }
                        Some("reasoning") => {
                            if let Some(text) = item.get("text").and_then(|x| x.as_str()) {
                                items.push(ThreadItem { ts: now_ts(), kind: "reason".into(), role: None, text: text.to_string() });
                            }
                        }
                        Some("command_execution") => {
                            let cmd = item.get("command").and_then(|x| x.as_str()).unwrap_or("");
                            let out = item.get("aggregated_output").and_then(|x| x.as_str()).unwrap_or("");
                            let exit_code = item.get("exit_code").and_then(|x| x.as_i64()).unwrap_or(0);
                            let status = item.get("status").and_then(|x| x.as_str()).unwrap_or("completed");
                            let sample = if out.len() > 240 { format!("{}", &out[..240]) } else { out.to_string() };
                            let payload = serde_json::json!({
                                "command": cmd,
                                "status": status,
                                "exit_code": exit_code,
                                "sample": sample,
                                "output_len": out.len()
                            });
                            items.push(ThreadItem { ts: now_ts(), kind: "cmd".into(), role: None, text: payload.to_string() });
                        }
                        _ => {}
                    }
                }
            }
            _ => {}
        }
    }
    let title = infer_title(first_assistant.as_deref().unwrap_or("Thread"));
    Ok(ThreadResponse { title, items, instructions, resume_id })
}

fn now_ts() -> u64 { std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).unwrap_or_default().as_secs() }

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_file(p: &Path, lines: &[&str]) { let mut f = fs::File::create(p).unwrap(); for l in lines { writeln!(f, "{}", l).unwrap(); } }

    #[test]
    fn scan_history_filters_new_format_and_limits() {
        let td = tempfile::tempdir().unwrap();
        let base = td.path().join("sessions");
        fs::create_dir_all(&base).unwrap();
        // Old-format file (should be ignored)
        let old = base.join("old.jsonl");
        write_file(&old, &[r#"{"id":"x","msg":{"type":"agent_message","message":"old format"}}"#]);
        // 6 new-format files; only 5 should be returned
        for i in 0..6u32 {
            let p = base.join(format!("rollout-{}.jsonl", i));
            write_file(&p, &[
                r#"{"type":"thread.started","thread_id":"t"}"#,
                r#"{"type":"item.completed","item":{"id":"a","type":"agent_message","text":"**Title** message here"}}"#,
            ]);
            std::thread::sleep(std::time::Duration::from_millis(5));
        }
        let items = scan_history(&base, 5).unwrap();
        assert_eq!(items.len(), 5, "should limit to 5 newest sessions");
        assert!(items.iter().all(|it| it.id.starts_with("rollout-")));
        let mut mtimes = items.iter().map(|i| i.mtime).collect::<Vec<_>>();
        let mut sorted = mtimes.clone();
        sorted.sort_by(|a,b| b.cmp(a));
        assert_eq!(mtimes, sorted);
    }

    #[test]
    fn parse_thread_extracts_items() {
        let td = tempfile::tempdir().unwrap();
        let p = td.path().join("rollout.jsonl");
        write_file(&p, &[
            r#"{"type":"thread.started","thread_id":"t"}"#,
            r#"{"type":"item.started","item":{"id":"c1","type":"command_execution","command":"echo hello","aggregated_output":"","status":"in_progress"}}"#,
            r#"{"type":"item.completed","item":{"id":"m1","type":"user_message","text":"Hi"}}"#,
            r#"{"type":"item.completed","item":{"id":"m2","type":"agent_message","text":"**Done**."}}"#,
        ]);
        let th = parse_thread(&p).unwrap();
        assert_eq!(th.title, "Done");
        assert!(th.items.iter().any(|i| i.kind == "cmd" && i.text.contains("echo hello")));
        assert!(th.items.iter().any(|i| i.kind == "message" && i.role.as_deref() == Some("assistant")));
    }
}
