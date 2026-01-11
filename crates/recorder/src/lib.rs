//! Recorder format parser and validator
//!
//! Recorder (.rlog) is a line-based flight recorder format for agent sessions.
//! This crate provides parsing, validation, and analysis tools.
//!
//! ## Features
//!
//! - `export`: Enable database export functionality (requires PostgreSQL)

pub mod convert;

#[cfg(feature = "export")]
pub mod export;

use lazy_static::lazy_static;
use regex::Regex;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};
use std::path::Path;
use thiserror::Error;

// ============================================================================
// ERRORS
// ============================================================================

#[derive(Error, Debug)]
pub enum ParseError {
    #[error("Invalid header: {0}")]
    InvalidHeader(String),

    #[error("Missing required header field: {0}")]
    MissingRequiredField(String),

    #[error("Invalid format version: {0}")]
    InvalidFormatVersion(String),

    #[error("Line {line}: {message}")]
    InvalidLine { line: usize, message: String },

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("YAML error: {0}")]
    Yaml(#[from] serde_yaml::Error),
}

#[derive(Debug, Clone)]
pub struct ValidationIssue {
    pub line: Option<usize>,
    pub severity: Severity,
    pub code: &'static str,
    pub message: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Severity {
    Error,
    Warning,
    Info,
}

// ============================================================================
// HEADER
// ============================================================================

#[derive(Debug, Clone, Deserialize)]
pub struct Header {
    pub format: String,
    pub id: String,

    #[serde(default)]
    pub mode: Option<String>,

    #[serde(default)]
    pub model: Option<String>,

    #[serde(default)]
    pub agent: Option<String>,

    #[serde(default)]
    pub version: Option<String>,

    // Execution substrate
    #[serde(default)]
    pub repo: Option<String>,

    pub repo_sha: String,

    #[serde(default)]
    pub branch: Option<String>,

    #[serde(default)]
    pub dirty: Option<bool>,

    #[serde(default)]
    pub sandbox_id: Option<String>,

    #[serde(default)]
    pub runner: Option<String>,

    #[serde(default)]
    pub toolset: Option<serde_yaml::Value>,

    // Capabilities
    #[serde(default)]
    pub skills: Option<Vec<String>>,

    #[serde(default)]
    pub mcp: Option<Vec<String>>,

    // Limits
    #[serde(default)]
    pub budget: Option<String>,

    #[serde(default)]
    pub duration: Option<String>,

    // Security
    #[serde(default)]
    pub classification: Option<String>,

    // Extensibility
    #[serde(default)]
    pub notes: Option<String>,

    // Codex specific fields
    #[serde(default)]
    pub client_version: Option<String>,

    #[serde(default)]
    pub slug: Option<String>,

    #[serde(default)]
    pub cwd: Option<String>,

    #[serde(default)]
    pub tokens_total_in: Option<u64>,

    #[serde(default)]
    pub tokens_total_out: Option<u64>,

    #[serde(default)]
    pub tokens_cached: Option<u64>,

    // Catch-all for extra.* fields
    #[serde(flatten)]
    pub extra: HashMap<String, serde_yaml::Value>,
}

// ============================================================================
// LINE TYPES
// ============================================================================

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LineType {
    /// User message: `u: message`
    User,
    /// Agent message: `a: message`
    Agent,
    /// Tool call: `t:tool args → result`
    Tool,
    /// Tool start (streaming): `t!:tool args → [running]`
    ToolStart,
    /// Tool progress: `t~:tool partial`
    ToolProgress,
    /// Observation (deferred result): `o: id=call_1 → result`
    Observation,
    /// Skill: `s:name action → result`
    Skill,
    /// Plan: `p:action id=p1 "title" → result`
    Plan,
    /// Mode: `m:chat` or `m:auto`
    Mode,
    /// Recall/memory: `r:query → result`
    Recall,
    /// Subagent: `x:type "task" → summary`
    Subagent,
    /// MCP call: `c:server.method args → result`
    Mcp,
    /// Question: `q: "Which database?" → [selected: Postgres]`
    Question,
    /// Comment/meta: `# comment`
    Comment,
    /// Lifecycle: `@start`, `@checkpoint`, `@end`
    Lifecycle,
    /// Phase: `@phase explore`, `@phase design`
    Phase,
    /// Thinking: `th: reasoning... sig=...`
    Thinking,
    /// Todos: `td: [pending] Task 1 [completed] Task 2`
    Todos,
    /// Empty line
    Empty,
    /// Continuation (indented)
    Continuation,
    /// Unknown/unparseable
    Unknown,
}

#[derive(Debug, Clone)]
pub struct ParsedLine {
    pub line_number: usize,
    pub raw: String,
    pub line_type: LineType,
    pub content: String,

