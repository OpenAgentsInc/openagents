---
artifact_schema: "openagents.fastfollow.implementation_receipt.v0"
artifact_id: "openagents.fastfollow.receipt.ff_d1_36.desktop_canonical_export_receipt_catalog.20260717"
class: "implementation_receipt"
status: "implemented"
disposition: "bounded_packet_landed"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "implementation"
fast_follow_revision: 3
base_revision: "50486fabbdc2e9d6d5fb30a5c824ecebe1674ddc"
claim_revision: "774a5b3908e3aac6b5626c9a0cd6e3b93356dd50"
implementation_revision: "pending_remote_landing"
proof_rung: "desktop_canonical_export_receipt_catalog"
observed_at: "2026-07-17T23:41:20Z"
---

# FF-D1-36 Desktop canonical-export receipt catalog receipt

## Authority and obligation reconciliation

The owner-accepted ordered program and durable FF-D1-36 claim in the
[accepted-plan ledger](../../sol/2026-07-16-fast-follow-expansion-accepted-plan.md)
admitted this packet after FF-D1-35 released. Current `origin/main`, prior Day
1 receipts and releases, Fast Follow revision 3, ProductSpec and AssuranceSpec
obligations, repository invariants, README/docs, open issues, known baselines,
Git configuration, dependencies, claims, and active worktrees were reconciled
before mutation.

Active work owned Desktop `main.ts`, renderer, installed-runtime, mobile, T3,
Full Auto, and teardown surfaces. Authoritative supersession/reversion
producers and named-group membership remain absent. No open reproducible bug,
claim, or worktree owned the two new Desktop catalog paths. AssuranceSpec
remained proof design rather than a provider-owned verdict.

## Implemented packet

- added one schema-versioned, owner-private catalog containing only decoded
  canonical owner-only `export_created` receipts with exact artifact-ref/
  SHA-256 binding;
- caps the catalog at 1,000 receipts and 1 MiB, atomically replaces its file,
  and applies owner-private directory/file modes where the platform supports
  them;
- makes exact replay unchanged while conflicting receipt, intent,
  idempotency, or artifact identity rejects without changing prior state;
- rejects corrupt, duplicate, oversized, unknown/extra-schema, malformed,
  forbidden-raw-field, and invalid-root state rather than salvaging authority;
- reopens only the complete decoded ref-only set and proves that set feeds
  FF-D1-35 into the original canonical event without storing its artifact
  content; and
- returns no catalog path, artifact bytes, transcript content, host/preload/
  renderer authority, Sync authority, or release authority.

The persisted-data boundary follows the repository's Effect schema guidance:
unknown JSON is decoded through a closed catalog envelope and every receipt is
decoded again through the canonical disclosure schema and semantic checks.

## Proof

| Check | Result |
| --- | --- |
| Focused catalog/acquisition/artifact-store/compiler/disclosure/authority tests | PASS — 42/42 |
| Desktop and Agent Runtime Schema typechecks | PASS |
| Fast Follow root coverage | PASS — 7/7 |
| Fast Follow package checks | PASS — 13/13 plus typecheck/distribution |
| Behavior-contract checks | PASS — 36/36 |
| ProductSpec focused test | PASS — 107/107 |
| Sol document tests and manifest | PASS — 19/19 |
| `pnpm run check` | PASS |
| `pnpm run check:fast` | PASS |
| Targeted AssuranceSpec suite | BASELINE FAIL — 189/190; environment digest |

An initially unscoped ProductSpec package invocation inherited the repository
root test workspace and was stopped after it reached unrelated API-worker
baseline failures. The exact ProductSpec test then passed 107/107. AssuranceSpec
reproduced only the previously recorded environment-profile digest snapshot
drift. This packet did not absorb or weaken those conditions and did not mutate
shared Git configuration.

## Honest boundary and next packet

This receipt closes only owner-private persistence of the exact canonical
export receipts consumed by FF-D1-35. It does not ingest broader historical
sessions, wire the Desktop host or UI, render pixels, produce supersession/
reversion facts, authorize named groups, register acquisition in `main.ts`, or
prove an installed runtime journey. Those residuals, owner acceptance, and Day
1 completion remain unclaimed.

The exact tested implementation tree will replace `pending_remote_landing`
when it lands on `origin/main`, before the documentation-only claim release.
