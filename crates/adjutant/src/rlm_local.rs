//! Local RLM executor for large-context analysis.
//!
//! Provides context operations (peek/grep/partition/map/summarize) with budgets,
//! logs tool calls for training, and synthesizes a final answer.

use crate::dspy::{LabeledToolCall, TrainingCollector};
use crate::{AdjutantError, Task, TaskPlan, ToolRegistry};
use chrono::Utc;
use dsrs::{Chat, LM, Message};
use serde::Deserialize;
use serde_json::json;
use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::Arc;

const CONTROLLER_SYSTEM_PROMPT: &str = "You are an RLM controller. Output JSON only.";
const MAP_SYSTEM_PROMPT: &str = "You are an RLM context mapper. Be concise and evidence-based.";
const SUMMARIZE_SYSTEM_PROMPT: &str =
    "You are an RLM summarizer. Extract only what is relevant to the task.";
const SYNTHESIZE_SYSTEM_PROMPT: &str =
    "You are an RLM synthesis agent. Provide a grounded summary and next steps.";

#[derive(Debug, Clone)]
pub struct RlmExecutorConfig {
    pub max_steps: usize,
    pub max_subcalls: usize,
    pub max_map_calls: usize,
    pub max_summary_calls: usize,
    pub max_recursion_depth: usize,
    pub max_peek_lines: usize,
    pub max_grep_lines: usize,
    pub max_chunk_chars: usize,
    pub max_digest_chars: usize,
    pub max_partition_items: usize,
}

impl Default for RlmExecutorConfig {
    fn default() -> Self {
        Self {
            max_steps: 8,
            max_subcalls: 12,
            max_map_calls: 6,
            max_summary_calls: 3,
            max_recursion_depth: 2,
            max_peek_lines: 200,
            max_grep_lines: 200,
            max_chunk_chars: 6000,
            max_digest_chars: 12000,
            max_partition_items: 8,
        }
    }
}

#[derive(Debug)]
pub struct RlmExecutionResult {
    pub summary: String,
    pub context_handle: String,
    pub repeated_actions: bool,
    pub steps: usize,
}

#[derive(Debug, Clone)]
enum RlmChunkKind {
    Peek,
    Grep,
    Partition,
    Map,
    Summary,
}

impl RlmChunkKind {
    fn as_str(&self) -> &'static str {
        match self {
            RlmChunkKind::Peek => "peek",
            RlmChunkKind::Grep => "grep",
            RlmChunkKind::Partition => "partition",
            RlmChunkKind::Map => "map",
            RlmChunkKind::Summary => "summary",
        }
    }

    fn priority(&self) -> u8 {
        match self {
            RlmChunkKind::Summary => 0,
            RlmChunkKind::Map => 1,
            RlmChunkKind::Grep => 2,
            RlmChunkKind::Peek => 3,
            RlmChunkKind::Partition => 4,
        }
    }
}

#[derive(Debug, Clone)]
struct RlmContextChunk {
    id: String,
    kind: RlmChunkKind,
    label: String,
    source: Option<String>,
    content: String,
}

struct RlmContextStore {
    run_id: String,
    chunks: Vec<RlmContextChunk>,
    next_id: usize,
}

impl RlmContextStore {
    fn new(run_id: String) -> Self {
        Self {
            run_id,
            chunks: Vec::new(),
            next_id: 1,
        }
    }

    fn handle(&self) -> String {
        format!("rlm://{}", self.run_id)
    }

    fn add_chunk(
        &mut self,
        kind: RlmChunkKind,
        label: impl Into<String>,
        source: Option<String>,
        content: String,
    ) -> String {
        let id = format!("c{}", self.next_id);
        self.next_id += 1;
        self.chunks.push(RlmContextChunk {
            id: id.clone(),
            kind,
            label: label.into(),
            source,
            content,
        });
        id
    }

    fn get_chunk(&self, id: &str) -> Option<&RlmContextChunk> {
        self.chunks.iter().find(|chunk| chunk.id == id)
    }

    fn list_chunks(&self) -> String {
        if self.chunks.is_empty() {
            return "none".to_string();
        }
        self.chunks
            .iter()
            .map(|chunk| {
                format!(
                    "{} [{}] {} ({} chars)",
                    chunk.id,
                    chunk.kind.as_str(),
                    chunk.label,
                    chunk.content.len()
                )
            })
            .collect::<Vec<_>>()
            .join("\n")
    }

