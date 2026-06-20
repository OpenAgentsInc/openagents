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
