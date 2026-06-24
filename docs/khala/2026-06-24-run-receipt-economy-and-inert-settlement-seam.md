# Run-receipt economy: skill-emitter, Blueprint wiring, and the INERT settlement seam

> Status: **build record**, 2026-06-24 (issue
> [openagents#6188](https://github.com/OpenAgentsInc/openagents/issues/6188)).
> Implements the evidence-only spine of "run = verified receipt → skill-emitter →
> Blueprint wiring → settlement", with **money kept INERT / OWNER-GATED**.
> Governed by the spec
> [`2026-06-24-khala-session-distiller-and-program-wiring-spec.md`](2026-06-24-khala-session-distiller-and-program-wiring-spec.md)
> and its audits. Not a product promise or public-claim copy. One model:
> `openagents/khala`.

## What landed (evidence-only)

1. **Run = dereferenceable verified receipt** — `apps/qa-runner/src/receipt.ts`.
   An **additive, namespaced** `receipt` field on the run `result.json`, written
   by a **post-run helper** (`writeReceiptForRun`) — the runner's control flow is
   untouched (a peer lane additively adds `verify`; the two merge trivially).
   The receipt carries a public-safe `receiptRef`, a sha256 `resultDigest`, the
   honest `verificationClass` (a failing run is `none`; a passing run with >=1
   outcome assertion is `exact_trace_replay`; a pass with no assertions is
   `seeded` — no exactness inflation), and `resultPath: "result.json"`. The
   augmented result is re-checked against the public-safety tripwire.

2. **Distiller skill-emitter (spec §E.1)** — `apps/qa-runner/src/skill-candidate.ts`.
   The distiller now emits, from the SAME capture as the committed e2e test, a
   **governed Blueprint optimizer skill candidate**: `moduleKind:
   'optimizer_candidate'`, `authorityBoundary: 'evidence_only'`,
   `requiresReleaseGate: true`, `selfPromotionAllowed: false`, `live: false`, on
   an honest NIP-SKL ladder tier (E/S/D/N). `evaluateReleaseGateWithoutApproval`
   proves the Release Gate **rejects** an unapproved candidate (no self-promotion,
   ever). It is a candidate ONLY — nothing is promoted.

3. **Blueprint program run-record wiring (spec §B)** —
   `apps/openagents.com/workers/api/src/blueprint/services/chat-program-runtime-khala.ts`.
   A thin adapter routes one Khala turn through the existing
   `executeBlueprintChatProgramTurn` and emits an **evidence-only**
   `BlueprintProgramRunRecord`. Selection rides the runtime's typed structured
   selector (no keyword routing); the record is asserted evidence-only
   (`directMutationDisabled`, `noDeploy/noEmail/noSpend/noSourceMutation`); any
   requested direct effect is **denied** (it must be an approval-gated Action
   Submission). No real inference, no writes, no spend — refs/digests only.

4. **Settlement seam — INERT / SPEC-ONLY / OWNER-GATED** —
   `apps/qa-runner/src/run-settlement.ts`. The typed 8-state
   (`authorized → paid → accepted → pending_payout → dispatched → confirmed →
   reconciled → margin`) run/skill rev-share split machine. It is:
   - **DEFAULT-OFF:** `armed` is false; there is **no** code path that moves sats,
     opens a wallet, builds an invoice, or contacts a payout rail.
   - **OWNER-GATED + SPEC-ONLY:** `arm()` requires the owner arming token **and**
     a payout executor this seam does not provide; it therefore **always errors**.
     There is no path that returns an armed, money-moving machine.
   - **HONEST:** every transition's `movedSats` is hard-wired `false`; the
     money-movement states (`dispatched`/`confirmed`) are recorded `intent_only`.
     A fully-advanced 8-state machine has still moved **zero** sats.

## Invariants carried (not weakened)

Evidence-only Blueprint; Action Submissions are the only (approval-gated) write
path; no keyword routing (typed selector only); nothing self-promotes (Release
Gate); no exactness inflation (ladder/verification labeling is law); public-safe
artifacts only (tripwires re-run); **settlement INERT / OWNER-GATED**; no promise
widening; one model `openagents/khala`.

## To arm settlement later (owner action — NOT done here)

Arming is intentionally absent. A future, owner-approved real settlement path
must: (a) provide a real payout executor, (b) gate it behind the owner arming
token and an explicit operator approval, (c) keep the receipt → split derivation
public-safe, and (d) only then may a `dispatched`/`confirmed` transition record
`externally_confirmed` with `movedSats: true`. None of that exists in this seam.

## Tests

- qa-runner: `bun test` (incl. `receipt.test.ts`, `skill-candidate.test.ts`,
  `run-settlement.test.ts`, and the updated `distiller.test.ts`).
- worker: `chat-program-runtime-khala.test.ts` + the full `blueprint/` suite +
  the `check:deploy` worker subset; `typecheck:api` clean.