    // Optional metadata extracted from line
    pub call_id: Option<String>,
    pub step: Option<u32>,
    pub timestamp: Option<String>,
    pub tid: Option<String>,
    pub span: Option<String>,
    pub latency_ms: Option<u64>,
    pub attempt: Option<String>,
    pub level: Option<String>,
    pub result: Option<String>,

    // Codex specific metadata
    pub parent_uuid: Option<String>,
    pub signature: Option<String>,
    pub tokens_in: Option<u64>,
    pub tokens_out: Option<u64>,
    pub tokens_cached: Option<u64>,
    pub interrupted: bool,
    pub model: Option<String>,
}

// ============================================================================
// PARSED SESSION
// ============================================================================

#[derive(Debug)]
pub struct ParsedSession {
    pub header: Header,
    pub lines: Vec<ParsedLine>,
    pub header_end_line: usize,
}

// ============================================================================
// VALIDATION RESULT
// ============================================================================

#[derive(Debug, Default)]
pub struct ValidationResult {
    pub issues: Vec<ValidationIssue>,
    pub stats: SessionStats,
}

/// Statistics collected from a recorder session
///
/// # Examples
///
/// ```
/// use recorder::{parse_content, validate};
///
/// let content = r#"---
/// format: rlog/1
/// id: test
/// repo_sha: abc123
/// ---
///
/// u: Create function
/// th: I'll write the code tokens_in=100 tokens_out=50
/// a: Here's the implementation
/// t:Write file="code.rs" → [ok]
/// "#;
///
/// let session = parse_content(content).unwrap();
/// let result = validate(&session);
///
/// assert_eq!(result.stats.user_messages, 1);
/// assert_eq!(result.stats.agent_messages, 1);
/// assert_eq!(result.stats.tool_calls, 1);
/// assert_eq!(result.stats.thinking_blocks, 1);
/// assert_eq!(result.stats.total_tokens_in, 100);
/// assert_eq!(result.stats.total_tokens_out, 50);
/// ```
#[derive(Debug, Default)]
pub struct SessionStats {
    pub total_lines: usize,
    pub user_messages: usize,
    pub agent_messages: usize,
    pub tool_calls: usize,
    pub observations: usize,
    pub subagents: usize,
    pub mcp_calls: usize,
    pub questions: usize,
    pub phases: usize,
    pub lifecycle_events: usize,
    pub comments: usize,
    pub unique_call_ids: usize,
    pub max_step: Option<u32>,
    pub has_timestamps: bool,
    pub blob_references: usize,
    pub redacted_values: usize,
    // Codex specific stats
    pub thinking_blocks: usize,
    pub todos_updates: usize,
    pub total_tokens_in: u64,
    pub total_tokens_out: u64,
    pub total_tokens_cached: u64,
}

impl ValidationResult {
    /// Get all validation errors
    ///
    /// # Examples
    ///
    /// ```
    /// use recorder::{parse_content, validate};
    ///
    /// let content = r#"---
    /// format: rlog/1
    /// id: test
    /// repo_sha: abc123
    /// ---
    ///
    /// u: Hello
    /// this is invalid
    /// "#;
    ///
    /// let session = parse_content(content).unwrap();
    /// let result = validate(&session);
    ///
    /// let errors: Vec<_> = result.errors().collect();
    /// // Invalid lines create warnings, not errors
    /// assert_eq!(errors.len(), 0);
    ///
    /// let warnings: Vec<_> = result.warnings().collect();
    /// assert!(warnings.len() > 0);
    /// ```
    pub fn errors(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues.iter().filter(|i| i.severity == Severity::Error)
    }

    pub fn warnings(&self) -> impl Iterator<Item = &ValidationIssue> {
        self.issues
            .iter()
            .filter(|i| i.severity == Severity::Warning)
    }

    pub fn is_valid(&self) -> bool {
        self.errors().count() == 0
    }

    pub fn error_count(&self) -> usize {
        self.errors().count()
    }

