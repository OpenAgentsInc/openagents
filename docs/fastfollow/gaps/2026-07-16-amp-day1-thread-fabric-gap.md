---
artifact_schema: "openagents.fastfollow.gap_assessment.v0"
artifact_id: "openagents.fastfollow.gap.amp.day1_thread_fabric_surfaces.44689c449110"
class: "target_gap_assessment"
status: "blocked"
disposition: "blocked_by_policy"
directive_id: "amp.day1_thread_fabric_surfaces"
stage: "gap_analysis"
target_revision: "15ce61bb58e5fcaf0b592f1ff485acf518071bbb"
target_tree: "ee71c5556aa51ec0bb54a73fcaa4423ec5195dea"
fast_follow_spec_id: "openagents.fast_follow"
fast_follow_revision: 2
fast_follow_document_digest: "b660b73e312fefa0339dead3641b4a2412ccdc31b978d3797c27cb407bf5a7de"
fast_follow_intent_digest: "unavailable_pre_ff01"
source_snapshot_digest: "173e86c427d8c62add2a9ae12ee0cc0de3aaabb5cb154e48a4e6336fffd1f210"
dedupe_digest: "44689c4491106c32e833d86bf44e658dbea3ee26e4bba6721c3e1d111214f83d"
visibility: "public_safe_repository_artifact"
observed_at: "2026-07-17T04:08:08Z"
---

# Day 1 Amp thread-fabric gap assessment

## Outcome

The first ordered Fast Follow directive has a material but currently
non-dispatchable gap. At target revision `15ce61bb58e`, OpenAgents already has:

- title and bounded content search over local coding history, with content
  results opening the exact original projected item;
- explicit Desktop **Steer now**, **Queue next**, and **Stop** controls;
- exact-turn compare-and-set steering and a restart-safe, idempotent local
  next-turn queue; and
- durable pending/accepted/terminal distinctions for the admitted MVP command
  paths.

The directive is not complete. Search does not carry later
supersession/revert/acceptance relations, thread share/export has no explicit
visibility schema or receipt, and steer/queue remain Desktop-local command
semantics rather than one shared Runtime Gateway and
`agent-runtime-schema` command algebra. `world-client` is a generic retained
Verse projection client, not a conversation-thread client, and the repository
has no active world backend.

The current Sol authority says there is no product-expansion lane and no open
`roadmap:sol` issue. The remaining work therefore has disposition
`blocked_by_policy`. This artifact does not create an issue, ProductSpec delta,
implementation claim, or permission to mutate product code. Under the seed's
`advance_when: current_directive_terminal_or_blocked` rule, a later Fast Follow
turn may advance to `amp.day2_routing_and_specialists` at `gap_analysis` while
this Day 1 disposition remains unchanged.

## Selection and exact identity

This is the first directive in
[`FASTFOLLOW.md`](../../../FASTFOLLOW.md)'s `initial_program`; no earlier
artifact under `docs/fastfollow/gaps`, `candidates`, or `receipts` recorded a
terminal or blocked disposition when the run started.

| Identity | Exact value |
| --- | --- |
| Target commit | `15ce61bb58e5fcaf0b592f1ff485acf518071bbb` |
| Target Git tree | `ee71c5556aa51ec0bb54a73fcaa4423ec5195dea` |
| FastFollowSpec | `openagents.fast_follow`, revision `2`, admitted |
| FastFollowSpec document SHA-256 | `b660b73e312fefa0339dead3641b4a2412ccdc31b978d3797c27cb407bf5a7de` |
| Intent digest | Not emitted because FF-01's canonical compiler has not landed; the exact document digest is bound instead and the limitation is part of the dedupe input |
| Directive / stage | `amp.day1_thread_fabric_surfaces` / `gap_analysis` |
| Target scopes | `apps/openagents-desktop`, `packages/agent-runtime-schema`, `packages/world-client` |
| Composite source snapshot SHA-256 | `173e86c427d8c62add2a9ae12ee0cc0de3aaabb5cb154e48a4e6336fffd1f210` |
| Gap dedupe SHA-256 | `44689c4491106c32e833d86bf44e658dbea3ee26e4bba6721c3e1d111214f83d` |

The dedupe digest binds the target commit, exact FastFollow document digest,
the explicit pre-FF-01 intent-digest limitation, composite source snapshot,
directive, stage, and ordered target scopes. A source, target, intent, scope,
or policy change requires a new assessment identity.

## Source snapshot and confidence

No external source code or instructions were executed. The reusable evidence
input is the existing public-safe teardown/synthesis corpus; no formal shared
StudyPacket service exists at FF-00.

