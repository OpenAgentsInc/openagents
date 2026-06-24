# Spec: Khala session capture, the distiller, and the Blueprint program wiring

> Status: **build spec**, 2026-06-24. This makes the three net-new mechanisms from
> the brain audit buildable: (B) wiring a Khala turn into the live Blueprint turn
> runtime, (C) capturing a session as a deterministic trace, and (D) the
> **distiller** that lowers a trace into a reusable artifact. It also (E) **unifies
> the distiller with the executor/autonomous-QA work** — the same "session →
> distilled artifact" pipeline emits *both* a Khala skill candidate and a committed
> e2e test. Companion to and governed by:
> [`2026-06-24-khala-brain-and-blueprint-hookup-audit.md`](2026-06-24-khala-brain-and-blueprint-hookup-audit.md)
> and [`2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md`](2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md).
> **Not a product promise or public-claim copy.** Every invariant in those audits
> holds here: evidence-only Blueprint, no keyword routing, no self-promotion, no
> exactness inflation, identity guard, INERT/OWNER-GATED settlement, no promise
> widening. One model: `openagents/khala` (no variants). **FUTURE** = speculative;
> **OWNER-GATED** = needs owner arming.

## 0. Scope and the one-line shape

The brain audit names three net-new pieces but does not spec them. This doc does:

```
  Khala turn ──(B) wiring──▶ chat-program-runtime ──▶ BlueprintProgramRunRecord (evidence-only)
       │                                                        │
       │                                                        ▼
  (C) capture ──────────────▶  KhalaSessionTrace  ──(D) distiller──┐
  (computer-use timeline +      (deterministic,                    │
   chat turns + verdicts)        replayable, digested)             ▼
                                                        ┌──────────────────────┐
                                                        │  TWO emitters (E):    │
                                                        │  1. skill candidate   │ → Blueprint optimizer_candidate → GEPA → Release Gate → NIP-SKL listing (marketplace)
                                                        │  2. e2e scenario      │ → executor-style Target scenario committed to repo (autonomous QA, epic #6174)
                                                        └──────────────────────┘
```

The insight that makes this one project instead of two: **"guide → capture →
distill → skill" (Khala brain) and "computer-use → distill to e2e test" (executor /
[`../feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md`](../feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md))
are the same pipeline.** One capture format, one distiller, two output adapters.