    pub fn warning_count(&self) -> usize {
        self.warnings().count()
    }
}

// ============================================================================
// REGEX PATTERNS
// ============================================================================

lazy_static! {
    // Line type detection
    static ref RE_USER: Regex = Regex::new(r"^u:\s*(.*)$").unwrap();
    static ref RE_AGENT: Regex = Regex::new(r"^a:\s*(.*)$").unwrap();
    static ref RE_TOOL_START: Regex = Regex::new(r"^t!:(\w+)(.*)$").unwrap();
    static ref RE_TOOL_PROGRESS: Regex = Regex::new(r"^t~:(\w+)(.*)$").unwrap();
    static ref RE_TOOL: Regex = Regex::new(r"^t:(\w+)(.*)$").unwrap();
    static ref RE_OBSERVATION: Regex = Regex::new(r"^o:\s*(.*)$").unwrap();
    static ref RE_SKILL: Regex = Regex::new(r"^s:(\S+)\s+(.*)$").unwrap();
    static ref RE_PLAN: Regex = Regex::new(r"^p:(\w+)(.*)$").unwrap();
    static ref RE_MODE: Regex = Regex::new(r"^m:\s*(\w+)").unwrap();
    static ref RE_RECALL: Regex = Regex::new(r"^r:\s*(.*)$").unwrap();
    static ref RE_SUBAGENT: Regex = Regex::new(r"^x:(\w+)(.*)$").unwrap();
    static ref RE_MCP: Regex = Regex::new(r"^c:(\w+)\.(\w+)(.*)$").unwrap();
    static ref RE_QUESTION: Regex = Regex::new(r"^q:\s*(.*)$").unwrap();
    static ref RE_COMMENT: Regex = Regex::new(r"^#\s*(.*)$").unwrap();
    static ref RE_LIFECYCLE: Regex = Regex::new(r"^@(\w+)(.*)$").unwrap();
    static ref RE_PHASE: Regex = Regex::new(r"^@phase\s+(\w+)(.*)$").unwrap();
    static ref RE_THINKING: Regex = Regex::new(r"^th:\s*(.*)$").unwrap();
    static ref RE_TODOS: Regex = Regex::new(r"^td:\s*(.*)$").unwrap();

    // Field extraction
    static ref RE_CALL_ID: Regex = Regex::new(r"\bid=(\w+)").unwrap();
    static ref RE_STEP: Regex = Regex::new(r"\bstep=(\d+)").unwrap();
    static ref RE_TIMESTAMP: Regex = Regex::new(r"\bts=(\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?)").unwrap();
    static ref RE_TID: Regex = Regex::new(r"\btid=(\w+)").unwrap();
    static ref RE_SPAN: Regex = Regex::new(r"\bspan=(\w+)").unwrap();
    static ref RE_LATENCY: Regex = Regex::new(r"\blatency_ms=(\d+)").unwrap();
    static ref RE_ATTEMPT: Regex = Regex::new(r"\battempt=(\d+(?:/\d+)?)").unwrap();
    static ref RE_LEVEL: Regex = Regex::new(r"\blevel=(\w+)").unwrap();
    static ref RE_RESULT: Regex = Regex::new(r"→\s*(.+)$").unwrap();

    // Codex specific field extraction
    static ref RE_PARENT: Regex = Regex::new(r"\bparent=([a-f0-9-]+)").unwrap();
    static ref RE_SIG: Regex = Regex::new(r"\bsig=(\S+)").unwrap();
    static ref RE_TOKENS_IN: Regex = Regex::new(r"\btokens_in=(\d+)").unwrap();
    static ref RE_TOKENS_OUT: Regex = Regex::new(r"\btokens_out=(\d+)").unwrap();
    static ref RE_TOKENS_CACHED: Regex = Regex::new(r"\btokens_cached=(\d+)").unwrap();
    static ref RE_INTERRUPTED: Regex = Regex::new(r"\binterrupted\b").unwrap();
    static ref RE_MODEL: Regex = Regex::new(r"\bmodel=(\S+)").unwrap();

    // Validation patterns
    static ref RE_BLOB: Regex = Regex::new(r"@blob\s+sha256=([a-f0-9]+)").unwrap();
    static ref RE_REDACTED: Regex = Regex::new(r"\[redacted:\w+\]").unwrap();
    static ref RE_ISO_TIMESTAMP: Regex = Regex::new(r"^\d{4}-\d{2}-\d{2}T\d{2}:\d{2}:\d{2}(?:\.\d+)?Z?$").unwrap();
    static ref RE_SHA256: Regex = Regex::new(r"^[a-f0-9]{6,64}$").unwrap();
}

// ============================================================================
// PARSER
// ============================================================================

/// Parse a recorder file from disk
///
/// **Memory usage**: Loads the entire file into memory. For very large files (>100MB),
/// consider splitting the file or processing in chunks. The recorder format requires
/// parsing YAML headers and maintaining line context, making true streaming difficult.
pub fn parse_file(path: &Path) -> Result<ParsedSession, ParseError> {
    let content = std::fs::read_to_string(path)?;
    parse_content(&content)
}

pub fn parse_content(content: &str) -> Result<ParsedSession, ParseError> {
    let lines: Vec<&str> = content.lines().collect();

    // Find header boundaries
    let (header, header_end) = parse_header(&lines)?;

    // Parse body lines
    let mut parsed_lines = Vec::new();
    for (i, line) in lines.iter().enumerate().skip(header_end) {
        let line_number = i + 1; // 1-indexed
        let parsed = parse_line(line_number, line);
        parsed_lines.push(parsed);
    }

    Ok(ParsedSession {
        header,
        lines: parsed_lines,
        header_end_line: header_end,
    })
}

fn parse_header(lines: &[&str]) -> Result<(Header, usize), ParseError> {
    // Find header start (first ---)
    let start = lines
        .iter()
        .position(|l| l.trim() == "---")
        .ok_or_else(|| ParseError::InvalidHeader("Missing header start '---'".into()))?;

    // Find header end (second ---)
    let end = lines
        .iter()
        .skip(start + 1)
        .position(|l| l.trim() == "---")
        .map(|p| p + start + 1)
        .ok_or_else(|| ParseError::InvalidHeader("Missing header end '---'".into()))?;

    // Extract header content
    let header_content = lines[start + 1..end].join("\n");

    // Parse YAML
    let header: Header = serde_yaml::from_str(&header_content)?;

    // Validate required fields
    if header.format.is_empty() {
        return Err(ParseError::MissingRequiredField("format".into()));
    }
    if header.id.is_empty() {
        return Err(ParseError::MissingRequiredField("id".into()));
    }
    if header.repo_sha.is_empty() {
        return Err(ParseError::MissingRequiredField("repo_sha".into()));
    }

    // Validate format version
    if !header.format.starts_with("rlog/") {
        return Err(ParseError::InvalidFormatVersion(header.format.clone()));
    }

    Ok((header, end + 1))
}

fn parse_line(line_number: usize, raw: &str) -> ParsedLine {
    let trimmed = raw.trim();

    // Determine line type and extract content
    let (line_type, content) = if trimmed.is_empty() {
        (LineType::Empty, String::new())
    } else if raw.starts_with("  ") || raw.starts_with('\t') {
        // Continuation line (indented)
        (LineType::Continuation, trimmed.to_string())
    } else if let Some(caps) = RE_USER.captures(trimmed) {
        (
            LineType::User,
            caps.get(1).map_or("", |m| m.as_str()).to_string(),
        )
    } else if let Some(caps) = RE_AGENT.captures(trimmed) {
        (
            LineType::Agent,
            caps.get(1).map_or("", |m| m.as_str()).to_string(),
        )
    } else if RE_TOOL_START.is_match(trimmed) {
        (LineType::ToolStart, trimmed[3..].to_string())
    } else if RE_TOOL_PROGRESS.is_match(trimmed) {
        (LineType::ToolProgress, trimmed[3..].to_string())
    } else if RE_TOOL.is_match(trimmed) {
        (LineType::Tool, trimmed[2..].to_string())
    } else if let Some(caps) = RE_OBSERVATION.captures(trimmed) {
        (
            LineType::Observation,
            caps.get(1).map_or("", |m| m.as_str()).to_string(),
        )
    } else if RE_SKILL.is_match(trimmed) {
        (LineType::Skill, trimmed[2..].to_string())
    } else if RE_PLAN.is_match(trimmed) {
        (LineType::Plan, trimmed[2..].to_string())
    } else if RE_MODE.is_match(trimmed) {
        (LineType::Mode, trimmed[2..].to_string())
    } else if RE_RECALL.is_match(trimmed) {
        (LineType::Recall, trimmed[2..].to_string())
    } else if RE_SUBAGENT.is_match(trimmed) {
        (LineType::Subagent, trimmed[2..].to_string())
    } else if RE_MCP.is_match(trimmed) {
        (LineType::Mcp, trimmed[2..].to_string())
    } else if let Some(caps) = RE_QUESTION.captures(trimmed) {
        (
            LineType::Question,
            caps.get(1).map_or("", |m| m.as_str()).to_string(),
        )
    } else if let Some(caps) = RE_COMMENT.captures(trimmed) {
        (
            LineType::Comment,
            caps.get(1).map_or("", |m| m.as_str()).to_string(),
        )
    } else if RE_PHASE.is_match(trimmed) {
        // @phase must be checked before generic @lifecycle
        (LineType::Phase, trimmed[7..].to_string())
    } else if RE_LIFECYCLE.is_match(trimmed) {
        (LineType::Lifecycle, trimmed[1..].to_string())
    } else if let Some(caps) = RE_THINKING.captures(trimmed) {
        (
            LineType::Thinking,
            caps.get(1).map_or("", |m| m.as_str()).to_string(),
        )
    } else if let Some(caps) = RE_TODOS.captures(trimmed) {
        (
            LineType::Todos,
            caps.get(1).map_or("", |m| m.as_str()).to_string(),
        )
    } else {
        (LineType::Unknown, trimmed.to_string())
    };

    // Extract optional metadata
    let call_id = RE_CALL_ID.captures(&content).map(|c| c[1].to_string());
    let step = RE_STEP.captures(&content).and_then(|c| c[1].parse().ok());
    let timestamp = RE_TIMESTAMP.captures(&content).map(|c| c[1].to_string());
    let tid = RE_TID.captures(&content).map(|c| c[1].to_string());
    let span = RE_SPAN.captures(&content).map(|c| c[1].to_string());
    let latency_ms = RE_LATENCY
        .captures(&content)
        .and_then(|c| c[1].parse().ok());
    let attempt = RE_ATTEMPT.captures(&content).map(|c| c[1].to_string());
    let level = RE_LEVEL.captures(&content).map(|c| c[1].to_string());
    let result = RE_RESULT.captures(&content).map(|c| c[1].to_string());

    // Codex specific metadata
    let parent_uuid = RE_PARENT.captures(&content).map(|c| c[1].to_string());
    let signature = RE_SIG.captures(&content).map(|c| c[1].to_string());
    let tokens_in = RE_TOKENS_IN
        .captures(&content)
        .and_then(|c| c[1].parse().ok());
    let tokens_out = RE_TOKENS_OUT
        .captures(&content)
        .and_then(|c| c[1].parse().ok());
    let tokens_cached = RE_TOKENS_CACHED
        .captures(&content)
        .and_then(|c| c[1].parse().ok());
    let interrupted = RE_INTERRUPTED.is_match(&content);
    let model = RE_MODEL.captures(&content).map(|c| c[1].to_string());

    ParsedLine {
        line_number,
        raw: raw.to_string(),
        line_type,
        content,
        call_id,
        step,
        timestamp,
        tid,
        span,
        latency_ms,
        attempt,
        level,
        result,
        parent_uuid,
        signature,
        tokens_in,
        tokens_out,
        tokens_cached,
        interrupted,
        model,
    }
}

// ============================================================================
// VALIDATOR
// ============================================================================

pub fn validate(session: &ParsedSession) -> ValidationResult {
    let mut result = ValidationResult::default();
    let mut call_ids: HashSet<String> = HashSet::new();
    let mut observation_ids: HashSet<String> = HashSet::new();
    let mut tool_start_ids: HashSet<String> = HashSet::new();
    let mut last_step: Option<u32> = None;
    let mut has_start = false;
    let mut has_end = false;

    // Validate header
    validate_header(&session.header, &mut result);

    // Collect stats and validate lines
    for line in &session.lines {
        result.stats.total_lines += 1;

        match line.line_type {
            LineType::User => result.stats.user_messages += 1,
            LineType::Agent => result.stats.agent_messages += 1,
            LineType::Tool => {
                result.stats.tool_calls += 1;
                if let Some(ref id) = line.call_id {
                    call_ids.insert(id.clone());
                }
            }
            LineType::ToolStart => {
                result.stats.tool_calls += 1;
                if let Some(ref id) = line.call_id {
                    call_ids.insert(id.clone());
                    tool_start_ids.insert(id.clone());
                }
            }
            LineType::ToolProgress => {
                // Validate that progress has matching start
                if let Some(ref id) = line.call_id
                    && !tool_start_ids.contains(id)
                    && !call_ids.contains(id)
                {
                    result.issues.push(ValidationIssue {
                        line: Some(line.line_number),
                        severity: Severity::Warning,
                        code: "W001",
                        message: format!("Tool progress references unknown call id: {}", id),
                    });
                }
            }
            LineType::Observation => {
                result.stats.observations += 1;
                if let Some(ref id) = line.call_id {
                    observation_ids.insert(id.clone());
                    // Check if observation references a known call
                    if !call_ids.contains(id) {
                        result.issues.push(ValidationIssue {
                            line: Some(line.line_number),
                            severity: Severity::Warning,
                            code: "W002",
                            message: format!("Observation references unknown call id: {}", id),
                        });
                    }
                }
            }
            LineType::Subagent => {
                result.stats.subagents += 1;
                if let Some(ref id) = line.call_id {
                    call_ids.insert(id.clone());
                }
            }
            LineType::Mcp => {
                result.stats.mcp_calls += 1;
                if let Some(ref id) = line.call_id {
                    call_ids.insert(id.clone());
                }
            }
            LineType::Question => {
                result.stats.questions += 1;
                if let Some(ref id) = line.call_id {
                    call_ids.insert(id.clone());
                }
            }
            LineType::Phase => {
                result.stats.phases += 1;
            }
            LineType::Comment => {
                result.stats.comments += 1;
            }
            LineType::Lifecycle => {
                result.stats.lifecycle_events += 1;
                // Check for @start and @end
                if line.content.starts_with("start") {
                    has_start = true;
                }
                if line.content.starts_with("end") {
                    has_end = true;
                }
            }
            LineType::Thinking => {
                result.stats.thinking_blocks += 1;
            }
            LineType::Todos => {
                result.stats.todos_updates += 1;
            }
            LineType::Unknown => {
                result.issues.push(ValidationIssue {
                    line: Some(line.line_number),
                    severity: Severity::Warning,
                    code: "W003",
                    message: format!("Unknown line format: {}", truncate(&line.raw, 50)),
                });
            }
            _ => {}
        }

        // Validate step ordering
        if let Some(step) = line.step {
            if let Some(last) = last_step
                && step < last
            {
                result.issues.push(ValidationIssue {
                    line: Some(line.line_number),
                    severity: Severity::Warning,
                    code: "W004",
                    message: format!("Step {} is less than previous step {}", step, last),
                });
            }
            last_step = Some(step);
            result.stats.max_step = Some(step);
        }

        // Track timestamps
        if line.timestamp.is_some() {
            result.stats.has_timestamps = true;
        }

        // Track token usage
        if let Some(tokens) = line.tokens_in {
            result.stats.total_tokens_in += tokens;
        }
        if let Some(tokens) = line.tokens_out {
            result.stats.total_tokens_out += tokens;
        }
        if let Some(tokens) = line.tokens_cached {
            result.stats.total_tokens_cached += tokens;
        }

        // Validate timestamp format
        if let Some(ref ts) = line.timestamp
            && !RE_ISO_TIMESTAMP.is_match(ts)
        {
            result.issues.push(ValidationIssue {
                line: Some(line.line_number),
                severity: Severity::Warning,
                code: "W005",
                message: format!("Invalid timestamp format: {}", ts),
            });
        }

        // Count blobs
        result.stats.blob_references += RE_BLOB.find_iter(&line.raw).count();

        // Count redacted values
        result.stats.redacted_values += RE_REDACTED.find_iter(&line.raw).count();

        // Validate blob sha256 format
        for cap in RE_BLOB.captures_iter(&line.raw) {
            let sha = &cap[1];
            if sha.len() < 6 {
                result.issues.push(ValidationIssue {
                    line: Some(line.line_number),
                    severity: Severity::Warning,
                    code: "W006",
                    message: format!("Blob sha256 too short: {}", sha),
                });
            }
        }
    }

    result.stats.unique_call_ids = call_ids.len();

    // Check for unresolved tool starts (streaming without completion)
    for start_id in &tool_start_ids {
        let has_completion = session.lines.iter().any(|l| {
            l.line_type == LineType::Tool
                && l.call_id.as_ref() == Some(start_id)
                && l.result.is_some()
        }) || observation_ids.contains(start_id);

        if !has_completion {
            result.issues.push(ValidationIssue {
                line: None,
                severity: Severity::Warning,
                code: "W007",
                message: format!("Tool start '{}' has no completion", start_id),
            });
        }
    }

    // Warn if no @start in long sessions
    if result.stats.total_lines > 50 && !has_start {
        result.issues.push(ValidationIssue {
            line: None,
            severity: Severity::Info,
            code: "I001",
            message: "Long session without @start lifecycle event".to_string(),
        });
    }

    // Warn if no @end in sessions with @start
    if has_start && !has_end {
        result.issues.push(ValidationIssue {
            line: None,
            severity: Severity::Info,
            code: "I002",
            message: "Session has @start but no @end".to_string(),
        });
    }

    result
}

fn validate_header(header: &Header, result: &mut ValidationResult) {
    // Check format version
    let version = header.format.strip_prefix("rlog/").unwrap_or("");
    if version != "1" && version != "1.0" {
        result.issues.push(ValidationIssue {
            line: None,
            severity: Severity::Warning,
            code: "W008",
            message: format!(
                "Unknown format version: {} (expected rlog/1)",
                header.format
            ),
        });
    }

    // Check repo_sha format (should be hex, 6-40 chars)
    if header.repo_sha.len() < 6 || header.repo_sha.len() > 40 {
        result.issues.push(ValidationIssue {
            line: None,
            severity: Severity::Warning,
            code: "W009",
            message: format!(
                "repo_sha '{}' has unusual length (expected 6-40 hex chars)",
                header.repo_sha
            ),
        });
    } else if !header.repo_sha.chars().all(|c| c.is_ascii_hexdigit()) {
        result.issues.push(ValidationIssue {
            line: None,
            severity: Severity::Warning,
            code: "W009",
            message: format!(
                "repo_sha '{}' contains non-hex characters (expected 6-40 hex chars)",
                header.repo_sha
            ),
        });
    }

    // Recommend sandbox_id for autonomous sessions
    if header.mode.as_deref() == Some("auto") && header.sandbox_id.is_none() {
        result.issues.push(ValidationIssue {
            line: None,
            severity: Severity::Info,
            code: "I003",
            message: "Auto mode session without sandbox_id".to_string(),
        });
    }

    // Recommend runner for reproducibility
    if header.runner.is_none() {
        result.issues.push(ValidationIssue {
            line: None,
            severity: Severity::Info,
            code: "I004",
            message: "No runner specified (recommended for replay)".to_string(),
        });
    }
}

fn truncate(s: &str, max_len: usize) -> String {
    if s.chars().count() <= max_len {
        s.to_string()
    } else {
        let truncated: String = s.chars().take(max_len - 3).collect();
        format!("{}...", truncated)
    }
}

// ============================================================================
// TESTS
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_minimal_header() {
        let content = r#"---
format: rlog/1
id: test_session
repo_sha: abc123
---

u: Hello
a: Hi there
"#;

        let session = parse_content(content).unwrap();
        assert_eq!(session.header.format, "rlog/1");
        assert_eq!(session.header.id, "test_session");
        assert_eq!(session.lines.len(), 3); // u, a, empty
    }

