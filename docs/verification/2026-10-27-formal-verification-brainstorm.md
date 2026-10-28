# Formal verification in OpenAgents: a brainstorm

*Draft, 2026-10-27. A thinking document, not a plan of record.*

This note asks where formal methods would pay for themselves in this
repository, mainly in Nostr and payments, and how Coax could make
verification part of the language instead of a separate toolchain.

It is grounded in what the repository holds today:

- the [OpenAgents Protocol](../protocol/openagents-protocol-spec.md)
  (§7.4 canonical bytes, §26 money, §35 conformance);
- the recovered `proto/` tree and its checks;
- the NIP mirrors in `nips/`;
- `coax/`, the compiler, and `metareal/`, the native home.

Payment and relay runtimes aren't in this checkout after the reset. Where
this note talks about them, it means the designs in the protocol spec and
the code that will implement them, most of it planned in Coax.

## 1. Why now

Three things make this the cheapest moment we will get.

1. **The contracts are fixed before the code.** The protocol spec
   already names the state machines that carry authority:
   - RUN, COORD and AUTO;
   - ENV and HOST grants;
   - ATT admission;
   - MKT record chains;
   - X402 status.

   §35.9 already requires "a bounded model or property test" for each of
   them before a package leaves alpha. Modeling them before implementing
   them costs a fraction of finding the bugs later.
2. **We own the language.** Coax doesn't have to guess what a program
   means. It has a bounded evaluator, fixed-width checked integers, no
   implicit conversions and no hidden control flow, and verification
   tools need exactly those properties. Each feature we add can be
   designed so a checker understands it.
3. **Money and signatures are unforgiving.** A double payment, a ledger
   that doesn't balance, an event ID computed over re-serialized bytes, or
   a grant that outlives its revocation are each one bug that costs
   trust permanently. Those are small, sharply defined properties, which
   is where formal methods are strongest.

The workspace rule already sets the discipline:
- narrow the production contract;
- model the bounded state space;
- run or document the checker;
- turn counterexamples into regression tests;
- never weaken runtime policy to make a model pass.

This note is about where to apply it first.

## 2. A ladder of assurance

"Formal verification" covers a wide range of cost. We should pick the
cheapest rung that actually covers the property and climb only when a
property earns it.

| Rung | What it gives | Cost | Tools |
| --- | --- | --- | --- |
| 0. Types and refusals | Whole classes of bug can't be written | Paid once, in language and schema design | Coax types, proto data classes, the generator's import refusals |
| 1. Property tests and fuzzing | Invariants hold on many generated inputs | Hours | Python `hypothesis` for tools, libFuzzer on generated C, `cargo fuzz`/`proptest` in Rust repos |
| 2. Differential testing | Two implementations agree | Hours to days | Coax vs a C reference (`examples/pulse`), evaluator vs generated C, our Nostr code vs reference libraries |
| 3. Bounded model checking | Every behavior up to a bound is safe, with a counterexample trace when one isn't | Days per model | TLA+ (TLC, Apalache), Alloy, CBMC/ESBMC on C |
| 4. Deductive proof | Holds for all inputs and all lengths | Weeks per property | Lean 4, Why3/Frama-C, Verus or Kani for Rust, SMT (Z3) behind small obligations |

The rungs aren't alternatives. Rung 0 removes what it can, rung 3 checks
the protocol design, and rungs 1–2 check that the code matches the design.
Rung 4 is for the few properties where a bound isn't convincing, such as
ledger conservation across all histories, or the core semantics of Coax
itself.

**Small-scope hypothesis.** Most protocol bugs show up with two or three
actors, a handful of messages and one crash. Bounded checking at small
sizes finds nearly everything for most of our state machines, so it is our
default for protocols.

## 3. Payments and money

The money rule (§26.1) is already partly verified by construction:
- `common.v1.Amount` can only hold msat or a Taproot Assets amount;
- `payment.v1.Rail` lists only allowed rails;
- the generator refuses imports of the Cashu and Fedimint mirrors.

That's rung 0 done right. Everything below adds the dynamic properties.

### 3.1 The ledger

`ledger.v1` is specified as double-entry and append-only, with every entry
pointing at the receipt that caused it. Properties to check:

- **Conservation.** For every posting, debits equal credits, and the sum
  of all account balances only changes through declared external entries
  (deposits, payouts).
