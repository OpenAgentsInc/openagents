//! System One hook 1 (#11486, `docs/coder/design/system-one-hooks.md`):
//! squeeze a noisy shell command's output before it enters an agent's
//! history.
//!
//! Output an agent reads is re-read on every later model call, so a 2,000
//! line test log costs its size once per turn for the rest of the session.
//! The hook rewrites a build, test, or lint command (the table in
//! [`shell::noisy`]) so it runs through the squeezer:
//!
//! - **Hook** ([`claude_hook`]): a `PreToolUse` hook on `Bash` that answers
//!   with `updatedInput` only, never a permission decision, so the session's
//!   own rules and the floors still decide whether the command runs.
//! - **Squeezer** ([`main`]): runs the command in bash with stdout and
//!   stderr in one stream and exits with its status. Output of at most
//!   [`PASS_LINES`] lines passes unchanged. Longer output keeps its head,
//!   its tail, and the middle chunks that decide the next step; the full
//!   output goes to a file the reply names.
//! - **Deterministic first**: a chunk with an error, failure, or panic line
//!   is flagged by rule. When the flagged chunks fit the budget, they are
//!   kept and nothing is asked. When too many or none are flagged, one
//!   Decisions API call (`openagents/decide`, Clef first; docs/api/decisions.md)
//!   rates each candidate chunk, and the most probable fit the budget. A
//!   failed call falls back to the rule.
//! - **Log**: one JSON line per squeezed run (how the chunks were picked,
//!   each probability, the model, door, and latency), for the outcome join
//!   (#11492).
//!
//! Environment: `CODER_SQUEEZE=off` turns the hook off,
//! `CODER_SQUEEZE_DIR` names where full outputs go (default: a
//! `coder-squeeze` directory under the temp dir), `CODER_SQUEEZE_LOG` names
//! the decision log (default `~/.openagents/decisions/squeeze.jsonl`, the shared `hook-log` format), and
//! `OPENAGENTS_DECISIONS_URL` another Decisions API root.

pub mod shell;

use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use serde_json::{Value, json};

/// The `coder` subcommand that runs the squeezer.
pub const CODER_SUBCOMMAND: &str = "coder-squeeze";
/// Output at or under this many lines passes unchanged.
pub const PASS_LINES: usize = 150;
/// Lines always kept from the start.
pub const HEAD_LINES: usize = 8;
/// Lines always kept from the end, where summaries are.
pub const TAIL_LINES: usize = 40;
/// Lines of middle chunks kept at most.
pub const MIDDLE_BUDGET: usize = 100;
/// The longest middle chunk.
const CHUNK_LINES: usize = 30;
/// A chunk may end at a blank line once it has this many lines.
const CHUNK_MIN: usize = 6;
/// Chunks one decision call rates at most (the API's question limit).
const MAX_QUESTIONS: usize = 64;
/// Characters of one chunk shown to the decision model.
const CHUNK_CHARS: usize = 1_500;
/// Characters of the whole decision input, well under its token limit.
const INPUT_CHARS: usize = 40_000;
/// A chunk is kept by decision at or above this probability.
pub const KEEP_AT: f64 = 0.5;
/// How long the squeezer waits for a decision.
const DECISION_TIMEOUT: Duration = Duration::from_secs(5);
/// Output bytes held in memory; the rest still reaches the full-output file.
const MEMORY_BYTES: usize = 16 << 20;
/// The decision model.
pub const MODEL: &str = "openagents/decide";

/// The hook's switch.
pub const OFF_VAR: &str = "CODER_SQUEEZE";
/// Where full outputs go.
pub const DIR_VAR: &str = "CODER_SQUEEZE_DIR";
/// Where decisions are logged.
pub const LOG_VAR: &str = "CODER_SQUEEZE_LOG";

/// Whether the person turned squeezing off.
#[must_use]
pub fn off() -> bool {
    std::env::var(OFF_VAR).is_ok_and(|value| value.trim().eq_ignore_ascii_case("off"))
}

/// The squeezer's command line for `program` and its subcommand words,
/// quoted for the shell.
#[must_use]
pub fn invoker(program: &Path, words: &[&str]) -> String {
    let mut out = shell::quote(&program.display().to_string());
    for word in words {
        out.push(' ');
        out.push_str(&shell::quote(word));
    }
    out
}

