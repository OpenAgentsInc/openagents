# Cloud primitives (fine-tuning + sandbox) scaffold advance — 2026-06-19

Weekend promise assault, DE-2. Advances the two sellable-Cloud-primitive
SCAFFOLDS (`cloud.fine_tuning_service.v1`, `cloud.sandbox_compute_service.v1`)
toward green WITHOUT flipping state. Both promises STAY **red**. Per the hard
rule, a green flip needs a dereferenceable PAID receipt + owner sign-off
(`proof.claim_upgrade_receipts.v1`); this change produces no paid receipt and the
surfaces stay flag-gated INERT by default.

## What shipped before this

EPIC #5510 (#5516 fine-tuning, #5517 sandbox) shipped each primitive as an
intake-only SCAFFOLD mirroring the inference gateway:

- a flag-gated INERT request surface (`CLOUD_FINE_TUNING_ENABLED` /
  `CLOUD_SANDBOX_COMPUTE_ENABLED`, default off → 404);
- a typed intake POST (`/v1/fine_tuning/jobs`, `/v1/sandboxes`) with a runtime-
  adapter seam and a **no-op** metering stub (`metered: false`).

Gap: no lifecycle read, and no real credit metering — the metering hook could
only ever report `metered: false`, so even the *shape* of a real charge was
unproven.

## What this change adds (agent-claimable, INERT)

1. **Shared cloud-metering seam** —
   `apps/openagents.com/workers/api/src/cloud/cloud-metering.ts`. The REAL
   receipt-first credit-debit the no-op stubs become once a primitive reports
   real runtime usage. It reuses the EXACT atomic credit-ledger discipline the
   inference gateway uses (`inference/metering-hook.ts` → `payments-ledger.ts`):
   a single `adjustment` PayIn funded by one `in` balance leg debiting the
   account's `agent_balances` row, with the `CHECK (balance_msat >= 0)` guard
   (never goes negative) and the `idempotency_key UNIQUE` guard (a replay never
   double-charges). The amount is computed by an **injected pure pricing
   function** from real usage — never a hardcoded price, never an estimate.

2. **Lifecycle READ seam + GET handlers** — each runtime adapter gains a `get`
   method, and each surface gains an OpenAI-shaped lifecycle read
   (`handleFineTuningJobGet`, `handleSandboxGet`). Cross-account isolation is
   enforced at the `get` seam: a job/sandbox is visible only to the account that
   created it. The stub adapter has no persistence, so a read resolves to 404 —
   the real adapter (the training lane / isolated-session substrate) plugs in
   later.

3. **Live metering hook factories** — `makeLedgerFineTuningMeteringHook`,
   `makeLedgerSandboxMeteringHook`. They settle through the shared seam and
   project the scaffold's own public-safe receipt ref
   (`receipt.cloud.fine_tuning.job.<id>` / `receipt.cloud.sandbox_compute.rental.<id>`)
   when a debit lands; null when it does not. At intake/provision (no usage yet)
   they report `metered: false` — no charge before real work.

### Honest scope — what is STILL missing for green

- **No real runtime adapter** is wired (fine-tuning → the training lane; sandbox
  → the isolated-session substrate). The scaffolds default to the stub.
- **No live pricing function** is wired on prod, and the surfaces stay INERT
  (flag-off → 404) on the live Worker.
- **No paid receipt exists.** Green for either promise requires: submit/rent →
  run on real substrate → real metered usage → real credit debit →
  dereferenceable PAID receipt, plus owner sign-off. That is settlement-gated
  and remains owner-gated.

## Verification

```
bun run --cwd workers/api test -- src/cloud/
```

43 tests pass (3 files). Highlights, against real `node:sqlite`-backed SQL with
the verbatim migration-0160 ledger constraints (`cloud-metering.test.ts`):

- decrements a funded balance receipt-first and reports metered;
- **never goes negative** — an over-charge fails the debit and reports not
  metered, balance unchanged;
- **idempotent per charge id** — a replayed settle never double-charges;
- a zero charge is metered (`zeroCharge`) and writes no ledger row.

Route tests additionally prove both GET surfaces are INERT (404) flag-off, 401
unauthenticated, 404 not_found on the no-persistence stub, project a resolved
record for the owning account, and **404 for a different account** (isolation).

`bun run check:architecture` passes (zero-debt gate).

## Promise impact

- `cloud.fine_tuning_service.v1` — **red → red.** Evidence/copy destale only:
  the intake scaffold now has a lifecycle read and a real, tested receipt-first
  metering seam (with the never-negative + idempotency guards proven against real
  SQL). The remaining blockers are narrowed to live intake being disabled, the
  real training-lane runtime being unwired, and billing/settlement being absent.
  Still no real runtime, no live pricing, no paid receipt.
- `cloud.sandbox_compute_service.v1` — **red → red.** Same advance, symmetric.
  The remaining blockers are narrowed to the live rent surface being disabled,
  live metering/billing being unwired, and the paid sandbox receipt being absent.
- `cloud.primitives_suite.v1` — unchanged (planned). The suite still has no
  unified balance and no customer buying multiple primitives end to end. Its
  blockers should name missing live sellable fine-tuning/sandbox services rather
  than claim the inert scaffolds do not exist.

No `promise_transition` is required (no state flips). Any future green flip
remains receipt-first and owner-signed.