    #[test]
    fn test_parse_line_types() {
        let cases = vec![
            ("u: Hello world", LineType::User),
            ("a: Response here", LineType::Agent),
            ("t:read file.txt → [ok]", LineType::Tool),
            ("t!:test cargo test → [running]", LineType::ToolStart),
            ("t~:test [50%]", LineType::ToolProgress),
            ("o: id=call_1 → [ok]", LineType::Observation),
            ("s:skill activate → [ok]", LineType::Skill),
            ("p:create id=p1 \"Plan\" → [ok]", LineType::Plan),
            ("m: auto", LineType::Mode),
            ("r: query → [results]", LineType::Recall),
            ("x:explore \"task\" → summary", LineType::Subagent),
            ("c:github.issues state=open → [5]", LineType::Mcp),
            ("# This is a comment", LineType::Comment),
            ("@start id=sess_1", LineType::Lifecycle),
            ("th: Analyzing the request...", LineType::Thinking),
            (
                "td: [pending] Fix bug [completed] Add test",
                LineType::Todos,
            ),
            ("", LineType::Empty),
            ("  continuation line", LineType::Continuation),
        ];

        for (raw, expected_type) in cases {
            let parsed = parse_line(1, raw);
            assert_eq!(parsed.line_type, expected_type, "Failed for: {}", raw);
        }
    }

