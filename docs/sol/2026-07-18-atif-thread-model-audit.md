# ATIF and OpenAgents thread-model audit

Date: 2026-07-18
Status: implemented audit; architecture decision recorded
Scope: ATIF v1.7 types, stored trace contract, web/mobile/desktop threads, runtime events, and public trace projection

## Decision

OpenAgents already has Effect types for the ATIF trace definition. The strict,
public-safe stored trace schema is
[`packages/atif/src/trace-schema.ts`](../../packages/atif/src/trace-schema.ts),
exported as `@openagentsinc/atif/trace`. The API worker re-exports that exact
schema from
[`apps/openagents.com/workers/api/src/atif-trace-schema.ts`](../../apps/openagents.com/workers/api/src/atif-trace-schema.ts),
and the restored `/trace/{uuid}` Start route now decodes it directly at the
browser boundary.

ATIF is not, and should not become, the canonical OpenAgents thread store.
Full ATIF v1.7 can faithfully describe a sequential agent execution, including
multimodal messages, continuations, and nested subagent trajectories. It cannot
losslessly represent the complete operational state of an OpenAgents thread:
sync cursors and revisions, mutable thread metadata, deletion/archive state,
provider continuity, streaming event lifecycles, pending approvals/questions,
event supersession/revert authority, disclosure policy, workbench artifacts,
or the live multi-agent graph.

The correct relationship is:

> OpenAgents thread/event models are canonical operational truth. ATIF is a
> versioned execution-evidence projection derived from that truth.

For trace-to-thread correlation, add a separate OpenAgents envelope or binding
schema. Do not fork ATIF or put OpenAgents-only lifecycle fields into ATIF
steps.

## Existing ATIF types

There are three deliberately different surfaces in `packages/atif`, but their
names currently make their differences easier to miss than they should be.

| Surface | Implementation | Runtime Effect schema? | Intended role | Important boundary |
| --- | --- | ---: | --- | --- |
| Stored/public trace | [`src/trace-schema.ts`](../../packages/atif/src/trace-schema.ts) | Yes, `Schema.Class` | Strict ingest/store/read contract | Pins `ATIF-v1.7`; string content only; no root/step `extra`, continuations, or subagents |
| Producer types | [`src/emit.ts`](../../packages/atif/src/emit.ts) | No, TypeScript interfaces | Dependency-free emitter target | Allows root `notes`/`extra` and some extra metrics, but still models messages as strings |
| Producer validator | [`src/validate.ts`](../../packages/atif/src/validate.ts) | Yes, `Schema.Struct` | Permissive pre-commit validation | Accepts multimodal arrays and more metrics, but is not a complete full-v1.7 schema |
| Public-safety redaction | [`src/redaction.ts`](../../packages/atif/src/redaction.ts) | Effect-backed boundary logic | Scrub before persistence | Separate from structural validity; a successful structural decode is not a publication grant |

The strict stored classes are:

- `AtifTrajectory`
- `AtifAgent`
- `AtifStep`
- `AtifToolCall`
- `AtifObservation` and `AtifObservationResult`
- `AtifStepMetrics` and `AtifFinalMetrics`
- `TraceVisibility`

`AtifTrajectory` is the model used by `POST /api/traces`,
`GET /api/traces/{uuid}`, and the restored viewer. Structural validation adds
non-empty/sequential steps, tool-result correlation, and agent-only field
rules. The tripwire separately rejects stored secret values, wallet/payment
material, local paths, and email addresses.

### Full ATIF v1.7 versus the current strict subset

The upstream Harbor RFC at the workspace reference path
`projects/repos/harbor/rfcs/0001-trajectory-format.md` defines more than the
strict OpenAgents storage shape currently admits:

- root `notes`, `extra`, `continued_trajectory_ref`, and
  `subagent_trajectories`;
- step `message` as text or typed multimodal content parts;
- step `extra`, `reasoning_effort`, `llm_call_count`, and
  `is_copied_context`;
- observation content as text or multimodal parts;
- observation `subagent_trajectory_ref` with embedded or external resolution;
- richer token, cache, RL, and per-token metrics.

This matters when someone says “ATIF supports subagents.” The format does; our
strict public store does not yet. A nested thread graph cannot be round-tripped
through the current `AtifTrajectory` class.

The comments in `packages/atif/src/trace-schema.ts` and `emit.ts` also point to
`docs/traces/README.md`, but that file is absent. The Harbor RFC is present only
in the external reference lane, not inside the `openagents` repository. The
package README is therefore the only in-repo overview today. A follow-up should
either add the missing owned trace specification or replace the stale pointer
with this audit plus a pinned upstream reference identifier.