    fn digest(&self, max_chars: usize) -> String {
        if max_chars == 0 {
            return String::new();
        }
        let mut chunks: Vec<&RlmContextChunk> = self.chunks.iter().collect();
        chunks.sort_by_key(|chunk| chunk.kind.priority());

        let mut out = String::new();
        for chunk in chunks {
            if out.len() >= max_chars {
                break;
            }
            let header = format!(
                "--- {} [{}] {} ---\n",
                chunk.id,
                chunk.kind.as_str(),
                chunk.label
            );
            let remaining = max_chars.saturating_sub(out.len());
            if remaining <= header.len() {
                break;
            }
            out.push_str(&header);
            let content_limit = remaining.saturating_sub(header.len());
            out.push_str(&truncate_chars(&chunk.content, content_limit));
            out.push('\n');
        }
        out
    }
}

#[derive(Debug, Deserialize)]
struct RlmActionPlan {
    actions: Vec<RlmAction>,
}

#[derive(Debug, Deserialize)]
#[serde(tag = "tool", rename_all = "snake_case")]
enum RlmAction {
    Peek {
        path: String,
        start_line: Option<usize>,
        end_line: Option<usize>,
    },
    Grep {
        pattern: String,
        scope: Option<String>,
    },
    Partition {
        scope: Option<String>,
        strategy: Option<String>,
        max_items: Option<usize>,
    },
    Map {
        query: String,
        chunk_ids: Vec<String>,
    },
    Summarize {
        chunk_ids: Vec<String>,
    },
    Finish {
        answer: Option<String>,
    },
}

pub struct RlmLocalExecutor<'a> {
    tools: &'a ToolRegistry,
    lm: Arc<LM>,
    training: Option<&'a TrainingCollector>,
    config: RlmExecutorConfig,
    context: RlmContextStore,
    action_history: HashSet<String>,
    repeated_actions: bool,
    steps: usize,
    subcalls: usize,
    map_calls: usize,
    summary_calls: usize,
}

impl<'a> RlmLocalExecutor<'a> {
    pub fn new(
        tools: &'a ToolRegistry,
        lm: Arc<LM>,
        training: Option<&'a TrainingCollector>,
    ) -> Self {
        let run_id = format!("rlm-{}", Utc::now().timestamp_millis());
        Self {
            tools,
            lm,
            training,
            config: RlmExecutorConfig::default(),
            context: RlmContextStore::new(run_id),
            action_history: HashSet::new(),
            repeated_actions: false,
            steps: 0,
            subcalls: 0,
            map_calls: 0,
            summary_calls: 0,
        }
    }

    pub fn with_config(mut self, config: RlmExecutorConfig) -> Self {
        self.config = config;
        self
    }

