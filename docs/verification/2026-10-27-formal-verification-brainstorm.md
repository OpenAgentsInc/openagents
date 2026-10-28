# Verification in OpenAgents: Nostr, payments and Coax

*Draft, 2026-10-27, revised the same day. Thinking document, not a plan of
record.*

This note asks how we make the code that carries money and signatures
trustworthy, mainly Nostr and payments. It sets two rules:

1. **When in doubt, do what Bitcoin Core does.** It is one of the most
   attacked and longest-running codebases there is, and its approach is
   public. Section 2 reviews it from the source.
2. **No external toolchains outside Coax if we can help it.** Coax already
   needs a C compiler, because it emits C. Everything else should be Coax:
   tests, exhaustive checks, fuzzing, models and test vectors. Section 7
   describes the language features that make that possible.

It is grounded in what this repository holds:
- the [OpenAgents Protocol](../protocol/openagents-protocol-spec.md)
  (§7.4 canonical bytes, §26 money, §35 conformance);
- `proto/` and its frozen baselines;
- the NIP mirrors in `nips/`;
- `coax/`, the compiler;
- `metareal/`, the native home.

Payment and relay runtimes aren't in this checkout after the reset; where
this note talks about them, it means the spec's designs and the Coax code
that will implement them.

## 1. Why now

- **The contracts are fixed before the code.** The protocol spec names
  the state machines that carry authority:
  - RUN, COORD and AUTO;
  - ENV and HOST grants;
  - ATT admission;
  - MKT record chains;
  - X402 status.

  §35.9 requires a bounded model or property test for each of them before
  a package leaves alpha. Doing that before the implementation is cheap.
- **We own the language.** Coax has a bounded evaluator inside the
  compiler, fixed-width checked integers, no implicit conversions and
  left-to-right evaluation. Each new feature can be designed so that it
  is easy to check.
- **Money and signatures are unforgiving.** Each of these is one bug that
  costs trust permanently:
  - a double payment;
  - a ledger that doesn't balance;
  - an event ID computed over re-serialized bytes;
  - a grant that outlives its revocation.

The workspace rule already sets the discipline:
- narrow the contract;
- check the bounded state space;
- turn every counterexample into a regression test;
- never weaken runtime policy to make a check pass.

## 2. What Bitcoin Core actually does

Reviewed at `bitcoin/bitcoin` commit `aa76bd2` (2026-10-27). Points are
cited by path.

**No external formal methods in the node.** There is no TLA+, Coq, Lean or
model checker in the tree. Assurance comes from many cheaper layers,
applied everywhere and run all the time.

1. **Assertions can never be compiled out.** `src/util/check.h` contains
   `#error "Cannot compile without assertions!"` when `NDEBUG` is
   defined. There are three levels:
   - `Assert` for invariants that must abort;
   - `Assume` for assumptions that abort in debug and fuzz builds and are
     tolerated in release;
   - `CHECK_NONFATAL` for errors reachable from user input, which return
     an error instead of crashing.

   `doc/developer-notes.md` prefers `static_assert`, "compile-time
   checking is preferred over run-time checking".
2. **Money is a range-checked integer.** `src/consensus/amount.h` defines
   `CAmount` as `int64_t`, `MAX_MONEY` as `21'000'000 * COIN`, and
   `MoneyRange(v)` as `0 <= v <= MAX_MONEY`. The comment there says the
   exact value of `MAX_MONEY` is consensus-critical. Consensus code checks
   the range at every accumulation: `src/consensus/tx_check.cpp` adds each
   output and re-checks `MoneyRange(nValueOut)` after each addition.
   `src/util/overflow.h` provides `CheckedAdd` and `SaturatingAdd` (and the
   shift equivalents), returning an optional instead of overflowing.
3. **Fuzzing is first-class.** `src/test/fuzz/` has about 143 harnesses:
   - network message processing and every deserializer;
   - integer and addition overflow, and multiplication overflow;
   - the script interpreter.

   A shared seed corpus lives in its own repository
   (`bitcoin-core/qa-assets`, `doc/fuzzing.md`). Harnesses run under
   sanitizers, MSan and Valgrind in CI.
4. **Sanitizers are part of the normal suite.** `ci/test/` has jobs for:
   - `address,float-divide-by-zero,integer,undefined`;
   - `thread`;
   - `memory`;
   - Valgrind.

   "The test suite should pass cleanly with the `thread` and `undefined`
   sanitizers" (`doc/developer-notes.md`).
