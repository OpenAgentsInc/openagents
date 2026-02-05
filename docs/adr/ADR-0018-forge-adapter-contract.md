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

### Implementation Status

| Component | Status |
|-----------|--------|
| ForgeAdapter trait | Not yet implemented |
| GitHub adapter | Ad-hoc (no trait) |
| GitAfter adapter | Spec only |
| NIP-34 builders | Partial (`crates/nostr/core/src/nip61.rs`) |

### Canonical owner

- Trait definition: TBD (target: `crates/adjutant/src/forge/mod.rs`)
- Bundle format: [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md)
- GitAfter tags: [crates/nostr/GIT_AFTER.md](../../crates/nostr/GIT_AFTER.md)
- Terminology: [GLOSSARY.md](../GLOSSARY.md) (`Forge Adapter`)

### ForgeAdapter trait (Illustrative)

This trait is **illustrative** — it defines the target interface but is not yet implemented in code.

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

**ADR-0018.R1** — All Forge Adapters MUST implement these operations:

| Operation | Description |
|-----------|-------------|
| `create_pr` | Create PR/patch from bundle |
| `push_branch` | Push commits to remote |
| `post_comment` | Comment on issue/PR |
| `update_pr_status` | Change PR state |
| `capabilities` | Report supported features |

### Bundle → PR mapping (Normative)

When creating a PR, adapters MUST:

1. **ADR-0018.R2** — Include `PR_SUMMARY.md` content in PR description (with truncation if needed)
2. **ADR-0018.R3** — Reference `session_id` in PR metadata (where supported)
3. **ADR-0018.R4** — Reference `policy_bundle_id` in PR metadata (where supported)
4. **ADR-0018.R5** — Link to trajectory via `trajectory_hash` if forge supports it
5. Preserve verification results in PR description

### PR truncation rules (Normative)

If `PR_SUMMARY.md` exceeds forge max length (e.g., GitHub ~65536 chars):

1. Truncate human-readable summary text
2. **ADR-0018.R6** — ALWAYS preserve these fields (in metadata or footer):
   - `session_id`
   - `policy_bundle_id`
   - `trajectory_hash`
   - `verification_passed`
3. **ADR-0018.R7** — Add truncation notice: `[Summary truncated. Full summary in PR_SUMMARY.md]`

### Forge capabilities (Illustrative)

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
| GitHub | No (comment only) | No (bot token) | No | Ad-hoc |
| GitAfter | Yes (NIP-34 tags) | Yes (npub) | Yes (NIP-57) | Spec only |
| Bare git | No | No | No | Ad-hoc |
| NIP-34 | Yes | Yes (npub) | Yes (NIP-57) | Partial |

### Canonical hash field: `trajectory_hash` (Normative)

**ADR-0018.R8** — The canonical field name is **`trajectory_hash`** (not `replay_hash`).

This is consistent with:
- [PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) receipt schema
- [ADR-0013](./ADR-0013-receipt-schema-payment-proofs.md) receipt fields
- [GIT_AFTER.md](../../crates/nostr/GIT_AFTER.md) tag definitions

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

### NIP-34 / GitAfter tags (Normative)

For forges supporting trajectory linking, PR events (kind:1618) MUST include these tags per [GIT_AFTER.md](../../crates/nostr/GIT_AFTER.md):

```json
{
  "tags": [
    ["trajectory", "<session_id>", "<relay_url>"],
    ["trajectory_hash", "<sha256-of-all-trajectory-events>"],
    ["policy_bundle_id", "<version>"]
  ]
}
```

The exact tag format is defined in GIT_AFTER.md; this ADR only requires their presence.

## Scope

What this ADR covers:
- ForgeAdapter trait interface (illustrative)
- Required operations and their semantics
- Bundle → PR mapping requirements
- Metadata and truncation rules
- Canonical hash field name

What this ADR does NOT cover:
- Authentication/credential management per forge
- Rate limiting and retry policies
- Forge-specific UI/UX considerations
- GitAfter protocol details (see GIT_AFTER.md)

## Invariants / Compatibility

| Invariant | Guarantee |
|-----------|-----------|
| Hash field name | Canonical: `trajectory_hash` |
| Bundle attribution | `policy_bundle_id` always preserved |
| PR description | Always includes PR_SUMMARY.md (possibly truncated) |
| Preserved fields | session_id, policy_bundle_id, trajectory_hash, verification_passed |

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
- Trait not yet implemented (requires implementation work)

**Neutral:**
- Each forge adapter still needs forge-specific implementation

## Alternatives Considered

1. **Forge-specific APIs only** — rejected (duplication, inconsistent attribution).
2. **Single GitHub-only adapter** — rejected (limits future forges).
3. **Generic "post to URL" interface** — rejected (loses structure).
4. **Use `replay_hash` instead** — rejected (`trajectory_hash` is already canonical).

## Compliance

| Rule ID | Enforced by test(s) | Status |
|---------|---------------------|--------|
| ADR-0018.R1 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r1_required_operations` | ⏳ Ignored |
| ADR-0018.R2 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r2_pr_includes_summary` | ⏳ Ignored |
| ADR-0018.R3 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r3_session_id_in_metadata` | ⏳ Ignored |
| ADR-0018.R4 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r4_policy_bundle_id_in_metadata` | ⏳ Ignored |
| ADR-0018.R5 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r5_trajectory_hash_link` | ⏳ Ignored |
| ADR-0018.R6 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r6_truncation_preserves_fields` | ✅ Pass |
| ADR-0018.R7 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r7_truncation_adds_notice` | ✅ Pass |
| ADR-0018.R8 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r8_canonical_hash_field_name` | ✅ Pass |
| ADR-0018.R8 | `crates/adjutant/tests/adr_0018_forge.rs::test_adr_0018_r8_trajectory_hash_in_protocol` | ✅ Pass |

**Note:** R1-R5 tests are ignored pending ForgeAdapter trait implementation.

## References

- [GLOSSARY.md](../GLOSSARY.md) — `Forge Adapter` definition
- [crates/dsrs/docs/ARTIFACTS.md](../../crates/dsrs/docs/ARTIFACTS.md) — Verified Patch Bundle format
- [crates/nostr/GIT_AFTER.md](../../crates/nostr/GIT_AFTER.md) — GitAfter design and tag format
- [docs/protocol/PROTOCOL_SURFACE.md](../protocol/PROTOCOL_SURFACE.md) — `trajectory_hash` in receipt schema
- [ADR-0002](./ADR-0002-verified-patch-bundle.md) — Verified Patch Bundle contract
- [ADR-0013](./ADR-0013-receipt-schema-payment-proofs.md) — receipt fields including `trajectory_hash`
- [ADR-0015](./ADR-0015-policy-bundles.md) — policy_bundle_id attribution
