# Sarah Nostr migration and cutover

- Date: 2026-07-24
- Class: implementation note
- Packet: `SARAH-NR-08`
- OpenAgents issue: [OpenAgentsInc/openagents#9222](https://github.com/OpenAgentsInc/openagents/issues/9222)
- Master tracker: [OpenAgentsInc/omega#31](https://github.com/OpenAgentsInc/omega/issues/31)
- Spec home: [2026-07-24-sarah-workroom-mvp-spec.md](./2026-07-24-sarah-workroom-mvp-spec.md) §24.9
- Record contract: [2026-07-24-sarah-nostr-record-contract.md](./2026-07-24-sarah-nostr-record-contract.md) §4
- Implementation: `packages/sarah/src/nostr-migration/`
- Hosted bridge: `apps/openagents.com/workers/api/src/sarah-nostr-turn-bridge.ts`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: stage machine and tests admitted. Production cutover flag stays off.

## 1. Purpose

This document describes the Sarah record migration from Khala Sync to the
OpenAgents-controlled Nostr relay.

It freezes:

1. the three stages in order
2. the stable identity map
3. the feature flag surface
4. export and rollback rules
5. what this packet does not change

It does not flip production to Nostr-primary by default.
It does not delete Cloud SQL rows.
It does not change the Sarah authority profile.

## 2. Stages

Run the stages in this order.

| Stage | Record authority | Khala write path | Nostr publish |
| --- | --- | --- | --- |
| `shadow` | Khala Sync | open | dual-publish |
| `cutover` | Nostr relay | open as derived projection | primary |
| `retirement` | Nostr relay | closed for the Sarah lane | primary |

Rules:

- Same-stage apply is idempotent.
- Forward path: `shadow` → `cutover` → `retirement`.
- Rollback path during the window: `retirement` → `cutover`, `cutover` →
  `shadow`.
- Illegal jumps (for example `shadow` → `retirement`) fail closed.

Code: `SarahNostrMigrationStageMachine` in
`packages/sarah/src/nostr-migration/stages.ts`.

## 3. Stable identity map

The Sarah owner conversation keeps one digest in two wire forms.

| Legacy Khala Sync reference | Nostr conversation tag |
| --- | --- |
| `thread.sarah.<24 lowercase hex>` | `sarah.<24 lowercase hex>` |

- A client that sees only the legacy form removes the `thread.` prefix.
- A client that sees only the Nostr tag adds the `thread.` prefix.
- The raw owner id never enters a tag, event, export, or log.

Code: `packages/sarah/src/nostr-migration/mapping.ts`.
The hosted bridge reuses the same helpers.

## 4. Feature flag

### 4.1 Primary flag

```text
SARAH_NOSTR_RECORD_MODE=khala|shadow|nostr
```

| Value | Meaning |
| --- | --- |
| `khala` | Default. No Nostr publish from hosted dispatch. |
| `shadow` | Dual-publish. Khala Sync stays the record. |
| `nostr` | Relay is the record (cutover path). |

Production default remains `khala` unless an operator sets the flag.

### 4.2 Legacy flag (SARAH-NR-05)

```text
SARAH_NOSTR_SHADOW_PUBLISH=1
```

If the operator does not set `SARAH_NOSTR_RECORD_MODE`, then
`SARAH_NOSTR_SHADOW_PUBLISH=1` selects mode `shadow`. If the operator sets both
flags, the primary flag takes precedence.

### 4.3 Hosted dispatch wiring

`tryCreateSarahNostrTurnBridgeFromEnv` in
`apps/openagents.com/workers/api/src/sarah-nostr-turn-bridge.ts` reads the
resolved mode through `resolveSarahNostrRecordMode`.

`runHostedRuntimeTurnDispatch` already calls that bridge for Sarah owner
threads. Fail-soft behavior stays: a bridge error does not fail the hosted
turn under shadow.

Shadow already exists from SARAH-NR-05. This packet adds the stage machine,
the primary mode flag, drift comparison, and export/rollback helpers around
that path.

Also required for a live bridge:

- `SARAH_NOSTR_IDENTITY_SECRET` (Secret Manager mount)
- `SARAH_NOSTR_OWNER_PUBKEY` (64 lowercase hex, public)

## 5. Drift comparison

`compareKhalaAndNostrDurableEvents` compares public-safe ladder rows only:

- entry kind (`turn.started`, `tool.call`, …)
- `seq`
- `turnRef`

It does not read prompts, tool output, ciphertext, or credentials.
It reports `missing_on_nostr`, `missing_on_khala`, and `entry_mismatch`.
A clean report is `ok: true` with zero items.

Use this during the shadow stage to prove the dual records agree before
cutover.

## 6. Export and rollback

### 6.1 Manifest schema

Schema id: `openagents.sarah.nostr_migration_manifest.v1`.

The manifest carries:

- stage
- `conversation` and `threadRef`
- ordered public event ids only
- `digestChain` (SHA-256 of the joined event ids)
- optional `rollbackWindowClosesAt`

It never carries content, ciphertext, prompts, secrets, or private paths.

### 6.2 Rollback rules

- Validate the manifest decode and `digestChain`.
- From `cutover`, the only legal rollback target is `shadow`.
- From `retirement`, the only legal rollback target is `cutover`.
- After `rollbackWindowClosesAt`, the helper refuses rollback.
- Do not delete old Cloud SQL rows during the window.

Code: `buildSarahNostrMigrationManifest` and
`validateSarahNostrMigrationRollback`.

## 7. What this packet does not do

1. Flip production default to `SARAH_NOSTR_RECORD_MODE=nostr`.
2. Stop Khala writes in production (retirement stays operator-gated).
3. Delete historical Khala Sync rows.
4. Make relay acceptance an OpenAgents admission.
5. Change Sarah authority grants or reserved actions.
6. Replace exact Cloud SQL metering with relay metrics.

## 8. Exit criteria

This packet is complete when:

1. The stage machine, mapping, drift comparator, and export/rollback helpers
   live under `packages/sarah/src/nostr-migration/` with tests.
2. Hosted dispatch honors `SARAH_NOSTR_RECORD_MODE` (and the legacy shadow
   flag).
3. Production default remains `khala` / shadow-opt-in.
4. This document is the operator note for the cutover path.

Full product exit from the specification (relay is the record, projection
agrees, rollback proven in production) needs an operator-gated cutover and
the journey proof in `SARAH-NR-09`.

## 9. Falsifiers

The migration is wrong if any of these becomes true.

1. A Cloud SQL row outranks the signed event it came from after cutover.
2. Khala Sync becomes a hidden second conversation log after retirement.
3. An operator deletes old rows during the rollback window.
4. A raw key, credential, or private path appears in a manifest or drift
   report.
5. The stage machine accepts an illegal stage jump.
6. Production flips to Nostr-primary without an explicit operator flag.
