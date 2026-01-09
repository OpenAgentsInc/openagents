# Job Types

This document provides detailed documentation for each job type in the protocol.

## Overview

Job types follow the naming convention:
```
oa.<category>_<name>.v<major>
```

Each job type has:
- A **request schema** defining inputs
- A **response schema** defining outputs
- A **default verification mode** (objective or subjective)
- A **schema version** following semver

## oa.code_chunk_analysis.v1

Analyzes a code chunk for summaries, symbols, suspected faults, and recommendations for next probes. Used during agent exploration of codebases.

### Request Schema

```rust
pub struct ChunkAnalysisRequest {
    /// The analysis task/question (e.g., "Find security issues")
    pub task: String,

    /// Optional: the user's original task for context
    pub user_task: Option<String>,

    /// The code chunk to analyze
    pub chunk: CodeChunk,

    /// Output constraints
    pub output_constraints: OutputConstraints,

    /// Verification settings (default: subjective + judge, redundancy=2)
    pub verification: Verification,
}

pub struct CodeChunk {
    /// File path relative to repository root
    pub path: String,

    /// Start line (1-indexed)
    pub start_line: u32,

    /// End line (1-indexed, inclusive)
    pub end_line: u32,

    /// The actual code content
    pub content: String,

    /// Optional: language identifier (e.g., "rust", "python")
    pub language: Option<String>,
}

pub struct OutputConstraints {
    pub max_summary_length: Option<u32>,
    pub max_symbols: Option<u32>,
    pub max_faults: Option<u32>,
    pub max_next_probes: Option<u32>,
}
```

### Response Schema

```rust
pub struct ChunkAnalysisResponse {
    /// Summary of what the code does
    pub summary: String,

    /// Symbols extracted (functions, structs, traits, etc.)
    pub symbols: Vec<Symbol>,

    /// Potential issues or bugs
    pub suspected_faults: Vec<SuspectedFault>,

    /// Recommended next files/locations to examine
    pub recommended_next_probes: Vec<NextProbe>,

    /// Confidence score (0.0 to 1.0)
    pub confidence: f32,

    /// Execution provenance
    pub provenance: Provenance,
}

pub struct Symbol {
    pub name: String,
    pub kind: String,  // "function", "struct", "trait", etc.
    pub line: u32,
    pub description: Option<String>,
}

pub struct SuspectedFault {
    pub line: u32,
    pub severity: String,  // "low", "medium", "high"
    pub description: String,
    pub suggestion: Option<String>,
}

pub struct NextProbe {
    pub path: String,
    pub reason: String,
    pub priority: String,  // "low", "medium", "high"
}
```

### Example Usage

```rust
use protocol::jobs::{ChunkAnalysisRequest, chunk_analysis::CodeChunk};

let request = ChunkAnalysisRequest {
    task: "Find authentication vulnerabilities".into(),
    user_task: Some("Security audit".into()),
    chunk: CodeChunk::new(
        "src/auth/login.rs",
        1,
        50,
        r#"
pub fn login(username: &str, password: &str) -> Result<Session, AuthError> {
    let user = db.find_user(username)?;
    if verify_password(password, &user.password_hash) {
        Ok(Session::create(user.id))
    } else {
        Err(AuthError::InvalidCredentials)
    }
}
"#
    ).with_language("rust"),
    ..Default::default()
};

let hash = request.compute_hash().unwrap();
```

### Verification

- **Mode**: Subjective (results require judgment)
- **Default Redundancy**: 2 providers
- **Adjudication**: Judge model evaluates responses

---

## oa.retrieval_rerank.v1

Reranks retrieval candidates by relevance to a user's task. Used after initial vector/keyword search to improve result quality.

### Request Schema

```rust
pub struct RerankRequest {
    /// The user's task/query for ranking relevance
    pub user_task: String,

    /// Candidates to rerank
    pub candidates: Vec<RerankCandidate>,

    /// Number of top candidates to return (default: 10)
    pub k: usize,

    /// Optional: rubric for ranking decisions
    pub ranking_rubric: Option<String>,

    /// Verification settings (default: subjective + majority, redundancy=2)
    pub verification: Verification,
}

pub struct RerankCandidate {
    /// Unique identifier
    pub id: String,

    /// Content to evaluate
    pub content: String,

    /// Optional: file path for code candidates
    pub path: Option<String>,

    /// Optional: original retrieval score
    pub original_score: Option<f32>,

    /// Optional: additional metadata
    pub metadata: HashMap<String, String>,
}
```

### Response Schema

```rust
pub struct RerankResponse {
    /// Top-k ranked candidates
    pub topk: Vec<RankedCandidate>,

    /// Execution provenance
    pub provenance: Provenance,
}

pub struct RankedCandidate {
    /// Rank position (1-indexed)
    pub rank: usize,

    /// Candidate ID (matches input)
    pub id: String,

    /// Relevance score (0.0 to 1.0)
    pub score: f32,

    /// Optional: explanation for ranking
    pub why: Option<String>,
}
```

### Example Usage

```rust
use protocol::jobs::{RerankRequest, rerank::RerankCandidate};

let request = RerankRequest {
    user_task: "How does the authentication system work?".into(),
    candidates: vec![
        RerankCandidate::new("1", "fn login() { ... }")
            .with_path("src/auth.rs")
            .with_score(0.8),
        RerankCandidate::new("2", "fn create_session() { ... }")
            .with_path("src/session.rs")
            .with_score(0.75),
        RerankCandidate::new("3", "fn parse_config() { ... }")
            .with_path("src/config.rs")
            .with_score(0.72),
    ],
    k: 5,
    ranking_rubric: Some("Prefer code directly related to auth flow".into()),
    ..Default::default()
};
```

