//! ATIF session export and clipboard support.

use crate::coder::tui::{Entry, Role, ToolCall};
use serde_json::{Value, json};
use std::fs;
use std::io::Write;
use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::{SystemTime, UNIX_EPOCH};

const SCHEMA_VERSION: &str = "ATIF-v1.7";
const AGENT_NAME: &str = "openagents-coder";
pub struct ExportedTrajectory {
    pub path: String,
    pub copied: bool,
    pub steps: usize,
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as u64
}

fn home_dir() -> Option<String> {
    std::env::var("HOME")
        .ok()
        .or_else(|| std::env::var("USERPROFILE").ok())
}

fn run_with_input(command: &str, args: &[&str], input: &str) -> Option<()> {
    let mut child = Command::new(command)
        .args(args)
        .stdin(Stdio::piped())
        .spawn()
        .ok()?;
    child
        .stdin
        .take()
        .and_then(|mut s| s.write_all(input.as_bytes()).ok());
    let status = child.wait().ok()?;
    if status.success() { Some(()) } else { None }
}

fn copy_to_clipboard(text: &str) -> bool {
    let candidates: &[(&str, &[&str])] = if cfg!(target_os = "macos") {
        &[("pbcopy", &[])]
    } else if cfg!(target_os = "windows") {
        &[("clip", &[])]
    } else {
        &[
            ("wl-copy", &[]),
            ("xclip", &["-selection", "clipboard"]),
            ("xsel", &["--clipboard", "--input"]),
        ]
    };
    for (cmd, args) in candidates {
        if run_with_input(cmd, args, text).is_some() {
            return true;
        }
    }
    false
}

fn iso_for_ms(at: u64) -> String {
    let seconds = at / 1_000;
    let ms = at % 1000;
    let days = i64::try_from(seconds / 86_400).unwrap_or(i64::MAX);
    let seconds_of_day = seconds % 86_400;
    let (year, month, day) = civil_date(days);
    let hour = seconds_of_day / 3_600;
    let minute = (seconds_of_day % 3_600) / 60;
    let second = seconds_of_day % 60;
    format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{ms:03}Z")
}

/// Convert days since 1970-01-01 to a Gregorian calendar date.
fn civil_date(days_since_epoch: i64) -> (i64, u64, u64) {
    let z = days_since_epoch + 719_468;
    let era = z.div_euclid(146_097);
    let day_of_era = z - era * 146_097;
    let year_of_era =
        (day_of_era - day_of_era / 1_460 + day_of_era / 36_524 - day_of_era / 146_096) / 365;
    let mut year = year_of_era + era * 400;
    let day_of_year = day_of_era - (365 * year_of_era + year_of_era / 4 - year_of_era / 100);
    let month_prime = (5 * day_of_year + 2) / 153;
    let day = day_of_year - (153 * month_prime + 2) / 5 + 1;
    let month = month_prime + if month_prime < 10 { 3 } else { -9 };
    year += i64::from(month <= 2);
    (year, month as u64, day as u64)
}

fn now_iso() -> String {
    iso_for_ms(now_ms())
}

/// Build TUI entries from the runtime message list so a headless turn can
/// write the same ATIF document `/export` writes from the full-screen session.
///
/// Harbor copies the newest file in `~/.openagents/exports` to the trial's
/// `trajectory.json`. After the adapter switched from piping `/export` into
/// `--plain` to `coder --headless`, that directory stayed empty and T2
/// (`--stat` before `-p`) could not be measured from tool argv.
pub fn entries_from_chat_messages(messages: &[crate::runtime::ChatMessage]) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut pending: Vec<(String, String, Value)> = Vec::new();
    for message in messages {
        match message.role.as_str() {
            "system" => {}
            "user" => {
                if let Some(text) = message.content.as_deref() {
                    if !text.is_empty() {
                        entries.push(Entry::new(Role::You, text));
                    }
                }
            }
            "assistant" => {
                if let Some(calls) = &message.tool_calls {
                    for call in calls {
                        pending.push(pending_tool_call(call));
                    }
                }
                if let Some(text) = message.content.as_deref() {
                    if !text.is_empty() {
                        entries.push(Entry::new(Role::Assistant, text));
                    }
                }
            }
            "tool" => {
                let id = message.tool_call_id.clone().unwrap_or_default();
                let idx = pending
                    .iter()
                    .position(|(call_id, _, _)| !call_id.is_empty() && *call_id == id)
                    .or_else(|| (!pending.is_empty()).then_some(0));
                if let Some(idx) = idx {
                    let (call_id, name, arguments) = pending.remove(idx);
                    let output = message.content.clone();
                    let mut entry = Entry::tool_call(&name);
                    entry.output = output.clone();
                    entry.tool = Some(ToolCall {
                        call_id: if call_id.is_empty() {
                            name.clone()
                        } else {
                            call_id
                        },
                        function_name: name,
                        arguments,
                        output,
                        error: None,
                        done: true,
                        duration_ms: Some(0),
                    });
                    entries.push(entry);
                }
            }
            _ => {}
        }
    }
    entries
}