- **Append-only.** No operation rewrites or deletes an entry; corrections
  are new entries that reference the corrected one.
- **Causality.** Every entry references exactly one receipt, and one
  receipt causes at most one posting (idempotency).
- **No unbacked balance.** A balance goes negative only on accounts whose
  policy allows credit, and never by more than the limit.
- **No overflow.** msat totals fit `u64` (about 1.8e19 msat, roughly 184
  million BTC), so checked arithmetic is enough. Accumulated totals still
  need a stated bound, because "it never overflows in practice" is how
  overflow bugs ship.

How: a TLA+ model of the ledger with concurrent posting, retries and a
crash between "receipt recorded" and "entry posted". The invariants are
conservation, idempotency and causality. Then property tests that replay
generated histories against the real implementation and check the same
invariants on its output. Conservation over all histories is a good first
candidate for a Lean proof, because the model is small and the property
is permanent.

### 3.2 Lightning payment attempts and x402

Paying a Lightning invoice has an outcome that is often handled badly:
**unknown**. The node accepted the payment, then the connection dropped.
The payment may still settle, or may already have.

- **Safety:**
  - never pay the same obligation twice;
  - never mark an obligation unpaid while an attempt might still settle;
  - a revealed preimage means the payment happened.
- **Liveness:** with nodes eventually reachable, every attempt reaches
  succeeded or failed; an unknown attempt is resolved by querying, never
  by retrying blind.
- **x402 proof consumption:**
  - a preimage buys at most one execution, durably, across restarts;
  - an execution never runs before its proof is recorded as consumed.

  The spec says the pre-reset `crates/x402` had "durable proof
  consumption and recovery". That's a claim a model should back.

How: one TLA+ model with three actors (payer, node, payee/merchant),
crash and restart of each, and a durable store. TLC at small bounds will
find the double-spend and lost-update traces quickly if they exist.
Recording the model next to the implementation, and turning each trace
into a test, is the main deliverable.

### 3.3 Market orders (NIP-MKT) and agent labor

`market.v1` has a sequenced private record chain (`seq`, `prev` digest)
and an order lifecycle: RFQ, quote, order, acknowledgement, payment,
delivery, review, dispute, refund, close. `labor.v1` adds verification,
rework (at most three rounds) and resolution.

Static invariants are cheap to state and should be checked functions, not
prose:
- the deadlines are ordered:
  `quote_expires_at ≤ order_confirm_by < delivery_due_at < review_due_at < payment_due_at < retain_until`;
- `amount_due_msat ≤ price_msat`;
- a refund is at most what was paid;
- `max_reworks ≤ 3`.

Dynamic properties need a model:
- each order ends in `Close` or a resolution by its last deadline;
- a cancel racing a delivery has one defined winner;
- the chain has no forks: `seq` is contiguous and `prev` matches;
- the provider is never paid twice for one order, and paid at all only
  if the order was accepted or resolved;
- a buyer who goes silent can't block resolution forever (the
  `buyer_unavailable` path).

How: TLA+ for the lifecycle with buyer, provider and resolver, and timers
for each deadline. This is the model most likely to find real design
bugs, because each record type was specified on its own and the
interleavings between them were never checked.

### 3.4 Spend grants

HOST `spend-grant`, `spend-request` and `spend-receipt` let an agent spend
within a budget. Properties:
- the sum of receipts never exceeds the grant's cap;
- no spend after expiry or revocation, including a spend already in
  flight when revocation lands;
- a grant can't be widened by replaying or reordering requests.

This one is small enough to model in an afternoon, and it guards real
money directly.

### 3.5 Units

`Amount` (msat or asset units) and `MicroAmount` (an accounting unit such
as USD) must never be added together. Coax should make that a type error,
not a review comment (§6.2 below).

### What we don't verify

We don't re-prove LND, LDK, Breez or Taproot Assets internals, or
Bitcoin's cryptography. We pin versions, run their published test vectors
where they exist, and model their observable behavior: an attempt can
succeed, fail or be unknown. Verification covers our use of them.

## 4. Nostr

### 4.1 Event identity and signatures

- **Event ID.** The ID is SHA-256 of the NIP-01 serialization. Check our
  serializer against reference implementations (`nostr-effect` in this
  workspace, plus rust-nostr and nostr-tools) on generated events with
  hostile strings: escapes, unicode, control bytes, long content.