5. **Expensive consistency checks are built into the product.**
   `-checkblockindex`, `-checkmempool` and `-checkaddrman`
   (`src/init.cpp`) re-verify whole data structures every *n* operations.
   They are on by default in regtest, so every functional test runs them.
6. **Static concurrency checks.** About 900 `GUARDED_BY` and
   `EXCLUSIVE_LOCKS_REQUIRED` annotations are checked by Clang's thread
   safety analysis. `DEBUG_LOCKORDER` adds run-time lock-order checking.
7. **Where it matters most, tests are exhaustive on a small instance.**
   - libsecp256k1 runs its group and signing code on a curve of order 13,
     where every case can be enumerated (`src/secp256k1/src/tests_exhaustive.c`,
     `EXHAUSTIVE_TEST_ORDER`).
   - minisketch decodes every sketch of small parameters
     (`src/minisketch/src/test.cpp`).

   The code is written so that a small instance exercises the same
   paths as the real one.
8. **Constant-time is tested, not assumed.** `src/secp256k1/src/ctime_tests.c`
   runs the secret-key code under Valgrind or MSan with secrets marked
   uninitialized, so any secret-dependent branch is reported.
9. **The one formal proof is narrow.** libsecp256k1's `sage/` scripts
   (`prove_group_implementations.sage`) symbolically check the elliptic
   curve formulas. External proof tools appear in exactly one place, for
   frozen math that rarely changes.
10. **Test vectors are shared data.** `src/test/data/` holds
    `script_tests.json`, `tx_valid.json`, `tx_invalid.json`, `sighash.json`,
    the BIP341 and BIP352 vectors, base58 and key encodings. Other
    implementations use the same files. Tests read vectors; they don't run
    other implementations.
11. **Builds are reproducible and attested.** Guix builds
    (`contrib/guix/`, `guix-attest`, `guix-verify`) let independent
    builders publish hashes of identical binaries.
12. **Dependencies are few and owned.** Libraries such as secp256k1 and
    minisketch are subtrees checked for consistency with upstream
    (`test/lint/git-subtree-check.sh`). `depends/` builds everything from
    source.
13. **Review is the bottleneck, deliberately.** "In-depth reviewing and
    testing are the bottleneck of the project" (`CONTRIBUTING.md`).
    Consensus-critical code is treated differently from everything else
    (`doc/developer-notes.md`, "Consensus Compatibility"). Changes there
    are rare, small, and reviewed by many people.

The lesson is that Bitcoin Core doesn't buy assurance with a proof
assistant. It buys it with:
- checks that are always on;
- money that can't leave its range;
- fuzzing and sanitizers on every parser;
- exhaustive tests of small instances;
- whole-structure consistency checks;
- shared vectors;
- reproducible builds;
- a hard line around consensus code.

We should copy that, and put as much of it as possible inside Coax.

## 3. The approach

**Native first.** Every check below is either Coax code or a Coax compiler
feature. The C compiler that builds Coax output is the one host tool we
accept. Its sanitizers (ASan, UBSan, TSan, MSan) come with it, and we use
them as Bitcoin Core does. We don't add TLA+, Alloy, Lean, Z3, CBMC or a
second language for tests.

The layers, cheapest first:

| Layer | What it gives | Bitcoin Core's version | Ours |
| --- | --- | --- | --- |
| Types and refusals | Bugs that can't be written | `CAmount`, `static_assert` | Coax types, `distinct` and range types, proto data classes, generator refusals |
| Always-on checks | Violations stop the program | `Assert`, no `NDEBUG` | Bounds, overflow, casts and asserts checked in every build; `assume` for expensive debug checks |
| Exhaustive small instances | Every case of a small model | secp256k1 order 13, minisketch | `#forall` and `#test` run by the compiler's evaluator |
| State exploration | Every interleaving up to a bound | (not used) | `state_machine` declarations, explored by the evaluator |
| Fuzzing | Hostile inputs at scale | `src/test/fuzz/`, qa-assets | `coax fuzz`, coverage counters emitted by Coax |
| Sanitizers | Memory, UB, races | ASan, UBSan, TSan, MSan, Valgrind | The same, through `COAX_CC` |
| Consistency checks | Whole-structure invariants in running code | `-checkmempool`, `-checkblockindex` | `#consistency` procedures run every *n* operations in test builds |
| Shared vectors | Agreement with other implementations | `src/test/data/*.json` | Pinned vector files read with `#embed` |
| Reproducible builds | Everyone ships the same bytes | Guix attest | Deterministic Coax output, hashes attested by independent builders |

