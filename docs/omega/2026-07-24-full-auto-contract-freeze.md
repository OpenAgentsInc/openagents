# Omega Full Auto contract freeze

- Date: 2026-07-24
- Class: contract freeze
- Packet: `OMEGA-FA-00`
- Omega issue: [OpenAgentsInc/omega#19](https://github.com/OpenAgentsInc/omega/issues/19)
- OpenAgents pin: `99e10b4f2eaacff8d0d8a01828c0dae62c826cfe`
- STE issue: 9
- Glossary revision: `openagents-ste-glossary-v1`
- Status: admitted for Omega Full Auto implementation packets

## 1. Purpose

This freeze locks the Full Auto product laws for Omega.
Later packets must not invent a second run lifecycle.
Later packets must not put durable run authority in GPUI state.

This freeze does not start `omega-effectd`.
This freeze does not start a GPUI launcher.
This freeze does not admit a release.

## 2. Authority chain

| Role | Artifact | Revision / pin |
| --- | --- | --- |
| Desktop run intent | `specs/desktop/full-auto.product-spec.md` | rev 14 |
| Desktop assurance | `specs/desktop/full-auto.assurance-spec.md` | rev 6 |
| Omega host delta | `specs/omega/full-auto.product-spec.md` | rev 1 |
| Omega assurance delta | `specs/omega/full-auto.assurance-spec.md` | rev 1 (proposed) |
| Port plan | `docs/omega/2026-07-24-full-auto-port-audit.md` | 2026-07-24 |
| Roadmap packet | `docs/omega/ROADMAP.md` §7.5 `OMEGA-OA-05` | current |

## 3. Digests Omega must consume

All digests are SHA-256 of the exact bytes at OpenAgents pin
`99e10b4f2eaacff8d0d8a01828c0dae62c826cfe` for the Desktop sources below.
The Omega host delta digests are the bytes of those files in the same freeze
commit that lands this document.

| Path | SHA-256 |
| --- | --- |
| `specs/desktop/full-auto.product-spec.md` | `5da9eba0601be1b2fe849dce96260e8a785d16590a60298d8180faf9442dba47` |
| `specs/desktop/full-auto.assurance-spec.md` | `be5b106c6bbe3bcdde503d1bcab6abaa559adb561528a1b68f28399f4caa9718` |
| `specs/omega/full-auto.product-spec.md` | `09f8c2c2c14df6f5272737e26b85dbe3f20704ce66345a9377353710a8d6dddc` |
| `specs/omega/full-auto.assurance-spec.md` | `531ff06bc62c29013f08d6f947ea862f6cf8b40d70a891c4a2f0a5505a107848` |
| `apps/openagents-desktop/src/full-auto-run-registry.ts` | `a3450722d60a4925b46e73903cae95e270f9e0c37420051a93e9a4087fa8ea4a` |
| `apps/openagents-desktop/src/full-auto-reconcile.ts` | `3db2b3950a28163809c3990731e97048fa8bb7fe2973d58a2b9b7cd628ab44ec` |
| `apps/openagents-desktop/src/full-auto-run-report.ts` | `ed63b5cbf1807d2c7bf6e380a1e2b91acd03d6364fc60efb95890454736b54f0` |
| `apps/openagents-desktop/src/full-auto-lane.ts` | `1359e6a1084f359331af3ced6138daf47f8c280c23b946e05811cd2c449bb078` |
| `apps/openagents-desktop/src/full-auto-capacity.ts` | `83495a3fd04c8963b9478512e576a0693dd50e67f21802127403038c1ba6d562` |
| `apps/openagents-desktop/src/full-auto-run-actions.ts` | `cb7ea71c1c90913a97a72d0a481735a57041d3a46603b3f38d39e34c5d9ad7e2` |

A later OpenAgents commit may replace these digests only with a new freeze
revision and an explicit Omega issue note.

## 4. Product laws

### 4.1 Form

- Full Auto is a dedicated run.
- Full Auto is never a composer toggle.
- Full Auto is never an ambient chat preference.
- GPUI, ACP panels, and ordinary chat paths are not run authority.

### 4.2 Hard numbers

| Law | Value | Source constant |
| --- | --- | --- |
| Active run limit | 8 | `FULL_AUTO_RUN_ACTIVE_LIMIT` |
| Default continuation cap | 20 | registry default when `maxTurns` is absent |
| Consecutive failure limit | 5 | `FULL_AUTO_MAX_CONSECUTIVE_FAILURES` |
| One active lease per thread | required | `full-auto-registry.ts` lease model |

### 4.3 Ten-state lifecycle

Exact states:

1. `draft`
2. `running`
3. `pausing`
4. `paused`
5. `retrying`
6. `stalled`
7. `completed`
8. `failed`
9. `stopped`
10. `cap_reached`

Active states: `running`, `pausing`, `paused`, `retrying`, `stalled`.
Terminal states: `completed`, `failed`, `stopped`, `cap_reached`.

Legal transitions (source: `FULL_AUTO_RUN_LEGAL_TRANSITIONS`):

| From | To |
| --- | --- |
| `draft` | `running`, `stopped` |
| `running` | `pausing`, `paused`, `retrying`, `stalled`, `completed`, `failed`, `stopped`, `cap_reached` |
| `pausing` | `paused`, `failed`, `stopped` |
| `paused` | `running`, `stopped` |
| `retrying` | `running`, `stalled`, `failed`, `cap_reached`, `stopped` |
| `stalled` | `retrying`, `failed`, `stopped` |
| `completed` | (none) |
| `failed` | (none) |
| `stopped` | (none) |
| `cap_reached` | (none) |

An illegal transition must refuse with a typed error.
Provider text must not close a run.
Only typed outcomes may close a run.

### 4.4 Non-overridable guardrails

Exact set from `FULL_AUTO_NON_OVERRIDABLE_GUARDRAILS`:

1. `workspace_binding`
2. `own_capacity_only`
3. `no_rate_limit_reset_triggering`

No config field, environment variable, or UI control may weaken this set.

### 4.5 First admitted action-lane set

Default lane: `codex-local`.

Required first Omega lane set:

1. `codex-local`
2. `claude-local`

Eligible when the host proves readiness (same Desktop policy table):

- `acp:grok-cli`
- `acp:cursor-agent`
- `harness:goose`
- `harness:opencode`
- `harness:pi`

Default launch routing order when the owner does not set Advanced policy:
`codex-local`, then `claude-local`.

### 4.6 Deferred for the first Omega port

These stay out of `OMEGA-FA-01` through `OMEGA-FA-07` unless a later freeze
admits them:

- MemoHarness adaptation, experience bank, and optimizer release
- HANDS-6 initiative / self-claim autonomy expansion

Desktop ProductSpec rev 14 still owns those criteria for Electron Desktop.
Omega must not claim them in the first port.

## 5. Redaction map

### 5.1 Private report

The private `FullAutoRunReport` may hold bounded turn identity, digests,
liveness observations, and owner-private mission fields.
It must stay on the Omega data root under mode `0600`.

### 5.2 Public-safe receipt

Schema id: `openagents.desktop.full_auto_run_receipt.v1`

Allowed fields (exact `FullAutoRunReceiptSchema`):

- `schema`, `runRef`, optional `threadRef`
- `objectiveDigest`, `doneConditionDigest`, `workspaceRefDigest`
- `state`, optional `startedAt`, optional `endedAt`
- `turnCap`, `successfulAttempts`, `failedAttempts`
- `providerIdentities` (opaque lane refs only)
- `providerTransitionCount`, `providerTransitionDispositions`
- `livenessGapCount`, `recoveryActionsUsed`
- `verifiedRefCount`, `claimedRefCount`
- `progressDisposition` (`unknown` until a typed source exists)
- `usageKnown`, `reportRevision`, `createdAt`

Forbidden in receipts, notifications, Sync projections, and public logs:

- raw objective or done-condition text
- workspace path plaintext
- account identity, API keys, or provider credentials
- assistant transcript text or segments
- shell output, prompts, or local auth paths
- wallet or settlement material

### 5.3 Notifications and Sync

Notifications and Sync may carry public-safe receipt fields and run state
only.
They must derive from the same redaction function as the receipt.
They must not become a second durable store.

## 6. Host binding for Omega

| Concern | Owner |
| --- | --- |
| Durable run mutation API | `full-auto-run-actions.ts` (or released successor) inside `omega-effectd` |
| Durable files | Omega channel data root under `full-auto/` |
| Process supervision | Omega Rust supervisor (FA-02) |
| Launcher and monitor UI | GPUI read and control surface (FA-03). Not run authority. |
| Desktop executor | Omega Desktop remains the sole executor for local own-capacity work |

Falsifier: a GPUI view, ACP panel, or ordinary chat path becomes Full Auto run
authority.

## 7. Acceptance for this freeze

Owner direction on 2026-07-24 ordered implementation of Omega issues `#19`
through `#30`.
This freeze is the admitted contract for that sequence.

Later FA packets may start only when they keep:

- one lifecycle
- the digests above, or a superseding freeze revision
- the redaction map
- the three non-overridable guardrails
- the deferred MemoHarness and initiative cut for the first port

## 8. Related Omega receipt

Omega records the consumer-side pointer at
`docs/src/development/omega-full-auto-contract-freeze.md`
in `OpenAgentsInc/omega`.
