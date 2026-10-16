# Verification

How we check that the parts of OpenAgents that guard money and authority do
exactly what they say, beyond ordinary tests. Started 2026-10-15.

- [tools.md](tools.md): each tool, what it proves, its costs, and when to
  add another tool next to it.
- [targets.md](targets.md): the parts of our system worth checking this way,
  in order, with the properties to check.

## Why bother

Most of our code is covered well enough by tests. A test checks the cases
someone thought of. Some bugs live only in an order of events nobody thought
of:
- a crash between "paid" and "credited";
- two requests for the same invoice at once;
- a retry after a timeout whose first attempt actually succeeded.

When the cost of that bug is money paid twice, value created from nothing,
or authority someone shouldn't have, we check **every** order of events, not
just the ones we wrote down.

LayerTree's Lean proofs are the example that prompted this
([layertree.tech/components/verification](https://layertree.tech/components/verification.html)):
- 72 machine-checked theorems: no inflation, conservation, exits pay their
  own key, no voucher replay;
- writing them found real spec bugs, including a merge rule that let value
  leave through the wrong exit key.

Their gap is ours to avoid: the proofs cover a model, while their Go code and
scripts are checked only by simulation and test vectors.

## The rule of thumb

Verify a part when **all three** hold:

1. **A small, exact rule** can be written down, such as "credited exactly
   once" or "the sum never grows".
2. **Breaking it is expensive and hard to undo**: money, custody,
   authority, someone's data.
3. **Tests can't reach every case**: concurrency, retries, crashes,
   adversarial input, or a state space bigger than anyone can enumerate.

Don't verify:
- routing quality or model behaviour (measure those with outcomes);
- UI;
- ordinary create/read/update/delete;
- code that changes weekly.

## Which tool, in one table

| What you're checking | Start with | Add when |
| --- | --- | --- |
| A protocol: several parties, retries, crashes, duplicate messages (x402 settlement, payouts, ledgers) | **Stateright** on the real Rust state machine | **TLA+** when you need "eventually" (liveness, fairness) or refinement between two designs. **Turmoil** to replay the real networked code under the bad schedules the model found. |
| A pure rule over data (a signer's predicate, a merkle-sum tree, a fee calculation) | **proptest** properties named after the rule | **Kani** to prove it for all inputs within bounds. **Lean** only if the maths outgrows Kani (unbounded structures, induction) or we want to share proofs with another project. |
| Threads and locks inside one process | **loom** | **shuttle** when loom's exhaustive search is too slow for the test |
| Parsers and decoders facing untrusted bytes (Nostr events, invoices, attestation reports) | **cargo-fuzz** with an `arbitrary` input | **Kani** on the decoder's core when a crash or panic would be a security bug |
| Unsafe code, FFI | **Miri** | Kani on the specific unsafe blocks |
| A whole function's contract, deeply (pre/postconditions, loop invariants) | Don't, by default | **Verus** or **Creusot**, only for a tiny core that justifies weeks of proof work |

The longer answers are in [tools.md](tools.md).

## How it fits our existing rules

The workspace already asks for this ("narrow the production contract, model
the bounded state space, run or document the checker, and convert meaningful
counterexamples into regression tests"; see `INVARIANTS.md`). Concretely:

1. **Each verified property is an invariant row.** It appears in the owning
   `INVARIANTS.md` with the model or proof named in its test column, exactly
   like a test name today.
2. **Every counterexample becomes a plain regression test.** It runs in the
   normal suite, so the bug stays fixed even if the model is later deleted.
3. **Checkers run in CI only when the code they cover changes.** They're
   slower than unit tests, so they're scoped by path.
4. **Models never hold production data or secrets.** Use made-up keys,
   made-up amounts, and fake money.
5. **Never weaken a runtime rule to make a model pass.** If the model and the
   code disagree, one of them is wrong; find out which.
6. **The Coder binary is frozen at 1.0.0** ([1.0-freeze](../coder/1.0-freeze.md)).
   Verification tools are dev-dependencies or separate crates. They never
   ship in a binary, so they're safe to add now. A bug they find in Coder
   code goes to [next-release.md](../coder/next-release.md).

## What we have today

- `proptest` is used in 5 crates.
- `arbitrary` and `loom` are in `Cargo.lock` only as dependencies of other
  crates; our own tests don't use them yet.
- No Stateright, TLA+, Kani, Lean, fuzzing targets or Miri runs.
- `INVARIANTS.md` states rules in prose, backed by named tests.

The first targets are x402 settlement
([targets.md](targets.md#1-x402-settlement-exactly-once)), before x402 moves
to our own LND node, and the Computers lifecycle
([targets.md](targets.md#2-computers-the-lifecycle-commands-files-and-usage)),
before the Computers API goes public.
