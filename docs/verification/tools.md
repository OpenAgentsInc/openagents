# Verification tools: what each proves, and when to add another

For each tool:
- what it is;
- what it's good at;
- where it stops;
- what to add next to it when it stops.

Ordered from the one we'll use most to the ones we'll rarely need.

## Stateright: our default for protocols

**What:** a model checker written in Rust ([stateright.rs](https://www.stateright.rs/)).
You describe a system as states and actions by implementing its `Model`
trait. The checker explores every reachable state and reports a path to any
state that breaks a property. MIT licence. Version 0.31.0, last released
July 2025.

**Why it's our default:**
- **The model can be the real code.** The Rust type that runs x402
  settlement in production can implement `Model`. The checker then explores
  the code we ship, not a copy of it. A TLA+ spec is a second description
  that drifts from the Rust. That drift is exactly the gap LayerTree has
  between its Lean model and its Go code.
- **Rust only:** no second language to maintain. This fits our rule that
  product code is Rust.
- **Speed:** its own comparison with TLA+'s checker reports 1.8× to 12.3×
  faster on one two-phase-commit model, growing with model size
  ([comparison](https://www.stateright.rs/comparison-with-tlaplus.html)).
  That's one model on one laptop, so treat it as a hint.
- **"Sometimes" properties** prove a good outcome is reachable ("a payment
  can settle"). They catch the classic mistake of a model that passes only
  because nothing can happen.
- **Actor models and a linearizability tester** for distributed pieces.
- **Stateright Explorer** for browsing the state space and the paths to each
  finding.

**Where it stops:**
- **No liveness or fairness.** It checks "always" and "sometimes", not
  "eventually" ("every payment eventually settles", "no request waits
  forever").
- **No refinement mapping:** it can't prove one design implements another.
- **No symmetry reduction** credited to it in its own comparison. Models with
  many interchangeable parties (ten identical nodes) blow up.
- **Slow maintenance.** If it's abandoned, the pinned version still runs; it's
  test-only.

**Supplement it with:**
- **TLA+** when "eventually" matters, or when comparing two designs at the
  whiteboard stage before there's any Rust.
- **Turmoil** when the model found a bad schedule and you want to run the
  *real networked code* (tokio, real handlers, a fake network) through that
  schedule.
- **Plain regression tests** for every counterexample. Always.

**How we use it:**
- A dev-dependency of the crate that owns the state machine.
- The model lives in that crate's `tests/` or a `#[cfg(test)]` module.
- Bounds are small and stated in the test: 2–3 parties, 1–2 invoices, up to
  2 crashes.
- Every run asserts both the safety properties and at least one
  "sometimes", so a vacuous pass fails.

## proptest: properties for pure rules

**What:** property-based testing. You state a rule ("for any tree and any
leaf, an exit reduces the sum by exactly the leaf's amount"), and proptest
generates many random inputs, then shrinks any failure to the smallest
example. Already used in 5 of our crates.

**Good at:** pure functions and data structures: merkle-sum trees, fee
maths, encoders and decoders that must round-trip, predicates.

**Stops at:** it's random, not exhaustive. It finds most bugs quickly, but
proves nothing.

**Supplement it with:** **Kani** when a rule has to hold for *every* input
(a signer's predicate, conservation in a ledger step), not just most.

**How we use it:** name each property after the rule it checks. When there
is a published proof (e.g. LayerTree's `ExitK.no_inflation`), use the same
name, so the mapping is obvious.

## Kani: proofs on our actual Rust

**What:** a bounded model checker for Rust, from AWS. It translates the real
compiled Rust to a form a solver can reason about, and proves an assertion
holds for **all** inputs within stated bounds. Proofs are Rust functions
marked `#[kani::proof]`.

**Good at:** proving a pure rule on the code we ship:
- "this predicate never approves a transaction that pays more than the tree
  holds";
- "this decoder never panics on any input up to 512 bytes";
- arithmetic: no overflow in fee or sat/msat conversion.

It closes the model-to-code gap entirely for the function it covers.

**Stops at:**
- **Bounds:** loops and recursion are unrolled to a fixed depth, so very
  large structures need care.
- **Size:** big functions can be slow or time out.
- **Async and protocols:** not its job; use Stateright.

**Supplement it with:**
- **Lean**, if a property needs induction over unbounded structures (any
  tree depth, any number of steps) and Kani's bounds aren't convincing.
- **proptest** at larger sizes than Kani can reach.

**How we use it:** `cargo kani` in a separate CI job, scoped to the crates
that have proofs. Proof harnesses sit next to the code under
`#[cfg(kani)]`. It's a dev tool, never in a shipped binary.

## loom and shuttle: threads inside one process

**loom** runs a concurrent test under every possible interleaving of its
threads and atomics, within bounds. It's exhaustive but slow; good for a
small lock-free structure or a hand-rolled mutex protocol. It's in our
lockfile only as another crate's dependency; our tests don't use it yet.

**shuttle** (AWS) runs randomized schedules instead. It's not exhaustive, but
it scales to bigger tests. Use it when loom's search doesn't finish.

**Use them when:** shared state across threads or tasks in one process
guards something that matters, e.g. an in-memory replay store or a
per-invoice lock. Most of our concurrency goes through a database; there, use
Stateright on the protocol instead.

## Turmoil: the real network code under bad schedules

**What:** deterministic simulation for tokio programs (from the tokio
project). The real servers and clients run on a simulated network that can
delay, drop, partition and reorder messages, with a seed so failures replay
exactly.

**Use it after Stateright:** the model says what orders of events matter;
Turmoil drives the real HTTP handlers and clients through those orders. Good
for the x402 front talking to LND, or the gateway talking to a payout
worker.

**Stops at:** it only runs the schedules you or its randomness pick. It's a
test, not a proof.

## cargo-fuzz: untrusted bytes

**What:** coverage-guided fuzzing (libFuzzer) with structured inputs from
`arbitrary`.

**Use it for:** every decoder that faces the internet or another party:
- Nostr events and NIP payloads;
- BOLT11 invoices;
- attestation reports;
- x402 headers;
- uploaded files we parse.

The goal is no panics, no hangs and no unbounded memory, plus round-trip
properties.

**Supplement it with:** **Kani** on the decoder's core when a panic there
would be a security bug, e.g. the enclave's report parser.

## Miri: unsafe code

**What:** an interpreter that detects undefined behaviour in Rust (bad
pointer use, data races, uninitialized memory).

**Use it when:** a crate has `unsafe` blocks or FFI. Run that crate's tests
under `cargo miri test`. It's cheap to add and finds real bugs.

## TLA+ (and Quint, Apalache, P): designs and "eventually"

**What:**
- **TLA+:** a specification language with the TLC model checker and the
  Apalache symbolic checker.
- **Quint:** a modern, typed syntax for the same ideas.
- **P:** a language for event-driven protocols. LayerTree plans a P model
  for its off-chain protocol.

**Use them when:**
- **"Eventually" is the property:** every payout reaches a final state, a
  stuck channel is always closed, a lease always expires.
- **Comparing designs before code exists:** a short TLA+ spec settles
  arguments faster than Rust prototypes.
- **Refinement:** proving the detailed design implements the simple one.
- **Symmetry:** many identical parties, e.g. N enclaves or N payout workers.

**Cost:** a second description of the system, which drifts. Keep each spec
small and tied to one invariant row, and check it again when the Rust
changes. Prefer **Quint** syntax if a new contributor will maintain it.

## Lean 4: maths-level proofs

**What:** an interactive theorem prover. You prove theorems by hand, with
machine checking. LayerTree uses it: 72 theorems, about 4,500 lines.

**Use it when:**
- a property needs induction over unbounded structures (any tree, any
  number of steps), and neither Kani's bounds nor proptest's randomness is
  convincing enough;
- or we want to share or reuse proofs with another project, e.g. LayerTree's
  theorems, if their licence allows.

**Cost:**
- The highest of everything here: weeks per non-trivial property, and
  specialist skill.
- It proves a model, so it still needs Kani or differential tests to connect
  to our Rust.

## Verus, Creusot, Prusti: deductive proofs of Rust

**What:** you annotate Rust functions with pre- and postconditions and loop
invariants, and a solver proves the code meets them for all inputs, with no
bounds.

**Use it when:** a tiny core (a few hundred lines) is important enough to
justify weeks of proof work, and Kani's bounds aren't enough. Candidate: a
future production version of the LayerTree enclave predicate.

**Not by default:** the annotation burden is high, and the tools are younger
than Kani.

## Choosing, as a sequence

1. Write the rule in one sentence. If you can't, stop: it isn't ready to
   verify.
2. **Is it a pure function?**
   - Use proptest.
   - If it must hold for every input, add Kani.
   - If it must hold for unbounded inputs, consider Lean or Verus.
3. **Is it a protocol** (parties, retries, crashes)?
   - Use Stateright on the real state machine.
   - If "eventually" matters, add a small TLA+ or Quint spec.
   - Add Turmoil to run the real network code through the schedules the
     model found.
4. **Is it threads in one process?** Use loom; switch to shuttle if loom is
   too slow.
5. **Does it parse untrusted bytes?** Use cargo-fuzz; add Kani on the core
   if a panic is a security bug.
6. **Is there `unsafe`?** Use Miri.
7. **Always:** add the invariant row, and turn every counterexample into a
   plain regression test.
