//! Bounded, local-only investigation extraction from complete history records.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use base64::Engine;
use coder_history::{CatalogRequest, Harness, History, TranscriptRequest};
use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Phase {
    Discovery,
    Editing,
    Recovery,
}

#[derive(Clone, Debug, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "snake_case")]
pub enum Activity {
    Search,
    Read,
    List,
    History,
    Edit,
    Check,
    Other,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Operation {
    pub call_id: String,
    pub name: String,
    pub arguments: Value,
    pub activity: Activity,
    pub phase: Phase,
    pub at_ms: Option<u64>,
    pub duration_ms: Option<u64>,
    pub failed: Option<bool>,
    pub output_bytes: Option<u64>,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Usage {
    pub prompt: u64,
    pub completion: u64,
    pub observations: usize,
    pub missing: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Episode {
    pub id: String,
    pub family: String,
    pub source_id: String,
    pub turn: usize,
    pub question: String,
    pub repository: Option<String>,
    pub revision: Option<String>,
    pub subagent: bool,
    pub parent: Option<String>,
    pub operations: Vec<Operation>,
    pub usage: Usage,
    /// A transcript's claim, never an independently checked outcome.
    pub claimed_outcome: Option<String>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct SourceCut {
    pub source_id: String,
    pub incarnation: String,
    pub bytes: u64,
    pub digest: String,
    pub pending_line: bool,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Unknown {
    pub source_id: String,
    pub offset: u64,
    pub reason: String,
    pub raw_base64: String,
}

#[derive(Clone, Debug, Default, Serialize, Deserialize)]
pub struct Corpus {
    pub schema: String,
    pub episodes: Vec<Episode>,
    pub cuts: Vec<SourceCut>,
    pub unknown: Vec<Unknown>,
    pub notices: Vec<String>,
}

#[derive(Clone, Copy)]
pub struct Limits {
    pub sources: usize,
    pub source_bytes: u64,
    pub record_bytes: usize,
    pub total_bytes: u64,
    pub before_ms: Option<u64>,
}

impl Default for Limits {
    fn default() -> Self {
        Self {
            sources: 4096,
            source_bytes: 64 * 1024 * 1024,
            record_bytes: 4 * 1024 * 1024,
            total_bytes: 512 * 1024 * 1024,
            before_ms: None,
        }
    }
}

pub fn import(history: &History, repository: &Path, limits: Limits) -> Result<Corpus, String> {
    let mut corpus = Corpus {
        schema: "openagents.investigations.v1".into(),
        ..Corpus::default()
    };
    let mut request = CatalogRequest::default();
    let mut count = 0;
    let mut total = 0;
    loop {
        let page = history.catalog(request).map_err(|e| e.to_string())?;
        for notice in page.notices {
            corpus.notices.push(notice.code);
        }
        for chat in page.entries {
            count += 1;
            if count > limits.sources {
                return Err("The selected histories exceed the source limit.".into());
            }
            let Some(source_id) = chat.source_id else {
                continue;
            };
            let Some((incarnation, cut)) = history.source_length(&source_id) else {
                continue;
            };
            if cut > limits.source_bytes {
                corpus.notices.push(format!("source_too_large:{source_id}"));
                continue;
            }
            total += cut;
            if total > limits.total_bytes {
                return Err("The selected histories exceed the byte limit.".into());
            }
            let mut bytes = Vec::new();
            let mut cursor = None;
            while (bytes.len() as u64) < cut {
                let page = history
                    .transcript(TranscriptRequest {
                        source_id: source_id.clone(),
                        cursor,
                        max_bytes: 32 * 1024,
                        end: None,
                    })
                    .map_err(|e| e.to_string())?;
                if page.incarnation != incarnation {
                    return Err("A history file changed during the read.".into());
                }
                let previous = bytes.len();
                for chunk in page.chunks {
                    if chunk.offset >= cut {
                        break;
                    }
                    if chunk.offset != bytes.len() as u64 {
                        return Err("A history page has a gap.".into());
                    }
                    let raw = base64::engine::general_purpose::STANDARD
                        .decode(&chunk.raw_base64)
                        .map_err(|_| "A history page could not be decoded.")?;
                    if chunk.end_offset != chunk.offset + raw.len() as u64 {
                        return Err("A history page has a wrong length.".into());
                    }
                    let remaining = usize::try_from(cut - chunk.offset)
                        .map_err(|_| "History size is too large.")?;
                    bytes.extend_from_slice(&raw[..remaining.min(raw.len())]);
                }
                cursor = Some(page.next);
                if bytes.len() == previous {
                    return Err("A history read stopped before its saved cut.".into());
                }
            }
            decode(
                &mut corpus,
                &source_id,
                chat.native_id.as_deref(),
                chat.harness,
                chat.subagent,
                &bytes,
                repository,
                limits,
            )?;
            corpus.cuts.push(SourceCut {
                source_id,
                incarnation,
                bytes: cut,
                digest: crate::digest(&bytes),
                pending_line: !bytes.is_empty() && !bytes.ends_with(b"\n"),
            });
        }
        let Some(next) = page.next else { break };
        request = CatalogRequest {
            cursor: Some(next),
            limit: 32,
        };
    }
    Ok(corpus)
}

fn text<'a>(value: &'a Value, field: &str) -> Option<&'a str> {
    value.get(field).and_then(Value::as_str)
}

fn time(value: &Value) -> Option<u64> {
    text(value, "timestamp")
        .and_then(atif::parse_iso)
        .or_else(|| value.get("at").and_then(Value::as_u64))
}

fn content(value: &Value) -> String {
    match value {
        Value::String(s) => s.clone(),
        Value::Array(items) => items
            .iter()
            .filter_map(|v| text(v, "text"))
            .collect::<Vec<_>>()
            .join("\n"),
        _ => String::new(),
    }
}

fn activity(name: &str, args: &Value) -> Activity {
    match name.rsplit('.').next().unwrap_or(name) {
        "Read" => return Activity::Read,
        "Grep" => return Activity::Search,
        "Glob" => return Activity::List,
        "Edit" | "Write" | "apply_patch" => return Activity::Edit,
        _ => {}
    }
    let literals = args.as_str().map(literal_commands).unwrap_or_default();
    let command = text(args, "command")
        .or_else(|| text(args, "cmd"))
        .or_else(|| literals.first().map(String::as_str))
        .unwrap_or("");
    let words: Vec<_> = command.split_whitespace().collect();
    if words.iter().any(|v| matches!(*v, "apply_patch")) {
        Activity::Edit
    } else if words
        .iter()
        .any(|v| matches!(*v, "cargo" | "rustc" | "pytest"))
    {
        Activity::Check
    } else if words
        .iter()
        .any(|v| matches!(*v, "rg" | "grep" | "find" | "fd"))
    {
        Activity::Search
    } else if words
        .iter()
        .any(|v| matches!(*v, "cat" | "sed" | "head" | "tail"))
    {
        Activity::Read
    } else if words.iter().any(|v| matches!(*v, "ls" | "tree" | "pwd")) {
        Activity::List
    } else if words.first() == Some(&"git") {
        Activity::History
    } else {
        Activity::Other
    }
}

/// Decode quoted shell fields in an orchestration call, without evaluating code.
pub fn literal_commands(source: &str) -> Vec<String> {
    let mut out = Vec::new();
    let mut rest = source;
    while let Some(at) = rest.find("cmd").or_else(|| rest.find("command")) {
        rest = &rest[at..];
        let key = if rest.starts_with("command") { 7 } else { 3 };
        let tail = rest[key..].trim_start_matches(['\'', '"']).trim_start();
        rest = &rest[key..];
        let Some(value) = tail.strip_prefix(':').map(str::trim_start) else {
            continue;
        };
        let Some(quote) = value.chars().next().filter(|q| matches!(q, '"' | '\'')) else {
            continue;
        };
        let mut escaped = false;
        for (i, c) in value.char_indices().skip(1) {
            if c == quote && !escaped {
                let raw = &value[..i + 1];
                let decoded = if quote == '"' {
                    serde_json::from_str::<String>(raw).ok()
                } else {
                    Some(
                        raw[1..raw.len() - 1]
                            .replace("\\'", "'")
                            .replace("\\n", "\n")
                            .replace("\\\\", "\\"),
                    )
                };
                if let Some(command) = decoded {
                    out.push(command);
                }
                rest = &value[i + 1..];
                break;
            }
            escaped = c == '\\' && !escaped;
        }
    }
    out
}

#[allow(clippy::too_many_arguments)]
pub fn decode(
    corpus: &mut Corpus,
    source: &str,
    native: Option<&str>,
    harness: Harness,
    subagent: bool,
    bytes: &[u8],
    repository: &Path,
    limits: Limits,
) -> Result<(), String> {
    let mut current: Option<Episode> = None;
    let mut repo = None;
    let mut revision = None;
    let mut parent = None;
    let mut active = false;
    let mut turn = 0;
    let mut seen = BTreeSet::new();
    let mut cumulative = (0u64, 0u64);
    let mut requests = BTreeMap::<String, (u64, u64)>::new();
    let mut phase = Phase::Discovery;
    let mut offset = 0;
    for raw in bytes.split_inclusive(|b| *b == b'\n') {
        let start = offset;
        offset += raw.len();
        let unknown = |corpus: &mut Corpus, reason: &str| {
            corpus.unknown.push(Unknown {
                source_id: source.into(),
                offset: start as u64,
                reason: reason.into(),
                raw_base64: base64::engine::general_purpose::STANDARD.encode(raw),
            })
        };
        if raw.len() > limits.record_bytes {
            unknown(corpus, "record_too_large");
            continue;
        }
        if !raw.ends_with(b"\n") {
            unknown(corpus, "partial_tail");
            continue;
        }
        let Ok(v) = serde_json::from_slice::<Value>(raw) else {
            unknown(corpus, "invalid_json");
            continue;
        };
        let p = v.get("payload").unwrap_or(&v);
        let record_type = text(&v, "type")
            .or_else(|| text(&v, "record"))
            .unwrap_or("");
        let timestamp = time(&v).or_else(|| time(p));
        if timestamp.is_some_and(|t| limits.before_ms.is_some_and(|cut| t >= cut)) {
            continue;
        }
        let session = v.get("session").unwrap_or(p);
        let cwd = text(&v, "cwd")
            .or_else(|| text(p, "cwd"))
            .or_else(|| text(session, "repository"));
        if let Some(cwd) = cwd {
            let path = Path::new(cwd);
            active = path == repository || path.starts_with(repository);
            repo = Some(cwd.to_owned());
        }
        if let Some(base) = p
            .pointer("/git/commit_hash")
            .and_then(Value::as_str)
            .or_else(|| text(p, "base_commit"))
        {
            revision = Some(base.to_owned());
        }
        if let Some(id) = text(&v, "parentSessionId").or_else(|| text(p, "parent_session_id")) {
            parent = Some(id.to_owned());
        }
        if !active {
            continue;
        }
        let step = v.get("step").unwrap_or(&v);
        let message = v.get("message").unwrap_or(p);
        let role = text(message, "role").or_else(|| text(step, "source"));
        let user = match harness {
            Harness::Claude => {
                record_type == "user"
                    && message.get("content").is_some_and(|c| {
                        c.is_string()
                            || c.as_array()
                                .is_some_and(|a| a.iter().any(|b| text(b, "type") == Some("text")))
                    })
            }
            Harness::Codex => record_type == "response_item" && role == Some("user"),
            _ => record_type == "step" && role.is_some_and(|r| r.eq_ignore_ascii_case("user")),
        };
        if user {
            if let Some(e) = current.take() {
                corpus.episodes.push(e);
            }
            turn += 1;
            phase = Phase::Discovery;
            let question = if matches!(harness, Harness::Coder) {
                text(step, "message").unwrap_or("").to_owned()
            } else {
                content(message.get("content").unwrap_or(&Value::Null))
            };
            let family = parent.as_deref().or(native).unwrap_or(source).to_owned();
            current = Some(Episode {
                id: crate::digest(format!("{source}:{start}").as_bytes()),
                family,
                source_id: source.into(),
                turn,
                question,
                repository: repo.clone(),
                revision: revision.clone(),
                subagent,
                parent: parent.clone(),
                operations: Vec::new(),
                usage: Usage {
                    missing: true,
                    ..Usage::default()
                },
                claimed_outcome: None,
            });
        }
        let Some(episode) = current.as_mut() else {
            continue;
        };
        let mut calls = Vec::<(String, String, Value)>::new();
        let mut results = Vec::<(String, Option<bool>, Option<u64>, Option<u64>)>::new();
        if matches!(harness, Harness::Claude) {
            if let Some(blocks) = message.get("content").and_then(Value::as_array) {
                for block in blocks {
                    match text(block, "type") {
                        Some("tool_use") => calls.push((
                            text(block, "id").unwrap_or("").into(),
                            text(block, "name").unwrap_or("").into(),
                            block.get("input").cloned().unwrap_or(Value::Null),
                        )),
                        Some("tool_result") => {
                            results.push((
                                text(block, "tool_use_id").unwrap_or("").into(),
                                block.get("is_error").and_then(Value::as_bool),
                                None,
                                Some(content(block.get("content").unwrap_or(&Value::Null)).len()
                                    as u64),
                            ))
                        }
                        _ => {}
                    }
                }
            }
            if let Some(usage) = message.get("usage") {
                let id = text(message, "id")
                    .or_else(|| text(&v, "requestId"))
                    .unwrap_or("");
                if !id.is_empty() {
                    if let (Some(input), Some(output)) = (
                        usage.get("input_tokens").and_then(Value::as_u64),
                        usage.get("output_tokens").and_then(Value::as_u64),
                    ) {
                        let input = input
                            + usage
                                .get("cache_read_input_tokens")
                                .and_then(Value::as_u64)
                                .unwrap_or(0)
                            + usage
                                .get("cache_creation_input_tokens")
                                .and_then(Value::as_u64)
                                .unwrap_or(0);
                        let previous = requests.entry(id.to_owned()).or_default();
                        episode.usage.prompt += input.saturating_sub(previous.0);
                        episode.usage.completion += output.saturating_sub(previous.1);
                        if *previous == (0, 0) {
                            episode.usage.observations += 1;
                        }
                        *previous = (previous.0.max(input), previous.1.max(output));
                        episode.usage.missing = false;
                    }
                }
            }
        } else if matches!(harness, Harness::Codex) {
            match text(p, "type") {
                Some("function_call" | "custom_tool_call") => {
                    let args = p
                        .get("arguments")
                        .or_else(|| p.get("input"))
                        .cloned()
                        .unwrap_or(Value::Null);
                    let args = args
                        .as_str()
                        .and_then(|s| serde_json::from_str(s).ok())
                        .unwrap_or(args);
                    calls.push((
                        text(p, "call_id")
                            .or_else(|| text(p, "id"))
                            .unwrap_or("")
                            .into(),
                        text(p, "name").unwrap_or("").into(),
                        args,
                    ));
                }
                Some("function_call_output" | "custom_tool_call_output") => results.push((
                    text(p, "call_id").unwrap_or("").into(),
                    None,
                    None,
                    Some(content(p.get("output").unwrap_or(&Value::Null)).len() as u64),
                )),
                _ if text(p, "type") == Some("token_count")
                    || record_type == "token_usage_record" =>
                {
                    if let Some(usage) = p
                        .pointer("/info/total_token_usage")
                        .or_else(|| p.get("thread_token_usage"))
                    {
                        let pair = (
                            usage.get("input_tokens").and_then(Value::as_u64),
                            usage.get("output_tokens").and_then(Value::as_u64),
                        );
                        if let (Some(input), Some(output)) = pair {
                            if input >= cumulative.0 && output >= cumulative.1 {
                                episode.usage.prompt += input - cumulative.0;
                                episode.usage.completion += output - cumulative.1;
                                episode.usage.observations += 1;
                                episode.usage.missing = false;
                            } else {
                                corpus.notices.push(format!("usage_reset:{source}:{start}"));
                            }
                            cumulative = (input, output);
                        }
                    }
                }
                _ => {}
            }
        } else if record_type == "step" {
            if let Ok(s) = serde_json::from_value::<atif::Step>(step.clone()) {
                if let Some(call) = s.call {
                    results.push((
                        call.id.clone(),
                        Some(call.outcome == atif::Outcome::Failed),
                        Some(call.milliseconds),
                        Some(call.output.len() as u64),
                    ));
                    calls.push((call.id, call.name, call.arguments));
                }
                if let Some((input, output)) = s.tokens {
                    episode.usage.prompt += input;
                    episode.usage.completion += output;
                    episode.usage.observations += 1;
                    episode.usage.missing = false;
                }
            } else {
                unknown(corpus, "unsupported_step");
            }
        }
        for (id, name, arguments) in calls {
            let id = if id.is_empty() {
                format!("offset-{start}-{}", episode.operations.len())
            } else {
                id
            };
            if !seen.insert(id.clone()) {
                continue;
            }
            let activity = activity(&name, &arguments);
            if activity == Activity::Edit {
                phase = Phase::Editing;
            }
            episode.operations.push(Operation {
                call_id: id,
                name,
                arguments,
                activity,
                phase: phase.clone(),
                at_ms: timestamp,
                duration_ms: None,
                failed: None,
                output_bytes: None,
            });
        }
        for (id, failed, duration, size) in results {
            if let Some(operation) = episode.operations.iter_mut().find(|op| op.call_id == id) {
                operation.failed = failed;
                operation.duration_ms = duration;
                operation.output_bytes = size;
                if operation.activity == Activity::Check && failed == Some(true) {
                    phase = Phase::Recovery;
                }
            }
        }
        if record_type == "end" || matches!(text(p, "type"), Some("task_complete")) {
            episode.claimed_outcome = text(&v, "state")
                .or_else(|| text(p, "last_agent_message"))
                .map(str::to_owned);
        }
        let recognized = matches!(
            record_type,
            "user"
                | "assistant"
                | "response_item"
                | "event_msg"
                | "session_meta"
                | "turn_context"
                | "step"
                | "session"
                | "end"
        );
        if !recognized {
            unknown(corpus, "unknown_record");
        }
    }
    if let Some(e) = current {
        corpus.episodes.push(e);
    }
    Ok(())
}

pub fn save(dir: &Path, corpus: &Corpus) -> Result<(), String> {
    let bytes = serde_json::to_vec(corpus).map_err(|e| e.to_string())?;
    crate::private::write(dir, "episodes.json", &bytes)
}

/// Export aggregate counts only. Questions, arguments, and source IDs remain local.
pub fn summary(corpus: &Corpus) -> Value {
    let mut kinds = BTreeMap::<String, usize>::new();
    for e in &corpus.episodes {
        for op in &e.operations {
            *kinds
                .entry(format!("{:?}", op.activity).to_lowercase())
                .or_default() += 1;
        }
    }
    json!({"schema":"openagents.investigation-summary.v1","episodes":corpus.episodes.len(),
        "sources":corpus.cuts.len(),"unknown_records":corpus.unknown.len(),"activities":kinds,
        "episodes_without_revision":corpus.episodes.iter().filter(|e|e.revision.is_none()).count()})
}
