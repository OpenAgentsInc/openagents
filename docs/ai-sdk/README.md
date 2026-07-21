# The OpenAgents AI SDK

**The product sentence.** The OpenAgents AI SDK is an Effect-native toolkit
for building agent applications with durable, cursor-exact streams — one
neutral event union from the model call to the rendered message, with
suspend and continue that actually persists, coding-agent harnesses,
redaction as a schema field, and recall instead of compaction.

**Status.** Staged incubation inside this monorepo per the decision in
[`../fable/2026-07-21-effect-native-openagents-ai-sdk-analysis.md`](../fable/2026-07-21-effect-native-openagents-ai-sdk-analysis.md)
(AISDK epic #9146). The public identity is npm (`@openagentsinc/ai` and the
roster below), not a repo boundary. The standalone repo
`OpenAgentsInc/openagents-ai` is reserved and stays a shell until the
extraction gate opens (see the criteria at the bottom).

## The layer diagram

```
L6  RECALL        @openagentsinc/history-corpus
                  corpus export, cursor-addressed entries, HistoryRecall,
                  Tier D deterministic recall, Effect-native recursive recall
------------------------------------------------------------------
L5  UI STREAM     agent-harness-contract: ui-message-chunk, ui-message-reducer,
                  smooth-stream, partial-object-stream
                  [pending: ChatTransport, STREAM-03 #9131]
------------------------------------------------------------------
L4  HARNESS       agent-harness-contract: AgentHarness adapter, session verbs,
                  capability-by-method-presence, slice runner, readiness
                  projection, skills, host tools, toolkit bridge,
                  ACP + opencode adapters
------------------------------------------------------------------
L3  SANDBOX       harness sandbox-provider contract, local-process provider,
                  @openagentsinc/ai-sdk-sandbox-local,
                  @openagentsinc/ai-sdk-sandbox-openagents,
                  @openagentsinc/managed-sandbox-contract (server authority)
------------------------------------------------------------------
L2  DURABLE LOG   agent-harness-contract: event-log + event-log-store —
                  seq-cursor append, replay, live attach, rerun boundaries
------------------------------------------------------------------
L1  VOCABULARY    @openagentsinc/agent-runtime-schema — KhalaRuntimeEvent
                  (one neutral event union, sequence = the durable cursor,
                  visibility + redactionClass + causalityRefs),
                  RuntimeInteraction, route schemas
------------------------------------------------------------------
L0  MODEL CALL    effect/unstable/ai (upstream, consumed, never forked) +
                  @openagentsinc/khala-ai-sdk-core — the LanguageModel Layer,
                  bidirectional StreamPart maps, ExecutionPlan fallback
```

**The one rule.** Every layer speaks `KhalaRuntimeEvent` upward. L0 maps
provider parts into it. L2 persists it. L4 emits it. L5 projects it to
renderable chunks. L6 exports it to a corpus. One event union. One durable
cursor.

## The roster

| Package | Layer | Publish |
| --- | --- | --- |
| `@openagentsinc/agent-runtime-schema` | L1 vocabulary | Yes — the foundation |
| `@openagentsinc/agent-harness-contract` | L2–L5 | Yes — the core |
| `@openagentsinc/khala-ai-sdk-core` | L0 bridge | Yes — neutral rename pending (AISDK-03 #9149) |
| `@openagentsinc/history-corpus` | L6 recall | Yes |
| `@openagentsinc/ai-sdk-sandbox-local` | L3 interop | Yes |
| `@openagentsinc/ai-sdk-sandbox-openagents` | L3 interop | Yes |
| `@openagentsinc/ai` | umbrella | Yes — curated re-exports (AISDK-02 #9148) |
| `@openagentsinc/agent-turn-runtime` | kernel | Candidate, second wave |
| `@openagentsinc/khala-tools` | tool registry | Candidate, second wave |

**App-internal, never SDK surface:** the desktop wiring
(`harness-projection.ts`, `harness-event-recorder.ts`, the Provider Lane
SPI, Full Auto orchestration, `ClaudeLocalEvent`, the Runtime Gateway),
`harness-conformance` (private fleet vocabulary — the public seam is
AISDK-05 #9151), Apple FM guided routing, account custody, the usage
ledger, and every settlement-bearing surface. Full Auto authority — leases,
the eight-run cap, journals, receipts — never leaks into a public package.

## What this SDK offers that neither upstream offers

- Durable cursor-exact streams — attach, replay, and rerun boundaries over
  a persisted seq-cursor log, not a best-effort in-memory bridge.
- Suspend and continue that persists — a turn freezes at an exact cursor
  and resumes in a different process with no gap and no duplicate.
- Coding-agent harnesses — Codex, Claude Code, ACP peers, and opencode as
  adapters behind one versioned contract with capability-by-method-presence.
- Redaction as a schema field — `visibility` and `redactionClass` gate
  every projection, so a public surface cannot widen what it sees.
- Recall instead of compaction — the full history stays durable and a
  typed recall service traverses it, deterministically first and
  recursively second.
- Honest failure vocabulary — typed model errors map onto operator-facing
  failure classes, and a fallback never launders an exhausted account.

## Consuming the SDK

Inside the monorepo, depend on the packages with `workspace:*`. Outside,
install from npm once the rc train publishes (AISDK-04 #9150). The umbrella
`@openagentsinc/ai` fronts the layer entry points with curated re-exports.
The runtime map for the desktop consumer is
[`../desktop/2026-07-21-openagents-desktop-chat-runtime-reference.md`](../desktop/2026-07-21-openagents-desktop-chat-runtime-reference.md)
and the program sequencing is
[`../desktop/2026-07-21-chat-runtime-unified-roadmap.md`](../desktop/2026-07-21-chat-runtime-unified-roadmap.md).

## The extraction gate (the standing AISDK-07 #9153 review)

Extraction to `OpenAgentsInc/openagents-ai` becomes right when ALL five
hold:

1. Effect v4 is stable — no beta pin, so a two-repo version story does not
   double beta-upgrade work.
2. The API surface is quiet — STREAM-03 landed, the naming pass done, and
   one release cycle without a breaking export change.
3. A second consumer exists — a sibling repo, an external user, or the
   mobile app consuming via npm rather than `workspace:*`.
4. The private-seam question is resolved — the failure-class boundary has a
   public-safe answer, so the extracted repo needs no private sibling.
5. The owner accepts the split — repo creation and the workspace contract
   change are owner-visible decisions.

Until all five hold, extraction would trade real velocity for aesthetic
separation. When they hold, AISDK-07 produces the extraction receipt: the
move set, a history-preserving migration, the catalog pin swap, and a
consumer conformance test, taken to the owner.