**Small-scope rule.** Most bugs show up with two or three actors, a few
messages and one crash. Exhaustive checks at small sizes find nearly all
of them, which is why secp256k1 tests at order 13. Authority code should
be written so that it runs unchanged on a small instance: amounts as
`u8`, three accounts, a deadline space of ten ticks.

**Authority-critical code.** Bitcoin Core treats consensus code
differently. We name ours, and hold it to stricter rules:
- the ledger;
- payment-attempt and x402 proof consumption;
- spend grants;
- Nostr signature and event-ID verification;
- JCS hashing;
- market record chains.

Section 9 lists the rules.

## 4. Payments and money

The money rule (§26.1) is already partly enforced by construction:
- `common.v1.Amount` can only hold msat or a Taproot Assets amount;
- `payment.v1.Rail` lists only allowed rails;
- the generator refuses imports of the Cashu and Fedimint mirrors.

### 4.1 Money is a range type

Follow `MoneyRange` exactly:

```coax
MAX_MSAT :: 21_000_000 * 100_000_000 * 1000;   // 2.1e18, fits s64
Msat :: distinct s64 #range(0, MAX_MSAT);
```

- Every construction, cast and arithmetic result is range-checked, in
  every build.
- Sums go through a checked add that returns failure instead of
  trapping, wherever the input comes from outside (the `CheckedAdd`
  pattern). Each running total is re-checked after each step, as
  `tx_check.cpp` does.
- `Msat`, Taproot Assets `AssetUnits` and accounting `MicroAmount` are
  distinct types: adding two of them is a compile error.

### 4.2 The ledger

`ledger.v1` is double-entry and append-only, and every entry points at the
receipt that caused it. Bitcoin Core never proves that its UTXO set
conserves value. Instead:
- every block is checked as it arrives (inputs at least outputs, all in
  range);
- whole-structure checks re-verify everything in tests;
- the UTXO set has a running commitment.

We do the same:
- **Per-posting checks, always on.** Debits equal credits; every amount is
  in range; the entry references exactly one receipt, and the receipt
  hasn't posted before. A failure refuses the posting.
- **A consistency check.** A `#consistency` procedure recomputes every
  balance from the entries. It confirms that no posting rewrote history,
  that no unbacked balance exists, and that the totals match. It runs
  every *n* postings in test builds, and on demand in production.
- **A running commitment.** Keep a hash over the entry sequence, like the
  UTXO set hash, so replicas and backups can prove they hold the same
  ledger without comparing it entry by entry.
- **Exhaustive small instances.** Run the ledger code with `u8` amounts and
  three accounts over every short sequence of postings, retries and one
  crash point, and check conservation and idempotency after each step.
- **Fuzzing.** Fuzz the posting and decoding paths with arbitrary bytes.

### 4.3 Lightning payment attempts and x402

A Lightning payment can end in **unknown**: the node accepted the payment,
then the connection dropped. The properties:
- never pay one obligation twice;
- never mark an obligation unpaid while an attempt might still settle;
- resolve an unknown attempt by querying, never by retrying blind;
- a revealed preimage means the payment happened;
- for x402, one preimage buys at most one execution, durably, and
  execution never starts before consumption is recorded.

Write the attempt lifecycle as a Coax `state_machine` (§7.5), with
payer, node and merchant, a durable store, and a crash at every step. The
evaluator explores every interleaving to a small bound. Each counterexample
is printed as an event sequence and kept as a test.

### 4.4 Market orders and labor (NIP-MKT, `labor.v1`)

Static rules are plain checked functions, run on every record:
- `quote_expires_at ≤ order_confirm_by < delivery_due_at < review_due_at < payment_due_at < retain_until`;
- `amount_due_msat ≤ price_msat`;
- a refund is at most what was paid;
- `max_reworks ≤ 3`;
- `seq` is contiguous and `prev` matches.

The lifecycle (RFQ through close, with cancel, dispute, rework and refund)
is a state machine with buyer, provider and resolver, and a timer per
deadline. Explore it exhaustively for:
- every order ending by its last deadline;
- one winner when cancel races delivery;
- no double payment;
- a silent buyer never blocking resolution.

### 4.5 Spend grants

For HOST spend grants, requests and receipts, the properties are:
- the sum of receipts stays within the cap;
- nothing is spent after expiry or revocation, including a spend already
  in flight when revocation lands;
- replaying or reordering requests never widens a grant.