    #[test]
    fn test_extract_metadata() {
        let line = parse_line(
            1,
            "t:read id=call_42 step=5 ts=2025-12-18T10:30:00Z latency_ms=1234 → [ok]",
        );

        assert_eq!(line.call_id, Some("call_42".to_string()));
        assert_eq!(line.step, Some(5));
        assert_eq!(line.timestamp, Some("2025-12-18T10:30:00Z".to_string()));
        assert_eq!(line.latency_ms, Some(1234));
        assert_eq!(line.result, Some("[ok]".to_string()));
    }

    #[test]
    fn test_validation_missing_header_field() {
        let content = r#"---
format: rlog/1
id: test
---
"#;
        let result = parse_content(content);
        assert!(result.is_err());
    }

    #[test]
    fn test_validation_unknown_lines() {
        let content = r#"---
format: rlog/1
id: test
repo_sha: abc123
---

u: Hello
this is not a valid line
a: Response
"#;
        let session = parse_content(content).unwrap();
        let result = validate(&session);

        assert!(result.warnings().any(|w| w.code == "W003"));
    }

    #[test]
    fn test_codex_code_metadata() {
        let line = parse_line(
            1,
            "th: Let me analyze... sig=Ep4E... parent=abc-123-def tokens_in=100 tokens_out=50 tokens_cached=25 model=codex-opus-4-5",
        );

        assert_eq!(line.line_type, LineType::Thinking);
        assert_eq!(line.signature, Some("Ep4E...".to_string()));
        assert_eq!(line.parent_uuid, Some("abc-123-def".to_string()));
        assert_eq!(line.tokens_in, Some(100));
        assert_eq!(line.tokens_out, Some(50));
        assert_eq!(line.tokens_cached, Some(25));
        assert_eq!(line.model, Some("codex-opus-4-5".to_string()));
        assert!(!line.interrupted);
    }

