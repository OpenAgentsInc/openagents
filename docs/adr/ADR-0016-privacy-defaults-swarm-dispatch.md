# ADR-0016: Privacy Defaults for Swarm Dispatch

## Status

**Accepted**

## Date

2026-01-13

## Context

When dispatching jobs to the swarm (untrusted NIP-90 providers), content protection is critical. Without defaults:
- developers forget to configure privacy,
- sensitive code leaks to untrusted providers,
- redaction/chunking behavior varies unpredictably,
- no clear escalation path for different repo sensitivity levels.

We need canonical defaults that are safe-by-default while allowing explicit opt-out for open-source workflows.

## Decision

**Swarm dispatch MUST apply a privacy policy. The recommended default for swarm dispatch is `private_repo`. Callers MAY override to `open_source` or `paranoid` explicitly.**

### Implementation Status

| Component | Status |
|-----------|--------|
| PrivacyPolicy struct | Implemented (`crates/dsrs/src/privacy/policy.rs`) |
| Preset constructors | Implemented (`open_source()`, `private_repo()`, `paranoid()`) |
| SwarmDispatcher integration | Implemented (`crates/dsrs/src/adapter/swarm_dispatch.rs`) |
| Default enforcement | Partial (struct Default ≠ swarm default) |

**Note:** The `PrivacyPolicy::default()` trait implementation returns permissive settings (no redaction, all jobs allowed). This ADR specifies that **swarm dispatch code** should apply `private_repo` when no policy is explicitly provided — this is a dispatch-layer default, not a struct Default.

### Canonical owner

- Preset definitions: [crates/dsrs/docs/PRIVACY.md](../../crates/dsrs/docs/PRIVACY.md) (canonical)
- Implementation: `crates/dsrs/src/privacy/policy.rs`
- Integration: `crates/dsrs/src/adapter/swarm_dispatch.rs`

### Privacy policy presets

Preset parameters are defined in [PRIVACY.md](../../crates/dsrs/docs/PRIVACY.md). This ADR specifies only the invariants:

| Preset | Purpose | Invariant |
|--------|---------|-----------|
| `open_source` | Public repos | No redaction, no restrictions |
| `private_repo` | Private repos | Path redaction, size limits, verification required |
| `paranoid` | Sensitive code | Full redaction, strict limits |

For exact parameter values (max sizes, context lines, allowed job types), see PRIVACY.md.

### Default behavior (Normative)

1. **ADR-0016.R1** — Swarm dispatch MUST apply `private_repo` if no policy is explicitly provided.
2. **ADR-0016.R2** — `SwarmDispatcher` MUST validate content against the active policy before dispatch.
3. **ADR-0016.R3** — Policy violations MUST either:
   - (a) Reject the job with an error, OR
   - (b) Auto-redact and proceed **only if** the preset explicitly permits automatic redaction
4. **ADR-0016.R4** — Callers MUST explicitly opt into `open_source` to disable redaction.

### Redaction modes

Defined in `crates/dsrs/src/privacy/redaction.rs`:

| Mode | Description |
|------|-------------|
| `None` | No redaction |
| `PathsOnly` | Redact file paths only |
| `Identifiers` | Redact paths + class/function names |
| `Full` | Full content anonymization |

### Redaction mapping

For modes that transform content (`PathsOnly`, `Identifiers`, `Full`):
- A `RedactedContent` struct holds the mapping from original → redacted
- Restoration is possible **only if** the mapping is preserved
- Callers requiring roundtrip restoration MUST retain the mapping artifact

This ADR does NOT guarantee all redaction is reversible — only that modes preserving the mapping support restoration.

### Trusted providers

Trust tiers for providers are defined in:
- Policy bundle configuration (per ADR-0015)
- [PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) provider reputation

This ADR only specifies that **privacy policy MAY vary by trust tier** — the trust model itself is defined elsewhere.

### Policy violations (Normative)

**ADR-0016.R5** — The `PolicyViolation` enum variants are stable. Per `crates/dsrs/src/privacy/policy.rs`:

```rust
pub enum PolicyViolation {
    JobTypeNotAllowed(String),
    UntrustedProvider(String),
    ContentTooLarge { size: usize, max: usize },
    FilePathsNotAllowed,
    VerificationRequired,
}
```

## Scope

What this ADR covers:
- Default privacy policy for swarm dispatch
- Invariants for preset behavior
- Violation handling behavior

What this ADR does NOT cover:
- Exact preset parameter values (see PRIVACY.md)
- Redaction algorithm implementation
- Provider trust model (see [PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md))

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Swarm dispatch default | `private_repo` when unspecified |
| Preset names | Stable: `open_source`, `private_repo`, `paranoid` |
| Violation handling | Never silently lose data without explicit permission |
| PolicyViolation enum | Stable variants |

Backward compatibility:
- New presets may be added.
- Existing preset parameters may be tightened (more restrictive) without a new ADR.
- Loosening parameters requires a superseding ADR.

## Consequences

**Positive:**
- Safe-by-default for private repositories
- Clear escalation path (open_source → private_repo → paranoid)
- Prevents accidental sensitive code leaks

**Negative:**
- Open-source projects must explicitly opt out of redaction
- Some performance overhead from redaction/chunking

**Neutral:**
- Struct Default differs from swarm dispatch default (requires documentation)

## Alternatives Considered

1. **No default (require explicit policy)** — rejected (too easy to forget, unsafe).
2. **Default to `open_source`** — rejected (unsafe for private repos).
3. **Default to `paranoid`** — rejected (too restrictive for most use cases).
4. **Make struct Default = private_repo** — rejected (breaks other use cases).

## Compliance

| Rule ID | Enforced by test(s) |
|---------|---------------------|
| ADR-0016.R1 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r1_private_repo_preset_properties` |
| ADR-0016.R1 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r1_open_source_is_permissive` |
| ADR-0016.R2 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r2_content_validation_size_limit` |
| ADR-0016.R2 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r2_content_validation_file_paths` |
| ADR-0016.R2 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r2_job_type_allowlist` |
| ADR-0016.R3 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r3_violations_reject` |
| ADR-0016.R4 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r4_explicit_opt_in_for_no_redaction` |
| ADR-0016.R5 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r5_policy_violation_variants_stable` |
| ADR-0016.R5 | `crates/dsrs/tests/adr_0016_privacy.rs::test_adr_0016_r5_policy_violation_display` |

## References

- [crates/dsrs/docs/PRIVACY.md](../../crates/dsrs/docs/PRIVACY.md) — canonical preset definitions
- [GLOSSARY.md](../GLOSSARY.md) — terminology
- [ADR-0004](./ADR-0004-lane-taxonomy.md) — lane taxonomy (Swarm lane)
- [ADR-0017](./ADR-0017-telemetry-trace-contract.md) — Layer C redaction references this policy
- `crates/dsrs/src/privacy/policy.rs` — implementation