    pub async fn execute(
        &mut self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<RlmExecutionResult, AdjutantError> {
        let mut finish_answer = None;
        let mut executed_any = false;

        if let Some(actions) = self.request_action_plan(task, plan).await {
            for action in actions {
                if self.steps >= self.config.max_steps {
                    break;
                }
                executed_any = true;
                if let Some(answer) = self.execute_action(action, task, plan).await? {
                    finish_answer = Some(answer);
                    break;
                }
            }
        }

        if !executed_any {
            self.run_fallback(task, plan).await?;
        }

        let summary = match finish_answer {
            Some(answer) => answer,
            None => self.synthesize_answer(task, plan).await?,
        };

        Ok(RlmExecutionResult {
            summary,
            context_handle: self.context.handle(),
            repeated_actions: self.repeated_actions,
            steps: self.steps,
        })
    }

    async fn request_action_plan(
        &mut self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Option<Vec<RlmAction>> {
        let files_preview = plan
            .files
            .iter()
            .take(20)
            .map(|path| path.display().to_string())
            .collect::<Vec<_>>()
            .join("\n");
        let chunk_list = self.context.list_chunks();

        let prompt = format!(
            "Task: {}\n\nDescription: {}\n\nPlan summary: {}\n\nFiles (sample):\n{}\n\n\
             Context handle: {}\n\nExisting chunks:\n{}\n\n\
             Constraints:\n- max_steps: {}\n- max_subcalls: {}\n- max_map_calls: {}\n\
             - max_summary_calls: {}\n- max_recursion_depth: {}\n\n\
             Choose context actions to answer the task.\n\
             Output JSON only. Format:\n\
             {{\"actions\":[{{\"tool\":\"grep\",\"pattern\":\"...\",\"scope\":\"...\"}}, ...]}}\n",
            task.title,
            task.description,
            plan.summary,
            files_preview,
            self.context.handle(),
            chunk_list,
            self.config.max_steps,
            self.config.max_subcalls,
            self.config.max_map_calls,
            self.config.max_summary_calls,
            self.config.max_recursion_depth
        );

        let (raw, tokens) = match self.call_lm(CONTROLLER_SYSTEM_PROMPT, &prompt).await {
            Ok(output) => output,
            Err(_) => return None,
        };

        let actions = parse_actions(&raw);
        self.record_tool_call(
            "rlm.plan",
            json!({
                "task_title": task.title,
                "task_description": task.description,
                "file_count": plan.files.len(),
            }),
            json!({ "raw": raw }),
            tokens,
            false,
        );
        actions
    }

    async fn execute_action(
        &mut self,
        action: RlmAction,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<Option<String>, AdjutantError> {
        if self.steps >= self.config.max_steps {
            return Ok(None);
        }
        self.steps += 1;

        let action_key = format!("{:?}", action);
        let was_repeated = !self.action_history.insert(action_key);
        if was_repeated {
            self.repeated_actions = true;
        }

        match action {
            RlmAction::Peek {
                path,
                start_line,
                end_line,
            } => {
                let start = start_line.unwrap_or(1).max(1);
                let end = end_line.unwrap_or(start + self.config.max_peek_lines.saturating_sub(1));
                let snippet = self
                    .peek_file(Path::new(&path), start_line, end_line)
                    .await?;
                let label = format!("{}:{}-{}", path, start, end);
                let chunk_id = self.context.add_chunk(
                    RlmChunkKind::Peek,
                    label.clone(),
                    Some(path.clone()),
                    snippet.clone(),
                );
                self.record_tool_call(
                    "rlm.peek",
                    json!({ "path": path, "start_line": start_line, "end_line": end_line }),
                    json!({ "chunk_id": chunk_id, "chars": snippet.len() }),
                    0,
                    was_repeated,
                );
            }
            RlmAction::Grep { pattern, scope } => {
                let output = self.grep(&pattern, scope.as_deref()).await?;
                let label = scope.clone().unwrap_or_else(|| "workspace".to_string());
                let chunk_id = self.context.add_chunk(
                    RlmChunkKind::Grep,
                    format!("{} in {}", pattern, label),
                    scope.clone(),
                    output.clone(),
                );
                self.record_tool_call(
                    "rlm.grep",
                    json!({ "pattern": pattern, "scope": scope }),
                    json!({ "chunk_id": chunk_id, "chars": output.len() }),
                    0,
                    was_repeated,
                );
            }
            RlmAction::Partition {
                scope,
                strategy,
                max_items,
            } => {
                let chunk_ids = self
                    .partition_files(plan, scope.as_deref(), strategy.as_deref(), max_items)
                    .await?;
                self.record_tool_call(
                    "rlm.partition",
                    json!({
                        "scope": scope,
                        "strategy": strategy,
                        "max_items": max_items
                    }),
                    json!({ "chunk_ids": chunk_ids }),
                    0,
                    was_repeated,
                );
            }
            RlmAction::Map { query, chunk_ids } => {
                if self.map_calls >= self.config.max_map_calls
                    || self.subcalls >= self.config.max_subcalls
                {
                    return Ok(None);
                }
                let (outputs, tokens) = self.map_chunks(task, &query, &chunk_ids).await?;
                self.record_tool_call(
                    "rlm.map",
                    json!({ "query": query, "chunk_ids": chunk_ids }),
                    json!({ "output_chunk_ids": outputs }),
                    tokens,
                    was_repeated,
                );
            }
            RlmAction::Summarize { chunk_ids } => {
                if self.summary_calls >= self.config.max_summary_calls
                    || self.subcalls >= self.config.max_subcalls
                {
                    return Ok(None);
                }
                let (chunk_id, tokens) = self.summarize_chunks(task, &chunk_ids).await?;
                self.record_tool_call(
                    "rlm.summarize",
                    json!({ "chunk_ids": chunk_ids }),
                    json!({ "chunk_id": chunk_id }),
                    tokens,
                    was_repeated,
                );
            }
            RlmAction::Finish { answer } => {
                if let Some(answer) = answer {
                    self.record_tool_call(
                        "rlm.finish",
                        json!({}),
                        json!({ "answer": answer }),
                        0,
                        was_repeated,
                    );
                    return Ok(Some(answer));
                }
            }
        }

        Ok(None)
    }

    async fn run_fallback(&mut self, task: &Task, plan: &TaskPlan) -> Result<(), AdjutantError> {
        let chunk_ids = self
            .partition_files(plan, Some("plan_files"), Some("balanced"), None)
            .await?;
        if chunk_ids.is_empty() {
            return Ok(());
        }
        let (map_outputs, map_tokens) = self
            .map_chunks(
                task,
                "Find key facts and relevant code references.",
                &chunk_ids,
            )
            .await?;
        if !map_outputs.is_empty() {
            let map_outputs_clone = map_outputs.clone();
            self.record_tool_call(
                "rlm.map",
                json!({
                    "query": "Find key facts and relevant code references.",
                    "chunk_ids": chunk_ids.clone()
                }),
                json!({ "output_chunk_ids": map_outputs_clone }),
                map_tokens,
                false,
            );
        }

        let summarize_ids = if map_outputs.is_empty() {
            &chunk_ids
        } else {
            &map_outputs
        };
        let (summary_id, summary_tokens) = self.summarize_chunks(task, summarize_ids).await?;
        if !summary_id.is_empty() {
            self.record_tool_call(
                "rlm.summarize",
                json!({ "chunk_ids": summarize_ids }),
                json!({ "chunk_id": summary_id }),
                summary_tokens,
                false,
            );
        }
        Ok(())
    }

    async fn peek_file(
        &self,
        path: &Path,
        start_line: Option<usize>,
        end_line: Option<usize>,
    ) -> Result<String, AdjutantError> {
        let result = self.tools.read(path).await?;
        if !result.success {
            return Ok(result.error.unwrap_or_else(|| "peek failed".to_string()));
        }
        let lines: Vec<&str> = result.content.lines().collect();
        if lines.is_empty() {
            return Ok(String::new());
        }
        let start = start_line.unwrap_or(1).max(1);
        let end = end_line.unwrap_or(start + self.config.max_peek_lines.saturating_sub(1));
        let start_idx = start.saturating_sub(1).min(lines.len());
        let end_idx = end.min(lines.len());
        if start_idx >= end_idx {
            return Ok(String::new());
        }
        Ok(lines[start_idx..end_idx].join("\n"))
    }

    async fn grep(&self, pattern: &str, scope: Option<&str>) -> Result<String, AdjutantError> {
        let scope_path = scope.filter(|s| !s.is_empty()).map(|s| PathBuf::from(s));
        let output = self.tools.grep(pattern, scope_path.as_deref()).await?;
        if !output.success {
            return Ok(output.error.unwrap_or_else(|| "grep failed".to_string()));
        }
        Ok(truncate_lines(&output.content, self.config.max_grep_lines))
    }

    async fn partition_files(
        &mut self,
        plan: &TaskPlan,
        scope: Option<&str>,
        strategy: Option<&str>,
        max_items: Option<usize>,
    ) -> Result<Vec<String>, AdjutantError> {
        let files = if matches!(scope, Some("plan_files") | None) {
            &plan.files
        } else {
            &plan.files
        };

        if files.is_empty() {
            return Ok(Vec::new());
        }

        let mut max_items = max_items.unwrap_or(self.config.max_partition_items).max(1);
        if matches!(strategy, Some("by_file")) {
            max_items = 1;
        }
        let mut chunk_ids = Vec::new();
        let mut current_files: Vec<PathBuf> = Vec::new();
        let mut current_content = String::new();
        let max_file_chars = (self.config.max_chunk_chars / max_items).max(256);

        for file in files.iter() {
            let snippet = self.read_file_truncated(file, max_file_chars).await?;
            if !current_files.is_empty() && current_files.len() >= max_items {
                let label = format!(
                    "partition {}-{}",
                    current_files.first().unwrap().display(),
                    current_files.last().unwrap().display()
                );
                let chunk_id = self.context.add_chunk(
                    RlmChunkKind::Partition,
                    label,
                    None,
                    current_content.clone(),
                );
                chunk_ids.push(chunk_id);
                current_files.clear();
                current_content.clear();
            }

            current_files.push(file.clone());
            current_content.push_str(&format!("\n--- {} ---\n{}\n", file.display(), snippet));
        }

        if !current_files.is_empty() {
            let label = format!(
                "partition {}-{}",
                current_files.first().unwrap().display(),
                current_files.last().unwrap().display()
            );
            let chunk_id =
                self.context
                    .add_chunk(RlmChunkKind::Partition, label, None, current_content);
            chunk_ids.push(chunk_id);
        }

        Ok(chunk_ids)
    }

    async fn read_file_truncated(
        &self,
        path: &Path,
        max_chars: usize,
    ) -> Result<String, AdjutantError> {
        let result = self.tools.read(path).await?;
        if !result.success {
            return Ok(result.error.unwrap_or_else(|| "read failed".to_string()));
        }
        Ok(truncate_chars(&result.content, max_chars))
    }

    async fn map_chunks(
        &mut self,
        task: &Task,
        query: &str,
        chunk_ids: &[String],
    ) -> Result<(Vec<String>, u32), AdjutantError> {
        let mut outputs = Vec::new();
        let mut tokens_used: u32 = 0;
        let remaining = self.config.max_map_calls.saturating_sub(self.map_calls);

        for chunk_id in chunk_ids.iter().take(remaining) {
            if self.subcalls >= self.config.max_subcalls
                || self.map_calls >= self.config.max_map_calls
            {
                break;
            }
            let Some(chunk) = self.context.get_chunk(chunk_id) else {
                continue;
            };
            let query_label = truncate_chars(query, 80);
            let prompt = format!(
                "Task: {}\n\nDescription: {}\n\nQuery: {}\n\nContext chunk ({})\n{}\n\n\
                 Return concise findings and cite the chunk id when relevant.",
                task.title,
                task.description,
                query,
                chunk.id,
                truncate_chars(&chunk.content, self.config.max_chunk_chars)
            );
            let (output, tokens) = self.call_lm(MAP_SYSTEM_PROMPT, &prompt).await?;
            tokens_used = tokens_used.saturating_add(tokens);
            self.subcalls += 1;
            self.map_calls += 1;
            let label = format!("map {} -> {}", chunk.id, query_label);
            let output_id = self.context.add_chunk(
                RlmChunkKind::Map,
                label,
                chunk.source.clone(),
                output.clone(),
            );
            outputs.push(output_id);
        }

        Ok((outputs, tokens_used))
    }

    async fn summarize_chunks(
        &mut self,
        task: &Task,
        chunk_ids: &[String],
    ) -> Result<(String, u32), AdjutantError> {
        if chunk_ids.is_empty() {
            return Ok((String::new(), 0));
        }
        let combined = self.collect_chunks(chunk_ids, self.config.max_chunk_chars);
        let prompt = format!(
            "Task: {}\n\nDescription: {}\n\nSummarize the following context:\n{}\n",
            task.title, task.description, combined
        );
        let (summary, tokens) = self.call_lm(SUMMARIZE_SYSTEM_PROMPT, &prompt).await?;
        self.subcalls += 1;
        self.summary_calls += 1;
        let chunk_id =
            self.context
                .add_chunk(RlmChunkKind::Summary, "summary".to_string(), None, summary);
        Ok((chunk_id, tokens))
    }

    async fn synthesize_answer(
        &mut self,
        task: &Task,
        plan: &TaskPlan,
    ) -> Result<String, AdjutantError> {
        let digest = self.context.digest(self.config.max_digest_chars);
        let prompt = format!(
            "Task: {}\n\nDescription: {}\n\nPlan summary: {}\n\nContext digest:\n{}\n\n\
             Provide a clear analysis, key findings, and any recommended next steps.",
            task.title, task.description, plan.summary, digest
        );
        let (answer, tokens) = self.call_lm(SYNTHESIZE_SYSTEM_PROMPT, &prompt).await?;
        self.record_tool_call(
            "rlm.synthesize",
            json!({ "digest_chars": digest.len() }),
            json!({ "answer": answer }),
            tokens,
            false,
        );
        Ok(answer)
    }

    fn collect_chunks(&self, chunk_ids: &[String], max_chars: usize) -> String {
        let mut out = String::new();
        for chunk_id in chunk_ids {
            if out.len() >= max_chars {
                break;
            }
            if let Some(chunk) = self.context.get_chunk(chunk_id) {
                let header = format!(
                    "--- {} [{}] {} ---\n",
                    chunk.id,
                    chunk.kind.as_str(),
                    chunk.label
                );
                let remaining = max_chars.saturating_sub(out.len());
                if remaining <= header.len() {
                    break;
                }
                out.push_str(&header);
                let content_limit = remaining.saturating_sub(header.len());
                out.push_str(&truncate_chars(&chunk.content, content_limit));
                out.push('\n');
            }
        }
        out
    }

    async fn call_lm(
        &self,
        system_prompt: &str,
        prompt: &str,
    ) -> Result<(String, u32), AdjutantError> {
        let chat = Chat::new(vec![Message::system(system_prompt), Message::user(prompt)]);
        let response = self
            .lm
            .call(chat, vec![])
            .await
            .map_err(|e| AdjutantError::RlmError(format!("RLM LM call failed: {}", e)))?;
        let tokens = response.usage.total_tokens.min(u64::from(u32::MAX)) as u32;
        Ok((response.output.content().to_string(), tokens))
    }

    fn record_tool_call(
        &self,
        signature: &str,
        inputs: serde_json::Value,
        outputs: serde_json::Value,
        cost_tokens: u32,
        was_repeated: bool,
    ) {
        let Some(collector) = self.training else {
            return;
        };
        let _ = collector.record_tool_call(LabeledToolCall {
            signature: signature.to_string(),
            inputs,
            outputs,
            step_utility: 0.5,
            verification_delta: 0,
            cost_tokens,
            cost_tool_calls: 1,
            was_repeated,
            recorded_at: Utc::now(),
        });
    }
}

fn parse_actions(raw: &str) -> Option<Vec<RlmAction>> {
    if let Ok(plan) = serde_json::from_str::<RlmActionPlan>(raw) {
        return Some(plan.actions);
    }
    if let Ok(actions) = serde_json::from_str::<Vec<RlmAction>>(raw) {
        return Some(actions);
    }
    let json = extract_json_block(raw)?;
    serde_json::from_str::<RlmActionPlan>(&json)
        .map(|plan| plan.actions)
        .ok()
        .or_else(|| serde_json::from_str::<Vec<RlmAction>>(&json).ok())
}

fn extract_json_block(raw: &str) -> Option<String> {
    let trimmed = raw.trim();
    if trimmed.starts_with('{') && trimmed.ends_with('}') {
        return Some(trimmed.to_string());
    }
    if trimmed.starts_with('[') && trimmed.ends_with(']') {
        return Some(trimmed.to_string());
    }
    let start_obj = trimmed.find('{');
    let end_obj = trimmed.rfind('}');
    if let (Some(start), Some(end)) = (start_obj, end_obj) {
        if end > start {
            return Some(trimmed[start..=end].to_string());
        }
    }
    let start_arr = trimmed.find('[');
    let end_arr = trimmed.rfind(']');
    if let (Some(start), Some(end)) = (start_arr, end_arr) {
        if end > start {
            return Some(trimmed[start..=end].to_string());
        }
    }
    None
}

fn truncate_chars(input: &str, max_chars: usize) -> String {
    if input.len() <= max_chars {
        return input.to_string();
    }
    if max_chars <= 3 {
        return input.chars().take(max_chars).collect();
    }
    let mut out = input.chars().take(max_chars - 3).collect::<String>();
    out.push_str("...");
    out
}

fn truncate_lines(input: &str, max_lines: usize) -> String {
    if max_lines == 0 {
        return String::new();
    }
    let mut lines = Vec::new();
    for (idx, line) in input.lines().enumerate() {
        if idx >= max_lines {
            break;
        }
        lines.push(line);
    }
    let mut out = lines.join("\n");
    if input.lines().count() > max_lines {
        out.push_str("\n...");
    }
    out
}