fn pending_tool_call(call: &Value) -> (String, String, Value) {
    let id = call
        .get("id")
        .and_then(Value::as_str)
        .unwrap_or("")
        .to_string();
    let function = call.get("function").cloned().unwrap_or_else(|| json!({}));
    let name = function
        .get("name")
        .and_then(Value::as_str)
        .unwrap_or("unknown")
        .to_string();
    let arguments = match function.get("arguments") {
        Some(Value::String(raw)) => {
            serde_json::from_str(raw).unwrap_or_else(|_| json!({ "raw": raw }))
        }
        Some(other) => other.clone(),
        None => json!({}),
    };
    (id, name, arguments)
}

/// Write the runtime transcript to `~/.openagents/exports` as ATIF-v1.7.
pub fn export_runtime_messages(
    messages: &[crate::runtime::ChatMessage],
    model: &str,
    repo: &str,
    branch: &str,
) -> ExportedTrajectory {
    export_trajectory(&entries_from_chat_messages(messages), model, repo, branch)
}

pub fn git_info() -> Option<(String, String)> {
    let repo = Command::new("git")
        .args(["rev-parse", "--show-toplevel"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })?;

    let branch = Command::new("git")
        .args(["branch", "--show-current"])
        .output()
        .ok()
        .and_then(|o| {
            let s = String::from_utf8_lossy(&o.stdout).trim().to_string();
            if s.is_empty() { None } else { Some(s) }
        })
        .unwrap_or_else(|| "unknown".to_string());

    Some((repo, branch))
}

fn is_interface_command(text: &str) -> bool {
    crate::composer::is_local_slash_input(text, super::commands::COMMANDS)
}

fn step_of(entry: &Entry, model: &str, tool: Option<&ToolCall>) -> Option<Value> {
    let timestamp = iso_for_ms(entry.at);
    match entry.role {
        Role::You if !is_interface_command(&entry.text) => Some(json!({
            "step_id": 0,
            "timestamp": timestamp,
            "source": "user",
            "message": entry.text,
        })),
        Role::Assistant if !entry.text.is_empty() => Some(json!({
            "step_id": 0,
            "timestamp": timestamp,
            "source": "agent",
            "message": entry.text,
            "model_name": entry.model.as_deref().unwrap_or(model),
        })),
        Role::Tool => {
            let tool = tool?;
            let content = entry.output.as_deref().unwrap_or("");
            let status = if content == crate::tools::CANCELLED_TOOL_RESULT {
                "cancelled"
            } else if content.starts_with("Timed out: the openagents CLI") {
                // A watchdog kill is not `completed` (#180): a hung call that
                // exports as done is how a dead turn reads as a live one.
                "timeout"
            } else if tool.error.is_some() {
                "failed"
            } else {
                "completed"
            };
            Some(json!({
                "step_id": 0,
                "timestamp": timestamp,
                "source": "agent",
                "message": "",
                "model_name": model,
                "tool_calls": [{
                    "tool_call_id": tool.call_id,
                    "function_name": tool.function_name,
                    "arguments": tool.arguments,
                }],
                "observation": {
                    "results": [{
                        "source_call_id": tool.call_id,
                        "content": content,
                        "status": status,
                        "duration_ms": tool.exported_duration_ms(),
                    }]
                }
            }))
        }
        _ => None,
    }
}

