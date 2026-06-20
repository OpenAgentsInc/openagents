# training.public_gradient_windows.v1 — promoted-window receipt emitter

Promise state: **planned** (unchanged — this change flips nothing).

## What this change adds

The gradient-window regime
(`apps/openagents.com/workers/api/src/tassadar-gradient-window-regime.ts`)
already decides whether a candidate public training window *may* promote
(quarantine → recompute → replicate → canary → explicit promotion gate, with
the compiled exact core held frozen). What it did not produce was the
public-safe **receipt** the runtime must emit once a window has actually
promoted — the dereferenceable artifact a reviewer or contributor would read to
confirm "this public window was accepted, recomputed, replicated, canaried, and
promoted, with the compiled core unchanged".

This change supplies that missing substrate:

- `tassadar-gradient-window-promotion-receipt.ts`
  - `TassadarGradientWindowPromotionReceipt` schema (schema version
    `openagents.training.public_gradient_window.promotion_receipt.v1`).
  - `buildTassadarGradientWindowPromotionReceipt(projection)` — converts a
    fully-passed regime projection into the canonical receipt. It **refuses**
    (throws `TassadarGradientWindowPromotionReceiptUnsafe`) unless the
    projection is at the `promoted` stage with promotion allowed, the compiled
    core unchanged, zero outstanding blockers, and non-empty recompute /
    replication / canary / promotion-decision / rollback lineage. It cannot
    fabricate a promoted-window claim from a window that did not promote.
  - `tassadarGradientWindowPromotionReceiptRef(windowRef)` — deterministic,
    public-safe receipt id derivation so the same promoted window always maps to
    the same receipt ref.
- `tassadar-gradient-window-promotion-receipt.test.ts` — exercises emission from
  a real promoted projection and the refusal paths (no promotion, mutated core).

## Which blocker this advances

`blocker.product_promises.public_gradient_promoted_window_receipts_missing`.

This is the receipt *format and emitter* the promoted-window blocker needs. It
does **not** clear the blocker: no real public contributor gradient window has
been accepted, promoted, paid, or settled, so no instance of this receipt has
been emitted from a live window. The blocker therefore stays listed.

## What genuinely remains

- A live accepted-window runtime that actually receives candidate windows and
  drives them through the regime
  (`blocker.product_promises.public_gradient_live_window_runtime_missing`).
- An emitted, dereferenceable instance of this receipt backed by a real
  promoted public window, plus a public route/feed serving it.
- Settlement receipts where real money moved
  (`blocker.product_promises.public_gradient_settlement_receipts_missing`).

## 2026-06-20 live-window-runtime intake admission edge

The live window runtime
(`blocker.product_promises.public_gradient_live_window_runtime_missing`) had no
front door: the regime gate
(`tassadar-gradient-window-regime.ts`) only *evaluates a window that has already
been processed* — it requires a full recompute/replication/canary receipt
bundle, so it answers "may this window promote?". Nothing decided whether a
freshly submitted candidate may even **enter quarantine** and consume those
verification resources.

This change adds that admission edge:

- `tassadar-gradient-window-intake.ts`
  - `admitTassadarGradientWindowToQuarantine(submission)` — a pure,
    deterministic function over an untrusted submission. It **rejects** anything
    malformed, unsafe (private/credential/payment material), compiled-core
    targeting, frozen-core mutating, non-forward-pass, or missing the required
    psionic-H1 / curated-data / construction / verification evidence, and
    otherwise **admits** the candidate to quarantine. It never throws on bad
    input (a hostile/malformed submission yields a `rejected` decision), so it
    is safe at the edge of a real runtime.
  - Admission grants **quarantine entry only** — no promotion, settlement,
    canonical-checkpoint mutation, compiled-core-gradient, or direct-submission
    authority. Admission is not acceptance: an admitted window can still be
    blocked by the regime gate.
  - Schema version
    `openagents.training.public_gradient_window.intake_admission.v1`.
- `tassadar-gradient-window-intake.test.ts` — exercises admission of a clean
  submission and the rejection paths (compiled-core targeting, frozen-core
  mutation, missing evidence, malformed input).

This advances the live-window-runtime blocker by building the runtime's
admission edge. It does **not** clear it: no live runtime yet receives real
public submissions over a route, no quarantine store persists admitted windows,
and no public window has been accepted, promoted, paid, or settled. The blocker
stays listed.

## 2026-06-20 status projection slice

`GET /api/public/training/public-gradient-windows` now exposes a public-safe,
live-at-read status projection for this promise.

The projection makes the current boundary machine-readable:

- `regimeGateAvailable: true`
- `promotionReceiptEmitterAvailable: true`
- `publicProjectionAvailable: true`
- `liveWindowRuntimeAvailable: false`
- `promotedWindowReceiptAvailable: false`
- `settlementReceiptAvailable: false`
- `emittedReceiptCount: 0`
- `acceptedPublicWindowCount: 0`
- `promotedPublicWindowCount: 0`
- `settlementReceiptCount: 0`
- `canonicalCheckpointMutationCount: 0`
- `greenGateSatisfied: false`

This does not clear any product blocker. It only gives reviewers and agents one
dereferenceable status endpoint for the already-landed regime gate and receipt
emitter. The promise remains **planned** until a real public window runtime
accepts candidate windows, at least one public window emits a promoted-window
receipt, and settlement receipts exist where real money moved.

No public window was accepted, no checkpoint was mutated, no assignment, spend,
or settlement occurred, no receipt-backed promotion row exists, and no green
transition is created by this slice.
