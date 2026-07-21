# AI SDK Extraction — Division of Labor and Status Audit

**Date:** 2026-07-21 (evening)
**Lane:** Fable strategy analysis
**Status:** Status record and coordination analysis. This document flips no
promise state and dispatches no work. Factual authorities remain current code
in both repositories, `docs/sol/MASTER_ROADMAP.md`, live issue state, and
receipts.
**Repositories:** `OpenAgentsInc/openagents` (the product monorepo) and
`OpenAgentsInc/ai` (the extracted Effect-native OpenAgents AI SDK,
Apache-2.0, npm scope `@openagentsinc/*`).

---

## 1. Where things stand (one day, end to end)

The whole arc landed on 2026-07-21, across two owner sessions and several
agents working in parallel:

1. **The harness layer** (HARN epic #9115, closed) — the `AgentHarness`
   contract, durable seq-cursor event log, slice runner, readiness
   projection, sandbox providers, ACP + opencode adapters, live desktop
   recorder.
2. **The streaming layer** (STREAM epic #9128, closed) — the
   `effect/unstable/ai` substrate, `UiMessageChunk` projection +
   `SubscriptionRef` reducer, `ChatTransport` over the event log,
   smoothStream, partial-object streaming, `ExecutionPlan` in-lane fallback,
   the Toolkit bridge.
3. **The recall layer** (RLM epic #9136, open tail) — `HistoryCorpus`,
   Tier D deterministic recall, the `history_recall` host tool on the lanes
   and kernel, and the Effect-native recursive engine — now a first-class
   **`@openagentsinc/rlm`** package (SDK-RLM-01..08 + 04A in the SDK repo,
   owner override honored: no Python anywhere).
4. **The extraction** (AISDK epic #9146, closed; owner override of the
   staged-incubation plan) — the SDK lives at `OpenAgentsInc/ai`
   (Apache-2.0, restored to the monorepo too at `f658d6aa91`), Vite Plus
   toolchain, rc trains published (`0.1.1-rc.1` → `0.2.0-rc.1`), and the
   monorepo **consumes it from npm** (`314a14da78`) — the seven in-tree
   packages are deleted.
5. **The product wins** — "Stand by." is dead: the `hosted_khala` provider
   streams `openagents.com/api/khala/chat` as the always-ready fallback
   (#9145), the single-delegate answer streams into the primary assistant
   bubble with honest attribution (#9127), delegated turns carry
   conversation history, and the desktop cut **version 0.1.0, first
   stable** (`26d1627722`).

## 2. Division of labor — what lives where

### The SDK repo (`OpenAgentsInc/ai`) owns

- **L0–L6 of the agent stack**: `agent-runtime-schema` (the
  `KhalaRuntimeEvent` vocabulary and durable cursor),
  `agent-harness-contract` (event log, harness, ui-stream, sandbox seams,
  toolkit bridge, transports), `ai-model` (the `LanguageModel` bridge and
  `ExecutionPlan` fallback), `history-corpus` (corpus + recall contracts),
  `rlm` (the recursive engine), the sandbox interop packages, and the
  `@openagentsinc/ai` umbrella.
- **Public API discipline**: rc trains, export-map audits, conformance
  fixtures, the repo-local gates.
- **Generic engine work**: anything a non-OpenAgents consumer could use
  unchanged.

### The monorepo (`OpenAgentsInc/openagents`) owns

- **Every consumption surface**: the desktop lanes and dispatch, the Apple
  FM router and guided generation, the `hosted_khala` provider, the
  `history_recall` desktop wiring, renderer transcript and promotion, the
  Runtime Gateway.
- **All authority**: Full Auto leases, the eight-run cap, journals,
  receipts, account custody, the exact usage ledger, settlement — none of
  this ever enters the SDK.
- **The private fleet vocabulary**: `harness-conformance` stays private and
  consumes the public `ModelFailureClass` seam (AISDK-05).
- **Product surfaces**: openagents.com, the `/aisdk` page, docs serving,
  promises, release trains.

### The standing rules

1. The SDK never gains authority, custody, settlement, or app wiring.
2. The monorepo never re-grows an in-tree copy of an SDK package — gaps go
   upstream as SDK issues, then a new rc train, then a version bump in the
   monorepo (`ai#2` established the pattern).
3. Breaking SDK export changes require a train bump the monorepo adopts in
   one commit (`#9154` is the receipt of the first such migration).
4. Engine issues are filed on `OpenAgentsInc/ai`. Consumption and product
   issues stay on the monorepo. Cross-cutting programs keep one epic on the
   monorepo with SDK child issues linked.
5. Version skew is temporary and tracked — as of this audit one straggler
   pin (`agent-runtime-schema@0.1.3-rc.1` beside the `0.2.0-rc.1` train)
   should converge on the next bump.

## 3. The ambitious next additions (owner ask: be ambitious)

Ranked by leverage, each a candidate SDK program (engine side) with a thin
monorepo consumption lane:

1. **Programs — our version of DSPy, Effect-native.** Typed
   prompt-programs: signatures as Effect Schemas, modules as composable
   typed programs over `LanguageModel`, and an optimizer harness
   (GEPA-style reflective/evolutionary search over prompt text and program
   structure, Pareto-scored by eval contracts). The monorepo pruned its
   DSPy/GEPA lineage on 2026-02-25 (`backroom` archives it) — this revives
   the idea on the clean substrate: programs emit `KhalaRuntimeEvent`
   streams, optimizers are bounded loops with the same caps/honesty
   contract as RLM, and MemoHarness policy bundles become an optimizer
   consumer. This is the largest differentiator no upstream ships.
2. **RLM deepening.** Multi-corpus recall (thread sets, repos, evidence
   packs), corpus indexing for Tier D speed, the RLM-07 evaluation harness
   as a public conformance suite, and — gated on that evidence — a trained
   orchestrator model admission path. Repo-scale corpora ("ask the
   codebase") is the obvious productization.
3. **Generative UI.** A schema-constrained UI-spec layer in the
   `UiMessageChunk` pipeline (json-render-style typed component catalogs) so
   agents stream typed UI, not just text — composes with partial-object
   streaming and Apple FM guided generation.
4. **Transport productization.** The web SSE `ChatTransport` Layer hardened
   for the Cloud Run monolith (resume-at-cursor over the public API),
   browser reducer bindings, and reconnect conformance fixtures.
5. **Provider gateway.** A registry/gateway Layer over `Model`s with
   `ExecutionPlan` policies per model class — the Vercel gateway idea
   re-derived as Layers, feeding the monorepo's inference mix honestly.
6. **Multi-harness Full Auto.** The monorepo residual: collapse the
   dispatch switch onto harness adapters so Full Auto rotates ANY admitted
   adapter (Codex, Claude, ACP peers, opencode) with suspend/continue and
   the durable log underneath — the HARN-03/06 close-out, now trivially
   nearer since the adapters live in the SDK.

## 4. Next actions toward Full Auto with RLMs and multiple harnesses

In flight or queued on the monorepo:

- **RLM-05 #9141** (in flight) — Tier S behind `HistoryRecall` via
  `@openagentsinc/ai`'s rlm engine: tier policy, citations, exact usage.
- **RLM-06 #9142** — the Full Auto long-run consumer: run-scoped corpus at
  continuation framing, recall failure never stalls a run, guardrails
  untouched.
- **RLM-07 #9143** — the dense-recall evaluation and honesty gate (the
  evidence that decides Tier S automation and any trained-model step).
- **Dispatch collapse** — route the live lanes through the SDK harness
  adapters (the HARN-03 residual), making Full Auto genuinely
  multi-harness.
- **`/aisdk` page + docs** (in flight) — the public page on
  openagents.com, then deploy.
- **RLM-08 #9144** stays deferred (separate cloud admission).

The SDK-side roadmap is maintained in the SDK repo
(`OpenAgentsInc/ai` `docs/ROADMAP.md`) — this audit and that roadmap
crosslink and must not drift.

## 5. Receipts index

Monorepo: `91de284512` (streaming/history fix), `f67b1fc26c` (#9127
promotion), `c34e561923` (#9145 hosted_khala), `3d15a41e94` (rename),
`010ea7c74c` (STREAM-03), `75e4cabfe6` (RLM-03), `314a14da78` (npm
consumption), `8998ddefae` (#9154 rlm train), `f658d6aa91` (Apache-2.0
restore), `26d1627722` (desktop 0.1.0). SDK repo: `adaa08e` (extraction),
`08ccd05`/`e9e28d5`/`103f32f` (rc trains), `b7bba33` (first-class rlm).
Epics: #9115 ✅, #9128 ✅, #9146 ✅, #9136 (tail open), ai#1–3 ✅,
SDK-RLM epic ai#4 ✅.
