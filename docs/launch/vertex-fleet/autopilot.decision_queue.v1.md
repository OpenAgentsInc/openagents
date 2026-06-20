# vertex-fleet: autopilot.decision_queue.v1

**Promise state:** `planned` (no state change this run — Hard Rule 1)

## Latest run (2026-06-20) — cross-client coordinator

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

- **`blocker.product_promises.decision_queue_api_missing`** — An HTTP route that
  exposes the full decision-type vocabulary (continue / steer / provide context /
  rerun tests / retry / stop) beyond `approve_pr_draft` still needs to be wired
  into the Worker API. The blueprint continuation-decision-queue service exists
  but has no HTTP surface.

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
  *Partially advanced* (further this run). The receipt type, builder, and
  validator landed earlier; this run adds the in-memory storage + audit-index
  layer (`createDecisionCloseoutLedger`) with exactly-once dedup, cross-client
  convergence, conflict refusal, and audit queries. The remaining gap is a
  *persistent* backing store (D1/KV) wrapping this same ledger contract, the
  call site that appends to the ledger when `createRemoteDecisionQueue().resolve`
  returns a terminal outcome, and a proof of at least one real receipt produced
  by a live paired-node resolution.
