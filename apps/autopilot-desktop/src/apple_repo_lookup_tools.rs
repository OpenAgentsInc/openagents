use std::fs;
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};

use anyhow::{Result, anyhow};
use openagents_kernel_core::ids::sha256_prefixed_text;
use psionic_apple_fm::{
    AppleFmGeneratedContent, AppleFmGenerationSchema, AppleFmTool, AppleFmToolCallError,
    AppleFmToolDefinition,
};
use psionic_data::AppleAdapterToolDefinition;
use psionic_eval::{AppleAdapterObservedSampleOutput, AppleAdapterObservedToolCall};
use serde::Serialize;
use serde_json::{Value, json};

const MAX_DOC_EXCERPT_CHARS: usize = 6_000;
const MAX_CODE_EXCERPT_HEAD_CHARS: usize = 2_500;
const MAX_CODE_EXCERPT_TAIL_CHARS: usize = 800;

#[derive(Clone, Default)]
pub(crate) struct AppleRepoLookupRecorder {
    state: Arc<Mutex<AppleRepoLookupRecorderState>>,
}

#[derive(Default)]
struct AppleRepoLookupRecorderState {
    tool_calls: Vec<AppleAdapterObservedToolCall>,
    events: Vec<AppleRepoLookupEvent>,
}

#[derive(Clone, Debug, Serialize)]
struct AppleRepoLookupEvent {
    tool_name: String,
    status: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_class: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    failure_code: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    detail: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    resolved_path: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    content_digest: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    byte_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    line_count: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    truncated: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    retryable: Option<bool>,
    #[serde(skip_serializing_if = "Option::is_none")]
    guidance: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_tool: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    suggested_paths: Option<Vec<String>>,
}

#[derive(Clone)]
pub(crate) struct AppleRepoLookupTool {
    definition: AppleFmToolDefinition,
    repo_root: PathBuf,
    recorder: AppleRepoLookupRecorder,
}

impl AppleRepoLookupRecorder {
    fn push(
        &self,
        tool_name: &str,
        succeeded: bool,
        arguments: Option<Value>,
        event: AppleRepoLookupEvent,
    ) -> Result<()> {
        let mut state = self
            .state
            .lock()
            .map_err(|_| anyhow!("Apple repo lookup recorder lock poisoned"))?;
        state.tool_calls.push(AppleAdapterObservedToolCall {
            tool_name: tool_name.to_string(),
            succeeded,
            arguments,
        });
        state.events.push(event);
        Ok(())
    }

    pub(crate) fn attach_to_output(
        &self,
        mut observed: AppleAdapterObservedSampleOutput,
    ) -> Result<AppleAdapterObservedSampleOutput> {
        let state = self
            .state
            .lock()
            .map_err(|_| anyhow!("Apple repo lookup recorder lock poisoned"))?;
        if !state.tool_calls.is_empty() {
            observed = observed.with_tool_calls(state.tool_calls.clone());
        }
        if !state.events.is_empty() {
            let events = serde_json::to_value(&state.events)?;
            observed
                .metadata
                .insert(String::from("apple_adapter.repo_lookup_events"), events);
            let harness_failures = state
                .events
                .iter()
                .filter(|event| event.failure_class.as_deref() == Some("harness"))
                .cloned()
                .collect::<Vec<_>>();
            if !harness_failures.is_empty() {
                observed.metadata.insert(
                    String::from("apple_adapter.harness_failures"),
                    serde_json::to_value(harness_failures)?,
                );
            }
            let model_request_failures = state
                .events
                .iter()
                .filter(|event| event.failure_class.as_deref() == Some("model_request"))
                .cloned()
                .collect::<Vec<_>>();
            if !model_request_failures.is_empty() {
                observed.metadata.insert(
                    String::from("apple_adapter.model_request_failures"),
                    serde_json::to_value(model_request_failures)?,
                );
            }
        }
        Ok(observed)
    }
}

