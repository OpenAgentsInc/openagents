# Privacy Module (Wave 7)

> **Status:** Needs audit
> **Last verified:** d44f9cd3f
> **Source of truth:** `crates/dsrs/src/privacy/`
> **Doc owner:** dsrs
> **If this doc conflicts with code, code wins.**

The privacy module provides content protection for dispatching jobs to the swarm. It enables redaction, chunking, and policy enforcement to protect sensitive code before sending to untrusted providers.

## Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                     PRIVACY PIPELINE                              │
├─────────────────────────────────────────────────────────────────┤
│                                                                   │
│   Content → Redaction → Chunking → Policy Check → Dispatch       │
│                                                                   │
│   ┌──────────┐   ┌──────────┐   ┌──────────┐   ┌──────────┐     │
│   │ PathRe-  │   │ Chunking │   │ Privacy  │   │ Swarm    │     │
│   │ dactor   │──▶│ Policy   │──▶│ Policy   │──▶│Dispatcher│     │
│   │ Ident-   │   │          │   │          │   │          │     │
│   │ Redactor │   │          │   │          │   │          │     │
│   └──────────┘   └──────────┘   └──────────┘   └──────────┘     │
│                                                                   │
└─────────────────────────────────────────────────────────────────┘
```

## Components

### 1. Redaction (`redaction.rs`)

Removes or anonymizes sensitive content before dispatch.

```rust
use dsrs::privacy::{RedactionMode, RedactionConfig, PathRedactor, IdentifierRedactor};

// Configure redaction
let config = RedactionConfig {
    mode: RedactionMode::PathsOnly,
    preserve_structure: true,
    preserve_types: true,
    custom_patterns: vec![],
    preserve_patterns: vec![],
};

// Redact file paths
let redactor = PathRedactor::new();
let redacted = redactor.redact(content, &config);
// /Users/alice/project/src/main.rs → /workspace/src/main.rs

// Redact identifiers
let id_redactor = IdentifierRedactor::new();
let redacted = id_redactor.redact(content, &config);
// MySecretClass → IDENT_001
```

#### Redaction Modes

| Mode | Description | Use Case |
|------|-------------|----------|
| `None` | No redaction | Public/open-source repos |
| `PathsOnly` | Redact file paths only | Moderate privacy |
| `Identifiers` | Redact paths + identifiers | High privacy |
| `Full` | Full content anonymization | Maximum privacy |

#### Redaction Output

```rust
pub struct RedactedContent {
    /// The redacted content
    pub content: String,
    /// Mapping from original → redacted (for restoration)
    pub mapping: HashMap<String, String>,
    /// Checksum for verification
    pub checksum: String,
}
```

### 2. Chunking (`chunking.rs`)

Controls how much content is sent to providers.

```rust
use dsrs::privacy::{ChunkingPolicy, Chunker, ContentChunk};

// Full content (default)
let policy = ChunkingPolicy::Full;

// Minimal spans with context
let policy = ChunkingPolicy::MinimalSpans { context_lines: 5 };

// Fixed-size chunks
let policy = ChunkingPolicy::fixed_size(4000);

// Chunk content
let chunker = Chunker::new(policy);
let chunks = chunker.chunk(content, Some("src/main.rs"));

for chunk in chunks {
    println!("Lines {}-{}: {} bytes",
        chunk.line_range.start,
        chunk.line_range.end,
        chunk.content.len()
    );
}
```

#### Chunking Policies

| Policy | Description |
|--------|-------------|
| `Full` | Send entire content (default) |
| `MinimalSpans { context_lines }` | Only changed lines + N lines context |
| `AstNodesOnly { node_types }` | Only specific AST nodes (functions, classes) |
| `FixedSize { max_chars, overlap }` | Fixed-size chunks with overlap |

#### Extracting Spans

```rust
// Extract a specific span with context
let chunk = chunker.extract_span(content, 10..20, Some("file.rs"));
// Returns lines 7-22 (with 3 lines context by default)
```

### 3. Privacy Policy (`policy.rs`)

Controls which jobs are allowed and with what settings.

```rust
use dsrs::privacy::PrivacyPolicy;

// Open source policy (minimal restrictions)
let policy = PrivacyPolicy::open_source();
assert!(policy.is_job_allowed("any.job.type"));