| Source | Exact evidence identity | Confidence and boundary |
| --- | --- | --- |
| Amp Code | Installed CLI `0.0.1784247472-g76909f`, binary SHA-256 `521a9473876d488a5f05f9ea8fca20c9686d3321422dea5f3f0283576f4d9bdc`; [teardown](../../teardowns/2026-07-16-amp-code-teardown.md) SHA-256 `9e179fe91857af32215db38297e281ce8315e599f6c42cb3019d0031df11f16c` | High for visible CLI/manual behaviors; limited for closed hosted admission, ordering, and persistence semantics |
| Codex runtime | Public source commit `08ba14b03d0b3ce3cfdf8c88c0469b9b1924953d`; [teardown](../../teardowns/2026-07-10-codex-agent-runtime-teardown.md) SHA-256 `ac3d14b5826b2e2d03c3ca88af42d90fd10398cf83f29f396070ec03f90bef2c` | High for the public Thread/Turn/Item and app-server source seams |
| Amp composition strategy | [Fable essay](../../fable/2026-07-16-amp-in-a-few-days-on-openagents.md) SHA-256 `02b34352fb15c32b7fd0ce82ee3cb9014c05afc3e78df30ba1ab3df0b42bb392` at the target commit | Sequencing evidence only; its implementation-state table is superseded by current code where they disagree |
| OpenAgents adaptation synthesis | [Adaptation analysis](../../teardowns/2026-07-10-openagents-product-adaptation-analysis.md) SHA-256 `b7a0cf7d5126225c3b5d74e2aa5d230c4ba9d50ea33bb0dd618bb8e24db7ccd8` at the target commit | Target-native design evidence, not dispatch authority |

Amp's published behavior establishes the usefulness of durable searchable
threads, distinct queue/steer/interrupt intent, export, and explicit
collaboration visibility. It does not establish Amp's exact server admission
law. Its commercial/closed core is design evidence only; no Amp code is copied.

## Target authority reconciliation

| Authority | Exact observation | Effect on this gap |
| --- | --- | --- |
| Repository law | [`AGENTS.md`](../../../AGENTS.md) and [`INVARIANTS.md`](../../../INVARIANTS.md) admit research/gap artifacts only under configured Fast Follow paths and require separate target authority for product mutation | This assessment may land; product code may not |
| Sol roadmap | `docs/sol/MASTER_ROADMAP.md` says there is no active Sol product-expansion lane and broader follow-ons require a new bounded owner decision and issue | Share/export, new visibility states, shared command expansion, and supersession UX are blocked |
| Live issue state | The live `roadmap:sol` query returned zero open issues at `2026-07-17T04:08:08Z`; #8712, [MVP-01 #8756](https://github.com/OpenAgentsInc/openagents/issues/8756), [CAP-06 #8838](https://github.com/OpenAgentsInc/openagents/issues/8838), and [CAP-07 #8839](https://github.com/OpenAgentsInc/openagents/issues/8839) are closed completed | There is no issue to claim and no existing candidate to duplicate; the latter two explain why the bounded local steer/queue work is already present |
| MVP ProductSpec | [`openagents-codex-workroom-mvp.product-spec.md`](../../mvp/openagents-codex-workroom-mvp.product-spec.md) admits session catalog/history and send/stop/steer/queue controls, but not thread sharing, export, public visibility, or a cross-provider history product | Existing controls are in authority; the remaining surface needs new intent |
| MVP AssuranceSpec | [`openagents-codex-workroom-mvp.assurance-spec.md`](../../mvp/openagents-codex-workroom-mvp.assurance-spec.md) proves the accepted MVP revision, not a future sharing/supersession/shared-command delta | Current proof cannot be reused as acceptance for the gap |
| World boundary | Root `AGENTS.md` retains `packages/world-client` as a read-only Verse projection and says a future world backend needs a new Google Cloud design and explicit product authority | `world-client` cannot be treated as an already-authorized thread service |

No GitHub issue or implementation claim was created. The selected stage is
`gap_analysis`; automatically opening a candidate issue would cross both the
Fast Follow candidate/admission boundary and the Sol non-revival boundary.

## Current implementation reconciliation

### Historical search and original-event navigation — partially implemented

- [`history-search.ts`](../../../apps/openagents-desktop/src/history-search.ts)
  deterministically ranks title/workspace-label and bounded content matches.
  A content result carries the exact `matchItemRef` and sequence.
- [`history-workspace.ts`](../../../apps/openagents-desktop/src/renderer/history-workspace.ts)
  opens that result in a window containing the matching source item and selects
  it. This closes the dated essay's “search UI absent” statement.
- [`merged-history.ts`](../../../apps/openagents-desktop/src/merged-history.ts)
  exposes the honest boundary: titles cover the bounded catalog, while content
  indexing covers only the 60 most recently active roots and 300 items per
  indexed root. The response reports `truncated`.
- [`codex-history-contract.ts`](../../../apps/openagents-desktop/src/codex-history-contract.ts)
  has source item identity, sequence, kind, status, fields, redaction, and
  completeness, but no typed supersedes/superseded-by, revert, review, or
  acceptance relations. Search therefore lands on an original projected item
  without explaining whether later evidence invalidated it.

Implementation axis: **partial**. Verification axis: **fixture-proven** by
`tests/history-search.test.ts` and `src/renderer/history-workspace.test.ts`;
the existing signed-MVP acceptance does not prove whole-archive content search
or supersession semantics.