impl AppleRepoLookupTool {
    fn new(
        tool_definition: &AppleAdapterToolDefinition,
        recorder: AppleRepoLookupRecorder,
    ) -> Result<Self> {
        let lookup_kind = LookupKind::for_tool_name(tool_definition.function.name.as_str())
            .ok_or_else(|| {
                anyhow!(
                    "unsupported repo lookup tool `{}`",
                    tool_definition.function.name
                )
            })?;
        Ok(Self {
            definition: AppleFmToolDefinition::new(
                tool_definition.function.name.clone(),
                tool_definition.function.description.clone(),
                AppleFmGenerationSchema::with_title_hint(
                    augment_lookup_argument_schema(
                        tool_definition.function.arguments.clone(),
                        lookup_kind,
                    ),
                    Some(tool_definition.function.name.as_str()),
                )?,
            ),
            repo_root: repo_root()?,
            recorder,
        })
    }

    fn handle_call(&self, arguments: &Value) -> std::result::Result<String, RepoLookupToolError> {
        let lookup_kind =
            LookupKind::for_tool_name(self.definition.name.as_str()).ok_or_else(|| {
                RepoLookupToolError::harness(
                    self.definition.name.as_str(),
                    "unsupported_tool_name",
                    format!(
                        "tool `{}` is not implemented by the Apple repo lookup harness",
                        self.definition.name
                    ),
                    None,
                    None,
                )
            })?;
        let requested_path = extract_requested_path(arguments).map_err(|detail| {
            RepoLookupToolError::model_request(
                self.definition.name.as_str(),
                lookup_kind,
                self.repo_root.as_path(),
                "invalid_arguments",
                detail,
                None,
                None,
            )
        })?;
        let resolved_path = resolve_repo_path(self.repo_root.as_path(), requested_path.as_str())
            .map_err(|error| {
                RepoLookupToolError::model_request(
                    self.definition.name.as_str(),
                    lookup_kind,
                    self.repo_root.as_path(),
                    error.failure_code,
                    error.detail,
                    Some(requested_path.clone()),
                    error.resolved_path,
                )
            })?;
        validate_lookup_path(
            lookup_kind,
            requested_path.as_str(),
            resolved_path.as_path(),
        )
        .map_err(|error| {
            RepoLookupToolError::model_request(
                self.definition.name.as_str(),
                lookup_kind,
                self.repo_root.as_path(),
                error.failure_code,
                error.detail,
                Some(requested_path.clone()),
                error.resolved_path.or_else(|| Some(resolved_path.clone())),
            )
        })?;
        let raw_bytes = fs::read(resolved_path.as_path()).map_err(|error| {
            RepoLookupToolError::harness(
                self.definition.name.as_str(),
                "read_failed",
                format!("failed to read {}: {error}", resolved_path.display()),
                Some(requested_path.clone()),
                Some(resolved_path.clone()),
            )
        })?;
        let raw_text = String::from_utf8_lossy(&raw_bytes).into_owned();
        let excerpt = build_excerpt(lookup_kind, raw_text.as_str());
        let event = AppleRepoLookupEvent {
            tool_name: self.definition.name.clone(),
            status: String::from("ok"),
            failure_class: None,
            failure_code: None,
            detail: None,
            path: Some(requested_path.clone()),
            resolved_path: Some(path_relative_to_repo(
                self.repo_root.as_path(),
                resolved_path.as_path(),
            )),
            content_digest: Some(sha256_prefixed_text(raw_text.as_str())),
            byte_count: Some(raw_bytes.len() as u64),
            line_count: Some(raw_text.lines().count() as u64),
            truncated: Some(excerpt.truncated),
            retryable: None,
            guidance: None,
            suggested_tool: None,
            suggested_paths: None,
        };
        Ok(json!({
            "tool_name": self.definition.name,
            "lookup_kind": lookup_kind.label(),
            "path": requested_path,
            "resolved_path": path_relative_to_repo(self.repo_root.as_path(), resolved_path.as_path()),
            "content_digest": event.content_digest.clone(),
            "byte_count": event.byte_count,
            "line_count": event.line_count,
            "truncated": event.truncated,
            "excerpt_strategy": excerpt.strategy,
            "content_excerpt": excerpt.content,
        })
        .to_string())
    }
}

