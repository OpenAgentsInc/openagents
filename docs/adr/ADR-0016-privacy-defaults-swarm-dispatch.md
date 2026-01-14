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

**Swarm dispatch MUST apply a privacy policy. The default is `private_repo`. Callers MAY override to `open_source` or `paranoid` explicitly.**

### Canonical owner

- Implementation: `crates/dsrs/src/privacy/` (redaction, chunking, policy)
- Integration: `crates/dsrs/src/adapter/swarm.rs` (SwarmDispatcher)
- Terminology: [GLOSSARY.md](../../GLOSSARY.md)

### Privacy policy presets (Normative)

| Preset | Redaction | Chunking | Max Size | File Paths | Verification | Use Case |
|--------|-----------|----------|----------|------------|--------------|----------|
| `open_source` | None | Full | Unlimited | Allowed | Optional | Public repos |
| `private_repo` | PathsOnly | MinimalSpans(5) | 50KB | Blocked | Required | **Default** |
| `paranoid` | Full | MinimalSpans(2) | 10KB | Blocked | Required | Sensitive code |

### Default behavior (Normative)

1. If no privacy policy is specified, `private_repo` is applied.
2. `SwarmDispatcher` MUST validate content against the active policy before dispatch.
3. Policy violations MUST reject the job (not silently redact and proceed).
4. Callers MUST explicitly opt into `open_source` if they want no redaction.

### Redaction modes (Normative)

| Mode | Description |
|------|-------------|
| `None` | No redaction |
| `PathsOnly` | Redact file paths (`/Users/alice/...` → `/workspace/...`) |
| `Identifiers` | Redact paths + class/function names |
| `Full` | Full content anonymization with mapping |

### Chunking policies (Normative)

| Policy | Description |
|--------|-------------|
| `Full` | Send entire content |
| `MinimalSpans { context_lines }` | Only relevant lines + N lines context |
| `AstNodesOnly { node_types }` | Only specific AST nodes |
| `FixedSize { max_chars, overlap }` | Fixed-size chunks |

### Trusted providers (Normative)

- Policies MAY specify `trusted_providers: Vec<String>` (npubs).
- Jobs to trusted providers MAY relax redaction (policy-dependent).
- `ReservePool` providers are implicitly trusted (OpenAgents-controlled).

### Policy violations

```rust
pub enum PolicyViolation {
    JobTypeNotAllowed(String),
    UntrustedProvider(String),
    ContentTooLarge { size: usize, max: usize },
    FilePathsNotAllowed,
    VerificationRequired,
}
```

Violations MUST be returned as errors, not silently handled.

## Scope

What this ADR covers:
- Default privacy policy for swarm dispatch
- Preset definitions and their parameters
- Validation behavior on policy violations

What this ADR does NOT cover:
- Redaction algorithm implementation details
- Provider reputation/trust scoring
- Per-job-type privacy overrides

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Default policy | `private_repo` when unspecified |
| Preset names | Stable: `open_source`, `private_repo`, `paranoid` |
| Violation behavior | Reject, never silently proceed |
| Redaction roundtrip | Redacted content MUST be restorable via mapping |

Backward compatibility:
- New presets may be added.
- Existing preset parameters may be tightened (more restrictive) without a new ADR.
- Loosening parameters requires a superseding ADR.

## Consequences

**Positive:**
- Safe-by-default for private repositories
- Clear escalation path (private_repo → paranoid)
- Prevents accidental sensitive code leaks

**Negative:**
- Open-source projects must explicitly opt out of redaction
- Some performance overhead from redaction/chunking

**Neutral:**
- Trusted provider lists require maintenance

## Alternatives Considered

1. **No default (require explicit policy)** — rejected (too easy to forget, unsafe).
2. **Default to `open_source`** — rejected (unsafe for private repos).
3. **Default to `paranoid`** — rejected (too restrictive for most use cases).

## References

- [crates/dsrs/docs/PRIVACY.md](../../crates/dsrs/docs/PRIVACY.md) — full privacy module documentation
- [GLOSSARY.md](../../GLOSSARY.md) — terminology
- [ADR-0004](./ADR-0004-lane-taxonomy.md) — lane taxonomy (Swarm lane)
- `crates/dsrs/src/privacy/` — implementation