/// The `PreToolUse` hook on `Bash` that wraps noisy commands in `invoker`.
#[must_use]
pub fn claude_hook(invoker: String) -> claude_agent_sdk::HookMatcher {
    let hook = claude_agent_sdk::hook_fn(move |input: Value, _tool_use_id| {
        let output = hook_output(&input, &invoker);
        async move {
            Ok(match output {
                Some(specific) => claude_agent_sdk::SyncHookJSONOutput {
                    hook_specific_output: Some(specific),
                    ..claude_agent_sdk::SyncHookJSONOutput::default()
                }
                .into(),
                None => claude_agent_sdk::SyncHookJSONOutput::continue_execution().into(),
            })
        }
    });
    claude_agent_sdk::HookMatcher::new(Some("Bash"), hook)
}

/// The squeezer as a Coder SDK hook (#11496): a built-in `PreToolUse`
/// rewrite on `Bash`, chained before the floors so they judge the wrapped
/// command. The same rule as [`claude_hook`], on every backend.
#[must_use]
pub fn sdk_hook(invoker: String) -> coder_sdk::Hook {
    coder_sdk::Hook::new(
        "squeeze",
        coder_sdk::Event::PreToolUse,
        coder_sdk::Source::BuiltIn,
        move |call: &coder_sdk::Call| {
            let input = json!({"tool_name": call.tool, "tool_input": call.input});
            Ok(match hook_output(&input, &invoker) {
                Some(specific) => coder_sdk::HookOutcome::rewrite(specific["updatedInput"].clone()),
                None => coder_sdk::HookOutcome::none(),
            })
        },
    )
    .matching("Bash")
}

/// The hook's `hookSpecificOutput` for one `PreToolUse` call, or `None` to
/// let the call through unchanged.
#[must_use]
pub fn hook_output(input: &Value, invoker: &str) -> Option<Value> {
    if off() || input["tool_name"].as_str() != Some("Bash") {
        return None;
    }
    let tool_input = &input["tool_input"];
    if tool_input["run_in_background"].as_bool() == Some(true) {
        return None;
    }
    let command = tool_input["command"].as_str()?;
    let rewritten = shell::rewrite(command, invoker)?;
    let mut updated = tool_input.clone();
    updated["command"] = Value::String(rewritten);
    Some(json!({"hookEventName": "PreToolUse", "updatedInput": updated}))
}

/// One run of a middle chunk: 0-based line indices, end exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chunk {
    pub start: usize,
    pub end: usize,
}

impl Chunk {
    fn len(self) -> usize {
        self.end - self.start
    }
}

/// The middle lines `[from, to)` cut into chunks: at a blank line once a
/// chunk has [`CHUNK_MIN`] lines, and at [`CHUNK_LINES`] lines.
#[must_use]
pub fn chunks(lines: &[&str], from: usize, to: usize) -> Vec<Chunk> {
    let mut out = Vec::new();
    let mut start = from;
    for i in from..to {
        let size = i + 1 - start;
        if size >= CHUNK_LINES || (size >= CHUNK_MIN && lines[i].trim().is_empty()) {
            out.push(Chunk { start, end: i + 1 });
            start = i + 1;
        }
    }
    if start < to {
        out.push(Chunk { start, end: to });
    }
    out
}

/// Whether a line reports an error, failure, or panic.
#[must_use]
pub fn flagged(line: &str) -> bool {
    let lower = line.to_ascii_lowercase();
    if lower.contains("error") || lower.contains("panicked") || lower.contains("traceback") {
        return !lower.contains("0 errors");
    }
    if lower.contains("exception") || line.contains("FAIL") || lower.starts_with("not ok") {
        return true;
    }
    if let Some(at) = lower.find("failed") {
        // "0 failed" in a passing summary is not a failure.
        return !lower[..at].trim_end().ends_with(" 0") && !lower[..at].trim_end().ends_with(";0");
    }
    lower.contains("assertion")
}

/// How the middle chunks were picked.
#[derive(Debug, Clone, PartialEq)]
pub enum Picked {
    /// The rule's flagged chunks fit the budget.
    Rule,
    /// The Decisions API rated the candidates.
    Decision {
        model: String,
        door: Option<String>,
        latency_ms: u64,
        /// `(chunk index, probability)` for every rated chunk.
        rated: Vec<(usize, f64)>,
    },
    /// The decision call failed; the rule's chunks in order.
    Fallback(String),
}

impl Picked {
    fn word(&self) -> &'static str {
        match self {
            Picked::Rule => "rule",
            Picked::Decision { .. } => "decision",
            Picked::Fallback(_) => "rule (decision unavailable)",
        }
    }
}

/// A decision the squeezer asks: the input text and the candidate chunks'
/// questions, answered with one probability per question name.
pub trait Judge {
    /// `Ok((model, door, latency_ms, probabilities by question name))`.
    ///
    /// # Errors
    ///
    /// The call failed; the caller falls back to the rule.
    #[allow(clippy::type_complexity)]
    fn rate(
        &self,
        input: &str,
        questions: &[(String, String)],
    ) -> Result<(String, Option<String>, u64, Vec<(String, f64)>), String>;
}