impl AppleFmTool for AppleRepoLookupTool {
    fn definition(&self) -> AppleFmToolDefinition {
        self.definition.clone()
    }

    fn call(&self, arguments: AppleFmGeneratedContent) -> Result<String, AppleFmToolCallError> {
        let argument_payload = arguments.content.clone();
        match self.handle_call(&argument_payload) {
            Ok(output) => {
                self.recorder
                    .push(
                        self.definition.name.as_str(),
                        true,
                        Some(argument_payload),
                        AppleRepoLookupEvent {
                            tool_name: self.definition.name.clone(),
                            status: String::from("ok"),
                            failure_class: None,
                            failure_code: None,
                            detail: None,
                            path: None,
                            resolved_path: None,
                            content_digest: None,
                            byte_count: None,
                            line_count: None,
                            truncated: None,
                            retryable: None,
                            guidance: None,
                            suggested_tool: None,
                            suggested_paths: None,
                        },
                    )
                    .map_err(|error| {
                        AppleFmToolCallError::new(self.definition.name.clone(), error.to_string())
                    })?;
                if let Ok(mut state) = self.recorder.state.lock() {
                    state.events.pop();
                    if let Ok(response) = serde_json::from_str::<Value>(output.as_str()) {
                        state.events.push(AppleRepoLookupEvent {
                            tool_name: self.definition.name.clone(),
                            status: String::from("ok"),
                            failure_class: None,
                            failure_code: None,
                            detail: None,
                            path: response
                                .get("path")
                                .and_then(Value::as_str)
                                .map(ToString::to_string),
                            resolved_path: response
                                .get("resolved_path")
                                .and_then(Value::as_str)
                                .map(ToString::to_string),
                            content_digest: response
                                .get("content_digest")
                                .and_then(Value::as_str)
                                .map(ToString::to_string),
                            byte_count: response.get("byte_count").and_then(Value::as_u64),
                            line_count: response.get("line_count").and_then(Value::as_u64),
                            truncated: response.get("truncated").and_then(Value::as_bool),
                            retryable: response.get("retryable").and_then(Value::as_bool),
                            guidance: response
                                .get("guidance")
                                .and_then(Value::as_str)
                                .map(ToString::to_string),
                            suggested_tool: response
                                .get("suggested_tool")
                                .and_then(Value::as_str)
                                .map(ToString::to_string),
                            suggested_paths: response
                                .get("suggested_paths")
                                .and_then(Value::as_array)
                                .map(|paths| {
                                    paths
                                        .iter()
                                        .filter_map(Value::as_str)
                                        .map(ToString::to_string)
                                        .collect::<Vec<_>>()
                                })
                                .filter(|paths| !paths.is_empty()),
                        });
                    }
                }
                Ok(output)
            }
            Err(error) => {
                self.recorder
                    .push(
                        self.definition.name.as_str(),
                        false,
                        Some(argument_payload),
                        error.event.clone(),
                    )
                    .map_err(|push_error| {
                        AppleFmToolCallError::new(
                            self.definition.name.clone(),
                            push_error.to_string(),
                        )
                    })?;
                if error.event.failure_class.as_deref() == Some("model_request") {
                    Ok(model_request_response_payload(&error.event))
                } else {
                    Err(AppleFmToolCallError::new(
                        self.definition.name.clone(),
                        error
                            .event
                            .detail
                            .unwrap_or_else(|| String::from("repo lookup failed")),
                    ))
                }
            }
        }
    }
}

fn augment_lookup_argument_schema(mut schema: Value, kind: LookupKind) -> Value {
    if let Some(path_schema) = schema
        .as_object_mut()
        .and_then(|object| object.get_mut("properties"))
        .and_then(Value::as_object_mut)
        .and_then(|properties| properties.get_mut("path"))
        .and_then(Value::as_object_mut)
    {
        path_schema.insert(
            String::from("description"),
            Value::String(kind.path_argument_description().to_string()),
        );
    }
    schema
}