## Thread structures elsewhere in the app

### Synced web/mobile chat

[`packages/khala-sync/src/chat.ts`](../../packages/khala-sync/src/chat.ts)
defines Effect Schema classes for `ChatThreadEntity` and `ChatMessageEntity`.
They contain facts outside ATIF:

- owner identity, title, status (`active | archived | deleted`), message count,
  and created/updated/last-message timestamps;
- optional repository binding and Codex account/auth-grant continuity refs;
- stable message IDs, author IDs, deletion timestamps, and bounded inline image
  attachments with hashes and bytes.

[`packages/khala-sync-client/src/conversation.ts`](../../packages/khala-sync-client/src/conversation.ts)
adds confirmed versions, scope cursors, live/catching-up/must-refetch states,
optimistic mutation counts, subscriptions, and a bounded 500-message retained
window. These are replication semantics, not agent-trajectory semantics.

### Desktop conversation and workbench

[`apps/openagents-desktop/src/chat-contract.ts`](../../apps/openagents-desktop/src/chat-contract.ts)
defines the Effect Schema for `DesktopThread` and `DesktopMessage`. A message can
carry typed per-turn model/token metadata, tool traces, recovery generation,
interactive provider questions, tool approvals, plan reviews, plans, child
agents, queued follow-ups, and renderer-local graph/meter projections.

[`apps/openagents-desktop/src/thread-store.ts`](../../apps/openagents-desktop/src/thread-store.ts)
also has thread-local retention, restore, rename, append/upsert/remove, and fork
semantics. A fork creates a distinct local thread; it is not merely another
sequential ATIF step.

The shared workbench vocabulary in
[`packages/ui/src/workbench/dispatch.tsx`](../../packages/ui/src/workbench/dispatch.tsx)
includes messages, reasoning, commands, file changes, tool calls, agents,
plans, approvals, meters, notices, compaction, sleep, review, and hook records.
Some can be projected into ATIF message/tool/observation fields, but several
have no lossless ATIF home.

### Runtime event log and authority