fn file_name(repository: &str, at_iso: &str) -> String {
    let safe = repository
        .split('/')
        .next_back()
        .unwrap_or(repository)
        .replace(
            |c: char| !c.is_alphanumeric() && c != '.' && c != '-' && c != '_',
            "-",
        );
    let stamp = at_iso.replace([':', '.'], "-");
    format!("{}-{}-atif.json", stamp, safe)
}

/// Repeat-execution waste, measured at export.
///
/// A session that runs the same suite twice pays the second run to learn
/// something the first already taught it. The exporter is where that cost
/// becomes visible without anyone hand-parsing the trajectory: every shell
/// line is reduced to its command heads (the same normalizer the #153 gate
/// refuses on, so the two can never disagree about identity), and any head
/// executed more than once reports how much of the total wall time the
/// repetitions beyond the first spent.
/// Child transcripts keyed by the parent tool call id.
///
/// Live Grok streams used to land one notice per thought-token. The
/// export now keeps one coalesced transcript per delegated call.
fn subagent_section(entries: &[Entry]) -> Value {
    let mut map = serde_json::Map::new();
    for entry in entries {
        if entry.role != Role::Tool || entry.subagent_lines.is_empty() {
            continue;
        }
        let call_id = entry
            .tool
            .as_ref()
            .map(|tool| tool.call_id.as_str())
            .filter(|id| !id.is_empty())
            .map(str::to_string)
            .unwrap_or_else(|| format!("entry-{}", entry.at));
        let tool = entry
            .tool
            .as_ref()
            .map(|tool| tool.function_name.as_str())
            .unwrap_or("delegate");
        map.insert(
            call_id,
            json!({
                "tool": tool,
                "transcript": coalesce_subagent_transcript(&entry.subagent_lines),
            }),
        );
    }
    Value::Object(map)
}

fn coalesce_subagent_transcript(lines: &[String]) -> String {
    let mut out = String::new();
    let mut words: Vec<String> = Vec::new();
    for line in lines {
        let text = line.trim().trim_start_matches('·').trim();
        if text.is_empty() {
            continue;
        }
        if is_thought_fragment(text) {
            words.push(text.to_string());
            continue;
        }
        flush_coalesced_words(&mut out, &mut words);
        if !out.is_empty() {
            out.push('\n');
        }
        out.push_str(text);
    }
    flush_coalesced_words(&mut out, &mut words);
    out
}

fn flush_coalesced_words(out: &mut String, words: &mut Vec<String>) {
    if words.is_empty() {
        return;
    }
    if !out.is_empty() {
        out.push('\n');
    }
    out.push_str(&words.join(" "));
    words.clear();
}

fn is_thought_fragment(text: &str) -> bool {
    if text.contains(char::is_whitespace) || text.contains('/') || text.contains('\\') {
        return false;
    }
    if text.contains('_') {
        return false;
    }
    text.len() <= 48
}

fn swarm_section(entries: &[Entry]) -> Vec<Value> {
    entries
        .iter()
        .filter_map(|entry| {
            let tool = entry.tool.as_ref()?;
            if !crate::swarm::is_swarm_tool(&tool.function_name) {
                return None;
            }
            Some(json!({
                "timestamp": iso_for_ms(entry.at),
                "tool": tool.function_name,
                "header": entry.text,
                "output": entry.output,
            }))
        })
        .collect()
}

