# vertex-fleet: autopilot.decision_queue.v1

**Promise state:** `planned` (no state change this run — Hard Rule 1)

## Latest run (2026-06-20) — worker-api closeout ledger (accumulation + audit)

**Blocker advanced:** `blocker.product_promises.receipt_backed_command_closeout_missing`

**New files:**

| File | Purpose |
|---|---|
| `apps/openagents.com/workers/api/src/autopilot-decision-closeout-ledger.ts` | `createAutopilotDecisionCloseoutLedger()` — the storage + audit-index layer the worker-api closeout receipt was missing. `buildAutopilotDecisionCloseoutReceipt` already produces ONE verifiable closeout per `actOnDecision` review resolution, but those receipts had nowhere to accumulate, so no audit could answer "which queued decisions were closed out, by whom, with what outcome?". This in-memory ledger keys on the receipt's exactly-once `closeoutRef` and mirrors the protocol-side `createDecisionCloseoutLedger` contract — EXCEPT it understands the live review path's applied↔duplicate distinction: an idempotent replay produces the SAME `closeoutRef` but a different `outcome`/`decidedAt`/`line`, which a naive line-equality dedup (what the remote ledger does) would mis-flag as a conflict. Instead it converges on the closeout's stable IDENTITY (decision/work-order/action/state/actor/refs), so a replay is `deduped: true` (keeping the canonical `applied` record) while a genuinely different second closeout for the same `closeoutRef` is refused (`conflict`). Validates on append; serves `get` / `byWorkOrder` / `byActor` / `byOutcome` / `summary`; snapshots are mutation-safe. Pure + in-memory so a D1/KV store can wrap the same contract later without changing callers. |
| `apps/openagents.com/workers/api/src/autopilot-decision-closeout-ledger.test.ts` | 9 tests: empty state, record/get, invalid rejection, applied→duplicate replay convergence (no growth, canonical applied kept), conflict refusal for a differing same-`closeoutRef` closeout, audit slices by work order / actor / outcome, summary by outcome+action, mutation-safe snapshots, ledger isolation. |

**What it proves:** the live review-path closeout receipt is now genuinely
*accumulable and auditable* — the missing half of "receipt-backed". A reviewer
can record every resolution into one ledger, replays converge to one canonical
closeout, and conflicting closeouts are refused, all enforced once in a pure,
test-backed place. No promise state changes; no persistence/HTTP wiring is
claimed (see "What remains").

---

## Earlier run (2026-06-20) — receipt-backed closeout on the LIVE review path

**Blocker advanced:** `blocker.product_promises.receipt_backed_command_closeout_missing`

**New files:**

| File | Purpose |
|---|---|
| `apps/openagents.com/workers/api/src/autopilot-decision-closeout.ts` | `buildAutopilotDecisionCloseoutReceipt()` + `validateAutopilotDecisionCloseoutReceipt()` — the canonical, tamper-verifiable closeout receipt for the ONE decision-queue act that is actually wired today: `autopilot-decision-routes.ts#actOnDecision` (the work-order review accept / reject / request_changes path). Until now that live HTTP route recorded a review decision and returned the projection but emitted **no closeout artifact** — nothing a later audit could dereference. The protocol-side `DecisionCloseoutReceipt` covers the *remote Pylon-bridge* path, a different surface unreachable from the worker-api review store. This receipt mirrors that contract (deterministic `line` reconstructed by the validator so any field tamper invalidates it; `closeoutRef` exactly-once key that is IDENTICAL across an idempotent replay, so a downstream ledger records one closeout per decision; public-safe `receiptRefs` only). |
| `apps/openagents.com/workers/api/src/autopilot-decision-closeout.test.ts` | 11 tests: applied vs duplicate classification, stable exactly-once `closeoutRef` across replay, action→resolvedState mapping for all three actions, public-safe ref normalization (trim/dedupe/sort/reject-unsafe), and validator tamper-detection (non-object, tampered outcome, action/state mismatch, unknown action, forged line). |