### Queue, Steer, and Stop — implemented locally; shared algebra incomplete

- [`desktop-command-contract.ts`](../../../apps/openagents-desktop/src/desktop-command-contract.ts)
  registers `chat.steer_current`, `chat.queue_next`, and `chat.stop` as one
  visible/palette/keybinding command vocabulary.
- [`react-composer.tsx`](../../../apps/openagents-desktop/src/renderer/react-composer.tsx)
  renders explicit **Steer now**, **Queue next**, and **Stop** affordances.
- [`composer-admission.ts`](../../../apps/openagents-desktop/src/composer-admission.ts)
  binds steering to the exact displayed active turn and never silently
  converts steering into queueing.
- [`codex-durable-queue.ts`](../../../apps/openagents-desktop/src/codex-durable-queue.ts)
  persists stable intent/message identity, orders next-turn work, and fails a
  process-interrupted promotion closed instead of replaying a possibly accepted
  provider effect.
- [`codex-turn-state.ts`](../../../apps/openagents-desktop/src/codex-turn-state.ts)
  compare-and-sets steering against the active regular turn and persists only
  hashed public-safe admission receipts.

These controls satisfy the accepted MVP's Desktop-local product shape.
However, [`agent-runtime-schema`](../../../packages/agent-runtime-schema/src/index.ts)
defines shared runtime intents for message append, turn start, interrupt,
continue, retry, and close, but not explicit steer-at-safe-boundary or
queue-until-yield. Likewise, the closed
[`runtime-gateway`](../../../apps/openagents-desktop/src/runtime-gateway-contract.ts)
exposes history search and start/interrupt/continue/retry/close, not a shared
steer/queue command and terminal-delivery receipt. The semantics still depend
on the Desktop local lane and provider-specific adapters.

Implementation axis: **landed for the Desktop local Codex path; partial for the
directive's canonical cross-surface contract**. Verification axis: **fixture
and accepted-MVP evidence for the bounded local path; absent for the shared
algebra**.

### Share, export, visibility, and receipts — not implemented

The Desktop command registry and Runtime Gateway have no thread share or
export command. Neither the history contract nor `agent-runtime-schema` has a
thread visibility/ownership/retention state machine or a share/export receipt.
Existing `AgentRuntimeVisibility` values classify event projection, not thread
collaboration policy. Diagnostic/support exports are unrelated and cannot be
reclassified as thread export. Closed [T14 #8871](https://github.com/OpenAgentsInc/openagents/issues/8871)
widened a web mirror's share projection; it neither admits nor implements a
Desktop thread-sharing contract.

Implementation axis: **absent**. Verification axis: **not mapped**.

### `world-client` target scope — no current conversation role

[`packages/world-client`](../../../packages/world-client/src/index.ts) decodes
World snapshots/deltas and maps generic World command receipts. It contains no
thread/history/share/export contract and has no active world service host. It
provides no current Day 1 implementation evidence and cannot receive new
backend authority from this directive.

Implementation axis: **not applicable to the current conversation product**.
Exception axis: **none**; changing that scope requires ordinary architecture
and product admission.

## Independent assessment axes

| Axis | Assessment |
| --- | --- |
| Source freshness | Fresh for the format-0.1 14-day window; all selected source artifacts were pinned or captured 2026-07-10 through 2026-07-16 |
| Evidence confidence | High for public Codex source and local OpenAgents code; mixed for visible Amp behavior; intentionally low for unobserved Amp server guarantees |
| Relevance | High: all remaining gaps are direct parts of the Day 1 desired outcome |
| Target fit | Partial: local-first history and explicit controls fit; cloud-canonical/unlisted semantics do not |
| Portability | High for typed supersession and command semantics; medium for sharing/export because visibility, retention, identity, and public projection need target policy |
| License/provenance | Study is permitted; Amp core is closed/commercial and no code transfer is proposed; Codex evidence is Apache-2.0 source-backed |
| Implementation | Search/open-at-item and local controls landed; supersession, sharing/export, and cross-surface steer/queue remain absent or partial |
| Verification | Existing focused fixtures and accepted MVP evidence cover only the landed bounded paths; no proof design exists for the remaining delta |
| Disposition | `blocked_by_policy` |
| Exception | None granted; no test, source, or receipt may reopen the closed product lane |

## Reopen conditions and close rule

The Day 1 gap may be reconsidered only after all of the following change:

1. an owner decision opens a bounded product lane;
2. an admitted ProductSpec names exact thread visibility/export and/or
   supersession/shared-command outcomes without widening local data authority;
3. an AssuranceSpec names privacy, loss-accounting, idempotency, restart,
   cross-surface, and rendered-pixel falsifiers;
4. one issue or accepted work packet owns exact paths and hot contracts; and
5. one isolated implementation claim binds the current target and candidate.

Until then, the honest close rule is: gap observed, existing implementation
reconciled, no candidate admitted, no product code changed, no verification or
public capability claim promoted.
