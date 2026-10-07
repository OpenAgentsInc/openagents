//! Closed legal benchmark tool surface.
//!
//! These tools intentionally stay replayable: shell, read, write, edit, glob,
//! grep, and deterministic document helpers. Shell execution is modeled as
//! sandbox-owned; callers must attach a sandbox runner before commands execute.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Component, Path, PathBuf};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};
use sha2::{Digest, Sha256};

use crate::{
    ExtractedArtifact, Metadata, ToolCallRecord, TranscriptEvent, TranscriptEventKind,
    stable_json_digest,
};

pub const LEGAL_BENCHMARK_TOOL_SCHEMA_VERSION: u16 = 1;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Ord, PartialOrd, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkToolName {
    Shell,
    Read,
    Write,
    Edit,
    Glob,
    Grep,
    Inventory,
    EmailSummary,
    SpreadsheetSummary,
    PdfSearch,
    EvidenceTable,
    ValidateDeliverables,
}

impl LegalBenchmarkToolName {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Shell => "shell",
            Self::Read => "read",
            Self::Write => "write",
            Self::Edit => "edit",
            Self::Glob => "glob",
            Self::Grep => "grep",
            Self::Inventory => "inventory",
            Self::EmailSummary => "email_summary",
            Self::SpreadsheetSummary => "spreadsheet_summary",
            Self::PdfSearch => "pdf_search",
            Self::EvidenceTable => "evidence_table",
            Self::ValidateDeliverables => "validate_deliverables",
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkPathRoot {
    Documents,
    Workspace,
    Output,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool", content = "input", rename_all = "snake_case")]
pub enum LegalBenchmarkToolInput {
    Shell {
        command: Vec<String>,
        timeout_ms: Option<u64>,
    },
    Read {
        root: LegalBenchmarkPathRoot,
        relative_path: String,
        prefer_extracted: bool,
    },
    Write {
        root: LegalBenchmarkPathRoot,
        relative_path: String,
        content: String,
        overwrite: bool,
    },
    Edit {
        root: LegalBenchmarkPathRoot,
        relative_path: String,
        find: String,
        replace: String,
        expected_replacements: Option<u32>,
    },
    Glob {
        root: LegalBenchmarkPathRoot,
        pattern: String,
        max_results: usize,
        include_hidden: bool,
    },
    Grep {
        root: LegalBenchmarkPathRoot,
        pattern: String,
        case_sensitive: bool,
        max_results: usize,
        include_hidden: bool,
    },
    Inventory {
        root: LegalBenchmarkPathRoot,
        max_results: usize,
        include_hidden: bool,
        include_hashes: bool,
    },
    EmailSummary {
        root: LegalBenchmarkPathRoot,
        relative_path: String,
        max_body_chars: usize,
    },
    SpreadsheetSummary {
        root: LegalBenchmarkPathRoot,
        relative_path: String,
        max_preview_rows: usize,
    },
    PdfSearch {
        root: LegalBenchmarkPathRoot,
        relative_path: String,
        query: String,
        page: Option<u32>,
        max_matches: usize,
    },
    EvidenceTable {
        entries: Vec<LegalBenchmarkEvidenceTableEntry>,
    },
    ValidateDeliverables {
        root: LegalBenchmarkPathRoot,
        required_paths: Vec<String>,
        max_results: usize,
    },
}

impl LegalBenchmarkToolInput {
    pub const fn tool_name(&self) -> LegalBenchmarkToolName {
        match self {
            Self::Shell { .. } => LegalBenchmarkToolName::Shell,
            Self::Read { .. } => LegalBenchmarkToolName::Read,
            Self::Write { .. } => LegalBenchmarkToolName::Write,
            Self::Edit { .. } => LegalBenchmarkToolName::Edit,
            Self::Glob { .. } => LegalBenchmarkToolName::Glob,
            Self::Grep { .. } => LegalBenchmarkToolName::Grep,
            Self::Inventory { .. } => LegalBenchmarkToolName::Inventory,
            Self::EmailSummary { .. } => LegalBenchmarkToolName::EmailSummary,
            Self::SpreadsheetSummary { .. } => LegalBenchmarkToolName::SpreadsheetSummary,
            Self::PdfSearch { .. } => LegalBenchmarkToolName::PdfSearch,
            Self::EvidenceTable { .. } => LegalBenchmarkToolName::EvidenceTable,
            Self::ValidateDeliverables { .. } => LegalBenchmarkToolName::ValidateDeliverables,
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum LegalBenchmarkToolFailureKind {
    SandboxUnavailable,
    SandboxFailed,
    InvalidPath,
    ReadForbidden,
    WriteForbidden,
    FileMissing,
    FileExists,
    InputTooLarge,
    BinaryFile,
    EditConflict,
    PatternInvalid,
    LimitExceeded,
    IoError,
    InternalError,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkPathTouch {
    pub root: LegalBenchmarkPathRoot,
    pub relative_path: String,
    pub access: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub before_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub after_hash: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkGrepMatch {
    pub root: LegalBenchmarkPathRoot,
    pub relative_path: String,
    pub line_number: u64,
    pub line: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkInventoryArtifact {
    pub root: LegalBenchmarkPathRoot,
    pub relative_path: String,
    pub byte_size: u64,
    pub media_type: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub extracted_text_available: bool,
    pub text_readable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub page_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sheet_count: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message_count: Option<u32>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEmailSummary {
    pub relative_path: String,
    pub from: Option<String>,
    pub to: Option<String>,
    pub subject: Option<String>,
    pub date: Option<String>,
    pub body_preview: String,
    pub attachment_count: u32,
    pub warning_count: u32,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkSpreadsheetSummary {
    pub relative_path: String,
    pub row_count: u64,
    pub column_count: u64,
    pub formula_count: u64,
    pub preview_rows: Vec<Vec<String>>,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkPdfSearchMatch {
    pub relative_path: String,
    pub page: u32,
    pub snippet: String,
    pub span_hash: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvidenceTableEntry {
    pub source_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    pub quote: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkEvidenceTableRow {
    pub evidence_id: String,
    pub source_ref: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub locator: Option<String>,
    pub quote_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub note: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkDeliverableValidation {
    pub relative_path: String,
    pub exists: bool,
    pub readable: bool,
    pub byte_size: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sha256: Option<String>,
    pub media_type: String,
    pub warnings: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(tag = "tool", content = "output", rename_all = "snake_case")]
pub enum LegalBenchmarkToolOutput {
    Shell {
        stdout: String,
        stderr: String,
        exit_code: Option<i32>,
        sandbox_receipt_ref: Option<String>,
    },
    Read {
        content: String,
        source: String,
        bytes_read: u64,
    },
    Write {
        relative_path: String,
        bytes_written: u64,
        after_hash: String,
    },
    Edit {
        relative_path: String,
        replacements: u32,
        before_hash: String,
        after_hash: String,
        bytes_written: u64,
    },
    Glob {
        matches: Vec<String>,
        truncated: bool,
    },
    Grep {
        matches: Vec<LegalBenchmarkGrepMatch>,
        binary_files_skipped: u32,
        truncated: bool,
    },
    Inventory {
        artifacts: Vec<LegalBenchmarkInventoryArtifact>,
        truncated: bool,
    },
    EmailSummary {
        summary: LegalBenchmarkEmailSummary,
    },
    SpreadsheetSummary {
        summary: LegalBenchmarkSpreadsheetSummary,
    },
    PdfSearch {
        matches: Vec<LegalBenchmarkPdfSearchMatch>,
        truncated: bool,
    },
    EvidenceTable {
        rows: Vec<LegalBenchmarkEvidenceTableRow>,
        markdown: String,
    },
    ValidateDeliverables {
        validations: Vec<LegalBenchmarkDeliverableValidation>,
        all_present_and_readable: bool,
        missing_count: u32,
        unreadable_count: u32,
    },
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkToolReceipt {
    pub schema_version: u16,
    pub tool_call_id: String,
    pub tool_name: LegalBenchmarkToolName,
    pub input_hash: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output_hash: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_kind: Option<LegalBenchmarkToolFailureKind>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub failure_detail: Option<String>,
    pub elapsed_ms: u64,
    pub bytes_read: u64,
    pub bytes_written: u64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub exit_code: Option<i32>,
    pub touched_paths: Vec<LegalBenchmarkPathTouch>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sandbox_receipt_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Metadata::is_empty")]
    pub metadata: Metadata,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct LegalBenchmarkToolExecution {
    pub receipt: LegalBenchmarkToolReceipt,
    pub input: LegalBenchmarkToolInput,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<LegalBenchmarkToolOutput>,
    pub transcript_events: Vec<TranscriptEvent>,
    pub tool_call_record: ToolCallRecord,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct LegalBenchmarkToolWorkspace {
    pub documents_root: PathBuf,
    pub workspace_root: PathBuf,
    pub output_root: PathBuf,
    pub extracted_text_by_path: BTreeMap<String, String>,
    pub max_read_bytes: usize,
    pub max_write_bytes: usize,
}

impl LegalBenchmarkToolWorkspace {
    pub fn new(
        documents_root: impl Into<PathBuf>,
        workspace_root: impl Into<PathBuf>,
        output_root: impl Into<PathBuf>,
    ) -> Self {
        Self {
            documents_root: documents_root.into(),
            workspace_root: workspace_root.into(),
            output_root: output_root.into(),
            extracted_text_by_path: BTreeMap::new(),
            max_read_bytes: 4 * 1024 * 1024,
            max_write_bytes: 4 * 1024 * 1024,
        }
    }

    pub fn with_extracted_artifacts(mut self, artifacts: &[ExtractedArtifact]) -> Self {
        for artifact in artifacts {
            self.extracted_text_by_path.insert(
                artifact.artifact.relative_path.clone(),
                artifact.text.clone(),
            );
        }
        self
    }
}

pub fn execute_legal_benchmark_tool(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
) -> LegalBenchmarkToolExecution {
    let started = Instant::now();
    match &input {
        LegalBenchmarkToolInput::Shell { .. } => failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::SandboxUnavailable,
            "shell tool requires an attached sandbox backend",
            Vec::new(),
            0,
            0,
            None,
            None,
        ),
        LegalBenchmarkToolInput::Read {
            root,
            relative_path,
            prefer_extracted,
        } => execute_read(
            workspace,
            input.clone(),
            *root,
            relative_path,
            *prefer_extracted,
            started,
        ),
        LegalBenchmarkToolInput::Write {
            root,
            relative_path,
            content,
            overwrite,
        } => execute_write(
            workspace,
            input.clone(),
            *root,
            relative_path,
            content,
            *overwrite,
            started,
        ),
        LegalBenchmarkToolInput::Edit {
            root,
            relative_path,
            find,
            replace,
            expected_replacements,
        } => execute_edit(
            workspace,
            input.clone(),
            *root,
            relative_path,
            find,
            replace,
            *expected_replacements,
            started,
        ),
        LegalBenchmarkToolInput::Glob {
            root,
            pattern,
            max_results,
            include_hidden,
        } => execute_glob(
            workspace,
            input.clone(),
            *root,
            pattern,
            *max_results,
            *include_hidden,
            started,
        ),
        LegalBenchmarkToolInput::Grep {
            root,
            pattern,
            case_sensitive,
            max_results,
            include_hidden,
        } => execute_grep(
            workspace,
            input.clone(),
            *root,
            pattern,
            *case_sensitive,
            *max_results,
            *include_hidden,
            started,
        ),
        LegalBenchmarkToolInput::Inventory {
            root,
            max_results,
            include_hidden,
            include_hashes,
        } => execute_inventory(
            workspace,
            input.clone(),
            *root,
            *max_results,
            *include_hidden,
            *include_hashes,
            started,
        ),
        LegalBenchmarkToolInput::EmailSummary {
            root,
            relative_path,
            max_body_chars,
        } => execute_email_summary(
            workspace,
            input.clone(),
            *root,
            relative_path,
            *max_body_chars,
            started,
        ),
        LegalBenchmarkToolInput::SpreadsheetSummary {
            root,
            relative_path,
            max_preview_rows,
        } => execute_spreadsheet_summary(
            workspace,
            input.clone(),
            *root,
            relative_path,
            *max_preview_rows,
            started,
        ),
        LegalBenchmarkToolInput::PdfSearch {
            root,
            relative_path,
            query,
            page,
            max_matches,
        } => execute_pdf_search(
            workspace,
            input.clone(),
            *root,
            relative_path,
            query,
            *page,
            *max_matches,
            started,
        ),
        LegalBenchmarkToolInput::EvidenceTable { entries } => {
            execute_evidence_table(input.clone(), entries.as_slice(), started)
        }
        LegalBenchmarkToolInput::ValidateDeliverables {
            root,
            required_paths,
            max_results,
        } => execute_validate_deliverables(
            workspace,
            input.clone(),
            *root,
            required_paths.as_slice(),
            *max_results,
            started,
        ),
    }
}

#[cfg(feature = "full")]
pub fn execute_shell_with_podman(
    input: LegalBenchmarkToolInput,
    podman: &psionic_sandbox::PodmanSandboxBackend,
    config: &psionic_sandbox::PodmanSandboxConfig,
) -> LegalBenchmarkToolExecution {
    use psionic_sandbox::SandboxCommandBackend;

    let started = Instant::now();
    let LegalBenchmarkToolInput::Shell { command, .. } = &input else {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::InternalError,
            "execute_shell_with_podman only accepts shell input",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    };
    match podman.run_sandbox_command(config, command.as_slice()) {
        Ok(output) => {
            let stdout = String::from_utf8_lossy(output.stdout.as_slice()).to_string();
            let stderr = String::from_utf8_lossy(output.stderr.as_slice()).to_string();
            let sandbox_receipt_ref = output.receipt.command_digest.clone();
            success_execution(
                input,
                started,
                LegalBenchmarkToolOutput::Shell {
                    stdout,
                    stderr,
                    exit_code: output.receipt.exit_code,
                    sandbox_receipt_ref: Some(sandbox_receipt_ref.clone()),
                },
                Vec::new(),
                output.receipt.stdout_bytes,
                0,
                output.receipt.exit_code,
                Some(sandbox_receipt_ref),
            )
        }
        Err(error) => failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::SandboxFailed,
            format!("sandbox command failed: {error}"),
            Vec::new(),
            0,
            0,
            None,
            None,
        ),
    }
}

pub fn legal_benchmark_tool_receipt_digest(
    receipt: &LegalBenchmarkToolReceipt,
) -> Result<String, serde_json::Error> {
    stable_json_digest("psionic.legal_benchmark.tool_receipt.v1", receipt)
}

fn execute_read(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    prefer_extracted: bool,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    if prefer_extracted {
        if let Some(text) = workspace.extracted_text_by_path.get(relative_path) {
            return success_execution(
                input,
                started,
                LegalBenchmarkToolOutput::Read {
                    content: text.clone(),
                    source: "extracted_text".to_string(),
                    bytes_read: u64::try_from(text.len()).unwrap_or(u64::MAX),
                },
                vec![LegalBenchmarkPathTouch {
                    root,
                    relative_path: relative_path.to_string(),
                    access: "read_extracted".to_string(),
                    before_hash: None,
                    after_hash: None,
                }],
                u64::try_from(text.len()).unwrap_or(u64::MAX),
                0,
                None,
                None,
            );
        }
    }
    let resolved = match resolve_existing_path(workspace, root, relative_path) {
        Ok(path) => path,
        Err(error) => return path_failure(input, started, root, relative_path, error),
    };
    let bytes = match fs::read(resolved.as_path()) {
        Ok(bytes) => bytes,
        Err(error) => {
            return failure_execution(
                input,
                started,
                LegalBenchmarkToolFailureKind::IoError,
                format!("failed to read file: {error}"),
                Vec::new(),
                0,
                0,
                None,
                None,
            );
        }
    };
    if bytes.len() > workspace.max_read_bytes {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::InputTooLarge,
            "read file exceeds workspace max_read_bytes",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    let Ok(content) = String::from_utf8(bytes.clone()) else {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::BinaryFile,
            "read refuses binary or non-UTF-8 files",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    };
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::Read {
            content,
            source: "raw_file".to_string(),
            bytes_read: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        },
        vec![LegalBenchmarkPathTouch {
            root,
            relative_path: relative_path.to_string(),
            access: "read".to_string(),
            before_hash: Some(sha256_hex(bytes.as_slice())),
            after_hash: None,
        }],
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        0,
        None,
        None,
    )
}

fn execute_write(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    content: &str,
    overwrite: bool,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    if !matches!(
        root,
        LegalBenchmarkPathRoot::Workspace | LegalBenchmarkPathRoot::Output
    ) {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::WriteForbidden,
            "write is allowed only in workspace or output roots",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    if content.len() > workspace.max_write_bytes {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::InputTooLarge,
            "write content exceeds workspace max_write_bytes",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    let resolved = match resolve_new_or_existing_path(workspace, root, relative_path) {
        Ok(path) => path,
        Err(error) => return path_failure(input, started, root, relative_path, error),
    };
    if resolved.exists() && !overwrite {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::FileExists,
            "write refused to overwrite existing file",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    if let Some(parent) = resolved.parent() {
        if let Err(error) = fs::create_dir_all(parent) {
            return failure_execution(
                input,
                started,
                LegalBenchmarkToolFailureKind::IoError,
                format!("failed to create parent directory: {error}"),
                Vec::new(),
                0,
                0,
                None,
                None,
            );
        }
    }
    if let Err(error) = fs::write(resolved.as_path(), content.as_bytes()) {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::IoError,
            format!("failed to write file: {error}"),
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    let after_hash = sha256_hex(content.as_bytes());
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::Write {
            relative_path: relative_path.to_string(),
            bytes_written: u64::try_from(content.len()).unwrap_or(u64::MAX),
            after_hash: after_hash.clone(),
        },
        vec![LegalBenchmarkPathTouch {
            root,
            relative_path: relative_path.to_string(),
            access: "write".to_string(),
            before_hash: None,
            after_hash: Some(after_hash),
        }],
        0,
        u64::try_from(content.len()).unwrap_or(u64::MAX),
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_edit(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    find: &str,
    replace: &str,
    expected_replacements: Option<u32>,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    if find.is_empty() {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::EditConflict,
            "edit find string cannot be empty",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    if !matches!(
        root,
        LegalBenchmarkPathRoot::Workspace | LegalBenchmarkPathRoot::Output
    ) {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::WriteForbidden,
            "edit is allowed only in workspace or output roots",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    let resolved = match resolve_existing_path(workspace, root, relative_path) {
        Ok(path) => path,
        Err(error) => return path_failure(input, started, root, relative_path, error),
    };
    let bytes = match fs::read(resolved.as_path()) {
        Ok(bytes) => bytes,
        Err(error) => {
            return failure_execution(
                input,
                started,
                LegalBenchmarkToolFailureKind::IoError,
                format!("failed to read file for edit: {error}"),
                Vec::new(),
                0,
                0,
                None,
                None,
            );
        }
    };
    let Ok(content) = String::from_utf8(bytes.clone()) else {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::BinaryFile,
            "edit refuses binary or non-UTF-8 files",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    };
    let replacements = u32::try_from(content.matches(find).count()).unwrap_or(u32::MAX);
    if replacements == 0 || expected_replacements.is_some_and(|expected| expected != replacements) {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::EditConflict,
            "edit replacement count did not match expectation",
            Vec::new(),
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            0,
            None,
            None,
        );
    }
    let edited = content.replace(find, replace);
    if edited.len() > workspace.max_write_bytes {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::InputTooLarge,
            "edited content exceeds workspace max_write_bytes",
            Vec::new(),
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            0,
            None,
            None,
        );
    }
    if let Err(error) = fs::write(resolved.as_path(), edited.as_bytes()) {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::IoError,
            format!("failed to write edited file: {error}"),
            Vec::new(),
            u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            0,
            None,
            None,
        );
    }
    let before_hash = sha256_hex(bytes.as_slice());
    let after_hash = sha256_hex(edited.as_bytes());
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::Edit {
            relative_path: relative_path.to_string(),
            replacements,
            before_hash: before_hash.clone(),
            after_hash: after_hash.clone(),
            bytes_written: u64::try_from(edited.len()).unwrap_or(u64::MAX),
        },
        vec![LegalBenchmarkPathTouch {
            root,
            relative_path: relative_path.to_string(),
            access: "edit".to_string(),
            before_hash: Some(before_hash),
            after_hash: Some(after_hash),
        }],
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
        u64::try_from(edited.len()).unwrap_or(u64::MAX),
        None,
        None,
    )
}

fn execute_glob(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    pattern: &str,
    max_results: usize,
    include_hidden: bool,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    if pattern.trim().is_empty() {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::PatternInvalid,
            "glob pattern cannot be empty",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    let root_path = root_path(workspace, root);
    let files = match list_files(root_path, include_hidden) {
        Ok(files) => files,
        Err(error) => {
            return failure_execution(
                input,
                started,
                LegalBenchmarkToolFailureKind::IoError,
                format!("failed to list files: {error}"),
                Vec::new(),
                0,
                0,
                None,
                None,
            );
        }
    };
    let mut matches = files
        .into_iter()
        .filter(|path| wildcard_matches(pattern, path.as_str()))
        .collect::<Vec<_>>();
    matches.sort();
    let truncated = matches.len() > max_results;
    matches.truncate(max_results);
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::Glob { matches, truncated },
        Vec::new(),
        0,
        0,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_grep(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    pattern: &str,
    case_sensitive: bool,
    max_results: usize,
    include_hidden: bool,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    if pattern.is_empty() {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::PatternInvalid,
            "grep pattern cannot be empty",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    let root_path = root_path(workspace, root);
    let files = match list_files(root_path, include_hidden) {
        Ok(files) => files,
        Err(error) => {
            return failure_execution(
                input,
                started,
                LegalBenchmarkToolFailureKind::IoError,
                format!("failed to list files: {error}"),
                Vec::new(),
                0,
                0,
                None,
                None,
            );
        }
    };
    let needle = if case_sensitive {
        pattern.to_string()
    } else {
        pattern.to_ascii_lowercase()
    };
    let mut matches = Vec::new();
    let mut binary_files_skipped = 0u32;
    let mut bytes_read = 0u64;
    'files: for relative_path in files {
        let path = root_path.join(relative_path.as_str());
        let bytes = match fs::read(path.as_path()) {
            Ok(bytes) => bytes,
            Err(_) => continue,
        };
        bytes_read = bytes_read.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let Ok(content) = String::from_utf8(bytes) else {
            binary_files_skipped = binary_files_skipped.saturating_add(1);
            continue;
        };
        for (line_index, line) in content.lines().enumerate() {
            let haystack = if case_sensitive {
                line.to_string()
            } else {
                line.to_ascii_lowercase()
            };
            if haystack.contains(needle.as_str()) {
                matches.push(LegalBenchmarkGrepMatch {
                    root,
                    relative_path: relative_path.clone(),
                    line_number: u64::try_from(line_index + 1).unwrap_or(u64::MAX),
                    line: line.to_string(),
                });
                if matches.len() >= max_results {
                    break 'files;
                }
            }
        }
    }
    let truncated = matches.len() >= max_results;
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::Grep {
            matches,
            binary_files_skipped,
            truncated,
        },
        Vec::new(),
        bytes_read,
        0,
        None,
        None,
    )
}

fn execute_inventory(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    max_results: usize,
    include_hidden: bool,
    include_hashes: bool,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    let root_path = root_path(workspace, root);
    let files = match list_files(root_path, include_hidden) {
        Ok(files) => files,
        Err(error) => {
            return failure_execution(
                input,
                started,
                LegalBenchmarkToolFailureKind::IoError,
                format!("failed to list files: {error}"),
                Vec::new(),
                0,
                0,
                None,
                None,
            );
        }
    };
    let truncated = files.len() > max_results;
    let mut artifacts = Vec::new();
    let mut bytes_read = 0u64;
    for relative_path in files.into_iter().take(max_results) {
        let path = root_path.join(relative_path.as_str());
        let bytes = match fs::read(path.as_path()) {
            Ok(bytes) => bytes,
            Err(error) => {
                artifacts.push(LegalBenchmarkInventoryArtifact {
                    root,
                    relative_path,
                    byte_size: 0,
                    media_type: String::from("application/octet-stream"),
                    sha256: None,
                    extracted_text_available: false,
                    text_readable: false,
                    page_count: None,
                    sheet_count: None,
                    message_count: None,
                    warnings: vec![format!("failed to read file: {error}")],
                });
                continue;
            }
        };
        bytes_read = bytes_read.saturating_add(u64::try_from(bytes.len()).unwrap_or(u64::MAX));
        let content = String::from_utf8(bytes.clone()).ok();
        let media_type = guess_media_type(relative_path.as_str());
        let mut warnings = Vec::new();
        if content.is_none()
            && !workspace
                .extracted_text_by_path
                .contains_key(&relative_path)
        {
            warnings.push(String::from("binary_or_non_utf8_without_extracted_text"));
        }
        artifacts.push(LegalBenchmarkInventoryArtifact {
            root,
            relative_path: relative_path.clone(),
            byte_size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            media_type,
            sha256: include_hashes.then(|| sha256_hex(bytes.as_slice())),
            extracted_text_available: workspace
                .extracted_text_by_path
                .contains_key(&relative_path),
            text_readable: content.is_some(),
            page_count: page_count_hint(
                relative_path.as_str(),
                content.as_deref(),
                bytes.as_slice(),
            ),
            sheet_count: sheet_count_hint(
                relative_path.as_str(),
                content.as_deref(),
                bytes.as_slice(),
            ),
            message_count: message_count_hint(relative_path.as_str(), content.as_deref()),
            warnings,
        });
    }
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::Inventory {
            artifacts,
            truncated,
        },
        Vec::new(),
        bytes_read,
        0,
        None,
        None,
    )
}

fn execute_email_summary(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    max_body_chars: usize,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    let (content, source, bytes_read) =
        match read_text_for_helper(workspace, root, relative_path, true) {
            Ok(value) => value,
            Err(error) => return path_or_read_failure(input, started, root, relative_path, error),
        };
    let headers = parse_email_headers(content.as_str());
    let body = email_body(content.as_str());
    let summary = LegalBenchmarkEmailSummary {
        relative_path: relative_path.to_string(),
        from: headers.get("from").cloned(),
        to: headers.get("to").cloned(),
        subject: headers.get("subject").cloned(),
        date: headers.get("date").cloned(),
        body_preview: truncate_chars(body.trim(), max_body_chars),
        attachment_count: u32::try_from(
            content
                .to_ascii_lowercase()
                .matches("content-disposition: attachment")
                .count(),
        )
        .unwrap_or(u32::MAX),
        warning_count: u32::from(source != "raw_file" && source != "extracted_text"),
    };
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::EmailSummary { summary },
        vec![LegalBenchmarkPathTouch {
            root,
            relative_path: relative_path.to_string(),
            access: "email_summary".to_string(),
            before_hash: None,
            after_hash: None,
        }],
        bytes_read,
        0,
        None,
        None,
    )
}

fn execute_spreadsheet_summary(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    max_preview_rows: usize,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    let (content, _source, bytes_read) =
        match read_text_for_helper(workspace, root, relative_path, true) {
            Ok(value) => value,
            Err(error) => return path_or_read_failure(input, started, root, relative_path, error),
        };
    let delimiter = if relative_path.ends_with(".tsv") {
        '\t'
    } else {
        ','
    };
    let rows = content
        .lines()
        .map(|line| split_delimited_row(line, delimiter))
        .collect::<Vec<_>>();
    let formula_count = rows
        .iter()
        .flat_map(|row| row.iter())
        .filter(|cell| cell.trim_start().starts_with('='))
        .count();
    let column_count = rows.iter().map(Vec::len).max().unwrap_or(0);
    let warnings = if relative_path.ends_with(".xlsx") {
        vec![String::from(
            "xlsx summary requires extracted text or a sandboxed office adapter for full fidelity",
        )]
    } else {
        Vec::new()
    };
    let summary = LegalBenchmarkSpreadsheetSummary {
        relative_path: relative_path.to_string(),
        row_count: u64::try_from(rows.len()).unwrap_or(u64::MAX),
        column_count: u64::try_from(column_count).unwrap_or(u64::MAX),
        formula_count: u64::try_from(formula_count).unwrap_or(u64::MAX),
        preview_rows: rows.into_iter().take(max_preview_rows).collect(),
        warnings,
    };
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::SpreadsheetSummary { summary },
        vec![LegalBenchmarkPathTouch {
            root,
            relative_path: relative_path.to_string(),
            access: "spreadsheet_summary".to_string(),
            before_hash: None,
            after_hash: None,
        }],
        bytes_read,
        0,
        None,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn execute_pdf_search(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    query: &str,
    page: Option<u32>,
    max_matches: usize,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    if query.trim().is_empty() {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::PatternInvalid,
            "pdf search query cannot be empty",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    let (content, _source, bytes_read) =
        match read_text_for_helper(workspace, root, relative_path, true) {
            Ok(value) => value,
            Err(error) => return path_or_read_failure(input, started, root, relative_path, error),
        };
    let needle = query.to_ascii_lowercase();
    let mut matches = Vec::new();
    for (page_index, page_text) in content.split('\u{000c}').enumerate() {
        let page_number = u32::try_from(page_index + 1).unwrap_or(u32::MAX);
        if page.is_some_and(|target| target != page_number) {
            continue;
        }
        let lowered = page_text.to_ascii_lowercase();
        if let Some(index) = lowered.find(needle.as_str()) {
            let snippet = snippet_around(page_text, index, query.len(), 120);
            let span_hash =
                sha256_hex(format!("{relative_path}|{page_number}|{snippet}").as_bytes());
            matches.push(LegalBenchmarkPdfSearchMatch {
                relative_path: relative_path.to_string(),
                page: page_number,
                snippet,
                span_hash,
            });
            if matches.len() >= max_matches {
                break;
            }
        }
    }
    let truncated = matches.len() >= max_matches;
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::PdfSearch { matches, truncated },
        vec![LegalBenchmarkPathTouch {
            root,
            relative_path: relative_path.to_string(),
            access: "pdf_search".to_string(),
            before_hash: None,
            after_hash: None,
        }],
        bytes_read,
        0,
        None,
        None,
    )
}

fn execute_evidence_table(
    input: LegalBenchmarkToolInput,
    entries: &[LegalBenchmarkEvidenceTableEntry],
    started: Instant,
) -> LegalBenchmarkToolExecution {
    let rows = entries
        .iter()
        .enumerate()
        .map(|(index, entry)| {
            let quote_hash = sha256_hex(entry.quote.as_bytes());
            LegalBenchmarkEvidenceTableRow {
                evidence_id: format!("evidence.table.{index}.{quote_hash}"),
                source_ref: entry.source_ref.clone(),
                locator: entry.locator.clone(),
                quote_hash,
                note: entry.note.clone(),
            }
        })
        .collect::<Vec<_>>();
    let markdown = render_evidence_markdown(entries, rows.as_slice());
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::EvidenceTable { rows, markdown },
        Vec::new(),
        0,
        0,
        None,
        None,
    )
}

fn execute_validate_deliverables(
    workspace: &LegalBenchmarkToolWorkspace,
    input: LegalBenchmarkToolInput,
    root: LegalBenchmarkPathRoot,
    required_paths: &[String],
    max_results: usize,
    started: Instant,
) -> LegalBenchmarkToolExecution {
    if !matches!(
        root,
        LegalBenchmarkPathRoot::Output | LegalBenchmarkPathRoot::Workspace
    ) {
        return failure_execution(
            input,
            started,
            LegalBenchmarkToolFailureKind::ReadForbidden,
            "deliverable validation is allowed only in output or workspace roots",
            Vec::new(),
            0,
            0,
            None,
            None,
        );
    }
    let mut validations = Vec::new();
    let mut bytes_read = 0u64;
    for relative_path in required_paths.iter().take(max_results) {
        match validate_one_deliverable(workspace, root, relative_path) {
            Ok((validation, bytes)) => {
                bytes_read = bytes_read.saturating_add(bytes);
                validations.push(validation);
            }
            Err(detail) => validations.push(LegalBenchmarkDeliverableValidation {
                relative_path: relative_path.clone(),
                exists: false,
                readable: false,
                byte_size: 0,
                sha256: None,
                media_type: String::from("application/octet-stream"),
                warnings: vec![detail],
            }),
        }
    }
    let missing_count = validations
        .iter()
        .filter(|validation| !validation.exists)
        .count();
    let unreadable_count = validations
        .iter()
        .filter(|validation| validation.exists && !validation.readable)
        .count();
    success_execution(
        input,
        started,
        LegalBenchmarkToolOutput::ValidateDeliverables {
            validations,
            all_present_and_readable: missing_count == 0 && unreadable_count == 0,
            missing_count: u32::try_from(missing_count).unwrap_or(u32::MAX),
            unreadable_count: u32::try_from(unreadable_count).unwrap_or(u32::MAX),
        },
        Vec::new(),
        bytes_read,
        0,
        None,
        None,
    )
}

fn success_execution(
    input: LegalBenchmarkToolInput,
    started: Instant,
    output: LegalBenchmarkToolOutput,
    touched_paths: Vec<LegalBenchmarkPathTouch>,
    bytes_read: u64,
    bytes_written: u64,
    exit_code: Option<i32>,
    sandbox_receipt_ref: Option<String>,
) -> LegalBenchmarkToolExecution {
    let output_hash = stable_json_digest("psionic.legal_benchmark.tool_output.v1", &output).ok();
    build_execution(
        input,
        Some(output),
        None,
        None,
        started,
        touched_paths,
        bytes_read,
        bytes_written,
        exit_code,
        sandbox_receipt_ref,
        output_hash,
    )
}

#[allow(clippy::too_many_arguments)]
fn failure_execution(
    input: LegalBenchmarkToolInput,
    started: Instant,
    failure_kind: LegalBenchmarkToolFailureKind,
    failure_detail: impl Into<String>,
    touched_paths: Vec<LegalBenchmarkPathTouch>,
    bytes_read: u64,
    bytes_written: u64,
    exit_code: Option<i32>,
    sandbox_receipt_ref: Option<String>,
) -> LegalBenchmarkToolExecution {
    build_execution(
        input,
        None,
        Some(failure_kind),
        Some(failure_detail.into()),
        started,
        touched_paths,
        bytes_read,
        bytes_written,
        exit_code,
        sandbox_receipt_ref,
        None,
    )
}

#[allow(clippy::too_many_arguments)]
fn build_execution(
    input: LegalBenchmarkToolInput,
    output: Option<LegalBenchmarkToolOutput>,
    failure_kind: Option<LegalBenchmarkToolFailureKind>,
    failure_detail: Option<String>,
    started: Instant,
    touched_paths: Vec<LegalBenchmarkPathTouch>,
    bytes_read: u64,
    bytes_written: u64,
    exit_code: Option<i32>,
    sandbox_receipt_ref: Option<String>,
    output_hash: Option<String>,
) -> LegalBenchmarkToolExecution {
    let input_hash =
        stable_json_digest("psionic.legal_benchmark.tool_input.v1", &input).unwrap_or_default();
    let tool_call_id = deterministic_tool_call_id(input.tool_name(), input_hash.as_str());
    let elapsed_ms = u64::try_from(started.elapsed().as_millis()).unwrap_or(u64::MAX);
    let receipt = LegalBenchmarkToolReceipt {
        schema_version: LEGAL_BENCHMARK_TOOL_SCHEMA_VERSION,
        tool_call_id: tool_call_id.clone(),
        tool_name: input.tool_name(),
        input_hash,
        output_hash,
        failure_kind,
        failure_detail,
        elapsed_ms,
        bytes_read,
        bytes_written,
        exit_code,
        touched_paths,
        sandbox_receipt_ref,
        metadata: BTreeMap::new(),
    };
    let output_value = output
        .as_ref()
        .and_then(|output| serde_json::to_value(output).ok());
    let error_kind = receipt.failure_kind.map(|kind| format!("{kind:?}"));
    let transcript_events =
        transcript_events(&tool_call_id, &input, output_value.clone(), &receipt);
    let tool_call_record = ToolCallRecord {
        tool_call_id,
        tool_name: receipt.tool_name.as_str().to_string(),
        call_event_index: 0,
        result_event_index: Some(1),
        input: serde_json::to_value(&input).unwrap_or(Value::Null),
        output: output_value,
        error_kind,
        elapsed_ms,
    };
    LegalBenchmarkToolExecution {
        receipt,
        input,
        output,
        transcript_events,
        tool_call_record,
    }
}

fn transcript_events(
    tool_call_id: &str,
    input: &LegalBenchmarkToolInput,
    output: Option<Value>,
    receipt: &LegalBenchmarkToolReceipt,
) -> Vec<TranscriptEvent> {
    let timestamp = now_ms();
    vec![
        TranscriptEvent {
            event_index: 0,
            event_kind: TranscriptEventKind::ToolCall,
            role: Some("assistant".to_string()),
            content: None,
            payload: Some(json!({
                "tool_call_id": tool_call_id,
                "tool_name": input.tool_name().as_str(),
                "input": input
            })),
            timestamp_ms: timestamp,
        },
        TranscriptEvent {
            event_index: 1,
            event_kind: TranscriptEventKind::ToolResult,
            role: Some("tool".to_string()),
            content: None,
            payload: Some(json!({
                "tool_call_id": tool_call_id,
                "output": output,
                "receipt": receipt
            })),
            timestamp_ms: timestamp,
        },
    ]
}

fn path_or_read_failure(
    input: LegalBenchmarkToolInput,
    started: Instant,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    detail: String,
) -> LegalBenchmarkToolExecution {
    let failure_kind = if detail.contains("binary") {
        LegalBenchmarkToolFailureKind::BinaryFile
    } else if detail.contains("path") || detail.contains("root") || detail.contains("exist") {
        LegalBenchmarkToolFailureKind::InvalidPath
    } else {
        LegalBenchmarkToolFailureKind::IoError
    };
    failure_execution(
        input,
        started,
        failure_kind,
        detail,
        vec![LegalBenchmarkPathTouch {
            root,
            relative_path: relative_path.to_string(),
            access: "rejected".to_string(),
            before_hash: None,
            after_hash: None,
        }],
        0,
        0,
        None,
        None,
    )
}

fn path_failure(
    input: LegalBenchmarkToolInput,
    started: Instant,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    detail: String,
) -> LegalBenchmarkToolExecution {
    failure_execution(
        input,
        started,
        LegalBenchmarkToolFailureKind::InvalidPath,
        detail,
        vec![LegalBenchmarkPathTouch {
            root,
            relative_path: relative_path.to_string(),
            access: "rejected".to_string(),
            before_hash: None,
            after_hash: None,
        }],
        0,
        0,
        None,
        None,
    )
}

fn read_text_for_helper(
    workspace: &LegalBenchmarkToolWorkspace,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
    prefer_extracted: bool,
) -> Result<(String, &'static str, u64), String> {
    if prefer_extracted && let Some(text) = workspace.extracted_text_by_path.get(relative_path) {
        return Ok((
            text.clone(),
            "extracted_text",
            u64::try_from(text.len()).unwrap_or(u64::MAX),
        ));
    }
    let path = resolve_existing_path(workspace, root, relative_path)?;
    let bytes =
        fs::read(path.as_path()).map_err(|error| format!("failed to read file: {error}"))?;
    if bytes.len() > workspace.max_read_bytes {
        return Err(String::from("read file exceeds workspace max_read_bytes"));
    }
    let content = String::from_utf8(bytes.clone())
        .map_err(|_| String::from("binary or non-UTF-8 file requires extracted text"))?;
    Ok((
        content,
        "raw_file",
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    ))
}

fn validate_one_deliverable(
    workspace: &LegalBenchmarkToolWorkspace,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
) -> Result<(LegalBenchmarkDeliverableValidation, u64), String> {
    let path = resolve_existing_path(workspace, root, relative_path)?;
    let bytes =
        fs::read(path.as_path()).map_err(|error| format!("failed to read file: {error}"))?;
    let readable = String::from_utf8(bytes.clone()).is_ok();
    let mut warnings = Vec::new();
    if !readable {
        warnings.push(String::from("non_utf8_or_binary"));
    }
    Ok((
        LegalBenchmarkDeliverableValidation {
            relative_path: relative_path.to_string(),
            exists: true,
            readable,
            byte_size: u64::try_from(bytes.len()).unwrap_or(u64::MAX),
            sha256: Some(sha256_hex(bytes.as_slice())),
            media_type: guess_media_type(relative_path),
            warnings,
        },
        u64::try_from(bytes.len()).unwrap_or(u64::MAX),
    ))
}

fn resolve_existing_path(
    workspace: &LegalBenchmarkToolWorkspace,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
) -> Result<PathBuf, String> {
    let path = resolve_new_or_existing_path(workspace, root, relative_path)?;
    if !path.exists() {
        return Err("path does not exist".to_string());
    }
    let canonical_root = root_path(workspace, root)
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize root: {error}"))?;
    let canonical_path = path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize path: {error}"))?;
    if !canonical_path.starts_with(canonical_root.as_path()) {
        return Err("path escapes selected root".to_string());
    }
    Ok(path)
}

fn resolve_new_or_existing_path(
    workspace: &LegalBenchmarkToolWorkspace,
    root: LegalBenchmarkPathRoot,
    relative_path: &str,
) -> Result<PathBuf, String> {
    validate_relative_path(relative_path)?;
    let root_path = root_path(workspace, root);
    let candidate = root_path.join(relative_path);
    let canonical_root = root_path
        .canonicalize()
        .map_err(|error| format!("failed to canonicalize root: {error}"))?;
    let parent = candidate.parent().unwrap_or(root_path);
    let canonical_parent = if parent.exists() {
        parent
            .canonicalize()
            .map_err(|error| format!("failed to canonicalize parent: {error}"))?
    } else {
        let mut ancestor = parent;
        while !ancestor.exists() {
            ancestor = ancestor.parent().ok_or_else(|| {
                "path parent does not have an existing ancestor under root".to_string()
            })?;
        }
        ancestor
            .canonicalize()
            .map_err(|error| format!("failed to canonicalize ancestor: {error}"))?
    };
    if !canonical_parent.starts_with(canonical_root.as_path()) {
        return Err("path escapes selected root".to_string());
    }
    Ok(candidate)
}

fn validate_relative_path(relative_path: &str) -> Result<(), String> {
    if relative_path.trim().is_empty() {
        return Err("relative path cannot be empty".to_string());
    }
    let path = Path::new(relative_path);
    if path.is_absolute() {
        return Err("relative path cannot be absolute".to_string());
    }
    for component in path.components() {
        match component {
            Component::Normal(_) => {}
            Component::CurDir => return Err("relative path cannot contain .".to_string()),
            Component::ParentDir => return Err("relative path cannot contain ..".to_string()),
            Component::RootDir | Component::Prefix(_) => {
                return Err("relative path cannot contain root or prefix".to_string());
            }
        }
    }
    Ok(())
}

fn root_path(workspace: &LegalBenchmarkToolWorkspace, root: LegalBenchmarkPathRoot) -> &Path {
    match root {
        LegalBenchmarkPathRoot::Documents => workspace.documents_root.as_path(),
        LegalBenchmarkPathRoot::Workspace => workspace.workspace_root.as_path(),
        LegalBenchmarkPathRoot::Output => workspace.output_root.as_path(),
    }
}

fn list_files(root: &Path, include_hidden: bool) -> Result<Vec<String>, std::io::Error> {
    let mut files = Vec::new();
    list_files_inner(root, root, include_hidden, &mut files)?;
    files.sort();
    Ok(files)
}

fn list_files_inner(
    root: &Path,
    current: &Path,
    include_hidden: bool,
    files: &mut Vec<String>,
) -> Result<(), std::io::Error> {
    for entry in fs::read_dir(current)? {
        let entry = entry?;
        let path = entry.path();
        let name = entry.file_name();
        let hidden = name.to_string_lossy().starts_with('.');
        if hidden && !include_hidden {
            continue;
        }
        if path.is_dir() {
            list_files_inner(root, path.as_path(), include_hidden, files)?;
        } else if path.is_file() {
            let relative = path
                .strip_prefix(root)
                .map(|path| path.to_string_lossy().to_string())
                .unwrap_or_else(|_| path.to_string_lossy().to_string());
            files.push(relative);
        }
    }
    Ok(())
}

fn wildcard_matches(pattern: &str, value: &str) -> bool {
    wildcard_matches_inner(pattern.as_bytes(), value.as_bytes())
}

fn wildcard_matches_inner(pattern: &[u8], value: &[u8]) -> bool {
    if pattern.is_empty() {
        return value.is_empty();
    }
    match pattern[0] {
        b'*' => {
            wildcard_matches_inner(&pattern[1..], value)
                || (!value.is_empty() && wildcard_matches_inner(pattern, &value[1..]))
        }
        b'?' => !value.is_empty() && wildcard_matches_inner(&pattern[1..], &value[1..]),
        literal => {
            !value.is_empty()
                && literal == value[0]
                && wildcard_matches_inner(&pattern[1..], &value[1..])
        }
    }
}

fn guess_media_type(relative_path: &str) -> String {
    match Path::new(relative_path)
        .extension()
        .and_then(|ext| ext.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase()
        .as_str()
    {
        "txt" | "text" | "md" | "csv" | "tsv" | "json" | "xml" | "html" | "eml" => {
            String::from("text/plain")
        }
        "pdf" => String::from("application/pdf"),
        "docx" => {
            String::from("application/vnd.openxmlformats-officedocument.wordprocessingml.document")
        }
        "xlsx" => String::from("application/vnd.openxmlformats-officedocument.spreadsheetml.sheet"),
        _ => String::from("application/octet-stream"),
    }
}

fn page_count_hint(relative_path: &str, content: Option<&str>, bytes: &[u8]) -> Option<u32> {
    if !relative_path.ends_with(".pdf") {
        return content
            .filter(|text| text.contains('\u{000c}'))
            .map(|text| u32::try_from(text.split('\u{000c}').count()).unwrap_or(u32::MAX));
    }
    if let Some(text) = content {
        return Some(
            u32::try_from(text.split('\u{000c}').count())
                .unwrap_or(u32::MAX)
                .max(1),
        );
    }
    let marker_count = bytes
        .windows(b"/Type /Page".len())
        .filter(|window| *window == b"/Type /Page")
        .count();
    Some(u32::try_from(marker_count.max(1)).unwrap_or(u32::MAX))
}

fn sheet_count_hint(relative_path: &str, content: Option<&str>, bytes: &[u8]) -> Option<u32> {
    if relative_path.ends_with(".csv") || relative_path.ends_with(".tsv") {
        return Some(1);
    }
    if !relative_path.ends_with(".xlsx") {
        return None;
    }
    if let Some(text) = content {
        let count = text.matches("sheet").count().max(1);
        return Some(u32::try_from(count).unwrap_or(u32::MAX));
    }
    let count = bytes
        .windows(b"xl/worksheets/sheet".len())
        .filter(|window| *window == b"xl/worksheets/sheet")
        .count()
        .max(1);
    Some(u32::try_from(count).unwrap_or(u32::MAX))
}

fn message_count_hint(relative_path: &str, content: Option<&str>) -> Option<u32> {
    if !relative_path.ends_with(".eml") {
        return None;
    }
    let text = content?;
    let count = text
        .lines()
        .filter(|line| line.starts_with("From:") || line.starts_with("From "))
        .count()
        .max(1);
    Some(u32::try_from(count).unwrap_or(u32::MAX))
}

fn parse_email_headers(content: &str) -> BTreeMap<String, String> {
    let mut headers = BTreeMap::new();
    for line in content.lines() {
        if line.trim().is_empty() {
            break;
        }
        if let Some((key, value)) = line.split_once(':') {
            headers.insert(key.trim().to_ascii_lowercase(), value.trim().to_string());
        }
    }
    headers
}

fn email_body(content: &str) -> &str {
    content
        .split_once("\r\n\r\n")
        .or_else(|| content.split_once("\n\n"))
        .map(|(_, body)| body)
        .unwrap_or(content)
}

fn truncate_chars(value: &str, max_chars: usize) -> String {
    value.chars().take(max_chars).collect()
}

fn split_delimited_row(line: &str, delimiter: char) -> Vec<String> {
    line.split(delimiter)
        .map(|cell| cell.trim_matches('"').trim().to_string())
        .collect()
}

fn snippet_around(content: &str, byte_index: usize, query_len: usize, radius: usize) -> String {
    let start = content[..byte_index]
        .char_indices()
        .rev()
        .nth(radius)
        .map(|(index, _)| index)
        .unwrap_or(0);
    let end_target = byte_index.saturating_add(query_len);
    let end = content[end_target.min(content.len())..]
        .char_indices()
        .nth(radius)
        .map(|(index, _)| end_target.saturating_add(index).min(content.len()))
        .unwrap_or(content.len());
    content[start..end].replace('\n', " ")
}

fn render_evidence_markdown(
    entries: &[LegalBenchmarkEvidenceTableEntry],
    rows: &[LegalBenchmarkEvidenceTableRow],
) -> String {
    let mut markdown = String::from(
        "| evidence_id | source_ref | locator | quote_hash | note |\n| --- | --- | --- | --- | --- |\n",
    );
    for (entry, row) in entries.iter().zip(rows.iter()) {
        markdown.push_str(&format!(
            "| {} | {} | {} | {} | {} |\n",
            row.evidence_id,
            row.source_ref.replace('|', "\\|"),
            row.locator.clone().unwrap_or_default().replace('|', "\\|"),
            row.quote_hash,
            entry.note.clone().unwrap_or_default().replace('|', "\\|")
        ));
    }
    markdown
}

fn deterministic_tool_call_id(tool_name: LegalBenchmarkToolName, input_hash: &str) -> String {
    let digest = sha256_hex(format!("{}|{input_hash}", tool_name.as_str()).as_bytes());
    format!("tool_call.{}.{}", tool_name.as_str(), digest)
}

fn sha256_hex(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|duration| u64::try_from(duration.as_millis()).unwrap_or(u64::MAX))
        .unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{ArtifactKind, DataClassification, SourceArtifact};

    fn workspace() -> LegalBenchmarkToolWorkspace {
        let root = tempfile::tempdir().expect("tempdir");
        let root_path = root.keep();
        let documents = root_path.join("documents");
        let workspace = root_path.join("workspace");
        let output = root_path.join("output");
        fs::create_dir_all(documents.join("nested")).expect("documents");
        fs::create_dir_all(workspace.join("nested")).expect("workspace");
        fs::create_dir_all(output.as_path()).expect("output");
        fs::write(documents.join("case.txt"), "Alpha clause\nBeta clause\n").expect("case");
        fs::write(
            documents.join("thread.eml"),
            "From: sender@example.com\nTo: lawyer@example.com\nSubject: Notice terms\nDate: Tue, 1 Jan 2026\n\nPlease review the termination notice.\nContent-Disposition: attachment\n",
        )
        .expect("eml");
        fs::write(
            documents.join("terms.csv"),
            "Name,Amount,Formula\nBase,10,=SUM(B2:B2)\n",
        )
        .expect("csv");
        fs::write(
            documents.join("brief.pdf"),
            "First page risk summary.\u{000c}Second page termination notice.",
        )
        .expect("pdf text");
        fs::write(workspace.join("notes.md"), "Alpha draft\nAlpha clause\n").expect("notes");
        fs::write(workspace.join("nested/summary.txt"), "Beta summary\n").expect("summary");
        fs::write(workspace.join(".hidden.txt"), "hidden Alpha\n").expect("hidden");
        fs::write(workspace.join("binary.bin"), [0, 159, 146, 150]).expect("binary");
        fs::write(output.join("memo.md"), "# Memo\n\nDone.\n").expect("memo");

        let extracted = ExtractedArtifact {
            artifact: SourceArtifact {
                artifact_id: "artifact.source.case.extracted".to_string(),
                artifact_kind: ArtifactKind::ExtractedText,
                relative_path: "case.txt".to_string(),
                original_filename: "case.txt".to_string(),
                media_type: "text/plain".to_string(),
                byte_size: 12,
                sha256: "hash".to_string(),
                data_classification: DataClassification::PublicReference,
                provenance: Some("receipt".to_string()),
            },
            text: "Extracted Alpha clause".to_string(),
        };
        LegalBenchmarkToolWorkspace::new(documents, workspace, output)
            .with_extracted_artifacts(&[extracted])
    }

    #[test]
    fn read_prefers_extracted_text_when_available() {
        let workspace = workspace();
        let execution = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Read {
                root: LegalBenchmarkPathRoot::Documents,
                relative_path: "case.txt".to_string(),
                prefer_extracted: true,
            },
        );
        assert!(execution.receipt.failure_kind.is_none());
        assert_eq!(execution.transcript_events.len(), 2);
        match execution.output.expect("read output") {
            LegalBenchmarkToolOutput::Read {
                content, source, ..
            } => {
                assert_eq!(source, "extracted_text");
                assert!(content.contains("Extracted Alpha"));
            }
            _ => panic!("unexpected output"),
        }
    }

    #[test]
    fn write_and_edit_are_limited_to_output_or_workspace_roots() {
        let workspace = workspace();
        let write = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Write {
                root: LegalBenchmarkPathRoot::Output,
                relative_path: "answer.md".to_string(),
                content: "Initial answer".to_string(),
                overwrite: false,
            },
        );
        assert!(write.receipt.failure_kind.is_none());

        let edit = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Edit {
                root: LegalBenchmarkPathRoot::Output,
                relative_path: "answer.md".to_string(),
                find: "Initial".to_string(),
                replace: "Final".to_string(),
                expected_replacements: Some(1),
            },
        );
        assert!(edit.receipt.failure_kind.is_none());
        assert_eq!(edit.receipt.touched_paths.len(), 1);

        let forbidden = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Write {
                root: LegalBenchmarkPathRoot::Documents,
                relative_path: "case.txt".to_string(),
                content: "bad".to_string(),
                overwrite: true,
            },
        );
        assert_eq!(
            forbidden.receipt.failure_kind,
            Some(LegalBenchmarkToolFailureKind::WriteForbidden)
        );
    }

    #[test]
    fn edit_conflict_is_structured() {
        let workspace = workspace();
        let execution = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Edit {
                root: LegalBenchmarkPathRoot::Workspace,
                relative_path: "notes.md".to_string(),
                find: "Alpha".to_string(),
                replace: "Gamma".to_string(),
                expected_replacements: Some(1),
            },
        );
        assert_eq!(
            execution.receipt.failure_kind,
            Some(LegalBenchmarkToolFailureKind::EditConflict)
        );
    }

    #[test]
    fn glob_orders_results_and_respects_hidden_policy() {
        let workspace = workspace();
        let execution = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Glob {
                root: LegalBenchmarkPathRoot::Workspace,
                pattern: "*.txt".to_string(),
                max_results: 10,
                include_hidden: false,
            },
        );
        match execution.output.expect("glob output") {
            LegalBenchmarkToolOutput::Glob { matches, truncated } => {
                assert!(!truncated);
                assert_eq!(matches, vec!["nested/summary.txt"]);
            }
            _ => panic!("unexpected output"),
        }
    }

    #[test]
    fn grep_skips_binary_files_and_records_matches() {
        let workspace = workspace();
        let execution = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Grep {
                root: LegalBenchmarkPathRoot::Workspace,
                pattern: "alpha".to_string(),
                case_sensitive: false,
                max_results: 10,
                include_hidden: false,
            },
        );
        match execution.output.expect("grep output") {
            LegalBenchmarkToolOutput::Grep {
                matches,
                binary_files_skipped,
                ..
            } => {
                assert_eq!(matches.len(), 2);
                assert_eq!(binary_files_skipped, 1);
            }
            _ => panic!("unexpected output"),
        }
    }

    #[test]
    fn inventory_records_hashes_and_document_hints() {
        let workspace = workspace();
        let execution = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Inventory {
                root: LegalBenchmarkPathRoot::Documents,
                max_results: 20,
                include_hidden: false,
                include_hashes: true,
            },
        );
        match execution.output.expect("inventory output") {
            LegalBenchmarkToolOutput::Inventory {
                artifacts,
                truncated,
            } => {
                assert!(!truncated);
                assert!(artifacts.iter().any(|artifact| {
                    artifact.relative_path == "thread.eml" && artifact.message_count == Some(1)
                }));
                assert!(artifacts.iter().any(|artifact| {
                    artifact.relative_path == "terms.csv"
                        && artifact.sheet_count == Some(1)
                        && artifact.sha256.is_some()
                }));
                assert!(artifacts.iter().any(|artifact| {
                    artifact.relative_path == "brief.pdf" && artifact.page_count == Some(2)
                }));
            }
            _ => panic!("unexpected output"),
        }
    }

    #[test]
    fn document_helper_tools_extract_high_score_evidence() {
        let workspace = workspace();
        let email = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::EmailSummary {
                root: LegalBenchmarkPathRoot::Documents,
                relative_path: "thread.eml".to_string(),
                max_body_chars: 80,
            },
        );
        match email.output.expect("email output") {
            LegalBenchmarkToolOutput::EmailSummary { summary } => {
                assert_eq!(summary.subject.as_deref(), Some("Notice terms"));
                assert_eq!(summary.attachment_count, 1);
            }
            _ => panic!("unexpected output"),
        }

        let spreadsheet = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::SpreadsheetSummary {
                root: LegalBenchmarkPathRoot::Documents,
                relative_path: "terms.csv".to_string(),
                max_preview_rows: 2,
            },
        );
        match spreadsheet.output.expect("spreadsheet output") {
            LegalBenchmarkToolOutput::SpreadsheetSummary { summary } => {
                assert_eq!(summary.row_count, 2);
                assert_eq!(summary.column_count, 3);
                assert_eq!(summary.formula_count, 1);
            }
            _ => panic!("unexpected output"),
        }

        let pdf = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::PdfSearch {
                root: LegalBenchmarkPathRoot::Documents,
                relative_path: "brief.pdf".to_string(),
                query: "termination".to_string(),
                page: Some(2),
                max_matches: 5,
            },
        );
        let snippet = match pdf.output.expect("pdf output") {
            LegalBenchmarkToolOutput::PdfSearch { matches, .. } => {
                assert_eq!(matches.len(), 1);
                assert_eq!(matches[0].page, 2);
                matches[0].snippet.clone()
            }
            _ => panic!("unexpected output"),
        };

        let evidence = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::EvidenceTable {
                entries: vec![LegalBenchmarkEvidenceTableEntry {
                    source_ref: "brief.pdf".to_string(),
                    locator: Some("page:2".to_string()),
                    quote: snippet,
                    note: Some("termination notice".to_string()),
                }],
            },
        );
        match evidence.output.expect("evidence output") {
            LegalBenchmarkToolOutput::EvidenceTable { rows, markdown } => {
                assert_eq!(rows.len(), 1);
                assert!(markdown.contains("termination notice"));
            }
            _ => panic!("unexpected output"),
        }

        let validation = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::ValidateDeliverables {
                root: LegalBenchmarkPathRoot::Output,
                required_paths: vec!["memo.md".to_string(), "missing.docx".to_string()],
                max_results: 10,
            },
        );
        match validation.output.expect("validation output") {
            LegalBenchmarkToolOutput::ValidateDeliverables {
                validations,
                all_present_and_readable,
                missing_count,
                ..
            } => {
                assert!(!all_present_and_readable);
                assert_eq!(missing_count, 1);
                assert_eq!(validations[0].relative_path, "memo.md");
                assert!(validations[0].sha256.is_some());
            }
            _ => panic!("unexpected output"),
        }
    }

    #[test]
    fn shell_without_sandbox_is_structured_failure() {
        let workspace = workspace();
        let execution = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Shell {
                command: vec!["echo".to_string(), "hello".to_string()],
                timeout_ms: Some(1000),
            },
        );
        assert_eq!(
            execution.receipt.failure_kind,
            Some(LegalBenchmarkToolFailureKind::SandboxUnavailable)
        );
        assert_eq!(
            execution.tool_call_record.error_kind.as_deref(),
            Some("SandboxUnavailable")
        );
    }

    #[test]
    fn traversal_paths_are_rejected() {
        let workspace = workspace();
        let execution = execute_legal_benchmark_tool(
            &workspace,
            LegalBenchmarkToolInput::Read {
                root: LegalBenchmarkPathRoot::Documents,
                relative_path: "../case.txt".to_string(),
                prefer_extracted: false,
            },
        );
        assert_eq!(
            execution.receipt.failure_kind,
            Some(LegalBenchmarkToolFailureKind::InvalidPath)
        );
    }
}