**Wiring (minimal, additive):** `actOnDecision` now builds the closeout and
returns it under a new `closeout` field on the act response (status unchanged;
existing route tests still pass — the field is additive). This makes the live
review path genuinely receipt-backed end-to-end through the HTTP surface, not
just a standalone builder. No promise state changes.

**What remains for this blocker:** a *persistent* ledger (D1/KV) that appends
these worker-api receipts keyed by `closeoutRef` (the protocol-side
`createDecisionCloseoutLedger` is the in-memory contract to mirror), plus the
remote-bridge receipt's own persistence + a live paired-node proof.

---

## Earlier run (2026-06-20) — route-side act classification

**Blocker advanced:** `blocker.product_promises.decision_queue_api_missing`

**New files:**

| File | Purpose |
|---|---|
| `apps/openagents.com/workers/api/src/autopilot-decision-act-routing.ts` | `classifyAutopilotDecisionActRoute()` — the branch decision the queue route is missing. `autopilot-decision-routes.ts#actOnDecision` hard-codes a single path (only `approve_pr_draft` → work-order review store; everything else rejected). The full-vocabulary act contract (`authorizeAutopilotDecisionAct`) already exists, but nothing decides *which* handler a given stored decision flows to. This pure module classifies a decision projection into one of three mutually-exclusive routes — `work_order_review` (legacy PR approval), `evidence_command` (the full vocabulary, carrying the `AutopilotDecisionActTarget` the authorizer needs), or `not_actionable` (informational/blocked kinds like `request_customer_input` / `create_followup_mission` / `mark_unavailable`). Routing is decided by kind only; status is carried on the target so `authorizeAutopilotDecisionAct` stays the single owner of the actionability ("too late") refusal. Also exports `isWorkOrderReviewDecision()` and `AUTOPILOT_DECISION_REVIEW_KIND`. |
| `apps/openagents.com/workers/api/src/autopilot-decision-act-routing.test.ts` | 8 tests: review-kind flag, review route, every evidence-command kind routed with correct target, informational kinds marked not-actionable with kind-named reason, routing-by-kind-carries-status, end-to-end handoff proving the produced target is *accepted* by `authorizeAutopilotDecisionAct` for an available decision and *refused* (`not_actionable`) once completed, and the full-projection convenience wrapper. |

**What it proves:** the route now has a tested, pure way to fan a stored
decision into the correct handler instead of rejecting all non-review kinds.
This is the missing link between the queue projection and the existing act
contract: `classify → (evidence_command) authorizeAutopilotDecisionAct → apply`.
No promise state changes; no store/route wiring is claimed (see "What remains").

---

## Earlier run (2026-06-20) — full-vocabulary decision-act contract

**Blocker advanced:** `blocker.product_promises.decision_queue_api_missing`

**New files:**

| File | Purpose |
|---|---|
| `apps/openagents.com/workers/api/src/autopilot-decision-act.ts` | `authorizeAutopilotDecisionAct()` — the validated request→command contract the decision-queue HTTP route is missing. Today `autopilot-decision-routes.ts` hard-rejects everything except `approve_pr_draft` ("Only approve_pr_draft decision actions are actionable through the decision queue."). This module accepts the full client vocabulary — `continue` / `steer` / `provide_context` / `rerun_tests` / `retry_account` / `stop` (plus `approve_pr_draft`) — decodes the wire request with Effect-Schema (`AutopilotDecisionActRequest`), and authorizes it against the stored decision facts. It enforces the CLAIM's two invariants in pure code: **route-authorized** (an act is refused unless the stored decision status is `available`/`recommended`, and unless the resolution matches the stored `actionKind`) and **evidence-only** (every produced `AutopilotDecisionActCommand` reports `directEffectPermitted: false` / `authorityBoundary: 'evidence_only'`, and only public-safe refs may cross — raw payloads/secrets are rejected). It emits the `closeoutRef` exactly-once key a receipt is later attributed to. |
| `apps/openagents.com/workers/api/src/autopilot-decision-act.test.ts` | 11 tests: schema decode of submit/decline, unknown-resolution & unknown-verb decode rejection, evidence-only command shape, full-vocabulary coverage (every actionable kind authorizes, proving the surface is no longer approve-only), kind-mismatch refusal, non-actionable-status refusal, `provide_context`/`steer` context-required-on-submit (and not on decline), unsafe-ref rejection, and ref normalization (trim/dedupe/sort). |