Prereq (separate, already specced): the refusal-posture prompt fix ships first as a
pure prompt change (**openagents#6178**); nothing here depends on it, but it is the
behavior that *produces* the "guide me through it" sessions this pipeline captures.

---

## B. Wiring: a Khala turn as a Blueprint program (evidence-only)

**Today:** `khala-chat-program.ts` assembles one system message and calls the model
directly; `blueprint/services/chat-program-runtime.ts` (`executeBlueprintChatProgramTurn`)
exists but is **never called from the Khala request path** (brain audit §3).

**Goal of this milestone:** route ONE Khala program (start with the
refusal-posture/offer program) through `chat-program-runtime.ts` and emit a
`BlueprintProgramRunRecord` as evidence — the first real Khala-on-Blueprint call.
No behavior regression for plain chat; no writes; no new promise.

### B.1 The request → program contract

Define a thin adapter (new, in the Khala inference path) — names illustrative:

```text
KhalaProgramTurnRequest {
  conversation:      ChatMessages          // existing Khala conversation
  signatureSelection: BlueprintSignatureLookupRequest  // built for signature-lookup.ts, NOT keyword
  contextPackRef?:   string                // narrows authority, never widens
  riskCeiling:       number                // numeric ceiling passed to the selector
  allowedSurfaces:   Surface[]             // bounded
}

khalaTurnToProgram(req) -> BlueprintProgramRunRecord   // authorityBoundary: 'evidence_only'
```

- **Selection is by typed selector only.** Build a `BlueprintSignatureLookupRequest`
  and call `lookupBlueprintSignatures` (`packages/probe/.../blueprint/signature-lookup.ts`).
  Never branch on user text (no-keyword-routing invariant).
- **First registered program:** `refusal_posture` / `offer` (the second Khala
  signature). The program's module is a `model_prompt` module version whose policy
  is the refusal posture; running it yields the assistant turn **plus** an
  evidence-only run record.
- **Output:** the existing `openagents` response block gains a `programRunRef`
  pointing at the `BlueprintProgramRunRecord` (dereferenceable, public-safe). No
  raw program internals leak (read projections, not runner state).

### B.2 Invariants (enforced, not optional)

- `authorityBoundary: 'evidence_only'`; `noDeploy/noEmail/noSpend/noSourceMutation/
  directMutationDisabled` all set. The program never acts.
- Any external effect is a **proposal** via an approval-gated `Action Submission`
  (`direct_execution=0`, `proposal_only=1`, ≥1 evidence ref, `approvalPolicyRef`).
- Identity guard (`khala-identity.ts`) still wraps the final text.
- Plain-chat parity: a turn that selects no special program behaves exactly as today.

### B.3 Acceptance

- One Khala turn produces a `BlueprintProgramRunRecord` (evidence-only) referenced
  from the response; selection went through `signature-lookup.ts`; tests prove the
  no-keyword path and the evidence-only/no-write boundary; no plain-chat regression.

---

## C. Capture: a session as a deterministic, replayable trace

**Goal:** turn a live Khala/computer-use session (especially a "guide me through it"
session) into a `KhalaSessionTrace` that is deterministic, replayable, public-safe,
and digestible — the distiller's input. Ride the existing capture substrate; do not
build a parallel one.

### C.1 Substrate to reuse

- **Computer-use timeline** (`packages/probe/.../computer-use/timeline.ts`, shipped
  in #6175) — named beats for browser/terminal/fs actions, with raw text/output/
  file-contents already withheld.
- **qa-runner** (`apps/qa-runner`, #6176) — already produces a per-run artifact set
  (result.json + video + trace) from a brain-driven session; this is the recorder.
- **Executor-trace loop** (`artanis-scheduled-runner.ts`) — deterministic traces with
  replay verdicts and closeout receipts (one Lightning closeout settled 2026-06-10).

### C.2 The trace schema (new, Effect Schema)

```text
KhalaSessionTrace {
  schemaVersion: 'openagents.khala.session_trace.v1'
  goal:          string                 // the user-stated goal of the session
  beats: Array<
      | { kind: 'chat_turn';   role; contentRef }          // ref/hash, never raw text
      | { kind: 'tool_call';   tool; argsHash; effect: 'read'|'mutate'|'spend' }
      | { kind: 'browser';     action: 'navigate'|'click'|'type'|'wait'|'screenshot'|'assert'; targetHint }
      | { kind: 'terminal';    commandHash; outcome: 'ok'|'fail' }
      | { kind: 'verdict';     verificationClass: 'none'|'seeded'|'test_passed'|'exact_trace_replay'|'failed' }
    >
  inputs:  TypedField[]                 // inferred typed inputs of the task
  outputs: TypedField[]                 // inferred typed outputs / acceptance
  receipts: string[]                    // replay/acceptance receipt refs
  digest:  string                       // content digest of the ordered beats
}
```

### C.3 Privacy & determinism (invariants)

- **No raw secrets/prompts/PII** in a beat — only refs/hashes/neutral classifiers,
  same discipline as the computer-use timeline and the benchmark report public-safety
  tripwire. Add an `assertSessionTracePublicSafe` tripwire test.
- **Deterministic:** the trace replays to the same digest; waits are conditions,
  never sleeps (carry the executor "no sleeps" rule).

### C.4 Acceptance

- A guided session yields a `KhalaSessionTrace` that (a) re-derives the same digest
  on replay for deterministic steps, (b) passes the public-safety tripwire,
  (c) is dereferenceable.

---

## D. The distiller: trace → reusable candidate

**Goal:** lower a `KhalaSessionTrace` into governed Blueprint candidates. This is the
"key net-new piece" the audit names. Reference shape: Claude `skill-creator`'s
"Capture Intent" step ("turn this into a skill").

### D.1 Contract

```text
distill(trace: KhalaSessionTrace) -> DistillResult {
  signatureCandidate: BlueprintProgramSignature        // typed I/O contract inferred from goal/inputs/outputs
  moduleCandidate:    BlueprintModuleVersion           // moduleKind: 'optimizer_candidate'
                                                       //  (or 'deterministic_reducer' for an exact/replayable step)
  verificationClass:  'none'|'seeded'|'test_passed'|'exact_trace_replay'
  emitters: {
    skill?: SkillCandidate                              // (E.1) marketplace adapter
    e2e?:   E2eScenarioCandidate                        // (E.2) autonomous-QA adapter
  }
}
```

### D.2 The candidate acceptance bar (a candidate is NOT promoted)

A `DistillResult` is admissible as a **candidate** only if:
1. **Replayable** — re-executing the trace reproduces the recorded outcome
   (digest match for deterministic steps; for stochastic, a `seeded` verdict).
2. **Typed** — `signatureCandidate` has a concrete typed I/O contract (no `any`).
3. **Honestly graded** — `verificationClass` reflects what was actually proven;
   a learned/statistical step is **never** labeled with exact (Tier-E) vocabulary.
4. **Public-safe** — no secrets in the candidate or its examples.
5. **Quality bar** (executor): asserts outcomes a user cares about, not
   implementation detail; no tautologies; deterministic waits.

### D.3 Governance (unchanged from the kernel)

- A candidate enters Blueprint as an `optimizer_candidate` `BlueprintModuleVersion`
  behind its signature → may be refined by **GEPA** → is promoted **only** through a
  `BlueprintReleaseGate` (operator-approved, `selfPromotionAttempt` rejected).
- Nothing the distiller produces is live until a human promotes it. **No
  self-promotion, ever.**

### D.4 Acceptance

- A captured guided session distills into a typed signature + `optimizer_candidate`
  module that (a) replays green, (b) carries an honest verification class, (c) is
  rejected by the Release Gate without an operator approval. Tests prove the
  acceptance bar and the self-promotion rejection.

---

## E. Two emitters — the unification with Rhys / executor (the "(c)" ask)

The distiller's value is that **one capture + one distiller serves two products**:

### E.1 Skill emitter → the capability marketplace (Khala demand/supply)

`SkillCandidate` = a NIP-SKL (`docs/nips/SKL.md`) skill manifest candidate placed on
the right verification-ladder tier (E/D/S/N) with honest labeling, feeding the
marketplace described in
[`2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md`](2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md).
On future use its trace decomposes and the split routes to the author (who may be the
user who guided the session) over NIP-AC/Lightning — **behind the INERT settlement
machine. OWNER-GATED.** No public marketplace today; boundary holds.

### E.2 E2E emitter → autonomous QA (what Rhys/executor needs)

`E2eScenarioCandidate` = a **black-box scenario against the executor-style `Target`
interface** (study `projects/repos/executor/e2e/src/{target,scenario}.ts`), committed
to the repo — the *exact* artifact the autonomous-QA feature request asks for:
"the agent develops with computer-use tools, then turns the session into committed
e2e tests; verify by reading the test + watching the video."

This is the connective tissue between the two threads:

| Khala brain audit term | Executor / QA term | Shared mechanism |
|---|---|---|
| guided session | computer-use develop session | `KhalaSessionTrace` (§C) |
| capture front-end | the recorder | computer-use timeline (#6175) + `apps/qa-runner` (#6176) |
| distill → skill candidate | distill → committed e2e test | `distill()` (§D) with two emitters |
| skill earns rev-share | run = verified receipt | the same trace-decomposed receipt |
| `signature-lookup` selection | `Target` capability selection | typed selectors, no keyword routing |

**So the distiller is the single highest-leverage build:** it completes the Khala
"refusal → skill → earn" loop *and* it is the "session → committed e2e test" the
executor flow (epic #6174, #6175/#6176/#6177) is missing. Build it once; wire two
emitters. The qa-runner's headline demo (#6177) becomes the first **input** to the
e2e emitter (record a real session → distill → committed scenario), and the
marketplace becomes the first input to the skill emitter.

### E.3 Acceptance for (c)

- The same `KhalaSessionTrace` + `distill()` produces, from one guided session:
  (a) a skill candidate (Blueprint optimizer_candidate, gated), and
  (b) an executor-style e2e `Target` scenario file — proving one pipeline, two
  artifacts. (Listing/settlement remain OWNER-GATED; the e2e scenario is committable
  evidence today.)

---

## F. Phasing (evidence-only first; money OWNER-GATED)

1. **B — wiring:** route one Khala turn (refusal-posture program) through
   `chat-program-runtime.ts`; emit an evidence-only `BlueprintProgramRunRecord`;
   discovery via `signature-lookup.ts`. *(No money, no new promise.)*
2. **C — capture:** define `KhalaSessionTrace` + `assertSessionTracePublicSafe`;
   record a guided/computer-use session into it via the #6175/#6176 substrate.
3. **D — distiller:** `distill(trace)` → typed signature + `optimizer_candidate`
   with the acceptance bar; Release-Gate rejection without operator approval.
4. **E.2 — e2e emitter:** emit an executor-style `Target` scenario from a trace
   (the executor/QA deliverable; committable today, evidence-only).
5. **E.1 — skill emitter:** emit a NIP-SKL skill candidate on a ladder tier. *(FUTURE.)*
6. **Settlement:** one trace-decomposed split over Lightning behind the 8-state
   machine. **OWNER-GATED.**

## G. Invariants (carry, do not weaken to ship)

Evidence-only Blueprint; Action Submissions are the only (approval-gated) write path;
no keyword routing (typed selector only); nothing self-promotes (Release Gate); no
exactness inflation (ladder labeling is law); identity guard holds; public-safe
artifacts only; settlement INERT/OWNER-GATED; no promise widening; one model
`openagents/khala`.

## H. Open decisions for the owner (before D/E build)

1. **Distiller authorship:** is the distiller itself a Khala program (Khala distills
   its own session) or a deterministic reducer over the trace? (Audit implies the
   former is FUTURE; a deterministic v1 reducer is safer/cheaper to start.)
2. **First emitter to build:** E.2 (e2e scenario — concrete, committable, serves
   Rhys now) vs E.1 (skill — needs the marketplace floor). Recommend **E.2 first**.
3. **Trace source v1:** capture from the `apps/qa-runner` computer-use session
   (deterministic, exists) before capturing arbitrary live `/khala` chat turns.
4. **Signature inference depth:** how much of `signatureCandidate` is inferred vs
   author-confirmed in the "Capture Intent" step.

## Pointers

- Audits: [`2026-06-24-khala-brain-and-blueprint-hookup-audit.md`](2026-06-24-khala-brain-and-blueprint-hookup-audit.md),
  [`2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md`](2026-06-24-khala-marketplace-tassadar-blueprint-fusion.md),
  [`2026-06-23-khala-blueprint-program-and-plugin-extensibility.md`](2026-06-23-khala-blueprint-program-and-plugin-extensibility.md).
- Executor/QA: [`../feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md`](../feature-requests/2026-06-24-autonomous-qa-e2e-from-computer-use.md);
  epic #6174 + #6175 (computer-use tools) + #6176 (`apps/qa-runner`) + #6177 (demo);
  `projects/repos/executor/e2e/src/{target,scenario,timeline}.ts`.
- Live code: `apps/openagents.com/workers/api/src/blueprint/` (schemas, `services/chat-program-runtime.ts`,
  `services/{tassadar-module-step,replay-module}.ts`, `repositories/tassadar-module-registry.ts`);
  selector `packages/probe/packages/runtime/src/blueprint/signature-lookup.ts`;
  capture `packages/probe/packages/runtime/src/computer-use/timeline.ts`, `apps/qa-runner/`;
  rails `docs/nips/SKL.md`, `docs/nips/AC.md`; settlement `omni-accepted-outcome-*.ts`.
</content>
