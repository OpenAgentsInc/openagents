# Chat Runtime Unified Roadmap — AI SDK Harvest + RLM Recall

**Date:** 2026-07-21
**Lane:** coordination record for the owner-approved chat-runtime program
**Status:** This document coordinates the owner-approved epics into one
sequence. It flips no promise state and changes no runtime authority.
`docs/sol/MASTER_ROADMAP.md` remains the canonical repository sequencing
authority — this document is subordinate to it and to live issue state.
**Sources:**
[`2026-07-21-openagents-desktop-chat-runtime-reference.md`](./2026-07-21-openagents-desktop-chat-runtime-reference.md)
(the runtime map),
[`../fable/2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md`](../fable/2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md)
(the STREAM plan),
[`../rlm/2026-07-21-rlm-integration-audit-and-roadmap.md`](../rlm/2026-07-21-rlm-integration-audit-and-roadmap.md)
(the RLM plan),
[`../fable/2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md`](../fable/2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md)
(the shipped harness harvest).

---

## 1. The one-paragraph picture

The chat runtime now stands on a shipped neutral substrate — the
`AgentHarness` contract, the durable cursor-exact event log, the slice
runner, the readiness projection, and the live recorder
(`packages/agent-harness-contract`, HARN epic #9115, closed). Two programs
build the next layer on that substrate. The **STREAM** program (epic #9128)
adds the missing live-to-UI layer: one Effect pipeline from the neutral
`KhalaRuntimeEvent` stream to a rendered, streaming message — projection,
pace, reduce, transport — plus the `effect/unstable/ai` model-call substrate.
The **RLM** program (epic #9136) adds memory without compaction: the durable
log and thread stores become a corpus that a recall service traverses — 
deterministic first, recursive-model second — so a turn can ask the past a
question instead of drowning in a bounded prompt window. Issue #9127 promotes
the single-delegate answer to the primary assistant bubble. Together these
give the runtime what the owner asked for — real streaming, real message
rendering, and real history.

## 2. Shipped baseline (done, on `main`)

| What | Where | Proof |
| --- | --- | --- |
| Harness adapter contract, session verbs, capability-by-method | `packages/agent-harness-contract` | HARN-01 #9116 (closed) |
| Durable seq-cursor event log — attach, replay, rerun | same package | HARN-02 #9117 (closed) |
| ClaudeLocalEvent → KhalaRuntimeEvent projection, live-wired recorder | `apps/openagents-desktop/src/harness-projection.ts`, `harness-event-recorder.ts` | HARN-03 #9118 (closed) |
| ACP + opencode harness adapters | same package | HARN-04 #9119, HARN-08 #9123 (closed) |
| Unified readiness projection feeding the Apple FM candidate set | `harness-readiness-source.ts` | HARN-05 #9120 (closed) |
| Slice runner + cursor liveness | same package | HARN-06 #9121 (closed) |
| Sandbox-provider contract + real local-process provider | same package | HARN-07 #9122 (closed) |
| Delegate answers stream token-by-token, delegated turns carry history | `desktop-codex-provider.ts`, `main.ts` | commit `91de284512` |

## 3. The STREAM program — epic #9128

Live-to-UI streaming. Full detail in the
[streaming harvest audit](../fable/2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md) §5–§6.

| Issue | Packet | Layer |
| --- | --- | --- |
| #9129 | STREAM-01 — `effect/unstable/ai` model-call substrate, `AiError` → failure classes | below the harness |
| #9130 | STREAM-02 — `UiMessageChunk` projection + `SubscriptionRef` reducer | the core |
| #9131 | STREAM-03 — `ChatTransport` over the harness event log — IPC + SSE Layers, resume at cursor | transport |
| #9132 | STREAM-04 — `smoothStream` operator | pace |
| #9133 | STREAM-05 — `ExecutionPlan` in-lane fallback evaluation (advisory) | beside |
| #9134 | STREAM-06 — partial-object streaming for guided output | structured |
| #9135 | STREAM-07 — harness host-tools ↔ Effect `Toolkit` reconciliation | tools |

## 4. The RLM program — epic #9136

History without compaction. Full detail in the
[RLM audit](../rlm/2026-07-21-rlm-integration-audit-and-roadmap.md) §4–§8.

| Issue | Packet | Tier |
| --- | --- | --- |
| #9137 | RLM-01 — `HistoryCorpus` deterministic export | corpus |
| #9138 | RLM-02 — Tier D deterministic recall, cited spans | zero model calls |
| #9139 | RLM-03 — `history_recall` host tool on lanes + kernel | wiring |
| #9140 | RLM-04 — sandboxed Python `rlms` leaf executor pilot | Tier S core |
| #9141 | RLM-05 — Tier S semantic recall behind `HistoryRecall` | semantic |
| #9142 | RLM-06 — Full Auto long-run recall consumer | consumer |
| #9143 | RLM-07 — dense-recall evaluation harness + honesty gate | evidence |
| #9144 | RLM-08 — managed-sandbox + evidence-pack extension | DEFERRED |

## 5. Renderer promotion — #9127

The single-delegate answer becomes the primary streaming assistant bubble
with honest attribution. The "Worked" group keeps activity detail. Depends on
the shipped streaming fix and composes with STREAM-02/03 once those land.

## 6. Sequencing — the waves

Dependencies drive the order. Independent packets run in parallel.

- **Wave 1 (parallel, in flight):** STREAM-01 #9129, STREAM-02 #9130,
  STREAM-04 #9132, STREAM-06 #9134, RLM-01 #9137, and #9127. All are pure
  additions or bounded renderer work with no shared files.
- **Wave 2 (after wave 1 lands):** STREAM-03 #9131 (needs STREAM-02 — 
  desktop IPC path, full desktop gate), STREAM-05 #9133 (needs STREAM-01),
  STREAM-07 #9135 (needs STREAM-02 chunks), RLM-02 #9138 and RLM-04 #9140
  (need RLM-01).
- **Wave 3:** RLM-03 #9139 (needs RLM-02 — desktop wiring, coordinates with
  STREAM-07), RLM-05 #9141 (needs RLM-03 + RLM-04).
- **Wave 4:** RLM-06 #9142 and RLM-07 #9143 (need RLM-05).
- **Deferred:** RLM-08 #9144 — requires its own cloud admission. Not in the
  current program.

## 7. Standing guardrails (every wave)

- The desktop gate (typecheck, full test suite, build, electron smoke, react
  smoke) is the bar for any `apps/openagents-desktop` change.
- `KhalaRuntimeEvent` stays the single neutral vocabulary and durable cursor.
  `ClaudeLocalEvent` stays the renderer behavior-contract surface until a
  deliberate, contract-covered cutover.
- Full Auto keeps durable authority — leases, the eight-run cap, journals,
  receipts, account custody. Nothing in either program moves it.
- The Apple FM advisory-recommendation versus authoritative-decision split is
  untouched. Recall output and stream projections are never authority.
- Raw history and raw events never leave owner-local execution. Redaction
  classes gate every projection and every corpus.
- No vendoring — ideas re-derived from the AI SDK and the RLM paper. No
  runtime dependency on `@ai-sdk/*` in the desktop main process.