This is a small state machine, and it guards real money directly.

### 4.6 What we don't verify

We don't re-check LND, LDK, Breez or Taproot Assets internals, or Bitcoin's
cryptography. We pin versions and keep their published vectors as data,
and we model their observable outcomes: succeeded, failed or unknown.

## 5. Nostr

- **Event IDs and signatures.** An event ID is SHA-256 of the NIP-01
  serialization. Keep a pinned vector file of events with expected IDs
  and signature results, generated once and hashed like
  `nips/upstream-lock.json`. Include hostile strings: escapes, unicode,
  control bytes. Coax tests read it with `#embed`, the way Bitcoin Core
  reads `sighash.json`. Signatures are verified over the bytes that
  arrived, never a re-serialization (§35.4); a test changes the
  representation and checks that verification still uses the original.
- **JCS (RFC 8785).** It hashes most OpenAgents records (§7.4). Keep vectors
  for each domain prefix (`"openagents.run-record.v1\n"` and the others),
  and fuzz the parser and canonicalizer with the §35.5 hostile inputs:
  - duplicate keys;
  - integers above 2^53−1;
  - non-finite numbers;
  - excessive depth;
  - oversize input.
- **Replaceable and addressable events.** "Newest `created_at`, ties to the
  lowest ID" is a merge rule. It must be commutative, associative and
  idempotent, so relays converge regardless of arrival order. Check it
  with `#forall` over every ordering of small event sets, including NIP-09
  deletions arriving before, between and after replacements.
- **Authority claims.** NIP-MP says a project's signer gains no authority
  over its repositories; NIP-OA defines how agents inherit push access.
  Model principals, attestations and grants as Coax data, and enumerate
  every chain of up to *k* valid events. Check that none grants push
  access without the repository owner's attestation.
- **Leases.** NIP-PL push leases and other expiring grants share one
  pattern: no delivery to an expired lease, renewal never revives a
  revoked lease, and bounded clock skew never extends a lease past its
  maximum. Write it as one reusable state machine.
- **Relay filters.** In the `immortal` relay (Rust, its own repository),
  keep NIP-01 filter matching as a short reference function. Fuzz the
  indexed implementation against it. That repository follows its own
  rules for tools.
- **Encryption.** For NIP-44 and NIP-59, use audited code and its official
  vectors. Test our handling: the right key per conversation, no plaintext
  in logs (§35.7), and no downgrade to NIP-04. For secret-key code in Coax,
  copy `ctime_tests.c`: run under MSan with secrets marked uninitialized,
  so a branch on a secret is reported.

## 6. Elsewhere

- **Protocol evolution.** Breaking-change checks against `proto/`'s frozen
  baselines are Bitcoin Core's previous-releases tests for us. Keep them,
  and add missing-versus-zero presence and data-class strictness (§35.6)
  as exhaustive checks over small messages.
- **The Metareal migration.** Migrations must be idempotent and
  collision-free, and never reinterpret a signature under a new domain.
  These are tested today; enumerate small catalogs too.
- **Runs, coordination and automation.** No effect without its approval; a
  cancelled run never produces a later effect; a lease is held by one
  holder. These are state machines, worth exploring before the P4
  implementation.
- **Determinism.** "The same replay at 1, 2, 4 and 8 workers" (Coax stage
  5) gets TSan in the suite as Bitcoin Core runs it. Later, effect
  annotations should be checked by the compiler the way Clang checks
  `GUARDED_BY`.
- **The iOS home.** Its protobuf admission (`metareal/home.coax`) is a
  bounded decoder. It is the first `coax fuzz` target, and it already has
  an ASan/UBSan harness.

## 7. Verification features for Coax

Coax already has what Bitcoin Core had to build or borrow:
- a bounded evaluator inside the compiler;
- integers with exact, checked semantics;
- bounds and cast checks in every build;
- deterministic output.

These features build on that, in rough order of value.

### 7.1 Checks that never turn off

Bitcoin Core refuses to compile without assertions. Coax should have the
same rule.
- Bounds, casts, shifts, division and `assert` are checked in every
  build, as they are today. No flag removes them.
- `assume(cond)` is for checks too expensive for release. It runs in
  debug, test and fuzz builds, and is compiled out of release only.
- Errors from external input are returned, not asserted: this is
  `CHECK_NONFATAL`'s role, and in Coax it is ordinary error values.
