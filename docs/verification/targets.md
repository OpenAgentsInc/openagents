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

## 2. Computers: the lifecycle, commands, files and usage

**Why second:**
- The [SDK delivery plan](../sdk/delivery-plan.md) (#11800) makes
  `oa-computers` a public, account-scoped API. People, our apps, Coder
  Desktop and agents will all create computers.
- It's the most concurrent thing we run: leased reconcilers, retried
  creates, drivers that answer "unknown", and wake, stop and delete racing
  each other.
- It already has the ideal shape for model checking. The lifecycle core in
  `coder-working-computer` is pure: `decide::decide` picks the next step from
  the record, and `transition::apply` changes it. A Stateright model can run
  those exact functions.
- Server-side only, so it's safe under the 1.0 freeze.

**Properties (always):**
- **K1: one computer per intent.** A create retried with the same
  idempotency key, by any number of clients or reconcilers, makes at most one
  computer and one meter (#11801, #11805).
- **K2: delete is final.** A deleted computer never comes back. No later
  wake, retry or stale reconciler revives it, and its meter stops.
- **K3: no done without observing.** An operation the driver answered
  "unknown" is reported done or failed only after an observation says so.
- **K4: one owner, always.** A computer, its files, its command sessions and
  its checkpoints are reachable only by the account that owns them, after
  any sequence of transfers, forks, restores and deletes
  (`admit_checkpoint_use`; #11801, #11803).
- **K5: the stored backend binding never changes** for a computer's life
  (#11809). Placement never chooses a driver lacking a profile the request
  needs, and refuses before any effect.
- **K6: command sessions lose nothing.** Output produced before an attach
  is still delivered. A cancel is confirmed or reported uncertain, never
  assumed (#11802).
- **K7: usage is conserved.** Reserved plus used never exceeds the quota.
  An uncertain reservation is settled exactly once, and nothing is charged
  twice (#11815).

**Properties (sometimes):**
- a computer can reach running, be stopped, woken, and deleted;
- a command can complete after a reconciler crash.

**Tools:**
- **Stateright** on the real `decide`/`apply` core plus the
  operations store, with 2 accounts, 2 computers, 2 reconcilers, up to 2
  crashes, and drivers that may answer done, failed or unknown.
- **Turmoil** to drive the real `oa-computers` HTTP API and a fake driver
  through the schedules the model finds.
- **cargo-fuzz** and **Kani** on the file-path check (#11803): no input path
  escapes the computer's root, traversal and symlink cases included.
- **proptest** on the egress allow-list matcher and credential-reference
  resolution (#11813): a reference resolves only for its owner, and the
  matcher never allows a host outside the profile.
- Counterexamples become cases in `computer-conformance`, so every driver
  (Boat, SPACE, own machine) is tested against them.

**Not verified here:** the Rust client's ergonomics, the OpenAPI document
(schema compatibility checks cover it), Coder Desktop's UI, and placement
*quality* (a measured decision, not a rule).

**Waits on:** nothing. K1–K5 can model today's internal API now. K6 and K7
start with #11802 and #11815.

## 3. The credit and affiliate ledger

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

## 4. LND payouts: never pay twice

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

## 5. The LayerTree TEE signer's rule check

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

## 6. Coder's hold rules and route admission (next release)

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