    #[test]
    fn test_codex_code_interrupted() {
        let line = parse_line(1, "t:Bash id=call_1 interrupted → [error]");

        assert_eq!(line.line_type, LineType::Tool);
        assert!(line.interrupted);
    }

    #[test]
    fn test_codex_code_header() {
        let content = r#"---
format: rlog/1
id: sess_test
repo_sha: abc123
client_version: "2.0.71"
slug: mighty-wishing-music
cwd: /Users/test/code
tokens_total_in: 1000
tokens_total_out: 500
tokens_cached: 200
---

u: Hello
"#;
        let session = parse_content(content).unwrap();
        assert_eq!(session.header.client_version, Some("2.0.71".to_string()));
        assert_eq!(
            session.header.slug,
            Some("mighty-wishing-music".to_string())
        );
        assert_eq!(session.header.cwd, Some("/Users/test/code".to_string()));
        assert_eq!(session.header.tokens_total_in, Some(1000));
        assert_eq!(session.header.tokens_total_out, Some(500));
        assert_eq!(session.header.tokens_cached, Some(200));
    }

    #[test]
    fn test_validate_thinking_and_todos() {
        let content = r#"---
format: rlog/1
id: test
repo_sha: abc123
---

u: Fix the bug
th: Let me analyze... tokens_in=100 tokens_out=50
a: I found the issue
td: [in_progress] Fix bug [pending] Add tests
"#;
        let session = parse_content(content).unwrap();
        let result = validate(&session);

        assert!(result.is_valid());
        assert_eq!(result.stats.thinking_blocks, 1);
        assert_eq!(result.stats.todos_updates, 1);
        assert_eq!(result.stats.total_tokens_in, 100);
        assert_eq!(result.stats.total_tokens_out, 50);
    }