fn model_request_response_payload(event: &AppleRepoLookupEvent) -> String {
    json!({
        "tool_name": event.tool_name,
        "status": "model_request_error",
        "failure_class": event.failure_class,
        "failure_code": event.failure_code,
        "detail": event.detail,
        "path": event.path,
        "resolved_path": event.resolved_path,
        "retryable": event.retryable.unwrap_or(true),
        "guidance": event.guidance,
        "suggested_tool": event.suggested_tool,
        "suggested_paths": event.suggested_paths,
    })
    .to_string()
}

pub(crate) fn build_repo_lookup_tools(
    tool_definitions: &[AppleAdapterToolDefinition],
    recorder: AppleRepoLookupRecorder,
) -> Result<Vec<Arc<dyn AppleFmTool>>> {
    tool_definitions
        .iter()
        .map(|tool| {
            Ok(Arc::new(AppleRepoLookupTool::new(tool, recorder.clone())?) as Arc<dyn AppleFmTool>)
        })
        .collect()
}

fn repo_root() -> Result<PathBuf> {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .map(Path::to_path_buf)
        .ok_or_else(|| anyhow!("failed to derive repo root from autopilot manifest path"))
}

fn extract_requested_path(arguments: &Value) -> std::result::Result<String, String> {
    arguments
        .as_object()
        .and_then(|object| object.get("path"))
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
        .ok_or_else(|| String::from("tool arguments must include a non-empty `path` string"))
}

fn resolve_repo_path(
    repo_root: &Path,
    requested_path: &str,
) -> std::result::Result<PathBuf, RepoLookupPathError> {
    let candidate = Path::new(requested_path);
    if candidate.is_absolute() {
        return Err(RepoLookupPathError::new(
            "absolute_path_disallowed",
            String::from("tool paths must be repo-relative, not absolute"),
        ));
    }
    if candidate
        .components()
        .any(|component| component == Component::ParentDir)
    {
        return Err(RepoLookupPathError::new(
            "path_traversal_disallowed",
            String::from("tool paths may not use `..` traversal"),
        ));
    }
    if requested_path.contains('*') || requested_path.contains('?') || requested_path.contains('[')
    {
        return Err(RepoLookupPathError::new(
            "glob_disallowed",
            String::from(
                "tool paths must point to one concrete repo file and may not use glob syntax",
            ),
        ));
    }
    let joined = repo_root.join(candidate);
    let canonical = joined.canonicalize().map_err(|error| {
        RepoLookupPathError::new(
            "path_not_found",
            format!("failed to resolve `{requested_path}` inside the repo: {error}"),
        )
    })?;
    if !canonical.starts_with(repo_root) {
        return Err(RepoLookupPathError::with_resolved(
            "path_outside_repo",
            format!(
                "resolved path `{}` escapes the repo root",
                canonical.display()
            ),
            canonical,
        ));
    }
    if !canonical.is_file() {
        return Err(RepoLookupPathError::with_resolved(
            "path_not_file",
            format!("resolved path `{}` is not a file", canonical.display()),
            canonical,
        ));
    }
    Ok(canonical)
}

fn validate_lookup_path(
    kind: LookupKind,
    requested_path: &str,
    resolved_path: &Path,
) -> std::result::Result<(), RepoLookupPathError> {
    let extension = resolved_path
        .extension()
        .and_then(|value| value.to_str())
        .map(|value| value.to_ascii_lowercase());
    let allowed = kind.allows_extension(extension.as_deref());
    if allowed {
        Ok(())
    } else {
        Err(RepoLookupPathError::with_resolved(
            "path_kind_mismatch",
            format!(
                "path `{requested_path}` does not match the allowed {} file types",
                kind.label()
            ),
            resolved_path.to_path_buf(),
        ))
    }
}

fn path_relative_to_repo(repo_root: &Path, resolved_path: &Path) -> String {
    resolved_path
        .strip_prefix(repo_root)
        .unwrap_or(resolved_path)
        .display()
        .to_string()
}

