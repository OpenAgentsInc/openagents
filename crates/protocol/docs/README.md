# Protocol Crate Documentation

The `protocol` crate provides the foundation for the OpenAgents swarm protocol. It defines typed job schemas with deterministic hashing, verification modes, and provenance tracking.

## Overview

Every job in the OpenAgents swarm follows a common structure:

```
┌─────────────────────────────────────────────────────────────┐
│                       JobEnvelope                           │
├─────────────────────────────────────────────────────────────┤
│  job_type: "oa.code_chunk_analysis.v1"                     │
│  schema_version: "1.0.0"                                    │
│  job_hash: "a1b2c3..." (SHA-256 of canonical JSON)         │
│  payload: { ... typed request/response ... }               │
└─────────────────────────────────────────────────────────────┘
```

## Quick Start

### Add Dependency

```toml
[dependencies]
protocol = { path = "../protocol" }
```

### Create a Job Request

```rust
use protocol::jobs::{ChunkAnalysisRequest, chunk_analysis::CodeChunk, JobEnvelope};

// Create a typed request
let request = ChunkAnalysisRequest {
    task: "Understand the authentication flow".into(),
    user_task: Some("Fix login bug".into()),
    chunk: CodeChunk::new("src/auth.rs", 10, 50, "fn login() { ... }"),
    ..Default::default()
};

// Wrap in envelope for transport
let envelope = JobEnvelope::from_request(request);

// Access metadata
println!("Job type: {}", envelope.job_type);
println!("Hash: {:?}", envelope.job_hash);
```

### Compute Deterministic Hash

```rust
use protocol::jobs::JobRequest;

let hash = request.compute_hash().unwrap();
// Same input always produces same hash
assert_eq!(hash.len(), 64); // SHA-256 hex string
```

## Documentation Index

| Document | Description |
|----------|-------------|
| [Job Types](./job-types.md) | Detailed documentation for each job type |
| [Hashing](./hashing.md) | Canonical JSON serialization and SHA-256 hashing |
| [Verification](./verification.md) | Objective vs subjective modes, adjudication strategies |
| [Provenance](./provenance.md) | Model tracking, sampling parameters, audit trails |
| [Versioning](./versioning.md) | Schema versioning rules and compatibility |

## Architecture

```
protocol/
├── hash.rs           # Canonical JSON + SHA-256
├── version.rs        # Semver versioning
├── verification.rs   # Objective/subjective modes
├── provenance.rs     # Audit trail
└── jobs/
    ├── mod.rs              # JobRequest/JobResponse traits
    ├── chunk_analysis.rs   # oa.code_chunk_analysis.v1
    ├── rerank.rs           # oa.retrieval_rerank.v1
    └── sandbox.rs          # oa.sandbox_run.v1
```

## Job Types Overview

| Job Type | Description | Verification | Default Redundancy |
|----------|-------------|--------------|-------------------|
| `oa.code_chunk_analysis.v1` | Analyze code for summaries, symbols, faults | Subjective + Judge | 2 |
| `oa.retrieval_rerank.v1` | Rerank retrieval candidates by relevance | Subjective + Majority | 2 |
| `oa.sandbox_run.v1` | Run commands in sandboxed environment | Objective | 1 |

## Key Concepts

### Deterministic Hashing

All job requests can be hashed deterministically using canonical JSON (RFC 8785 JCS principles):
- Object keys sorted lexicographically
- No whitespace between tokens
- Numbers in shortest form

This ensures identical inputs produce identical hashes across all implementations (Rust, TypeScript, Python, etc.).

### Verification Modes

Jobs are classified as either:
- **Objective**: Results can be verified deterministically (tests, build status)
- **Subjective**: Results require judgment (code summaries, analysis)

Subjective jobs can use multiple providers (redundancy) and adjudication strategies to ensure quality.

### Provenance Tracking

Every job response includes provenance information:
- Model ID used for inference
- Sampling parameters (temperature, seed, etc.)
- Input/output hashes for verification
- Provider identity and timestamp

## Examples

### Full Request/Response Cycle

```rust
use protocol::jobs::{
    ChunkAnalysisRequest, ChunkAnalysisResponse,
    chunk_analysis::{CodeChunk, Symbol, NextProbe},
    JobRequest, JobResponse, JobEnvelope,
};
use protocol::provenance::Provenance;

// 1. Create request
let request = ChunkAnalysisRequest {
    task: "Find security vulnerabilities".into(),
    chunk: CodeChunk::new("src/auth.rs", 1, 100, "...code..."),
    ..Default::default()
};

// 2. Wrap in envelope
let envelope = JobEnvelope::from_request(request.clone());
let job_hash = envelope.job_hash.clone();

// 3. (Provider executes job and returns response)
let response = ChunkAnalysisResponse {
    summary: "Authentication module with password hashing".into(),
    symbols: vec![
        Symbol {
            name: "hash_password".into(),
            kind: "function".into(),
            line: 15,
            description: Some("Hashes passwords using bcrypt".into()),
        },
    ],
    suspected_faults: vec![],
    recommended_next_probes: vec![
        NextProbe {
            path: "src/session.rs".into(),
            reason: "Related session management".into(),
            priority: "medium".into(),
        },
    ],
    confidence: 0.9,
    provenance: Provenance::new("claude-3-sonnet")
        .with_input_hash(job_hash.unwrap())
        .with_tokens(500, 150),
};

// 4. Wrap response in envelope
let response_envelope = JobEnvelope::from_response(response);
```

### Version Compatibility Check

```rust
use protocol::version::SchemaVersion;

let client_version = SchemaVersion::new(1, 0, 0);
let server_version = SchemaVersion::new(1, 2, 0);

if client_version.is_compatible_with(&server_version) {
    // Safe to communicate
    if server_version.can_read(&client_version) {
        // Server can read client's older format
    }
}
```

## Integration with Pylon

The protocol crate is designed to integrate with Pylon for swarm job execution:

```rust
// In Pylon provider
use protocol::jobs::{JobEnvelope, ChunkAnalysisRequest, ChunkAnalysisResponse};

// Receive job envelope from Nostr
let envelope: JobEnvelope<ChunkAnalysisRequest> = serde_json::from_str(&event.content)?;

// Verify job hash
let expected_hash = envelope.payload.compute_hash()?;
assert_eq!(envelope.job_hash.as_ref(), Some(&expected_hash));

// Execute job...
let response = execute_chunk_analysis(&envelope.payload).await?;

// Send response
let response_envelope = JobEnvelope::from_response(response);
```

## Testing

Run all tests:

```bash
cargo test -p protocol
```

The crate includes:
- 48 unit tests for all modules
- 10 doc tests with runnable examples
- Serde round-trip tests for all types
- Hash determinism verification tests