- **Verify the original bytes.** A signature is verified over the bytes
  that arrived, never over a re-serialization (§35.4). A property test can
  prove it: mutate the representation without changing the meaning, and
  verification must still use the original bytes.
- **JCS (RFC 8785).** It hashes most OpenAgents records (§7.4). Check it
  differentially against a reference implementation, and fuzz it with the
  §35.5 hostile inputs:
  - duplicate keys;
  - integers above 2^53−1;
  - non-finite numbers;
  - excessive depth.

  The domain-separation prefixes (`"openagents.run-record.v1\n"` and the
  others) get golden vectors.

### 4.2 Replaceable and addressable events

Relays and clients keep "the latest" replaceable event (newest
`created_at`, ties broken by lowest ID) per key. That rule is a merge
function, and it should be a **join-semilattice**: commutative,
associative and idempotent. If it is, any two relays that have seen the
same events agree regardless of arrival order. That's easy to state and
check:
- property tests on random event sets and orders;
- a short Lean proof, because the rule is a total order.

NIP-09 deletions interact with it (a delete followed by an older replace),
and that interaction is where bugs live. Add it to the model.

### 4.3 Authority models

Several Block NIPs make negative authority claims.
- NIP-MP: "a project's signer gains no authority over any member
  repository".
- NIP-OA: owner attestation, and how agents inherit push access.

The right tool is Alloy, or a small Datalog-style rule check:
- model principals, keys, attestations and grants;
- ask whether any chain of valid events grants push access without an
  attestation from the repository owner.

Alloy either finds such a chain or shows that none exists up to the
bound. It's the same question as "can membership become permission?",
asked by machine.

### 4.4 Leases and state machines

NIP-PL push leases, and other NIPs with expiry and renewal, are small
timed state machines. One shared TLA+ pattern for "lease with renewal and
clock skew" covers all of them. The properties:
- no delivery to an expired lease;
- renewal never resurrects a revoked lease;
- bounded clock skew never extends a lease past its stated maximum.

### 4.5 Relays and filters

The relay itself lives in the `immortal` repository (Rust). Two things are
worth doing there:
1. Write NIP-01 filter matching as a small reference function. Run the
   relay's indexed implementation against it on random events and filters
   (rung 2). Index shortcuts are where matching bugs hide.
2. Use Kani or proptest on the decoders, size limits and canonical-ID
   code, and `cargo fuzz` on everything that parses network input.

### 4.6 Encryption

For NIP-44 and NIP-59 wrapping we use audited code and its official test
vectors. What we verify is our handling:
- the right key per conversation;
- no plaintext in logs (§35.7);
- no downgrade to NIP-04.

## 5. Everything else worth a model

- **Protocol evolution.** Breaking-change checks against the frozen
  baselines in `proto/` are already verification. Add property tests for
  missing-versus-zero presence and for the data classes (§35.6): strict
  classes refuse unknown fields, lenient ones keep or drop them as
  specified.
- **The Metareal migration.** Legacy-ID migration must be idempotent,
  collision-free and never reinterpret an old signature under a new
  domain. These already have tests; they're also checkable statements
  for an exhaustive check over a small catalog.
- **Runs, coordination and automation.** Leases, cancellation, approvals
  and exactly-once tool effects. Two key properties:
  - no effect runs without its approval;
  - a cancelled run never produces a later effect.

  These are classic TLA+ targets, worth modeling before the P4
  implementation.
- **Determinism (Coax stage 5).** "The same replay at 1, 2, 4 and 8
  workers" is a spec gate. A model of the job scheduler with effect
  annotations can show that declared-disjoint effects can't race. Tests
  then check the real thing.
- **The iOS home.** Its protobuf admission (`metareal/home.coax`) refuses
  unknown and duplicate fields and bounds every varint. That's a small
  decoder, a good first target for exhaustive checking of short inputs
  and for libFuzzer on the generated C (the sanitizer harness already
  exists).

## 6. Making verification native in Coax

This is the part only we can do. Coax already has three things most
languages lack for this:
- **A bounded, side-effect-free evaluator in the compiler.** `#run` and
  `#assert` execute real code while compiling.
- **Exact integer semantics.** Every integer has a fixed width. Overflow,
  shifts, casts and bounds are checked in debug and at compile time.