- **Release overflow.** Release builds currently wrap integer overflow.
  Following Bitcoin Core, authority-critical code traps on overflow in
  every build. Decision: `#arithmetic checked` is the default for the
  modules named in §3. Elsewhere, wrapping in release stays allowed for
  measured hot loops, and is spelled out (`wrap_add`, `cast,trunc`) where
  it's intended.

### 7.2 Distinct and range types

`Msat :: distinct s64 #range(0, MAX_MSAT);` has the same layout as `s64`
and a different name. There are no implicit conversions; casts are
explicit and checked; every result is range-checked. This is `CAmount`
plus `MoneyRange`, enforced by the compiler instead of by convention.
`EventId :: distinct [32] u8;` and `Pubkey :: distinct [32] u8;` can't be
swapped.

### 7.3 `#forall` and `#test`: exhaustive checks in the compiler

```coax
#forall a: u8, b: u8 {
  #assert merge(a, b) == merge(b, a);
}

#test "refunds never exceed payment" {
  for paid: 0..<1000 { for refund: 0..<1000 {
    #assert refund_ok(paid, refund) == (refund <= paid);
  } }
}
```

- `#forall` enumerates `u8`, `bool`, enums and small ranges completely.
  This is the secp256k1 exhaustive suite as a language feature.
- A failure prints the counterexample as a Coax value, ready to paste into
  a regression test.
- Tests get their own step budget, separate from the 100,000 steps that
  keep ordinary compiles fast, and run with `coax test`.
- Code is written so it can run on a small instance. For example, the
  ledger takes its amount type and its maximum as constants. Until Coax
  has generics, that means a small-instance build of the same file.

### 7.4 `#consistency`: whole-structure checks

```coax
check_ledger :: (l: Ledger) #consistency { ... }
```

A `#consistency` procedure:
- runs every *n* mutating operations in test builds;
- runs on demand in production;
- never runs in production unless asked.

This is `-checkmempool` and `-checkblockindex`, declared in the source
next to the structure it checks.

### 7.5 `state_machine` declarations with built-in exploration

```coax
Attempt :: state_machine {
  states: created, sent, unknown, succeeded, failed;
  created -> sent        when pay;
  sent -> succeeded      when settled;
  sent -> failed         when rejected;
  sent -> unknown        when connection_lost;
  unknown -> succeeded   when query_settled;
  unknown -> failed      when query_failed;
  #invariant paid_count <= 1;
  #eventually succeeded or failed;
}
```

One declaration gives the implementation, its tests and its model:
- the compiler checks that every state is reachable, every event is
  handled, and the table is the only way to change state;
- `coax test` explores every event sequence of several actors, with
  crashes, up to a stated depth, checking invariants at each step;
- a failure prints the shortest event sequence.

This replaces the TLA+ models the first draft proposed. The model can't
drift from the code, because it is the code.

### 7.6 `coax fuzz`: fuzzing without libFuzzer

We control code generation, so we don't need an external fuzzer.
- A fuzz target is a procedure taking `[] u8`.
- `coax fuzz` builds it with coverage counters that the emitter inserts on
  each branch, then runs a mutating driver written in Coax.
- The driver keeps a corpus directory of inputs that reach new branches.
- Fuzz builds turn on `assume` and the consistency checks, as Bitcoin
  Core's fuzz builds turn `Assume` into an abort. Crashes become fixtures.
- With a clang `COAX_CC`, the same target can also run under ASan or
  UBSan, or under libFuzzer when someone wants it.
- The shared corpus lives in its own data repository, like `qa-assets`.

### 7.7 `#embed`: test vectors as pinned data

Compile-time code is sealed from I/O, and that stays true. But
`#embed "vectors/sighash.json"` brings a file's bytes in as a constant:
- the file is declared, and its hash is recorded in the build;
- so the build stays reproducible, and vectors are data, not tool
  output.

Coax tests parse the vectors with Coax's own JSON reader, which we need
anyway for JCS, so the vector tests exercise that reader too.

### 7.8 The evaluator as reference semantics

The evaluator is a second implementation of Coax. Generate random
well-typed programs, run each in the evaluator and as compiled C in debug
and release, and require identical output or the same trap. A wrong
compiler is the one bug every other check would trust.

### 7.9 Reproducible, attested builds

Coax output is already deterministic, and `tests/tools.py` checks it. Add
`coax build --manifest`, which records the hashes of:
- the source;
- the `#embed` inputs;
- the generated C;
- the result.

Independent builders compare manifests before a release, as Guix
attestations do.

### What we leave out