fn build_excerpt(kind: LookupKind, raw_text: &str) -> LookupExcerpt {
    match kind {
        LookupKind::Doc => excerpt_full_or_trimmed(raw_text, MAX_DOC_EXCERPT_CHARS, "full_or_head"),
        LookupKind::Code => excerpt_head_tail(
            raw_text,
            MAX_CODE_EXCERPT_HEAD_CHARS,
            MAX_CODE_EXCERPT_TAIL_CHARS,
        ),
    }
}

fn excerpt_full_or_trimmed(
    raw_text: &str,
    max_chars: usize,
    strategy: &'static str,
) -> LookupExcerpt {
    let content = raw_text.chars().collect::<Vec<_>>();
    if content.len() <= max_chars {
        return LookupExcerpt {
            content: raw_text.to_string(),
            truncated: false,
            strategy,
        };
    }
    LookupExcerpt {
        content: content.into_iter().take(max_chars).collect(),
        truncated: true,
        strategy,
    }
}

fn excerpt_head_tail(raw_text: &str, head_chars: usize, tail_chars: usize) -> LookupExcerpt {
    let content = raw_text.chars().collect::<Vec<_>>();
    if content.len() <= head_chars + tail_chars {
        return LookupExcerpt {
            content: raw_text.to_string(),
            truncated: false,
            strategy: "full",
        };
    }
    let head = content.iter().take(head_chars).collect::<String>();
    let tail = content
        .iter()
        .skip(content.len().saturating_sub(tail_chars))
        .collect::<String>();
    LookupExcerpt {
        content: format!("{head}\n\n... [truncated code excerpt] ...\n\n{tail}"),
        truncated: true,
        strategy: "head_tail",
    }
}

#[derive(Clone, Copy)]
enum LookupKind {
    Doc,
    Code,
}

impl LookupKind {
    fn for_tool_name(tool_name: &str) -> Option<Self> {
        match tool_name {
            "lookup_doc" => Some(Self::Doc),
            "lookup_code" => Some(Self::Code),
            _ => None,
        }
    }

    const fn label(self) -> &'static str {
        match self {
            Self::Doc => "doc",
            Self::Code => "code",
        }
    }

    const fn tool_name(self) -> &'static str {
        match self {
            Self::Doc => "lookup_doc",
            Self::Code => "lookup_code",
        }
    }

    const fn other(self) -> Self {
        match self {
            Self::Doc => Self::Code,
            Self::Code => Self::Doc,
        }
    }

    fn allows_extension(self, extension: Option<&str>) -> bool {
        match self {
            Self::Doc => matches!(extension, Some("md" | "mdx" | "txt")),
            Self::Code => matches!(
                extension,
                Some(
                    "rs" | "swift"
                        | "toml"
                        | "sh"
                        | "py"
                        | "js"
                        | "ts"
                        | "tsx"
                        | "jsx"
                        | "json"
                        | "yaml"
                        | "yml"
                        | "proto"
                        | "c"
                        | "h"
                        | "cpp"
                        | "m"
                        | "mm"
                )
            ),
        }
    }

    const fn path_argument_description(self) -> &'static str {
        match self {
            Self::Doc => {
                "Repo-relative concrete documentation file path. Use markdown/text files such as `docs/OWNERSHIP.md`. Do not use directories, globs, absolute paths, or `..`."
            }
            Self::Code => {
                "Repo-relative concrete source/config file path. Use files such as `apps/autopilot-desktop/src/apple_adapter_training_control.rs`. Do not use directories, globs, absolute paths, or `..`."
            }
        }
    }

    const fn retry_guidance(self) -> &'static str {
        match self {
            Self::Doc => {
                "Retry with one concrete repo-relative markdown/text file path. For ownership or product docs, prefer paths under `docs/`."
            }
            Self::Code => {
                "Retry with one concrete repo-relative source/config file path. For desktop behavior, prefer files under `apps/autopilot-desktop/src/`."
            }
        }
    }
}

struct RepoLookupPathError {
    failure_code: &'static str,
    detail: String,
    resolved_path: Option<PathBuf>,
}

impl RepoLookupPathError {
    fn new(failure_code: &'static str, detail: String) -> Self {
        Self {
            failure_code,
            detail,
            resolved_path: None,
        }
    }