- **No hidden behavior.** No implicit conversions, no exceptions, no
  constructors, and left-to-right evaluation. What a line does is on the
  line.

Those make several features cheap.

### 6.1 Compile-time exhaustive checks (`#test`, `#forall`)

The evaluator can run tests while compiling, and for small domains it can
run all of them:

```coax
deadlines_ordered :: (t: Terms) -> bool { ... }

#forall a: u8, b: u8 {
  #assert add_msat(a, b) == add_msat(b, a);
}

#test "a refund never exceeds payment" {
  for paid: 0..<1000 { for refund: 0..<1000 {
    #assert ifx refund <= paid then refund_ok(paid, refund) else !refund_ok(paid, refund);
  } }
}
```

- `#forall` over `u8`, `bool`, enums and small ranges enumerates every
  value. That turns the small-scope hypothesis into a compiler feature.
- Failures print the counterexample as a Coax value, so it can be pasted
  into a regression test.
- Tests need their own step budget, separate from the 100,000 steps that
  keep ordinary compilation fast, plus a `coax test` command that runs
  them.

This is the cheapest high-value step. It reuses the evaluator we already
have and needs no external tool.

### 6.2 Distinct types for units and identities

```coax
Msat :: distinct u64;
AssetUnits :: distinct u64;
EventId :: distinct [32] u8;
```

- A distinct type has the same layout but a different name: no implicit
  conversion, explicit `cast` only.
- msat plus asset units, or an event ID passed where a pubkey goes,
  becomes a type error.
- Combined with checked arithmetic, money arithmetic can't silently wrap
  or mix units.

This is rung 0 for §3.5.

### 6.3 Checked arithmetic everywhere money lives

Release builds currently wrap on integer overflow. For money and protocol
code that default is wrong. Proposal: a per-file or per-procedure
directive such as `#arithmetic checked`. Overflow then traps in release
too, and the compiler refuses wrapping arithmetic unless it's spelled out
(`wrap_add`, `cast,trunc`). The generated C already has the checked
helpers. The cost is a branch per operation, which money code can afford.

### 6.4 Contracts: `#requires`, `#ensures`, `#invariant`

```coax
post :: (ledger: [] Entry, e: Entry) -> bool
  #requires e.debit == e.credit
  #ensures  balance_sum(ledger) == old(balance_sum(ledger))
{ ... }
```

Contracts have three uses, and each can come separately:
1. **Debug checks.** They are checked at run time in debug builds, like
   asserts with better messages.
2. **Compile-time evidence.** They are checked in `#run` and `#forall`
   executions, so every compile-time test also tests the contracts.
3. **Proof obligations.** They are exported to a verifier (§6.6). Ghost
   state (`#ghost` locals and fields, erased from generated code) lets a
   contract mention history (`old(...)`) without costing anything at run
   time.

### 6.5 State machines as declarations

Most of §3 and §4 is state machines. If Coax could declare them, one
declaration would give the implementation, the model and the tests:

```coax
OrderState :: state_machine {
  states: quoted, ordered, delivered, accepted, disputed, refunded, closed;
  ordered -> delivered  when provider_submits;
  ordered -> refunded   when cancel_before_delivery;
  delivered -> accepted when review_accepts;
  delivered -> disputed when review_rejects;
  ...
  #invariant paid_total <= price;
  #eventually closed;
}
```

The compiler checks:
- every state is reachable and every event is handled;
- the transition table is the only way to change state.

It can also explore the state space with the existing evaluator: a
bounded breadth-first search over generated event sequences, checking
invariants at each step. That's a small built-in model checker, with
counterexamples printed as event sequences. For models too large for that,
the same declaration can emit a TLA+ module, so the model can't drift
from the code. This is the long-term answer to "the model and the code
disagree".

### 6.6 Paths to external verifiers

Coax has two properties that make export to external checkers realistic:
- **Fixed-width arithmetic.** Coax integers are SMT bit-vectors exactly:
  same width, same overflow rules. A pure procedure over integers,
  structs and fixed arrays translates directly to an SMT query. `#prove`
  on a pure procedure could discharge its contracts with Z3 and report a
  counterexample model when one exists.
- **Plain C output.** The generated C is ordinary C11 with explicit
  checks, which CBMC and ESBMC handle well. Running CBMC over the emitted
  C proves the absence of undefined behavior and failed checks up to a
  loop bound, with no new frontend. The sanitizer runs we already do in
  `metareal/` are the dynamic version of this.

