# Verification targets

The parts of OpenAgents worth checking beyond tests, in the order we'd do
them. Each one passes the rule of thumb in the [README](README.md):
- a small exact rule;
- an expensive failure;
- cases tests can't reach.

Each target lists:
- the properties, in one sentence each;
- the tools ([tools.md](tools.md));
- the bounds;
- what it waits on.

## 1. x402 settlement, exactly once

**Why first:**
- It's the smallest real money protocol we have.
- It's about to move to our own LND node
  ([x402 over LND](../payments/2026-10-15-x402-over-lnd.md)).
- It's server-side, so it's safe under the 1.0 freeze.
- It will tell us within days whether Stateright earns its place.

**The system:** a buyer gets a 402 challenge with an invoice, pays it, and
retries the request with the preimage. The server checks settlement
(`LookupInvoice`), checks its replay store, then credits or serves once.

**Properties (always):**
- **S1:** a paid invoice is credited or served at most once, whatever the
  retries, duplicates or crashes.
- **S2:** nothing is credited or served without a settled invoice for the
  exact amount and resource.
- **S3:** a preimage for one resource never unlocks another.
- **S4:** a crash between "settled" and "credited" never loses the payment:
  after restart the credit happens, or the payment shows as unclaimed for
  the buyer to retry.

**Properties (sometimes, to rule out a vacuous pass):**
- a payment can settle and be credited;
- a retry after a crash can succeed.

**Tools:**
- **Stateright** on the real settlement state machine, with 2 buyers, 2
  invoices, up to 2 crashes, and duplicate delivery of every message.
- Then **Turmoil** to drive the real pay front through the schedules the
  model found.
- **cargo-fuzz** on the x402 header and BOLT11 parsing.

**Waits on:** nothing; it can start now. It matters most before the
receiver switches to LND.

## 2. The credit and affiliate ledger

**The system:**
- **Credits:** bought by x402 or Lightning top-up, spent on usage.
- **Affiliate commission:** 20% of net Pro revenue for 12 months, a 30-day
  hold, then clawback on refund. Payout is monthly, at 10,000 sats or more,
  and only after the owner approves.

**Properties (always):**
- **L1:** money is conserved: credits in minus spent minus refunded equals
  the balance, for every account, after any sequence of events.
- **L2:** commission is never earned on revenue that was refunded inside the
  hold. A clawback after the hold never takes a balance below what's already
  been paid out without recording a debt.
- **L3:** no affiliate is paid twice for the same month.
- **L4:** self-referral never earns.

**Tools:**
- **Stateright** for the event sequences, with refunds, clawbacks and
  payouts interleaved.
- **proptest** for the pure commission arithmetic.
- **Kani** on sat and dollar conversions (rounding, overflow).

**Waits on:** billing going live for the Pro side. The credit side can start
with target 1.

## 3. LND payouts: never pay twice

**The system:** an outbound payout through `SendPaymentV2`, tracked with
`TrackPaymentV2`. Our sender already refuses to resend when the outcome is
unknown ([x402 over LND](../payments/2026-10-15-x402-over-lnd.md)).

**Properties (always):**
- **P1:** a payout is sent at most once per approved request, across worker
  crashes, restarts and lost responses.
- **P2:** an unknown outcome is resolved only by looking it up, never by
  sending again.
- **P3:** the sum of payouts never exceeds the approved amount or the
  hot-wallet cap.

**Property (eventually):**
- **P4:** every payout reaches succeeded, failed, or needs a person.

**Tools:**
- **Stateright** for P1–P3.
- A small **TLA+/Quint** spec for P4, since it's liveness.
- **Turmoil** against `lnd-sdk`'s fake node.

**Waits on:** LND phase 2 (#11757) and the owner's custody decisions
(#11761). Do it before payouts move to our node.

## 4. The LayerTree TEE signer's rule check

**The system:** the spec
[LayerTree on mainnet with a TEE](../research/layertree/2026-10-15-mainnet-tee-spec.md).
An enclave co-signer replaces the missing covenant opcodes, and its rule
check is the whole security of the tree.

**Properties (always):** LayerTree's own Lean theorems, mapped one to one:
- **no inflation** (`ExitK.no_inflation`);
- **conservation** over any step sequence (`Steps.sum`);
- **exits pay the leaf's own key** (`exit_available`);
- **no voucher replay** (`voucher_no_replay`);
- **path maintenance** (`path_maintenance`).

The predicate approves nothing these forbid.

**Tools:**
- **proptest** properties named after each theorem (in the lab crate,
  `crates/layertree-tee-lab`).
- Then **Kani** proofs on the Rust predicate itself.
- **cargo-fuzz** and Kani on the attestation report parser.
- **Lean** only if we need unbounded induction or to reuse their proofs, and
  only after a licence allows.
- Later, if multiple enclaves coordinate off-chain, **TLA+** or their planned
  **P** model for that protocol's liveness.

**Waits on:** the owner's go on the TEE path. The property tests can live in
the lab crate now.

## 5. Coder's hold rules and route admission (next release)

**The system:**
- Hard-to-undo steps wait for Y or N: deploys, merges, secrets, payments and
  DNS.
- A run only uses routes the owner's policy admits.

**Properties (always):**
- **C1:** no step classed as hard to undo runs without a recorded Y.
- **C2:** no run uses a model, route or limit outside its policy.
- **C3:** a refused step never runs later through a retry or another
  harness.

**Tools:**
- **proptest** and **Kani** on the classifier's decision table.
- **Stateright** on the hold, approve and run sequence across harnesses.

**Waits on:** the Coder binary is frozen at 1.0.0, so a finding here goes to
[next-release.md](../coder/next-release.md). Worth modelling now so the next
release starts with the bugs already known.

## Not targets

These are measured with outcomes and tested normally; don't verify them:
- routing quality;
- decision accuracy;
- model output;
- UI;
- docs;
- ordinary CRUD.