    fn with_resolved(failure_code: &'static str, detail: String, resolved_path: PathBuf) -> Self {
        Self {
            failure_code,
            detail,
            resolved_path: Some(resolved_path),
        }
    }
}

struct LookupExcerpt {
    content: String,
    truncated: bool,
    strategy: &'static str,
}

#[derive(Debug)]
struct RepoLookupToolError {
    event: AppleRepoLookupEvent,
}

impl std::fmt::Display for RepoLookupToolError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(
            self.event
                .detail
                .as_deref()
                .unwrap_or("repo lookup tool failed"),
        )
    }
}

impl std::error::Error for RepoLookupToolError {}

impl RepoLookupToolError {
    fn model_request(
        tool_name: &str,
        kind: LookupKind,
        repo_root: &Path,
        failure_code: &'static str,
        detail: String,
        path: Option<String>,
        resolved_path: Option<PathBuf>,
    ) -> Self {
        let advice = repo_lookup_recovery_advice(
            repo_root,
            kind,
            failure_code,
            path.as_deref(),
            resolved_path.as_deref(),
        );
        Self {
            event: AppleRepoLookupEvent {
                tool_name: tool_name.to_string(),
                status: String::from("failed"),
                failure_class: Some(String::from("model_request")),
                failure_code: Some(failure_code.to_string()),
                detail: Some(detail),
                path,
                resolved_path: resolved_path.map(|path| path.display().to_string()),
                content_digest: None,
                byte_count: None,
                line_count: None,
                truncated: None,
                retryable: Some(true),
                guidance: Some(advice.guidance),
                suggested_tool: advice
                    .suggested_tool
                    .map(|tool| tool.tool_name().to_string()),
                suggested_paths: (!advice.suggested_paths.is_empty())
                    .then_some(advice.suggested_paths),
            },
        }
    }

    fn harness(
        tool_name: &str,
        failure_code: &'static str,
        detail: String,
        path: Option<String>,
        resolved_path: Option<PathBuf>,
    ) -> Self {
        Self {
            event: AppleRepoLookupEvent {
                tool_name: tool_name.to_string(),
                status: String::from("failed"),
                failure_class: Some(String::from("harness")),
                failure_code: Some(failure_code.to_string()),
                detail: Some(detail),
                path,
                resolved_path: resolved_path.map(|path| path.display().to_string()),
                content_digest: None,
                byte_count: None,
                line_count: None,
                truncated: None,
                retryable: None,
                guidance: None,
                suggested_tool: None,
                suggested_paths: None,
            },
        }
    }
}

struct RepoLookupRecoveryAdvice {
    guidance: String,
    suggested_tool: Option<LookupKind>,
    suggested_paths: Vec<String>,
}

fn repo_lookup_recovery_advice(
    repo_root: &Path,
    kind: LookupKind,
    failure_code: &str,
    requested_path: Option<&str>,
    resolved_path: Option<&Path>,
) -> RepoLookupRecoveryAdvice {
    let mut suggested_tool = None;
    let mut suggested_paths = Vec::new();
    match failure_code {
        "path_kind_mismatch" => {
            suggested_tool = Some(kind.other());
            if let Some(path) = resolved_path {
                suggested_paths.push(path_relative_to_repo(repo_root, path));
            }
        }
        "path_not_file" => {
            if let Some(path) = resolved_path.filter(|path| path.is_dir()) {
                suggested_paths = suggested_paths_for_directory(repo_root, path, kind, 5);
                if suggested_paths.is_empty() {
                    let alternative_kind = kind.other();
                    let alternative_paths =
                        suggested_paths_for_directory(repo_root, path, alternative_kind, 5);
                    if !alternative_paths.is_empty() {
                        suggested_tool = Some(alternative_kind);
                        suggested_paths = alternative_paths;
                    }
                }
            }
        }
        "path_not_found" | "glob_disallowed" => {
            if let Some(requested_path) = requested_path {
                suggested_paths = suggested_paths_for_request(repo_root, requested_path, kind, 5);
                if suggested_paths.is_empty() {
                    let alternative_kind = kind.other();
                    let alternative_paths =
                        suggested_paths_for_request(repo_root, requested_path, alternative_kind, 5);
                    if !alternative_paths.is_empty() {
                        suggested_tool = Some(alternative_kind);
                        suggested_paths = alternative_paths;
                    }
                }
            }
        }
        _ => {}
    }
    RepoLookupRecoveryAdvice {
        guidance: kind.retry_guidance().to_string(),
        suggested_tool,
        suggested_paths,
    }
}