Later, a Lean 4 model of core Coax semantics could support two claims:
- that the evaluator and the C emitter agree (translation validation);
- that well-typed programs don't hit undefined behavior.

That's a research-sized project; §7 puts it last on purpose.

### 6.7 Verify the generator, not each output

`#import_proto` will generate bounded codecs for every package. Verifying
each generated decoder is wasteful. Instead:
- verify the template once, with CBMC on representative instances;
- fuzz the generated decoders with libFuzzer on the emitted C;
- keep the §35 conformance vectors as the shared oracle.

The same applies to `state_machine`: check the lowering once, then trust
each instance's generated transition function.

### 6.8 The evaluator as reference semantics

The compile-time evaluator is already a second implementation of Coax. We
should make it the reference:
- generate random well-typed programs, Csmith-style;
- run each in the evaluator and as compiled C, in debug and release;
- require identical output, or the same trap.

Chapter 12 of the book shows this by hand. Making it systematic catches
emitter bugs before agents ship code that depends on them.

## 7. A staged plan

| Stage | Work | Exit evidence |
| --- | --- | --- |
| V0. Inventory | List every invariant the protocol spec states, with an owner, its rung and where it is checked. Add the missing ones to `INVARIANTS.md` as they get owners. | A table in `docs/verification/`, linked from `INVARIANTS.md` |
| V1. Cheap and native | `#test` and `#forall` in Coax with their own budget and a `coax test` command; MKT deadline and amount invariants written as checked Coax functions; JCS and NIP-01 differential tests against references | Tests run under `coax/build.sh test`; counterexamples print as values |
| V2. First models | TLA+ models for the ledger (conservation, idempotency), Lightning attempts with unknown outcomes, x402 proof consumption, and spend grants; Alloy for NIP-OA/NIP-MP authority | Each model checked at stated bounds in a few minutes locally, with traces kept as tests |
| V3. Money in Coax | `distinct` types, `#arithmetic checked`, and debug contracts; the first ledger and payment code written with them | Unit mixing and silent wrap are compile errors in money code |
| V4. Machines and tools | `state_machine` declarations with the built-in bounded explorer and TLA+ export; CBMC over emitted C; `#prove` through Z3 for pure procedures | One real protocol (MKT orders) implemented from a declaration, with its model generated from the same source |
| V5. Deep proofs | Lean proofs of ledger conservation and the replaceable-event lattice; a Lean model of core Coax for translation validation | Proof artifacts checked locally, with stated assumptions |

V1 and V2 are independent and can start now. V3 waits on the payment code
itself (protocol phase P5); V4 waits on Coax aggregate and protocol
ingestion work (Coax stage 2–3, protocol phase P1).

## 8. Where things live

- `verify/<domain>/` for models: `verify/ledger/Ledger.tla`,
  `verify/nostr/authority.als`. Each directory gets a README that states:
  - the production contract it models;
  - its bounds;
  - how to run it;
  - the last result and its date.
- Counterexamples become regression tests in the owning package, never
  only in the model.
- `scripts/verify.sh` runs every model at its documented bounds.
- Checks run locally or on owned machines; there are no GitHub Actions
  (an `INVARIANTS.md` rule). A model must finish within a stated time
  budget, or it gets split.
- Traces never contain production data, secrets or real keys.
- Changing a model's invariant is a policy change: update `INVARIANTS.md`
  in the same commit.

## 9. Open questions

1. TLA+ or a Coax-native explorer first for protocol models? TLA+ is
   proven and fast to start. Native state machines avoid drift but take
   longer. A reasonable split: TLA+ for V2, native for V4, with V4
   replacing the hand models it can generate.
2. Should release builds of money code trap on overflow by default
   (§6.3), or only where a file opts in?
3. How much proof is enough for ledger conservation: a bounded model plus
   property tests, or a Lean proof before real funds move?
4. Who owns the reference implementations used for differential Nostr
   testing, and how are their versions pinned (`nips/upstream-lock.json`
   already pins the text)?
5. Is a Lean model of Coax worth it before Coax has users beyond the iOS
   home, or only once it carries money?

The common thread: write each invariant once, as close to the code as
possible, and have a machine check it at the cheapest rung that's
convincing. Coax gives us a rare chance to put most of those checks in the
language itself.