// Private repo policy (strict)
let policy = PrivacyPolicy::private_repo();
assert!(policy.is_job_allowed("oa.sandbox_run.v1")); // Only sandbox allowed
assert!(!policy.is_job_allowed("oa.code_analysis.v1")); // Analysis blocked

// Paranoid policy (maximum privacy)
let policy = PrivacyPolicy::paranoid();
assert_eq!(policy.max_content_size, Some(10_000)); // 10KB max
```

#### Policy Configuration

```rust
pub struct PrivacyPolicy {
    /// Redaction settings
    pub redaction: RedactionConfig,
    /// Chunking policy
    pub chunking: ChunkingPolicy,
    /// Allowed job types (empty = all allowed)
    pub allowed_job_types: HashSet<String>,
    /// Trusted provider pubkeys
    pub trusted_providers: Vec<String>,
    /// Require verification for all jobs
    pub require_verification: bool,
    /// Maximum content size (bytes)
    pub max_content_size: Option<usize>,
    /// Allow sending file paths
    pub allow_file_paths: bool,
}
```

#### Builder Pattern

```rust
let policy = PrivacyPolicy::new()
    .with_redaction_mode(RedactionMode::Identifiers)
    .with_chunking(ChunkingPolicy::minimal())
    .with_max_content_size(50_000)
    .allow_job_type("oa.sandbox_run.v1")
    .allow_job_type("oa.test_run.v1")
    .trust_provider("npub1abc...")
    .require_verification();
```

#### Policy Violations

```rust
pub enum PolicyViolation {
    JobTypeNotAllowed(String),
    UntrustedProvider(String),
    ContentTooLarge { size: usize, max: usize },
    FilePathsNotAllowed,
    VerificationRequired,
}

// Validate content
match policy.validate_content(content) {
    Ok(()) => println!("Content allowed"),
    Err(PolicyViolation::ContentTooLarge { size, max }) => {
        println!("Content {} exceeds max {}", size, max);
    }
    Err(e) => println!("Violation: {}", e),
}
```

## Integration with SwarmDispatcher

The privacy module integrates with `SwarmDispatcher`:

```rust
use dsrs::adapter::SwarmDispatcher;
use dsrs::privacy::PrivacyPolicy;

let dispatcher = SwarmDispatcher::new(config)
    .with_privacy_policy(PrivacyPolicy::private_repo());

// Dispatch will:
// 1. Check job type against allowlist
// 2. Redact content according to policy
// 3. Chunk content if needed
// 4. Validate content size
// 5. Add trusted provider tags
// 6. Dispatch to swarm
let result = dispatcher.dispatch_job(request).await?;
```

## Preset Policies

### Open Source (`open_source()`)

For public repositories with no privacy concerns:

- No redaction
- Full content
- All job types allowed
- No provider restrictions
- No verification required

### Private Repo (`private_repo()`)

For private repositories with moderate privacy:

- Path-only redaction
- Minimal spans (5 lines context)
- Only sandbox jobs allowed
- Verification required
- 50KB max content
- No file paths in content

### Paranoid (`paranoid()`)

For maximum privacy:

- Full redaction
- Minimal spans (2 lines context)
- Only sandbox jobs
- Verification required
- 10KB max content
- No file paths

## File Paths

The privacy module detects and optionally blocks file paths:

```rust
// Detected paths:
// - /Users/*, /home/*, /var/*, /tmp/*
// - C:\Users\*, D:\*

if !policy.allow_file_paths {
    // Paths like "/Users/alice/secret/data.txt" will trigger violation
}
```

## Roundtrip Restoration

Redacted content can be restored using the mapping:

```rust
let redactor = PathRedactor::new();
let redacted = redactor.redact(content, &config);

// Later, restore original paths
let restored = redactor.restore(&redacted);
assert_eq!(restored, original);
```

## Testing

```bash
# Run privacy tests
cargo test -p dsrs privacy

# Test redaction
cargo test -p dsrs test_path_redaction
cargo test -p dsrs test_identifier_redaction

# Test policies
cargo test -p dsrs test_private_repo_policy
cargo test -p dsrs test_paranoid_policy
```

## See Also

- [SIGNATURES.md](./SIGNATURES.md) - Optimizable signatures
- [EVALUATION.md](./EVALUATION.md) - Eval harness
- [ARCHITECTURE.md](./ARCHITECTURE.md) - Core architecture