### Verification

- **Mode**: Subjective (ranking is judgment-based)
- **Default Redundancy**: 2 providers
- **Adjudication**: Majority vote on rankings

---

## oa.sandbox_run.v1

Runs commands in a sandboxed environment with configurable security policies. Used for executing tests, builds, and other deterministic operations.

### Request Schema

```rust
pub struct SandboxRunRequest {
    /// Sandbox configuration
    pub sandbox: SandboxConfig,

    /// Repository to mount
    pub repo: RepoMount,

    /// Commands to execute
    pub commands: Vec<SandboxCommand>,

    /// Environment variables
    pub env: HashMap<String, String>,

    /// Verification settings (default: objective, redundancy=1)
    pub verification: Verification,
}

pub struct SandboxConfig {
    /// Provider (e.g., "docker", "firecracker")
    pub provider: String,

    /// Docker image digest (sha256:...) for reproducibility
    pub image_digest: String,

    /// Network policy
    pub network_policy: NetworkPolicy,

    /// Resource limits
    pub resources: ResourceLimits,
}

pub enum NetworkPolicy {
    None,       // No network access
    Localhost,  // Localhost only
    Full,       // Full network access
}

pub struct ResourceLimits {
    pub memory_mb: u32,      // Default: 512
    pub cpus: f32,           // Default: 1.0
    pub timeout_secs: u32,   // Default: 60
    pub disk_mb: Option<u32>,
}

pub struct RepoMount {
    /// Repository URL or local path
    pub source: String,

    /// Git ref (branch, tag, commit)
    pub git_ref: Option<String>,

    /// Mount path in sandbox (default: "/workspace")
    pub mount_path: String,
}

pub struct SandboxCommand {
    /// Command to execute
    pub cmd: String,

    /// Working directory
    pub workdir: Option<String>,

    /// Continue on failure
    pub continue_on_fail: bool,
}
```

### Response Schema

```rust
pub struct SandboxRunResponse {
    /// Environment information
    pub env_info: EnvInfo,

    /// Results from each command
    pub runs: Vec<CommandResult>,

    /// Artifacts produced
    pub artifacts: Vec<Artifact>,

    /// Overall status
    pub status: SandboxStatus,

    /// Error message if status is Error
    pub error: Option<String>,

    /// Execution provenance
    pub provenance: Provenance,
}

pub struct EnvInfo {
    pub image_digest: String,
    pub hostname: Option<String>,
    pub system_info: Option<String>,
}

pub struct CommandResult {
    pub cmd: String,
    pub exit_code: i32,
    pub duration_ms: u64,
    pub stdout_sha256: String,
    pub stderr_sha256: String,
    pub stdout_preview: Option<String>,
    pub stderr_preview: Option<String>,
}

pub struct Artifact {
    pub path: String,
    pub sha256: String,
    pub size_bytes: u64,
    pub mime_type: Option<String>,
}

pub enum SandboxStatus {
    Success,    // All commands succeeded
    Failed,     // One or more commands failed
    Timeout,    // Execution timed out
    Cancelled,  // Execution was cancelled
    Error,      // Internal error
}
```

### Example Usage

```rust
use protocol::jobs::{
    SandboxRunRequest,
    sandbox::{SandboxConfig, SandboxCommand, NetworkPolicy, ResourceLimits},
};

let request = SandboxRunRequest {
    sandbox: SandboxConfig {
        provider: "docker".into(),
        image_digest: "sha256:abc123def456...".into(),
        network_policy: NetworkPolicy::None,
        resources: ResourceLimits {
            memory_mb: 1024,
            cpus: 2.0,
            timeout_secs: 300,
            ..Default::default()
        },
    },
    commands: vec![
        SandboxCommand::new("cargo build --release"),
        SandboxCommand::new("cargo test").continue_on_fail(),
    ],
    ..Default::default()
};
```

### Verification

- **Mode**: Objective (deterministic results)
- **Default Redundancy**: 1 provider
- **Adjudication**: None (results are reproducible)

### Security Considerations

1. **Image Digest**: Always use full SHA-256 digests, not tags
2. **Network Policy**: Default to `None` for untrusted code
3. **Resource Limits**: Set appropriate limits to prevent abuse
4. **Output Hashing**: All outputs are hashed for verification

---

## Adding New Job Types

To add a new job type:

1. Create a new module in `src/jobs/` (e.g., `my_job.rs`)
2. Define request and response structs
3. Implement `JobRequest` trait for the request
4. Implement `JobResponse` trait for the response
5. Add to `registered_job_types()` in `jobs/mod.rs`
6. Export from `jobs/mod.rs` and `lib.rs`

```rust
use crate::verification::Verification;
use crate::version::SchemaVersion;

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MyJobRequest {
    pub input: String,
    pub verification: Verification,
}

impl JobRequest for MyJobRequest {
    const JOB_TYPE: &'static str = "oa.my_job.v1";
    const SCHEMA_VERSION: SchemaVersion = SchemaVersion::new(1, 0, 0);

    fn verification(&self) -> &Verification {
        &self.verification
    }
}
```
