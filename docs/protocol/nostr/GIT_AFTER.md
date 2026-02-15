# GitAfter (Nostr Git Trajectory Linking)

This document defines the **GitAfter** tags used to link forge artifacts (PRs/patches) back to an OpenAgents trajectory.

Scope:
- Tag shapes for referencing `session_id`, `trajectory_hash`, and `policy_bundle_id`.
- Intended to be used by forge adapters (see `docs/adr/ADR-0018-forge-adapter-contract.md`).

## Canonical Tags

When publishing a PR/patch event to a forge that supports trajectory linking, include:

```jsonc
[
  ["trajectory", "<session_id>", "<relay_url>"],
  ["trajectory_hash", "<sha256-of-trajectory>"],
  ["policy_bundle_id", "<policy_bundle_id>"]
]
```

Notes:
- `trajectory_hash` is canonical (not `replay_hash`). See `docs/adr/ADR-0018-forge-adapter-contract.md`.
- `policy_bundle_id` is canonical. See `docs/adr/ADR-0015-policy-bundles.md`.

## Relationship To Verified Patch Bundles

Verified Patch Bundles are stored locally as:
- `PR_SUMMARY.md`
- `RECEIPT.json`
- `REPLAY.jsonl`

See `docs/execution/ARTIFACTS.md` and `docs/execution/REPLAY.md`.