[`@openagentsinc/agent-runtime-schema` `src/index.ts`](https://github.com/OpenAgentsInc/ai/blob/main/packages/agent-runtime-schema/src/index.ts)
models an append-only lifecycle with run/step boundaries, streaming text and
reasoning deltas, tool proposal/input/approval/start/completion/failure,
external-agent lifecycle, artifacts, usage, and terminal outcomes. ATIF records
the settled execution; it is not a replay-equivalent encoding of every
intermediate runtime event.

Additional Effect schemas make the distinction explicit:

- [`live-agent-graph.ts`](https://github.com/OpenAgentsInc/ai/blob/main/packages/agent-runtime-schema/src/live-agent-graph.ts)
  owns agents, parent/child edges, current tools, and attention/blocker state.
- [`thread-event-authority.ts`](https://github.com/OpenAgentsInc/ai/blob/main/packages/agent-runtime-schema/src/thread-event-authority.ts)
  owns accepted, superseded, and reverted event relations and rejects ambiguous
  histories.
- [`thread-disclosure.ts`](https://github.com/OpenAgentsInc/ai/blob/main/packages/agent-runtime-schema/src/thread-disclosure.ts)
  owns visibility/export intents, workspace/named-group/internet audiences,
  administrator access, idempotency, expected versions, and receipts.
- [`thread-export-artifact.ts`](https://github.com/OpenAgentsInc/ai/blob/main/packages/agent-runtime-schema/src/thread-export-artifact.ts)
  compiles an exact, deterministic, owner-only canonical event bundle with
  event sequence and authority state.

These are thread truth and authority surfaces. ATIF has no field whose presence
can replace any of them.

## Representability matrix

| OpenAgents fact | Strict stored ATIF | Full ATIF v1.7 | Lossless? | Projection rule |
| --- | ---: | ---: | ---: | --- |
| User/system/agent text turns | Yes | Yes | Usually | Map settled message events to steps |
| Agent reasoning | Yes, string | Yes, string | Only if source exposes settled reasoning | Never infer missing reasoning |
| Tool calls and final observations | Yes | Yes | Usually | Preserve stable call IDs and correlate results |
| Token/cost metrics | Partial | Broad | Only measured fields | Missing remains unmeasured, never zero-filled |
| Images/multimodal message parts | No | Yes | Full-format only | Public-safe blob refs; never local absolute paths |
| Continuations/context copies | No | Yes | Full-format only | Use continuation and copied-context fields |
| Parent/subagent trajectories | No | Yes | Execution graph only | Use trajectory IDs and subagent refs, not session ID as identity |
| Thread title/archive/delete/owner | No | No | No | Envelope/thread store only |
| Sync cursor, entity version, optimistic state | No | No | No | Sync store only |
| Streaming deltas and in-flight lifecycle | No | No | No | Runtime event log only; ATIF exports settled result |
| Pending question/approval/plan review | No | No | No | Runtime interaction schema only |
| File diffs, plans, hooks, compaction, meters | No direct typed home | `extra` can carry opaque data | No common semantics | Keep canonical workbench events; optionally summarize |
| Fork ancestry and mutable branch topology | No | Subagents are not thread forks | No | Typed OpenAgents relationship refs |
| Accepted/superseded/reverted authority | No | No | No | Thread event authority schema only |
| Workspace/group/admin disclosure policy | No | No | No | Thread disclosure schema only |
| Stable event bundle for exact replay/search | No | No | No | `openagents.thread_export_artifact.v1` |

## Can every OpenAgents thread be represented as ATIF as-is?

Three answers are needed because “represented” is ambiguous:

1. **Readable transcript:** mostly yes after projection. Ordinary user and
   assistant text can become ATIF steps, with tool calls and final observations
   attached.
2. **Execution evidence:** yes for a settled linear agent run; full ATIF v1.7
   can also link continuations and nested subagents. The current strict store
   needs widening before it can carry those full-format features.
3. **Lossless thread round-trip:** no. Re-importing ATIF cannot reconstruct the
   canonical OpenAgents thread, sync state, live interactions, event authority,
   disclosure policy, or full workbench history.

Using ATIF as the primary thread model would either discard those facts or push
them into untyped `extra` bags. Both outcomes weaken existing Effect schemas and
authority boundaries.

## Recommended extension boundary

Keep ATIF byte-compatible and introduce a separate, versioned Effect Schema
binding rather than changing `ATIF-v1.7`:

```ts
OpenAgentsAtifThreadBindingV1 {
  schema: "openagents.atif_thread_binding.v1"
  bindingRef: Ref
  threadRef: Ref
  trajectoryId: string
  sourceEventRange: {
    firstSequence: number
    lastSequence: number
    eventRefsSha256: string
  }
  relationship?:
    | { kind: "root" }
    | { kind: "continuation"; previousTrajectoryId: string }
    | { kind: "delegated"; parentTrajectoryId: string; parentEventRef: Ref }
  canonicalExportArtifactRef?: Ref
  authoritySnapshotRef?: Ref
  disclosureReceiptRef?: Ref
}
```

The binding should contain refs and digests, not raw duplicate thread content.
The trajectory remains a normal ATIF document. The canonical event bundle
remains separately owner-authorized. Public trace publication can expose only a
redacted binding subset; the existence of a binding grants no disclosure,
acceptance, payout, or claim authority.

For a multi-agent thread, publish a bundle of ATIF trajectories plus bindings.
Use full ATIF `subagent_trajectories` when a parent execution truly delegated a
child run. Use OpenAgents relationship refs for UI forks, provider continuity,
or other thread topology that is not an ATIF subagent execution.

## Recommended follow-up sequence

1. Name the schema tiers explicitly: strict public stored trajectory,
   producer trajectory, and complete ATIF v1.7. Add a conformance matrix so
   widening one tier cannot silently drift the others.
2. Add an owned Effect Schema for the complete public-safe ATIF v1.7 features
   OpenAgents intends to support, especially multimodal content, continuation,
   copied context, and subagent refs/trajectories. Keep the current strict
   decoder available until stored-row compatibility is proven.
3. Add `OpenAgentsAtifThreadBindingV1` with ref/digest-only safety checks and
   tests that reject raw bodies, prompts, tokens, paths, and ambiguous ranges.
4. Implement a deterministic projector from accepted canonical thread events
   to ATIF plus binding. Its tests should prove stable IDs, ordering,
   tool-observation correlation, missing-metric behavior, and redaction before
   persistence.
5. Keep reverse import explicitly non-authoritative. Imported ATIF may create a
   new evidence view or draft transcript, never mutate an existing canonical
   thread or synthesize accepted/reverted/disclosure state.

## Route consequence

The restored `/trace/{uuid}` route is correctly scoped as a projection viewer:
it decodes `@openagentsinc/atif/trace`, renders the settled trajectory and
media, propagates only the owner read token to visibility-gated reads, and
states the all-false authority contract. It does not pretend the trace is the
thread database. The owner-scoped historical list remains the separate
`GET /traces` / `GET /api/traces` surface; a curated public historical feed is
not implemented by this restoration.