/// Repeat-execution waste, measured at export.
///
/// A session that runs the same suite twice pays the second run to learn
/// something the first already taught it. The exporter is where that cost
/// becomes visible without anyone hand-parsing the trajectory: every shell
/// line is reduced to its command heads (the same normalizer the #153 gate
/// refuses on, so the two can never disagree about identity), and any head
/// executed more than once reports how much of the total wall time the
/// repetitions beyond the first spent.
fn waste_section(entries: &[Entry]) -> (Vec<Value>, u64, usize) {
    use std::collections::BTreeMap;

    // Head -> (executions, total seconds across all executions).
    let mut families: BTreeMap<String, (u64, u64)> = BTreeMap::new();
    for entry in entries {
        let Some(tool) = entry.tool.as_ref() else {
            continue;
        };
        if tool.function_name != "shell"
            && tool.function_name != "bash"
            && tool.function_name != "run"
        {
            continue;
        }
        let Some(command) = tool
            .arguments
            .get("command")
            .and_then(serde_json::Value::as_str)
        else {
            continue;
        };
        let Some(heads) = crate::tools::command_heads(command) else {
            continue;
        };
        let duration_ms = tool.exported_duration_ms();
        for head in heads {
            let entry_ = families.entry(head).or_insert((0, 0));
            entry_.0 += 1;
            entry_.1 += duration_ms;
        }
    }

    let mut total_wasted_ms = 0u64;
    let mut repeated_heads = 0usize;
    let section = families
        .into_iter()
        .filter_map(|(head, (executions, total_ms))| {
            if executions <= 1 {
                return None;
            }
            // Repetitions beyond the first carry their equal share of the
            // time. A single execution of a suite is not waste; the second
            // and third are, whatever they were hunting for.
            let wasted_ms = total_ms * (executions - 1) / executions;
            total_wasted_ms += wasted_ms;
            repeated_heads += 1;
            Some(json!({
                "head": head,
                "executions": executions,
                "approx_wasted_seconds": wasted_ms / 1000,
            }))
        })
        .collect();
    (section, total_wasted_ms / 1000, repeated_heads)
}

/// Tool-call batching, measured at export.
///
/// One model round is the span of consecutive `Role::Tool` entries with no
/// assistant text or user turn between them: the calls the model emitted
/// together, whatever the harness executed serially. Consecutive batches are
/// the batching lever's meter (`docs/coder/cursorize.md` R1): a session that
/// fires one call per round re-sends its transcript once per call, and the
/// cost is quadratic in exactly the way the lever targets.
///
/// Inter-round narration is assistant text that a tool round *follows and
/// another tool round precedes* — the prose between rounds, not the final
/// report. `None`-shaped results (`tool_rounds == 0`, or no narration
/// samples) are the caller's signal to omit the field rather than export a
/// zero a reader would read as measured.
fn round_metrics(entries: &[Entry]) -> RoundMetrics {
    let mut rounds = 0u64;
    let mut calls = 0u64;
    let mut max_per_round = 0u64;
    let mut in_round = false;
    let mut current = 0u64;
    // Text seen since the last round ended; promoted to inter-round only
    // when another round actually starts.
    let mut pending_narration: Vec<usize> = Vec::new();
    let mut inter_round_text: Vec<usize> = Vec::new();
    for entry in entries {
        match entry.role {
            Role::Tool if entry.tool.is_some() => {
                if !in_round {
                    if rounds > 0 {
                        inter_round_text.append(&mut pending_narration);
                    } else {
                        pending_narration.clear();
                    }
                    rounds += 1;
                    in_round = true;
                }
                current += 1;
                calls += 1;
            }
            Role::Assistant => {
                if in_round {
                    max_per_round = max_per_round.max(current);
                    current = 0;
                    in_round = false;
                }
                if !entry.text.trim().is_empty() {
                    pending_narration.push(entry.text.chars().count());
                }
            }
            Role::You => {
                if in_round {
                    max_per_round = max_per_round.max(current);
                    current = 0;
                    in_round = false;
                }
                pending_narration.clear();
            }
            _ => {}
        }
    }
    if in_round {
        max_per_round = max_per_round.max(current);
    }
    let mean = if rounds > 0 {
        calls as f64 / rounds as f64
    } else {
        0.0
    };
    RoundMetrics {
        rounds,
        calls,
        mean,
        max_per_round,
        median_narration_chars: median_usize(&mut inter_round_text),
    }
}

struct RoundMetrics {
    rounds: u64,
    calls: u64,
    mean: f64,
    max_per_round: u64,
    median_narration_chars: Option<u64>,
}