**What it proves:** the decision-queue API now has a tested, mergeable contract
for the *full* decision vocabulary the node can raise — not just the legacy PR
approval. The route layer can decode → `authorizeAutopilotDecisionAct` → apply,
with route-authorization and evidence-only safety enforced once, in one pure,
test-backed place. No promise state changes; no store/route wiring is claimed
(see "What remains").

---

## Earlier run (2026-06-20) — cross-client coordinator

**Blocker advanced:** `blocker.product_promises.cross_client_exactly_once_decisions_missing`

**New files:**

| File | Purpose |
|---|---|
| `packages/autopilot-control-protocol/src/decision-closeout-coordinator.ts` | `createDecisionCloseoutCoordinator()` — the composing layer that wires N per-surface `RemoteDecisionQueue`s (desktop / web / Expo) to ONE shared `DecisionCloseoutLedger`. `ingest()` fans a node decision event out to every paired surface; `resolve({client, …})` relays on that surface's own bridge, builds exactly ONE canonical closeout receipt on a terminal outcome, appends it to the shared ledger, and broadcasts the resolution to the OTHER surfaces so their cards disable. A second surface resolving the same decision hits the local exactly-once gate (already-resolved via broadcast), never reaches the wire, and produces NO second receipt (`alreadyClosed: true`). |
| `packages/autopilot-control-protocol/src/decision-closeout-coordinator.test.ts` | 9 tests: empty/duplicate-surface guards, `ingest` fan-out to all surfaces, single-closeout-on-resolve with the other surfaces disabled and off-wire, second-surface no-double-closeout proof, answer-verb passthrough, injected shared ledger (persistent-store seam), unknown-surface throw, node-reported duplicate. |

**What it proves:** the dereferenceable cross-client exactly-once flow that was
previously only available per-client now exists end-to-end: a decision resolved
on one client surface is **seen as closed on the others** (state → `resolved`
via the subscribe/history broadcast, pending list emptied, transport never
called) and the shared audit ledger holds **exactly one** canonical closeout
receipt attributed to the surface that actually resolved it. Each surface still
relays through its own capability-scoped `BridgeTransport` — no new authority.

Exported from `packages/autopilot-control-protocol/src/index.ts`.

---

## Earlier run — receipt closeout storage layer

**Blocker advanced:** `blocker.product_promises.receipt_backed_command_closeout_missing`

**New files:**

| File | Purpose |
|---|---|
| `packages/autopilot-control-protocol/src/decision-closeout-receipt.ts` | `DecisionCloseoutReceipt` type, `buildDecisionCloseoutReceipt()` builder, and `validateDecisionCloseoutReceipt()` validator. Now also exports the single-sourced `TERMINAL_DECISION_OUTCOMES` / `DECISION_CLIENTS` vocabularies that the validator and the ledger share |
| `packages/autopilot-control-protocol/src/decision-closeout-receipt.test.ts` | 20 tests covering all terminal outcomes, all client surfaces, answer-verb hasAnswer logic, terminal-vs-transient classification, and tamper-detection via line reconstruction |
| `packages/autopilot-control-protocol/src/decision-closeout-ledger.ts` | `createDecisionCloseoutLedger()` — the storage + audit-index layer for closeout receipts (added 2026-06-20). Validates on append, enforces exactly-once per decision `requestId`, idempotently converges byte-identical cross-client re-appends (`deduped: true`), refuses a conflicting second closeout for a closed command (`reason: "conflict"`), and answers audit queries `get` / `byClient` / `byActor` / `byOutcome` / `summary` |
| `packages/autopilot-control-protocol/src/decision-closeout-ledger.test.ts` | 10 tests: empty state, record/get, invalid rejection, idempotent cross-client convergence, conflict refusal, audit slices, vocabulary summary, append order, mutation-safe snapshots, ledger isolation |

**What it does:**