/// The Decisions API at [`jev::decisions::OPENAGENTS`] or
/// `OPENAGENTS_DECISIONS_URL`.
pub struct Decisions;

impl Judge for Decisions {
    fn rate(
        &self,
        input: &str,
        questions: &[(String, String)],
    ) -> Result<(String, Option<String>, u64, Vec<(String, f64)>), String> {
        let url = std::env::var("OPENAGENTS_DECISIONS_URL")
            .ok()
            .filter(|url| !url.trim().is_empty())
            .map_or_else(
                || jev::decisions::OPENAGENTS.to_owned(),
                |url| url.trim_end_matches('/').to_owned(),
            );
        let client = jev::decisions::DecisionsClient::new(url)
            .map_err(|error| error.to_string())?
            .timeout(DECISION_TIMEOUT);
        let mut request = jev::decisions::DecisionsRequest::new(MODEL, input);
        for (name, instructions) in questions {
            request = request.predicate(name.clone(), instructions.clone());
        }
        let started = Instant::now();
        // Its own thread and runtime, so it works whether or not the caller
        // is already inside one (the briefed agent's `main` is).
        let response = std::thread::scope(|scope| {
            scope
                .spawn(|| {
                    tokio::runtime::Builder::new_current_thread()
                        .enable_all()
                        .build()
                        .map_err(|error| error.to_string())?
                        .block_on(client.create(&request))
                        .map_err(|error| error.to_string())
                })
                .join()
                .unwrap_or_else(|_| Err("the decision call panicked".to_owned()))
        })?;
        let latency = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
        let rated = questions
            .iter()
            .filter_map(|(name, _)| Some((name.clone(), response.probability(name)?)))
            .collect();
        Ok((response.model, response.openagents.door, latency, rated))
    }
}

/// The question asked of one chunk.
fn question(index: usize) -> String {
    format!(
        "Does [chunk {index}] show something the agent needs to decide its next step: an \
         error, a failing test or assertion, a panic, or a warning about code it is changing? \
         Compile or download progress, passing tests, and routine status lines are not needed."
    )
}

/// The middle chunks to keep, in output order, and how they were picked.
pub fn pick(
    lines: &[&str],
    middle: &[Chunk],
    command: &str,
    status: i32,
    judge: &dyn Judge,
) -> (Vec<usize>, Picked) {
    let flagged_chunks: Vec<usize> = (0..middle.len())
        .filter(|&i| {
            lines[middle[i].start..middle[i].end]
                .iter()
                .any(|line| flagged(line))
        })
        .collect();
    let flagged_lines: usize = flagged_chunks.iter().map(|&i| middle[i].len()).sum();
    if !flagged_chunks.is_empty() && flagged_lines <= MIDDLE_BUDGET {
        return (flagged_chunks, Picked::Rule);
    }
    let pool: Vec<usize> = if flagged_chunks.is_empty() {
        (0..middle.len()).collect()
    } else {
        flagged_chunks.clone()
    };
    // The earliest and the latest candidates: first errors usually cause
    // the rest, and the last ones are closest to the summary.
    let candidates: Vec<usize> = if pool.len() > MAX_QUESTIONS {
        let half = MAX_QUESTIONS / 2;
        pool[..half]
            .iter()
            .chain(&pool[pool.len() - half..])
            .copied()
            .collect()
    } else {
        pool
    };
    let tail_from = lines.len().saturating_sub(10);
    let mut input = format!(
        "A coding agent ran this shell command and must decide its next step.\n\
         Command: {command}\nExit status: {status}\n\nThe output's last lines:\n{}\n\n\
         Chunks from the middle of the output:\n",
        lines[tail_from..].join("\n")
    );
    let mut questions = Vec::new();
    for &i in &candidates {
        let text: String = lines[middle[i].start..middle[i].end]
            .join("\n")
            .chars()
            .take(CHUNK_CHARS)
            .collect();
        let block = format!(
            "\n[chunk {i}] (lines {}-{})\n{text}\n",
            middle[i].start + 1,
            middle[i].end
        );
        if input.len() + block.len() > INPUT_CHARS {
            break;
        }
        input.push_str(&block);
        questions.push((format!("c{i}"), question(i)));
    }
    let in_budget = |order: &mut dyn Iterator<Item = usize>| {
        let mut kept = Vec::new();
        let mut used = 0;
        for i in order {
            if used + middle[i].len() <= MIDDLE_BUDGET {
                used += middle[i].len();
                kept.push(i);
            }
        }
        kept.sort_unstable();
        kept
    };
    match judge.rate(&input, &questions) {
        Ok((model, door, latency_ms, rated)) => {
            let mut rated: Vec<(usize, f64)> = rated
                .into_iter()
                .filter_map(|(name, p)| Some((name.strip_prefix('c')?.parse().ok()?, p)))
                .collect();
            rated.sort_by_key(|&(i, _)| i);
            let mut by_p: Vec<(usize, f64)> = rated
                .iter()
                .copied()
                .filter(|&(_, p)| p >= KEEP_AT)
                .collect();
            by_p.sort_by(|a, b| b.1.total_cmp(&a.1));
            let kept = in_budget(&mut by_p.iter().map(|&(i, _)| i));
            (
                kept,
                Picked::Decision {
                    model,
                    door,
                    latency_ms,
                    rated,
                },
            )
        }
        Err(error) => (
            in_budget(&mut flagged_chunks.into_iter()),
            Picked::Fallback(error),
        ),
    }
}

