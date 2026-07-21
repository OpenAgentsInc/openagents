# The OpenAgents AI SDK — An Effect-Native SDK Analysis And The Repo Placement Decision

**Date:** 2026-07-21
**Lane:** Fable strategy analysis
**Status:** Analysis and recommendation only. This document flips no promise
state, changes no runtime authority, mints no issue, creates no repository,
and dispatches no work. Factual status authorities remain current code,
`docs/sol/MASTER_ROADMAP.md`, live issue state, contracts, and receipts. The
proposal packets named below require Sol admission and owner acceptance
before any dispatch.
**Companions:**
[`2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md`](./2026-07-21-ai-sdk-and-effect-ai-streaming-harvest-audit.md)
(the STREAM-01..07 streaming harvest),
[`2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md`](./2026-07-20-ai-sdk-harness-abstraction-harvest-analysis.md)
(the HARN harvest that produced `packages/agent-harness-contract`),
[`../desktop/2026-07-21-chat-runtime-unified-roadmap.md`](../desktop/2026-07-21-chat-runtime-unified-roadmap.md)
(the STREAM and RLM wave sequencing),
[`../sol/2026-07-18-vercel-ai-sdk-source-derived-effect-conversion-audit.md`](../sol/2026-07-18-vercel-ai-sdk-source-derived-effect-conversion-audit.md)
(the earlier reuse-not-duplicate posture).
**Sources:** the shipped packages `packages/agent-harness-contract`,
`packages/agent-runtime-schema`, `packages/khala-ai-sdk-core` (including the
STREAM-01 `effect-ai.ts` module on `origin/main` commit `38b28b8d97`),
`packages/history-corpus`, `packages/harness-conformance`,
`packages/ai-sdk-sandbox-local`, `packages/ai-sdk-sandbox-openagents`, the
Vercel AI SDK reference clone `~/work/projects/repos/ai` (read-only, ideas
re-derived, no code vendored), Effect `4.0.0-beta.94` `effect/unstable/ai`,
the workspace root contract at `/Users/christopherdavid/work/CLAUDE.md`
(the `nostr-effect`, `three-effect`, and `effect-native` precedents), and
`docs/effect-native/README.md`.

---

## 0. Purpose and the one-sentence conclusion

The question: OpenAgents has now shipped a real, coherent set of
Effect-native AI runtime packages — the harness contract, the durable event
log, the UI message stream, the model-call substrate over
`effect/unstable/ai`, and the history corpus. Should this become a named,
possibly public **OpenAgents AI SDK**, and should that SDK live in the
monorepo under `packages/`, or in a standalone repo like `nostr-effect`,
`three-effect`, and `effect-native`?

The one-sentence conclusion: **yes, name it and publish it — and incubate it
in the monorepo now, with a reserved standalone repo and explicit extraction
criteria, because the SDK is the live substrate of the fast-moving desktop
app, its API is mid-churn on an Effect beta pin, its only real consumers are
in this repo, and the workspace's own `effect-native` precedent shows that
the honest pattern is design-and-code in the monorepo first, standalone repo
when the contract is stable.**

---

## 1. What the OpenAgents AI SDK is

### 1.1 The coherent product

The SDK is not a plan. Most of it is shipped code on `main`. The product
statement is one paragraph:

> The OpenAgents AI SDK is an Effect-native toolkit for building durable
> agent applications. It consumes `effect/unstable/ai` for the model call.
> It adds what neither Effect nor the Vercel AI SDK provides — a neutral
> runtime event vocabulary with visibility and redaction classes, a durable
> cursor-exact event log with suspend and continue, a versioned harness
> contract that drives third-party coding-agent runtimes (Codex, Claude
> Code, ACP peers, opencode) behind one surface, a Schema-encodable UI
> message stream with a progressive reducer and a tool-call state machine,
> sandbox-provider seams, a unified readiness projection, and recall over
> history instead of compaction.

### 1.2 The layer diagram