`buildDecisionCloseoutReceipt()` produces a canonical, verifiable receipt
whenever `createRemoteDecisionQueue().resolve()` finishes with a terminal
outcome (`applied`, `duplicate`, `expired`, `revoked`, `stale`,
`unauthorized`, `unsupported`, `error`).

The receipt captures:
- `requestId` — the node's exactly-once decision key
- `actionRef` — what decision was about
- `verb` — what the client chose (approve / deny / answer)
- `outcome` — the classified transport result
- `client` — which surface resolved it (`desktop` | `web` | `expo`)
- `actor` — who triggered it (owner, autopilot, agent ref)
- `decidedAt` — ISO timestamp
- `hasAnswer` — whether a free-text answer was forwarded
- `line` — deterministic human-readable string; tamper of any field invalidates it

Transient outcomes (`offline`, `overloaded`) are excluded by type — they are
not closed-out; the queue replays them on drain.

The module is exported from `packages/autopilot-control-protocol/src/index.ts`
and is shared across desktop / web / Expo (the three client surfaces).

## What remains (still open after this run)

- **`blocker.product_promises.decision_queue_api_missing`** —
  *Partially advanced (2026-06-20).* The validated request→command **contract**
  for the full decision-type vocabulary (continue / steer / provide context /
  rerun tests / retry / stop, plus `approve_pr_draft`) now exists and is tested
  (`apps/openagents.com/workers/api/src/autopilot-decision-act.ts`,
  `authorizeAutopilotDecisionAct`). The route-side branch decision that picks
  the handler per stored decision now also exists and is tested
  (`autopilot-decision-act-routing.ts`, `classifyAutopilotDecisionActRoute`).
  The remaining gap is the *wiring*: `autopilot-decision-routes.ts#actOnDecision`
  still only resolves `approve_pr_draft` via the work-order review store — it
  needs to (a) call `classifyAutopilotDecisionActRoute` on the stored decision,
  (b) for the `evidence_command` route, call `authorizeAutopilotDecisionAct`,
  and (c) persist the resulting evidence-only command + closeout, with
  idempotency on `closeoutRef`. (A prerequisite for non-review kinds: the queue
  only projects review/blocked decision records from work orders today, so an
  evidence-command act needs a stored decision-record source to read facts
  from.) The blueprint
  continuation-decision-queue projection service is still read-only (no act
  surface). No promise state changes until a real act over a live paired node
  produces a dereferenceable receipt.

- **`blocker.product_promises.cross_client_exactly_once_decisions_missing`** —
  *Partially advanced (2026-06-20).* `createDecisionCloseoutCoordinator` now
  exercises the cross-client coordination in a pure, test-backed harness: a
  decision resolved on one surface is seen as closed on the others and yields
  exactly one closeout receipt. The remaining gap is the *live* wiring — feeding
  the coordinator real `session.subscribe`/`session.history` events from a
  remote-reachable paired node and capturing one receipt from an actual
  phone/web/desktop resolution (gated behind the Pylon remote bridge transport,
  #5000 / #5004).

- **`blocker.product_promises.receipt_backed_command_closeout_missing`** —
  *Partially advanced* (further again 2026-06-20). Earlier runs built the
  remote-bridge receipt type/builder/validator and the in-memory ledger
  (`createDecisionCloseoutLedger`). This run extends the closeout coverage to the
  **live worker-api HTTP path**: `actOnDecision` now produces and returns a
  canonical, tamper-verifiable `AutopilotDecisionCloseoutReceipt`
  (`autopilot-decision-closeout.ts`) for every review resolution, with an
  exactly-once `closeoutRef` stable across idempotent replays. The worker-api
  *in-memory* accumulation/audit layer now also exists and is tested
  (`autopilot-decision-closeout-ledger.ts`, `createAutopilotDecisionCloseoutLedger`):
  it converges idempotent replays and refuses conflicting closeouts keyed by
  `closeoutRef`. The remaining gap is a *persistent* backing store (D1/KV)
  wrapping that same contract, the `actOnDecision` call site that appends the
  built receipt into the ledger, and a proof of at least one real receipt
  produced by a live paired-node resolution.