/// Lines of context kept around a flagged line.
const CONTEXT: usize = 2;

/// The parts of a kept chunk to show: the windows around its flagged
/// lines, or all of it when no line is flagged (a decision kept it whole).
fn shown_ranges(lines: &[&str], chunk: Chunk) -> Vec<std::ops::Range<usize>> {
    let mut out: Vec<std::ops::Range<usize>> = Vec::new();
    for i in (chunk.start..chunk.end).filter(|&i| flagged(lines[i])) {
        let start = i.saturating_sub(CONTEXT).max(chunk.start);
        let end = (i + CONTEXT + 1).min(chunk.end);
        match out.last_mut() {
            Some(last) if start <= last.end => last.end = last.end.max(end),
            _ => out.push(start..end),
        }
    }
    if out.is_empty() {
        out.push(chunk.start..chunk.end);
    }
    out
}

/// The squeezed reply: head, kept chunks with markers for what is omitted,
/// tail, and a footer naming the full output.
#[must_use]
pub fn render(
    lines: &[&str],
    middle: &[Chunk],
    kept: &[usize],
    picked: &Picked,
    full: Option<&Path>,
    status: i32,
) -> String {
    let total = lines.len();
    let head_end = HEAD_LINES.min(total);
    let tail_start = total.saturating_sub(TAIL_LINES).max(head_end);
    let mut out = String::new();
    let mut shown = 0;
    let mut push = |out: &mut String, range: std::ops::Range<usize>| {
        for line in &lines[range.clone()] {
            out.push_str(line);
            out.push('\n');
        }
        shown += range.len();
    };
    push(&mut out, 0..head_end);
    let mut cursor = head_end;
    for range in kept.iter().flat_map(|&i| shown_ranges(lines, middle[i])) {
        if range.start > cursor {
            out.push_str(&format!(
                "… [lines {}-{} omitted] …\n",
                cursor + 1,
                range.start
            ));
        }
        cursor = range.end;
        push(&mut out, range);
    }
    if tail_start > cursor {
        out.push_str(&format!(
            "… [lines {}-{} omitted] …\n",
            cursor + 1,
            tail_start
        ));
    }
    push(&mut out, tail_start..total);
    let where_full = full.map_or_else(
        || "the full output could not be saved; rerun with a filter to see more".to_owned(),
        |path| format!("full output: {} (read it with an offset)", path.display()),
    );
    out.push_str(&format!(
        "[squeezed: showed {shown} of {total} lines, middle chunks kept by {}; {where_full}; exit {status}]\n",
        picked.word()
    ));
    out
}

/// What the squeezer logs for one run: one shared-format [`hook_log::Decision`]
/// (per-chunk ratings in `detail.rated`), as JSON.
fn record(command: &str, status: i32, total: usize, kept: &[usize], picked: &Picked) -> Value {
    let how = match picked {
        Picked::Rule => hook_log::How::Rule,
        Picked::Decision { .. } => hook_log::How::Decision,
        Picked::Fallback(_) => hook_log::How::Fallback,
    };
    let mut d = hook_log::Decision::new("squeeze", "keep_chunk", "keep_chunks", how);
    d.detail = json!({
        "command": command,
        "exit": status,
        "lines": total,
        "kept_chunks": kept,
    });
    match picked {
        Picked::Decision {
            model,
            door,
            latency_ms,
            rated,
        } => {
            d.model = Some(model.clone());
            d.door = door.clone();
            d.latency_ms = Some(*latency_ms);
            d.threshold = Some(KEEP_AT);
            d.detail["rated"] = json!(
                rated
                    .iter()
                    .map(|(i, p)| json!({"chunk": i, "p": p}))
                    .collect::<Vec<_>>()
            );
        }
        Picked::Fallback(error) => d.detail["error"] = json!(error),
        Picked::Rule => {}
    }
    serde_json::to_value(&d).unwrap_or(Value::Null)
}