fn suggested_paths_for_request(
    repo_root: &Path,
    requested_path: &str,
    kind: LookupKind,
    limit: usize,
) -> Vec<String> {
    let search_root =
        request_search_root(repo_root, requested_path).unwrap_or_else(|| repo_root.to_path_buf());
    suggested_paths_for_directory(repo_root, search_root.as_path(), kind, limit)
}

fn request_search_root(repo_root: &Path, requested_path: &str) -> Option<PathBuf> {
    let wildcard_index = requested_path
        .find(['*', '?', '['])
        .unwrap_or(requested_path.len());
    let prefix = requested_path[..wildcard_index].trim_end_matches('/');
    let relative = if prefix.is_empty() { "." } else { prefix };
    let candidate = repo_root.join(relative);
    if candidate.is_dir() {
        return Some(candidate);
    }
    candidate
        .parent()
        .filter(|path| path.is_dir())
        .map(Path::to_path_buf)
}

fn suggested_paths_for_directory(
    repo_root: &Path,
    directory: &Path,
    kind: LookupKind,
    limit: usize,
) -> Vec<String> {
    let mut suggestions = Vec::new();
    collect_matching_files(repo_root, directory, kind, limit, &mut suggestions);
    suggestions
}

fn collect_matching_files(
    repo_root: &Path,
    directory: &Path,
    kind: LookupKind,
    limit: usize,
    suggestions: &mut Vec<String>,
) {
    if suggestions.len() >= limit {
        return;
    }
    let Ok(entries) = fs::read_dir(directory) else {
        return;
    };
    let mut entries = entries.filter_map(|entry| entry.ok()).collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.path());
    for entry in entries {
        if suggestions.len() >= limit {
            break;
        }
        let path = entry.path();
        if path.is_dir() {
            collect_matching_files(repo_root, path.as_path(), kind, limit, suggestions);
            continue;
        }
        let extension = path
            .extension()
            .and_then(|value| value.to_str())
            .map(|value| value.to_ascii_lowercase());
        if kind.allows_extension(extension.as_deref()) {
            suggestions.push(path_relative_to_repo(repo_root, path.as_path()));
        }
    }
}

#[cfg(test)]
mod tests {
    use anyhow::Result;
    use psionic_apple_fm::{AppleFmGeneratedContent, AppleFmTool};
    use serde_json::json;

    use super::{AppleRepoLookupRecorder, AppleRepoLookupTool, build_repo_lookup_tools, repo_root};
    use psionic_data::{
        AppleAdapterToolDefinition, AppleAdapterToolFunctionDefinition, AppleAdapterToolType,
    };

    fn tool_definition(name: &str) -> AppleAdapterToolDefinition {
        AppleAdapterToolDefinition {
            tool_type: AppleAdapterToolType::Function,
            function: AppleAdapterToolFunctionDefinition {
                name: name.to_string(),
                description: Some(format!("{name} test tool")),
                arguments: json!({
                    "type": "object",
                    "properties": {
                        "path": {"type": "string"}
                    },
                    "required": ["path"],
                    "additionalProperties": false
                }),
            },
        }
    }

    #[test]
    fn lookup_doc_reads_real_repo_docs() -> Result<()> {
        let tool = AppleRepoLookupTool::new(
            &tool_definition("lookup_doc"),
            AppleRepoLookupRecorder::default(),
        )?;
        let payload = tool.handle_call(&json!({"path": "docs/MVP.md"}))?;
        assert!(payload.contains("\"path\":\"docs/MVP.md\""));
        assert!(payload.contains("content_excerpt"));
        Ok(())
    }