/// Median without allocation games: sorts in place; `None` for no samples.
fn median_usize(values: &mut Vec<usize>) -> Option<u64> {
    if values.is_empty() {
        return None;
    }
    values.sort_unstable();
    let mid = values.len() / 2;
    Some(if values.len() % 2 == 0 {
        ((values[mid - 1] + values[mid]) / 2) as u64
    } else {
        values[mid] as u64
    })
}

fn trajectory_document(
    entries: &[Entry],
    model: &str,
    repo: &str,
    branch: &str,
    session_id: &str,
    at_iso: &str,
) -> (Value, usize) {
    let mut steps = Vec::new();
    let mut notices = Vec::new();
    let mut turn_outcomes = Vec::new();
    let mut pending_reasoning = String::new();

    for entry in entries {
        if entry.role == Role::Reasoning {
            if !pending_reasoning.is_empty() {
                pending_reasoning.push('\n');
            }
            pending_reasoning.push_str(&entry.text);
            continue;
        }
        if let Some(notice) = match entry.role {
            Role::Notice | Role::Output => Some(json!({
                "timestamp": iso_for_ms(entry.at),
                "text": entry.text,
            })),
            _ => None,
        } {
            notices.push(notice);
        }
        if entry.role == Role::Notice && entry.text == "Turn canceled." {
            turn_outcomes.push(json!({
                "timestamp": iso_for_ms(entry.at),
                "status": "cancelled",
                "turn_id": entry.turn_id,
            }));
        }
        // A milestone note is the model's own words about its work, so it
        // enters the trajectory as an agent step, not a notice (#189): a
        // reader of the export sees the checkpoint beside the tool calls
        // that led to it.
        if entry.role == Role::Notice && entry.text.starts_with("Checkpoint: ") {
            let mut step = json!({
                "step_id": 0,
                "timestamp": iso_for_ms(entry.at),
                "source": "agent",
                "message": entry.text.replacen("Checkpoint: ", "", 1),
                "checkpoint": true,
            });
            if let Some(obj) = step.as_object_mut() {
                obj.insert("step_id".to_string(), json!(steps.len() + 1));
            }
            steps.push(step);
            continue;
        }

        if let Some(mut step) = step_of(entry, model, entry.tool.as_ref()) {
            if let Some(obj) = step.as_object_mut() {
                obj.insert("step_id".to_string(), json!(steps.len() + 1));
                if !pending_reasoning.is_empty() {
                    obj.insert(
                        "reasoning_content".to_string(),
                        json!(std::mem::take(&mut pending_reasoning)),
                    );
                }
            }
            steps.push(step);
        }
    }

    let step_count = steps.len();
    let (repeated_heads, total_wasted_seconds, repeated_head_count) = waste_section(entries);
    let round = round_metrics(entries);
    let mut final_metrics = serde_json::Map::new();
    final_metrics.insert("total_steps".to_string(), json!(step_count));
    if round.rounds > 0 {
        final_metrics.insert("tool_rounds".to_string(), json!(round.rounds));
        final_metrics.insert("tool_calls_total".to_string(), json!(round.calls));
        final_metrics.insert("tool_calls_per_round_mean".to_string(), json!(round.mean));
        final_metrics.insert(
            "tool_calls_per_round_max".to_string(),
            json!(round.max_per_round),
        );
        // Narration needs a sample on both sides of a round to be
        // "inter-round"; a session that never narrated between rounds omits
        // the field rather than exporting a measured-looking zero.
        if let Some(median) = round.median_narration_chars {
            final_metrics.insert("median_inter_round_text_chars".to_string(), json!(median));
        }
    }
    // Waste is computable for every session — zero is a true zero here.
    final_metrics.insert(
        "wasted_seconds_total".to_string(),
        json!(total_wasted_seconds),
    );
    final_metrics.insert("wasted_heads_total".to_string(), json!(repeated_head_count));
    (
        json!({
            "schema_version": SCHEMA_VERSION,
            "session_id": session_id,
            "trajectory_id": session_id,
            "agent": {
                "name": AGENT_NAME,
                "version": crate::VERSION,
                "model_name": model,
            },
            "steps": steps,
            "final_metrics": final_metrics,
            "extra": {
                "exporter": "openagents.coder.atif_export.v1",
                "exported_at": at_iso,
                "repository": repo,
                "branch": branch,
                "notices": notices,
                "subagent": subagent_section(entries),
                "turn_outcomes": turn_outcomes,
                "waste": {
                    "repeated_command_heads": repeated_heads,
                },
                "swarm": swarm_section(entries),
            }
        }),
        step_count,
    )
}