    #[test]
    fn test_validate_repo_sha_non_hex() {
        let content = r#"---
format: rlog/1
id: test
repo_sha: unknown
---

u: Test message
a: Response
"#;
        let session = parse_content(content).unwrap();
        let result = validate(&session);

        // Should have warning about non-hex characters
        let has_non_hex_warning = result
            .issues
            .iter()
            .any(|issue| issue.code == "W009" && issue.message.contains("non-hex characters"));
        assert!(
            has_non_hex_warning,
            "Expected warning about non-hex repo_sha"
        );
    }

    #[test]
    fn test_validate_repo_sha_valid_hex() {
        let content = r#"---
format: rlog/1
id: test
repo_sha: abc123def456
---

u: Test message
a: Response
"#;
        let session = parse_content(content).unwrap();
        let result = validate(&session);

        // Should not have warning about non-hex characters
        let has_non_hex_warning = result
            .issues
            .iter()
            .any(|issue| issue.code == "W009" && issue.message.contains("non-hex characters"));
        assert!(
            !has_non_hex_warning,
            "Should not warn about valid hex repo_sha"
        );
    }

    #[test]
    fn test_validate_repo_sha_invalid_length() {
        let content = r#"---
format: rlog/1
id: test
repo_sha: abc
---

u: Test message
a: Response
"#;
        let session = parse_content(content).unwrap();
        let result = validate(&session);

        // Should have warning about length
        let has_length_warning = result
            .issues
            .iter()
            .any(|issue| issue.code == "W009" && issue.message.contains("unusual length"));
        assert!(has_length_warning, "Expected warning about repo_sha length");
    }
}