fn append_log(line: &Value) {
    if let Ok(d) = serde_json::from_value::<hook_log::Decision>(line.clone()) {
        hook_log::append_default(&d);
    }
}

fn spill_path() -> Option<PathBuf> {
    let dir = std::env::var_os(DIR_VAR)
        .filter(|dir| !dir.is_empty())
        .map_or_else(|| std::env::temp_dir().join("coder-squeeze"), PathBuf::from);
    std::fs::create_dir_all(&dir).ok()?;
    let stamp = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |d| d.as_millis());
    Some(dir.join(format!("{stamp}-{}.log", std::process::id())))
}

/// The exit status a shell would report.
fn exit_code(status: std::process::ExitStatus) -> i32 {
    if let Some(code) = status.code() {
        return code;
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::ExitStatusExt;
        if let Some(signal) = status.signal() {
            return 128 + signal;
        }
    }
    1
}

/// Runs `command` in bash with both streams in one pipe, copying it to the
/// full-output file as it arrives; returns the text held in memory, the
/// file, and the exit status.
fn run(command: &str) -> std::io::Result<(String, Option<PathBuf>, i32)> {
    let (mut reader, writer) = std::io::pipe()?;
    let mut child = Command::new("bash")
        .arg("-c")
        .arg(command)
        .stdin(Stdio::inherit())
        .stdout(writer.try_clone()?)
        .stderr(writer)
        .spawn()?;
    let path = spill_path();
    let mut file = path
        .as_ref()
        .and_then(|path| std::fs::File::create(path).ok());
    let mut held = Vec::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = match reader.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => read,
            Err(error) if error.kind() == std::io::ErrorKind::Interrupted => continue,
            Err(error) => return Err(error),
        };
        if let Some(out) = file.as_mut()
            && out.write_all(&buffer[..read]).is_err()
        {
            file = None;
        }
        let room = MEMORY_BYTES.saturating_sub(held.len());
        held.extend_from_slice(&buffer[..read.min(room)]);
    }
    let status = exit_code(child.wait()?);
    let path = if file.is_some() { path } else { None };
    Ok((String::from_utf8_lossy(&held).into_owned(), path, status))
}

/// Squeezes `text`, the output of `command`, with `judge`; returns the
/// reply and the log line, or `None` when it passes unchanged.
pub fn squeeze(
    text: &str,
    command: &str,
    status: i32,
    full: Option<&Path>,
    judge: &dyn Judge,
) -> Option<(String, Value)> {
    let lines: Vec<&str> = text.lines().collect();
    if lines.len() <= PASS_LINES {
        return None;
    }
    let head_end = HEAD_LINES.min(lines.len());
    let tail_start = lines.len().saturating_sub(TAIL_LINES).max(head_end);
    let middle = chunks(&lines, head_end, tail_start);
    let (kept, picked) = pick(&lines, &middle, command, status, judge);
    let reply = render(&lines, &middle, &kept, &picked, full, status);
    let line = record(command, status, lines.len(), &kept, &picked);
    Some((reply, line))
}

/// The squeezer: `args` is the one command to run. Returns its exit status.
#[must_use]
pub fn main(args: &[String]) -> i32 {
    let [command] = args else {
        eprintln!("usage: {CODER_SUBCOMMAND} COMMAND");
        return 2;
    };
    let (text, full, status) = match run(command) {
        Ok(ran) => ran,
        Err(error) => {
            eprintln!("squeeze: could not run the command: {error}");
            return 127;
        }
    };
    let mut stdout = std::io::stdout().lock();
    // Never lose the output: a squeeze that fails shows it unchanged.
    let squeezed =
        std::panic::catch_unwind(|| squeeze(&text, command, status, full.as_deref(), &Decisions))
            .unwrap_or(None);
    match squeezed {
        None => {
            let _ = stdout.write_all(text.as_bytes());
            if let Some(path) = full {
                let _ = std::fs::remove_file(path);
            }
        }
        Some((reply, line)) => {
            let _ = stdout.write_all(reply.as_bytes());
            let mut line = line;
            line["detail"]["full"] = json!(full);
            append_log(&line);
        }
    }
    let _ = stdout.flush();
    status
}

#[cfg(test)]
mod tests;
