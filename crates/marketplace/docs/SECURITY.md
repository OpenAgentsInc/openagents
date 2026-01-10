# Skill Execution Security

Marketplace skill execution is sandboxed and now uses DSPy classifiers to
evaluate risk, permission scope, and resource limits before running scripts.

## Security Pipeline

1. **Path safety** (SafePathValidationSignature)
2. **Filesystem permissions** (FilesystemPermissionSignature)
3. **Resource limits** (ResourceLimitSignature)
4. **Risk classification** (SkillSecurityClassifier)
5. **Approval gating** for High/Critical risk

If DSPy is unavailable, the system falls back to conservative heuristics
(path traversal checks, scripts/ prefix enforcement, and default limits).

## Approval Policy

High and Critical risk executions require a human approval token. The execution
request must include an `ApprovalContext` with `approved: true`, otherwise it
fails with `ApprovalRequired`.

## Usage Example

```rust
use marketplace::skills::execution::{ApprovalContext, SandboxConfig, ScriptExecution};

let exec = ScriptExecution::new(
    "pdf-parser",
    "scripts/process.py",
    SandboxConfig::default(),
    10_000,
)
.with_manifest(serde_json::json!({
    "name": "pdf-parser",
    "version": "0.1.0"
}))
.with_approval(ApprovalContext::approved(Some("alice@example.com".to_string())));

exec.validate()?;
```

## Logged Decisions

The executor logs DSPy security decisions (risk level, concerns, recommended
sandbox, permission reviews) for audit and training data collection.
