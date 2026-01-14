# ADR-0018: Forge Adapter Contract

## Status

**Accepted**

## Date

2026-01-13

## Context

OpenAgents produces Verified Patch Bundles (PR_SUMMARY.md, RECEIPT.json, REPLAY.jsonl) that must be published to collaboration surfaces ("forges"). Currently supported or planned forges include:
- GitHub (PRs, issues, comments)
- GitAfter (NIP-34 patches, agent-native git)
- Bare git (branches, commits)
- NIP-34 (Nostr git primitives)

Without a contract:
- each forge integration duplicates mapping logic,
- trajectory linking is inconsistent,
- bundle attribution (`policy_bundle_id`) may be lost,
- no clear interface for adding new forges.

## Decision

**Forge Adapters implement a common trait that maps Verified Patch Bundles to forge-specific operations. Adapters MUST preserve bundle attribution and trajectory references.**

### Canonical owner

- Trait definition: `crates/adjutant/src/forge/mod.rs` (planned)
- Bundle format: [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md)
- Terminology: [GLOSSARY.md](../../GLOSSARY.md) (`Forge Adapter`)

### ForgeAdapter trait (Normative)

```rust
#[async_trait]
pub trait ForgeAdapter: Send + Sync {
    /// Create a pull request / patch from a Verified Patch Bundle
    async fn create_pr(&self, bundle: &VerifiedPatchBundle, opts: CreatePrOpts) -> Result<PrHandle>;

    /// Push a branch to the forge
    async fn push_branch(&self, branch: &str, commit: &str) -> Result<()>;

    /// Post a comment on an issue or PR
    async fn post_comment(&self, target: CommentTarget, body: &str) -> Result<()>;

    /// Update PR status (draft, ready, closed)
    async fn update_pr_status(&self, handle: &PrHandle, status: PrStatus) -> Result<()>;

    /// Get forge capabilities
    fn capabilities(&self) -> ForgeCapabilities;
}
```

### Required operations (Normative)

All Forge Adapters MUST implement:

| Operation | Description |
|-----------|-------------|
| `create_pr` | Create PR/patch from bundle |
| `push_branch` | Push commits to remote |
| `post_comment` | Comment on issue/PR |
| `update_pr_status` | Change PR state |
| `capabilities` | Report supported features |

### Bundle → PR mapping (Normative)

When creating a PR, adapters MUST:

1. Include `PR_SUMMARY.md` content in PR description
2. Reference `session_id` in PR metadata (where supported)
3. Reference `policy_bundle_id` in PR metadata (where supported)
4. Link to trajectory if forge supports it (NIP-34, GitAfter)
5. Preserve verification results in PR description

### Forge capabilities

```rust
pub struct ForgeCapabilities {
    /// Supports trajectory linking (NIP-34, GitAfter)
    pub trajectory_linking: bool,
    /// Supports structured metadata
    pub structured_metadata: bool,
    /// Supports native agent identity
    pub agent_identity: bool,
    /// Supports bounties/payments
    pub bounties: bool,
    /// Maximum PR description length
    pub max_description_len: Option<usize>,
}
```

### Supported forges

| Forge | Trajectory Linking | Agent Identity | Bounties | Status |
|-------|-------------------|----------------|----------|--------|
| GitHub | No (comment only) | No (bot token) | No | 🟢 Implemented |
| GitAfter | Yes (NIP-34 tags) | Yes (npub) | Yes (NIP-57) | 🔵 Specified |
| Bare git | No | No | No | 🟢 Implemented |
| NIP-34 | Yes | Yes (npub) | Yes (NIP-57) | 🔵 Specified |

### PR metadata format (Normative)

For forges that support structured metadata, include:

```json
{
  "openagents": {
    "session_id": "sess_abc123",
    "policy_bundle_id": "v1.2.3",
    "trajectory_hash": "sha256:...",
    "confidence": 0.92,
    "verification_passed": true
  }
}
```

For GitHub (no structured metadata), append to PR description:

```markdown
---
<!-- OpenAgents metadata -->
<!-- session_id: sess_abc123 -->
<!-- policy_bundle_id: v1.2.3 -->
<!-- trajectory_hash: sha256:... -->
```

### Trajectory linking (NIP-34 / GitAfter)

For forges supporting trajectory linking, PR events MUST include:

```json
{
  "tags": [
    ["trajectory", "<session_id>", "<relay_url>"],
    ["trajectory_hash", "sha256:..."],
    ["policy_bundle_id", "v1.2.3"]
  ]
}
```

## Scope

What this ADR covers:
- ForgeAdapter trait interface
- Required operations and their semantics
- Bundle → PR mapping requirements
- Metadata preservation rules

What this ADR does NOT cover:
- Authentication/credential management per forge
- Rate limiting and retry policies
- Forge-specific UI/UX considerations
- GitAfter protocol details (see GIT_AFTER.md)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Trait interface | Stable method signatures |
| Bundle attribution | `policy_bundle_id` always preserved |
| PR description | Always includes PR_SUMMARY.md content |
| Capabilities | Stable capability flags |

Backward compatibility:
- New optional methods may be added with default implementations.
- New capabilities may be added.
- Removing required methods requires superseding ADR.

## Consequences

**Positive:**
- Unified interface for all forge integrations
- Bundle attribution preserved across forges
- Easy to add new forge backends
- Trajectory linking where supported

**Negative:**
- Lowest-common-denominator for features (GitHub limitations)
- Metadata in comments is fragile (GitHub)

**Neutral:**
- Each forge adapter still needs forge-specific implementation

## Alternatives Considered

1. **Forge-specific APIs only** — rejected (duplication, inconsistent attribution).
2. **Single GitHub-only adapter** — rejected (limits future forges).
3. **Generic "post to URL" interface** — rejected (loses structure).

## References

- [GLOSSARY.md](../../GLOSSARY.md) — `Forge Adapter` definition
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — Verified Patch Bundle format
- [crates/nostr/GIT_AFTER.md](../../crates/nostr/GIT_AFTER.md) — GitAfter design
- [ADR-0002](./ADR-0002-verified-patch-bundle.md) — Verified Patch Bundle contract
- [ADR-0015](./ADR-0015-policy-bundles.md) — policy_bundle_id attribution