A proof assistant, an SMT solver, and translation validation in Lean.
Bitcoin Core uses an external prover only for secp256k1's frozen curve
formulas. If a frozen math core of ours ever needs a symbolic proof, that
is the precedent: one narrow script, beside the code it proves, never a
second toolchain for everything.

## 8. A staged plan

| Stage | Work | Exit evidence |
| --- | --- | --- |
| V0. Inventory | List every invariant the protocol spec states: its owner, its layer and where it's checked. Mark the authority-critical code. | A table here, linked from `INVARIANTS.md` |
| V1. Native checks | `#forall`, `#test` and `coax test` with their own budget; `assume`; MKT deadline and amount rules as checked Coax functions | Runs in `coax/build.sh test`; counterexamples print as values |
| V2. Money types | `distinct`, `#range`, checked arithmetic by default in authority modules; `MAX_MSAT` and checked sums | Unit mixing, out-of-range amounts and silent wrap are compile errors or traps in every build |
| V3. Fuzzing and data | `coax fuzz` with emitted coverage, starting with `metareal/home.coax`; `#embed` with pinned Nostr, JCS and BIP vectors; sanitizer jobs through `COAX_CC` | A fuzz corpus and vector suites in the local test run |
| V4. Machines | `state_machine` with built-in exploration and `#consistency`; Lightning attempts, x402, spend grants, ledger and MKT orders written with them | Each machine explored to a stated depth in minutes; traces kept as tests |
| V5. Release discipline | `coax build --manifest`, independent attestation, and the evaluator-versus-C random-program check | Two builders produce identical manifests for a release |

V1 can start now. V2 and V4 must land before the payment code they
protect (protocol phase P5). V3 follows Coax aggregate and `#import_proto`
work (Coax stage 2–3, protocol phase P1).

## 9. Rules for authority-critical code

These are adapted from how Bitcoin Core treats consensus code.

- It lives in named modules, listed in `INVARIANTS.md`.
- Checked arithmetic, range types, and checks that never turn off.
- Every rule is a checked function, with its own exhaustive `#forall` or
  `#test` and its own vectors.
- Behavior changes and refactors never share a commit.
- A change needs an independent review: a second agent working from the
  code, not from the author's summary. The owner signs off on changes to
  money rules.
- Counterexamples, fuzz crashes and production incidents become
  permanent tests.
- New dependencies are refused by default. Anything accepted is pinned,
  vendored with its hash, and checked against upstream.
- Checks run locally or on owned machines; there are no GitHub Actions
  (an `INVARIANTS.md` rule). Traces never contain production data,
  secrets or real keys.

## 10. Decisions on the open questions

The first draft ended with five questions. With Bitcoin Core as the
tiebreaker and no external toolchains, the answers are:

1. **TLA+ or a Coax-native explorer?** Native. Bitcoin Core uses no model
   checker; its strength is checks that run all the time. Our protocol
   models are `state_machine` declarations explored by the evaluator
   (§7.5). Until V4 lands, the same models are written as ordinary Coax
   `#test` procedures that enumerate event sequences, so nothing waits on
   an external tool.
2. **Should money code trap on overflow in release?** Yes, in every build.
   That follows `MoneyRange` and the rule against compiling without
   assertions. Authority modules use checked arithmetic and range types
   by default (§7.1, §7.2). Wrapping elsewhere stays an explicit,
   measured choice.
3. **How much proof before real funds move?** Bitcoin Core's level, not a
   proof assistant:
   - per-posting checks that are always on;
   - a `#consistency` recomputation of the whole ledger;
   - a running ledger commitment;
   - exhaustive small-instance runs;
   - fuzzing;
   - independent review.

   All of these must pass before the ledger holds real funds.
4. **How are reference implementations for Nostr pinned?** We don't run
   them in tests. Like `src/test/data/`, we keep pinned vector files,
   generated once, with their source and hash recorded, alongside the
   NIP text pins in `nips/upstream-lock.json`. Tests read them through
   `#embed`.
5. **Is a Lean model of Coax worth it?** No. Bitcoin Core relies on tests,
   fuzzing and review for its compiler-adjacent risk. We do the same with
   the evaluator-versus-C random-program check (§7.8) and attested
   reproducible builds (§7.9). A narrow external proof is a last resort,
   like secp256k1's Sage scripts, and only for frozen math.

The common thread: write each invariant once, next to the code, in Coax,
and keep it on. Bitcoin Core earns trust through many cheap checks that
never stop running. Coax lets us build those checks into the language
itself.