    #[test]
    fn lookup_code_reads_real_repo_code() -> Result<()> {
        let tool = AppleRepoLookupTool::new(
            &tool_definition("lookup_code"),
            AppleRepoLookupRecorder::default(),
        )?;
        let payload = tool.handle_call(&json!({
            "path": "apps/autopilot-desktop/src/apple_adapter_training_control.rs"
        }))?;
        assert!(payload.contains("\"lookup_kind\":\"code\""));
        assert!(payload.contains("content_excerpt"));
        Ok(())
    }

    #[test]
    fn lookup_tools_reject_doc_code_kind_mismatches() -> Result<()> {
        let tool = AppleRepoLookupTool::new(
            &tool_definition("lookup_doc"),
            AppleRepoLookupRecorder::default(),
        )?;
        let error = tool
            .handle_call(&json!({
                "path": "apps/autopilot-desktop/src/apple_adapter_training_control.rs"
            }))
            .expect_err("code path should be rejected by doc tool");
        assert_eq!(
            error.event.failure_code.as_deref(),
            Some("path_kind_mismatch")
        );
        Ok(())
    }

    #[test]
    fn lookup_tool_returns_retryable_payload_for_wrong_lookup_kind() -> Result<()> {
        let tool = AppleRepoLookupTool::new(
            &tool_definition("lookup_doc"),
            AppleRepoLookupRecorder::default(),
        )?;
        let payload = tool.call(AppleFmGeneratedContent::new(json!({
            "path": "apps/autopilot-desktop/src/apple_adapter_training_control.rs"
        })))?;
        let payload: serde_json::Value = serde_json::from_str(payload.as_str())?;
        assert_eq!(
            payload.get("status").and_then(serde_json::Value::as_str),
            Some("model_request_error")
        );
        assert_eq!(
            payload
                .get("failure_code")
                .and_then(serde_json::Value::as_str),
            Some("path_kind_mismatch")
        );
        assert_eq!(
            payload
                .get("suggested_tool")
                .and_then(serde_json::Value::as_str),
            Some("lookup_code")
        );
        assert_eq!(
            payload
                .get("suggested_paths")
                .and_then(serde_json::Value::as_array)
                .and_then(|paths| paths.first())
                .and_then(serde_json::Value::as_str),
            Some("apps/autopilot-desktop/src/apple_adapter_training_control.rs")
        );
        Ok(())
    }

    #[test]
    fn lookup_tool_returns_retryable_payload_for_directory_paths() -> Result<()> {
        let tool = AppleRepoLookupTool::new(
            &tool_definition("lookup_doc"),
            AppleRepoLookupRecorder::default(),
        )?;
        let payload = tool.call(AppleFmGeneratedContent::new(json!({
            "path": "apps/autopilot-desktop"
        })))?;
        let payload: serde_json::Value = serde_json::from_str(payload.as_str())?;
        assert_eq!(
            payload.get("status").and_then(serde_json::Value::as_str),
            Some("model_request_error")
        );
        assert_eq!(
            payload
                .get("failure_code")
                .and_then(serde_json::Value::as_str),
            Some("path_not_file")
        );
        assert_eq!(
            payload
                .get("suggested_tool")
                .and_then(serde_json::Value::as_str),
            Some("lookup_code")
        );
        assert!(
            payload
                .get("suggested_paths")
                .and_then(serde_json::Value::as_array)
                .into_iter()
                .flatten()
                .filter_map(serde_json::Value::as_str)
                .any(|path| path.starts_with("apps/autopilot-desktop/"))
        );
        Ok(())
    }

    #[test]
    fn repo_lookup_builder_preserves_supported_tool_inventory() -> Result<()> {
        let tools = build_repo_lookup_tools(
            &[
                tool_definition("lookup_doc"),
                tool_definition("lookup_code"),
            ],
            AppleRepoLookupRecorder::default(),
        )?;
        assert_eq!(tools.len(), 2);
        assert!(repo_root()?.join("docs/MVP.md").exists());
        Ok(())
    }
}