Every layer below already exists on `main` except the transport layer
(STREAM-03 #9131, wave 2).

```
L6  RECALL        @openagentsinc/history-corpus
                  corpus export, cursor-addressed entries, HistoryRecall
                  contract, Tier D deterministic recall (RLM-01/02)
------------------------------------------------------------------
L5  UI STREAM     agent-harness-contract/ui-message-chunk + ui-message-reducer
                  + smooth-stream + partial-object-stream (STREAM-02/04/06)
                  [pending: ChatTransport, STREAM-03 — IPC + SSE Layers]
------------------------------------------------------------------
L4  HARNESS       agent-harness-contract — AgentHarness adapter, session
                  verbs (promptTurn / suspendTurn / continueTurn / compact /
                  detach / stop / destroy), capability-by-method-presence,
                  slice runner, readiness projection, skills, host tools,
                  toolkit bridge (STREAM-07), ACP + opencode adapters
------------------------------------------------------------------
L3  SANDBOX       harness sandbox-provider contract + local-process provider,
                  @openagentsinc/ai-sdk-sandbox-local,
                  @openagentsinc/ai-sdk-sandbox-openagents,
                  @openagentsinc/managed-sandbox-contract (server authority)
------------------------------------------------------------------
L2  DURABLE LOG   agent-harness-contract/event-log + event-log-store —
                  seq-cursor append, replay, live attach, rerun boundaries
------------------------------------------------------------------
L1  VOCABULARY    @openagentsinc/agent-runtime-schema — KhalaRuntimeEvent
                  (the single neutral event union, sequence = durable
                  cursor, visibility + redactionClass + causalityRefs),
                  RuntimeInteraction, route schemas, AI SDK ingestion parts
------------------------------------------------------------------
L0  MODEL CALL    effect/unstable/ai (UPSTREAM, consumed, never forked) —
                  LanguageModel, Response.StreamPart, Tool/Toolkit, Model,
                  AiError, ExecutionPlan, Chat
                  + @openagentsinc/khala-ai-sdk-core — the LanguageModel
                  Layer over the existing transport, bidirectional
                  StreamPart maps (STREAM-01)
```

The one-directional rule that makes this an SDK rather than a pile: every
layer speaks `KhalaRuntimeEvent` upward. L0 maps provider parts into it. L2
persists it. L4 emits it. L5 projects it to renderable chunks. L6 exports it
to a corpus. There is exactly one event union and one durable cursor.

### 1.3 Publishable surface versus app-internal

The SDK surface is the set of packages that are already `private: false`
with `publishConfig.access: public` under the `@openagentsinc/` scope:

| Package | SDK layer | Publish |
| --- | --- | --- |
| `agent-runtime-schema` | L1 vocabulary | Yes — the foundation type |
| `agent-harness-contract` | L2–L5 | Yes — the core of the SDK |
| `khala-ai-sdk-core` | L0 bridge | Yes, after a naming pass (§2.4) |
| `history-corpus` | L6 | Yes |
| `ai-sdk-sandbox-local` / `ai-sdk-sandbox-openagents` | L3 interop | Yes — the honest Vercel interop seam |
| `agent-turn-runtime` | kernel above L4 | Candidate, second wave |
| `khala-tools` | tool registry | Candidate, second wave (Khala coupling) |

App-internal, never SDK surface:

- The desktop wiring — `harness-projection.ts`, `harness-event-recorder.ts`,
  the Provider Lane SPI, Full Auto orchestration, `ClaudeLocalEvent`, the
  Runtime Gateway. These are the desktop's consumption of the SDK, and Full
  Auto authority (leases, the eight-run cap, journals, receipts) must never
  leak into a public package.
- `harness-conformance` — deliberately `private: true`. It owns
  `HarnessFailureClass` and the fleet failure vocabulary. The STREAM-01
  module comment already records the boundary decision: the `AiError` to
  failure-class mapping lives in the private package precisely because
  `khala-ai-sdk-core` publishes to npm. That split is a live seam the SDK
  must resolve deliberately (packet AISDK-05).
- `apple-fm-runtime` guided routing, account custody, the usage ledger, and
  every settlement-bearing surface.

### 1.4 What the SDK offers that neither upstream offers

This is the reason a named SDK is worth having at all. Against
`effect/unstable/ai`:

- Effect AI stops at `Stream<Response.StreamPart>`. It has no UI chunk
  protocol, no progressive message reducer, no transport, no resume path,
  no SSE encoding. The streaming harvest audit verified this by directory
  search.
- Effect AI has no coding-agent runtime concept. `LanguageModel` is one
  request or stream. It cannot express a Codex session with native history,
  suspend and continue, or its own tool execution.
- Effect AI has no durable event log, no replay cursor, no redaction or
  visibility model, and no readiness or capacity projection.

Against the Vercel AI SDK:

- The Vercel harness hands resume state to the caller and hopes. OpenAgents
  persists it — the durable seq-cursor log with attach, replay, and rerun
  boundaries is a strictly stronger guarantee than the AI SDK bridge's
  in-memory per-turn log.
- The Vercel SDK has no neutral event union with `visibility`,
  `redactionClass`, and `causalityRefs`. Public-safety is a first-class
  field on every event in this SDK, not an afterthought at the edge.
- The Vercel SDK has no recall story. Its answer to long history is
  compaction. The OpenAgents answer is a cursor-addressed corpus plus a
  recall contract with cited spans and an honesty record.
- The Vercel SDK assumes sandbox-always cloud posture. This SDK carries the
  owner-local danger profile as explicit policy with the sandbox provider
  optional per session.
- The Vercel SDK is Promise-and-callback shaped. This SDK is Effect end to
  end — typed errors, interruption, `Layer` provider swap, `Stream`,
  `SubscriptionRef`, Schema at every boundary.

The differentiator sentence for a README: **durable, redaction-aware,
cursor-exact agent streams with coding-agent harnesses and recall — on
Effect.**

---

## 2. Relationship to upstream

### 2.1 Consume `effect/unstable/ai` — never fork it

STREAM-01 already set the posture in code. `khala-ai-sdk-core/effect-ai.ts`
satisfies the Effect AI `LanguageModel` service with a Layer over the
existing transport and maps `Response.StreamPart` bidirectionally onto the
ingestion vocabulary. The SDK treats Effect AI as its L0 and rides the
catalog pin (`effect: 4.0.0-beta.94` today). When Effect promotes the
package out of `unstable`, the SDK follows the pin. The SDK never
re-implements `LanguageModel`, `Tool`, `Toolkit`, `AiError`, `Model`,
`ExecutionPlan`, or `Chat`. The 2026-07-18 conversion audit already rejected
parallel `effect-ai-schema` or `effect-ai-core` packages. That rejection
stands.

### 2.2 Re-derive Vercel AI SDK ideas — never vendor

The shipped posture is also already set. The harness contract README states
it plainly: ideas re-derived, no upstream code vendored, no runtime
dependency on `@ai-sdk/harness`. Apache-2.0 would permit source ingestion
with notices, and the 2026-07-18 audit documents the exact conditions. The
engineering judgment stays the same: upstream harness velocity (~115
commits in five weeks, explicit breaking-change warnings) makes a tracking
fork expensive, and every copied line is local maintenance surface. The two
honest interop seams remain the `HarnessV1SandboxProvider` implementations
(`ai-sdk-sandbox-local`, `ai-sdk-sandbox-openagents`) and the `ai` package
dependency inside `khala-ai-sdk-core` for the wire transport. Both are
boundaries, not architecture.

### 2.3 Upstream candidates — what OpenAgents should offer to Effect

Effect deliberately stops at the response-part stream. Some of what
OpenAgents built below the app layer is generic enough to offer upstream,
which shrinks the long-term owned surface:

- **The smooth-stream pacing operator** (STREAM-04). Pure, small, generic
  over any delta stream. A natural `effect/unstable/ai` or Stream-utility
  contribution.
- **Partial-object streaming** (STREAM-06). Progressive partial decodes
  with a guarded finalize. Effect AI owns the terminal `generateObject`
  decode and lacks exactly this half.
- **A UI message chunk protocol and reducer.** This is a larger
  conversation. If Effect ever wants a `useChat` equivalent, the
  Schema-encodable chunk union and the `SubscriptionRef` reducer are the
  shape. OpenAgents should keep its redaction-aware projection either way,
  because `visibility` gating is an OpenAgents policy concern.

Never upstream: `KhalaRuntimeEvent` (it carries OpenAgents policy fields),
the harness contract (it encodes OpenAgents authority boundaries), the
recall contract, and anything touching readiness, capacity, custody, or
settlement.

### 2.4 The npm name story

The repository rule is fixed: the scope is `@openagentsinc/` and never
`@openagents/`, publishes follow `apps/pylon/docs/npm-publishing-runbook.md`
(pack plus tarball publish, `rc` dist-tag for pre-stable). Within that:

- The granular packages keep their names. They are already coherent —
  `@openagentsinc/agent-harness-contract`,
  `@openagentsinc/agent-runtime-schema`, `@openagentsinc/history-corpus`.
- One package name is wrong for a public SDK. `khala-ai-sdk-core` reads as
  "the core of the Khala AI SDK", uses the internal product codename, and
  collides with the Vercel product's name. Its public destiny is a neutral
  name such as `@openagentsinc/ai-model` (or absorption into the umbrella
  below). A rename is disruptive to in-repo imports, so it is its own
  bounded packet with alias retention, not a side effect.
- An umbrella meta-package `@openagentsinc/ai` is the discoverable front
  door: curated re-exports of the layer entry points, one README with the
  layer diagram, one version that pins the roster. The umbrella is cheap,
  reversible, and gives the SDK a name without moving any code.

The product name is **OpenAgents AI SDK**. The npm front door is
`@openagentsinc/ai`. The GitHub identity question is §3.

---

## 3. The placement decision

### 3.1 The three candidate shapes

**A. In-monorepo `packages/` (the status quo, made explicit).** The SDK
packages stay where they are. An umbrella package and a docs index give
them a name.

Pros:

- Atomic evolution with the live consumer. The desktop is mid-flight on the
  STREAM and RLM waves. STREAM-03 (transport) will touch the SDK and the
  desktop IPC path in one change. A cross-repo version dance during this
  phase would be pure friction.
- The pre-push gates are load-bearing quality infrastructure. `pnpm run
  check`, the behavior-contract oracles, the conformance suites, the STE
  and doc-coverage checks — a fresh repo starts with none of them.
- The catalog pin. Every package rides `effect: 4.0.0-beta.94` from one
  `pnpm-workspace.yaml` line. Effect v4 is a moving beta. One repo means
  one upgrade per beta, not two coordinated ones.
- Publish machinery exists. Pylon already publishes `@openagentsinc/*` from
  this monorepo, the runbook is written, the token lives in workspace
  secrets, and the SDK packages are already `private: false` with
  `publishConfig`.
- Zero migration cost now.

Cons:

- Public API discipline is harder. In-repo consumers can reach into any
  export, and nothing structurally prevents an app-internal type from
  leaking into a published surface.
- External contributors face a very large monorepo with owner-specific
  contracts.
- Repo weight and the optics of "the SDK is wherever these ten packages
  happen to sit".

**B. Standalone repo now (`OpenAgentsInc/openagents-ai` or similar).** Move
the SDK packages out, publish from there, consume from the monorepo via
npm.

Pros:

- Clean public identity and independent versioning, matching the
  `nostr-effect` and `three-effect` precedent for shared Effect primitives.
- A real external-adoption surface — issues, discussions, focused CI.
- Forced API discipline. The monorepo becomes an npm consumer like anyone
  else.

Cons:

- Cross-repo drift during the highest-churn phase the SDK will ever have.
  Every STREAM or RLM packet that touches both the SDK and the desktop
  becomes publish-bump-consume, or worse, a divergent fork of intent.
- The precedent condition is not met. `nostr-effect` and `three-effect`
  earn standalone repos because **multiple sibling repos** consume them.
  The AI SDK has exactly one consuming repo today — this one. There is no
  second sibling consumer to serve.
- The `effect-native` precedent cuts the other way and is the closest
  analogue: the repo was reserved fresh (LICENSE only), while the design
  dossier and all real decisions live in `openagents/docs/effect-native/`,
  with the workspace contract saying "when Effect Native work begins, it
  lands here". The workspace already knows that a standalone repo created
  before its contract stabilizes sits empty or, worse, stale.
- CI, gate, secret, and release duplication for a beta-pinned API.

**C. Staged: incubate in-monorepo, extract on explicit criteria.** Shape A
now, with the standalone identity reserved and the extraction gate written
down, exactly the `effect-native` pattern.

### 3.2 The decision

**Recommendation: C — incubate in the monorepo now, reserve the standalone
repo, extract only when the written criteria are met.** In the interim the
SDK is real and public through npm, not through a repo boundary.

The rationale, compressed:

1. The SDK is the live substrate of the desktop app **right now** — the
   STREAM and RLM waves are in flight and touch both sides — so atomic
   evolution beats clean separation this quarter.
2. The workspace precedent for a standalone Effect repo requires multiple
   sibling consumers (`nostr-effect`, `three-effect`) — the AI SDK has one
   consumer, this monorepo, so the precedent condition fails today.
3. The closest precedent, `effect-native`, already blessed the staged
   pattern — repo reserved, design and early code in the monorepo,
   extraction when work begins in earnest.
4. The monorepo carries the load-bearing quality gates, the single Effect
   beta catalog pin, and working `@openagentsinc/*` publish machinery — a
   fresh repo would lose all three at the moment of maximum churn.
5. npm is the public identity that matters for adoption — `private: false`
   packages plus an `@openagentsinc/ai` umbrella give the SDK a public face
   with zero migration cost, and the repo extraction stays a reversible,
   criteria-gated future step.

### 3.3 The extraction criteria (written down now, evaluated later)

Extraction to the standalone repo becomes the right call when **all** of
the following hold:

1. **Effect v4 is stable.** The SDK no longer rides a beta pin, so a
   two-repo version story does not double beta-upgrade work.
2. **The API surface is quiet.** STREAM-03 (transport) has landed, the
   naming pass (AISDK-03) is done, and the published packages have gone at
   least one release cycle without a breaking export change.
3. **A second consumer exists.** A sibling repo, an external user, or the
   mobile app consuming via npm rather than `workspace:*` — the
   `nostr-effect` condition.
4. **The private-seam question is resolved.** The `harness-conformance`
   failure-class boundary (§1.3) has a public-safe answer, so the extracted
   repo does not need a private sibling to function.
5. **The owner accepts the split.** Repo creation and the workspace
   contract change are owner-visible decisions under the delegation
   profile.

Until all five hold, extraction would trade real velocity for aesthetic
separation.

---

## 4. Migration and extraction mechanics (for when the gate opens)

Recorded now so the extraction is a mechanical follow-through, not a fresh
design.

### 4.1 What moves

The move set is the publishable roster of §1.3: `agent-runtime-schema`,
`agent-harness-contract`, the renamed model-call package, `history-corpus`,
`ai-sdk-sandbox-local`, `ai-sdk-sandbox-openagents`, the umbrella
`@openagentsinc/ai`, and — if by then admitted to the surface —
`agent-turn-runtime` and a de-Khala-fied tool registry. Move with full Git
history via `git filter-repo` or an equivalent, so provenance survives.

### 4.2 What stays

Everything in the app-internal list of §1.3 stays: desktop wiring, Provider
Lane SPI, Full Auto, `ClaudeLocalEvent` and its behavior contracts,
`harness-conformance` (or its successor), Apple FM routing, custody,
ledgers, and every settlement or authority surface. The monorepo swaps its
`workspace:*` dependencies for pinned npm versions of the extracted
packages.

### 4.3 Versioning and pinning across repos

- The standalone repo owns semver and publishes to npm under
  `@openagentsinc/`. Fixed-version releases across the roster (one version
  train) keep the matrix small.
- The monorepo consumes via its catalog: one `pnpm-workspace.yaml` catalog
  entry per SDK package, bumped in one commit per release train.
- The Effect peer range must be identical in both repos. The standalone
  repo declares `effect` as a peer dependency with the same range the
  monorepo catalog pins.
- Conformance fixtures travel with the SDK. The monorepo keeps a thin
  consumer conformance test that runs the published package against the
  desktop projections, so drift surfaces in the consumer gate rather than
  in production.

### 4.4 The workspace contract change

At extraction time the workspace root `CLAUDE.md` gains a sibling entry in
the established shape, approximately:

> `openagents-ai/` is the shared Effect-native OpenAgents AI SDK for this
> workspace — the neutral runtime event vocabulary, the durable event log,
> the agent harness contract, the UI message stream, and history recall.
> When changing agent-runtime, harness, event-log, UI-stream, or recall
> behavior in sibling repos, use `openagents-ai` directly or extend it
> first instead of rebuilding parallel AI runtime primitives.

And the `openagents/` entry notes that the SDK packages moved and that the
monorepo consumes them by npm pin.

---

## 5. Proposal packets — AISDK-01..07

All packets follow the recommended path C. None is dispatched by this
document. Each requires Sol admission and owner acceptance. Owner sign-off
is explicitly required where noted.

- **AISDK-01 — Name and index the SDK surface.** A `docs/ai-sdk/README.md`
  that states the product sentence, carries the §1.2 layer diagram, lists
  the publishable roster versus the app-internal exclusions, and records
  the §3.3 extraction criteria as the standing gate. Docs only.
  *Verification:* STE inspection, neutral-language guard, link and
  doc-coverage checks green.
- **AISDK-02 — The `@openagentsinc/ai` umbrella package.** A meta-package
  with curated re-exports of the layer entry points and the SDK README. No
  logic. *Verification:* typecheck, `pnpm run check`, `pnpm pack` dry run
  proving the tarball contains only the intended surface.
- **AISDK-03 — Public naming reconciliation.** Rename `khala-ai-sdk-core`
  to a neutral public name (candidate `@openagentsinc/ai-model`), retain a
  deprecation alias one release, and sweep public READMEs for internal
  codenames on the SDK surface. *Verification:* full workspace check green,
  no consumer import breaks, alias resolution test.
- **AISDK-04 — Publish the rc train.** Publish the roster to npm under
  `--tag rc` per the Pylon runbook, with per-package READMEs and export-map
  audits. This is the SDK's public debut and needs owner acceptance of the
  public claim set. *Verification:* published tarballs install and
  typecheck in a clean out-of-repo smoke project against the pinned Effect
  peer range.
- **AISDK-05 — Resolve the failure-class seam.** Decide the public home for
  the `AiError` to failure-class mapping now split across public
  `khala-ai-sdk-core` and private `harness-conformance`. Either a public
  neutral failure vocabulary in `agent-runtime-schema` or a documented
  private-extension point. *Verification:* conformance suite green on both
  sides of the boundary, no private symbol in any published tarball.
- **AISDK-06 — Reserve the standalone repo.** Create
  `OpenAgentsInc/openagents-ai` as LICENSE-plus-README only, exactly the
  `effect-native` reservation pattern, with the README pointing at the
  monorepo docs index and the extraction criteria. Owner sign-off required
  for repo creation and for the workspace contract note. *Verification:*
  repo exists, contains no code, workspace `CLAUDE.md` note lands with the
  docs-only push rule.
- **AISDK-07 — Extraction gate review.** A standing re-evaluation packet:
  when the five §3.3 criteria hold, produce the extraction receipt
  (move set, history-preserving migration, catalog pin swap, consumer
  conformance test) and take it to the owner. Not schedulable today.
  *Verification:* the receipt itself plus both-repo green gates at cutover.

Sequencing: AISDK-01 and AISDK-02 are pure additions and can proceed under
normal admission. AISDK-03 and AISDK-05 touch live imports and need the
usual oracle coverage. AISDK-04 and AISDK-06 carry owner-visible public
claims and require explicit owner acceptance. AISDK-07 waits on the gate.

---

## 6. Bottom line

The OpenAgents AI SDK already exists in everything but name. Effect owns
the model call and stops at the response-part stream. Vercel shipped the
ideas above that line and OpenAgents re-derived them onto stronger durable
contracts — one neutral event union, one cursor, suspend and continue that
actually persists, redaction as a schema field, harnesses for real
coding-agent runtimes, and recall instead of compaction. That stack is a
publishable product no upstream offers. Name it, front it with
`@openagentsinc/ai`, publish the rc train from the monorepo where the
gates, the catalog pin, and the live desktop consumer already are, and
reserve the standalone repo with the extraction criteria written down. The
repo boundary is the last step of a stable API, not the first step of an
unstable one — the workspace already learned that lesson with
`effect-native`, and this analysis simply applies it.