/// Materialize the current local session as an atomic ATIF snapshot.
///
/// `updates.jsonl` remains the crash-safe source of truth. This file is a
/// complete, directly consumable interchange artifact derived from it.
///
/// Swarm send/receive events are typed `swarm_message` in that jsonl. This
/// exporter carries them as `swarm.inbox` / `swarm_send` tool observations
/// (source `agent`, never source `user`) when the TUI drew the matching
/// entry. A `swarm_message` with no TUI entry is omitted here and stays in
/// the jsonl archive.
pub fn write_session_trajectory(
    entries: &[Entry],
    model: &str,
    repo: &str,
    branch: &str,
    session_id: &str,
    directory: &std::path::Path,
) -> std::io::Result<usize> {
    let at_iso = now_iso();
    let (document, steps) = trajectory_document(entries, model, repo, branch, session_id, &at_iso);
    let path = directory.join("trajectory.atif.json");
    let temp = directory.join(format!(".trajectory.atif.json.{}.tmp", std::process::id()));
    let bytes = format!(
        "{}\n",
        serde_json::to_string_pretty(&document)
            .map_err(|error| std::io::Error::new(std::io::ErrorKind::InvalidData, error))?
    );
    let mut options = fs::OpenOptions::new();
    options.create(true).truncate(true).write(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    let mut file = options.open(&temp)?;
    file.write_all(bytes.as_bytes())?;
    file.sync_all()?;
    fs::rename(&temp, &path).inspect_err(|_| {
        let _ = fs::remove_file(&temp);
    })?;
    Ok(steps)
}

pub fn export_trajectory(
    entries: &[Entry],
    model: &str,
    repo: &str,
    branch: &str,
) -> ExportedTrajectory {
    let at_iso = now_iso();
    let session_id = format!("{}-{}", repo, at_iso);
    let (document, steps) = trajectory_document(entries, model, repo, branch, &session_id, &at_iso);

    let directory = home_dir()
        .map(|h| PathBuf::from(h).join(".openagents").join("exports"))
        .unwrap_or_else(|| PathBuf::from(".openagents").join("exports"));
    fs::create_dir_all(&directory).unwrap_or_default();

    let path = directory.join(file_name(repo, &at_iso));
    let path_str = path.to_string_lossy().to_string();
    fs::write(
        &path,
        format!(
            "{}\n",
            serde_json::to_string_pretty(&document).unwrap_or_default()
        ),
    )
    .ok();

    let copied = copy_to_clipboard(&path_str);
    ExportedTrajectory {
        path: path_str,
        copied,
        steps,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_swarm_inbox_entry_exports_as_a_tool_not_as_user_speech() {
        let mut entry = Entry::tool_call("swarm ← session-b [question]");
        entry.tool = Some(ToolCall {
            call_id: "swarm-inbox-1".to_string(),
            function_name: crate::swarm::INBOX_TOOL.to_string(),
            arguments: serde_json::json!({
                "from": "session-b",
                "kind": "question",
                "count": 1
            }),
            output: Some(r#"{"body":"what failed?"}"#.to_string()),
            error: None,
            done: true,
            duration_ms: Some(0),
        });
        let (document, count) = trajectory_document(
            &[entry],
            "model",
            "/repo",
            "main",
            "session",
            "2026-08-27T00:00:00.000Z",
        );
        assert_eq!(count, 1);
        let step = &document["steps"][0];
        assert_eq!(step["source"], "agent");
        assert_eq!(
            step["tool_calls"][0]["function_name"],
            crate::swarm::INBOX_TOOL
        );
        assert_eq!(
            document["extra"]["swarm"][0]["tool"],
            crate::swarm::INBOX_TOOL
        );
        assert_ne!(step["source"], "user");
    }

    #[test]
    fn chat_messages_keep_shell_argv_for_stat_before_p() {
        let messages = vec![
            crate::runtime::ChatMessage {
                role: "user".to_string(),
                content: Some("find the leak".to_string()),
                tool_calls: None,
                tool_call_id: None,
                images: Vec::new(),
            },
            crate::runtime::ChatMessage {
                role: "assistant".to_string(),
                content: None,
                tool_calls: Some(vec![json!({
                    "id": "call-1",
                    "type": "function",
                    "function": {
                        "name": "shell",
                        "arguments": "{\"command\":\"git log --stat -5 && git log -p -1\"}"
                    }
                })]),
                tool_call_id: None,
                images: Vec::new(),
            },
            crate::runtime::ChatMessage {
                role: "tool".to_string(),
                content: Some(" file.rs | 2 +-\n".to_string()),
                tool_calls: None,
                tool_call_id: Some("call-1".to_string()),
                images: Vec::new(),
            },
            crate::runtime::ChatMessage {
                role: "assistant".to_string(),
                content: Some("recovered".to_string()),
                tool_calls: None,
                tool_call_id: None,
                images: Vec::new(),
            },
        ];
        let entries = entries_from_chat_messages(&messages);
        let (document, count) = trajectory_document(
            &entries,
            "glm-5.3-flash",
            "/app",
            "main",
            "session",
            "2026-08-28T00:00:00.000Z",
        );
        assert_eq!(count, 3);
        let tool = document["steps"]
            .as_array()
            .unwrap()
            .iter()
            .find(|step| step.get("tool_calls").is_some())
            .expect("shell call missing from ATIF");
        assert_eq!(tool["tool_calls"][0]["function_name"], "shell");
        assert_eq!(
            tool["tool_calls"][0]["arguments"]["command"],
            "git log --stat -5 && git log -p -1"
        );
        assert_eq!(document["steps"][2]["message"], "recovered");
    }

    #[test]
    fn timestamps_do_not_depend_on_an_external_runtime() {
        assert_eq!(iso_for_ms(0), "1970-01-01T00:00:00.000Z");
        assert_eq!(iso_for_ms(951_827_696_789), "2000-02-29T12:34:56.789Z");
        assert_eq!(iso_for_ms(1_798_310_400_000), "2026-12-26T18:40:00.000Z");
    }

    #[test]
    fn step_ids_are_contiguous_when_notices_are_skipped() {
        let entries = vec![
            Entry::new(Role::Notice, "local notice"),
            Entry::new(Role::You, "/goal local command"),
            Entry::new(Role::You, "/Users/name/repo inspect this path"),
            Entry::new(Role::You, "question"),
            Entry::new(Role::Output, "local output"),
            Entry::new(Role::Reasoning, "inspect the evidence"),
            Entry::new(Role::Assistant, "answer"),
        ];
        let (document, count) = trajectory_document(
            &entries,
            "model",
            "/repo",
            "main",
            "session",
            "2026-08-26T00:00:00.000Z",
        );
        let ids = document["steps"]
            .as_array()
            .unwrap()
            .iter()
            .map(|step| step["step_id"].as_u64().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(count, 3);
        assert_eq!(ids, vec![1, 2, 3]);
        assert_eq!(
            document["steps"][2]["reasoning_content"],
            "inspect the evidence"
        );
        assert_eq!(
            document["steps"][0]["message"],
            "/Users/name/repo inspect this path"
        );
    }

    #[test]
    fn subagent_lines_export_as_a_coalesced_transcript_under_the_call() {
        let mut entry = Entry::tool_call("delegate read it");
        entry.tool = Some(ToolCall {
            call_id: "d1".to_string(),
            function_name: "delegate".to_string(),
            arguments: serde_json::json!({"prompt": "read it"}),
            output: Some("Done · 1 tool uses · 1s\nreport".to_string()),
            error: None,
            done: true,
            duration_ms: Some(1000),
        });
        entry.push_subagent_line("· started on grok");
        entry.push_subagent_line("the");
        entry.push_subagent_line("crate");
        entry.push_subagent_line("name");
        entry.push_subagent_line("read_file");
        entry.push_subagent_line("· read Cargo.toml");
        let (document, count) = trajectory_document(
            &[entry],
            "model",
            "/repo",
            "main",
            "session",
            "2026-08-27T00:00:00.000Z",
        );
        assert_eq!(count, 1);
        let notices = document["extra"]["notices"].as_array().unwrap();
        assert!(
            notices.is_empty(),
            "child stream must not land as one notice per token: {notices:?}"
        );
        let record = &document["extra"]["subagent"]["d1"];
        assert_eq!(record["tool"], "delegate");
        assert_eq!(
            record["transcript"].as_str().unwrap(),
            "started on grok\nthe crate name\nread_file\nread Cargo.toml"
        );
    }

    fn tool_entry(call_id: &str, command: &str) -> Entry {
        let mut entry = Entry::tool_call(format!("bash {command}"));
        entry.tool = Some(ToolCall {
            call_id: call_id.to_string(),
            function_name: "bash".to_string(),
            arguments: serde_json::json!({ "command": command }),
            output: Some("ok".to_string()),
            error: None,
            done: true,
            duration_ms: Some(100),
        });
        entry
    }

    fn text_entry(text: &str) -> Entry {
        let mut entry = Entry::new(Role::Assistant, text.to_string());
        entry.finish_text();
        entry
    }

    #[test]
    fn final_metrics_carry_round_batching_and_narration_shape() {
        // Round 1: two calls in one round. Narration between. Round 2: one
        // call. A final report after — that is the report, not narration.
        let entries = vec![
            tool_entry("c1", "git status"),
            tool_entry("c2", "ls docs"),
            text_entry("checking the tree"), // 17 chars, between rounds
            tool_entry("c3", "cargo check -p openagents-cli"),
            text_entry("done"), // final report, not between rounds
        ];
        let (document, _) = trajectory_document(
            &entries,
            "model",
            "/repo",
            "main",
            "session",
            "2026-08-29T00:00:00.000Z",
        );
        let metrics = &document["final_metrics"];
        assert_eq!(metrics["tool_rounds"], 2);
        assert_eq!(metrics["tool_calls_total"], 3);
        assert_eq!(metrics["tool_calls_per_round_mean"], 1.5);
        assert_eq!(metrics["tool_calls_per_round_max"], 2);
        assert_eq!(metrics["median_inter_round_text_chars"], 17);
    }

    #[test]
    fn a_session_without_tools_omits_round_metrics_rather_than_zeroes() {
        let entries = vec![text_entry("a plain answer with no tool work")];
        let (document, _) = trajectory_document(
            &entries,
            "model",
            "/repo",
            "main",
            "session",
            "2026-08-29T00:00:00.000Z",
        );
        let metrics = &document["final_metrics"];
        assert!(metrics.get("tool_rounds").is_none());
        assert!(metrics.get("tool_calls_total").is_none());
        assert!(metrics.get("tool_calls_per_round_mean").is_none());
        assert!(metrics.get("median_inter_round_text_chars").is_none());
        // Waste is computable for every session; zero is a true zero.
        assert_eq!(metrics["wasted_heads_total"], 0);
        assert_eq!(metrics["wasted_seconds_total"], 0);
        assert_eq!(metrics["total_steps"], 1);
    }

    #[test]
    fn final_metrics_sum_waste_across_repeated_heads() {
        let entries = vec![
            tool_entry("c1", "cargo test -p openagents-cli"),
            tool_entry("c2", "cargo test -p openagents-cli"),
            tool_entry("c3", "cargo test -p openagents-cli"),
        ];
        let (document, _) = trajectory_document(
            &entries,
            "model",
            "/repo",
            "main",
            "session",
            "2026-08-29T00:00:00.000Z",
        );
        let metrics = &document["final_metrics"];
        // Three executions of one head: 2/3 of the total time is waste.
        assert_eq!(metrics["wasted_heads_total"], 1);
        assert_eq!(metrics["wasted_seconds_total"], 0); // 300ms total, 200ms wasted -> 0s at whole-second resolution
        assert_eq!(
            document["extra"]["waste"]["repeated_command_heads"]
                .as_array()
                .unwrap()
                .len(),
            1
        );
    }
}
