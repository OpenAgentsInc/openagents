# 25. What verification establishes

The checks in Chapter 10 and the agent workflow in Chapter 19 are useful
because they make concrete mistakes fail early. They do not turn a
compiler, a program or its author into a trusted authority. This chapter
makes the limits explicit so you can combine the evidence correctly.

## Four questions, four kinds of evidence

| Question | Useful evidence | What it does not establish |
| --- | --- | --- |
| Did the implementation satisfy these expectations? | Executed `#test` and finite `#forall` cases | Correctness outside the cases or assumptions |
| Do the checks notice these implementation changes? | Killed mutants at the tool's mutation sites | Exhaustive exploration of all possible defects |
| Does the emitted program behave correctly here? | Native reference tests, sanitizers and target tests | Correctness on every ABI, optimization or device |
| May this operation be performed? | Actual authentication, grants, signatures and domain admission | Not answered by a passing codec or a test score |

A `#forall` enumerates its finite domain. It is not a symbolic theorem
prover. A 100% mutation score refers to the generated mutants, not every
program an agent could write. Execution coverage means a procedure was
reached, not that all of its outcomes were checked.

`#authority` requires executed check coverage and runs checks when the
file compiles. It does not automatically run mutation testing on every
product build. The compiler's bootstrap explicitly runs both, and a
product's verification script must do the same for its own authority
files.

## The author can edit the checks too

Chapter 19's compiler-enforced gate operates on the source it is given.
An agent with write access can remove `#authority`, narrow a `#forall`
domain, delete a test, alter an expected value, weaken a build script or
refresh a seed with a compromised compiler. The language cannot make
those changes impossible merely by containing a directive.

Protect the verification policy outside the change being evaluated:

- Keep the list of authority files and required commands in reviewed
  repository policy. Review changes to that list and to the build gates.
- Review check changes alongside code changes. Explain a changed expected
  result from the contract, rather than accepting the implementation's
  new output as its own oracle.
- Preserve independently versioned compatibility baselines and original
  signed bytes. A migration must not quietly rewrite its reference data.
- Require fresh command results for the exact commit being released.
  Keep the command, exit status, tool revision and relevant evidence.
  If a rebase changes only unrelated files, retain results for the identical
  hashed source/tool closure; do not rerun unaffected suites for every remote
  commit. Changed dependencies invalidate the affected evidence.

These are development controls, not extra runtime permissions. They also
apply to a person editing code. On owned infrastructure, use the reviewed
verification entry points; the OpenAgents repository does not use GitHub
Actions.

A useful review experiment is to make a temporary deliberate defect and
confirm the relevant gate fails. The permanent fix should retain the
regression case. Do not mark a surviving mutant equivalent merely to
obtain a green score; first determine whether the contract or the checks
miss an observable distinction.

## Independent oracles matter

Comparing a new implementation with the old output proves compatibility
on the compared inputs. It does not show that either implementation is
right. The old implementation may carry the same bug, and a generated
expected file may simply bless it.

Use an oracle independent of the implementation when one is available.
For protobuf, exercise official `protoc` encoders and decoders in both
directions. For numeric conversion, use exact integer or rational
expectations. For a scene, compare source content and real interactions,
not just a screenshot that contains the right title.

The capstone's golden packet is a small independent wire expectation,
and its optional `protoc` test checks the same bytes with another
implementation. That still does not verify signatures, access rights or
the meaning of an arbitrary HUD action. Those are separate contracts.

## The evaluator and the native program

Coax checks execute in the compiler's evaluator. Your app executes emitted
C compiled for a real target, and sometimes generated Metal too. Those
paths share source, but they are not the same execution environment.

The evaluator does not own native addresses or operating-system objects.
For example, a generated writer's runtime overlap test cannot run at
compile time. A check that tries it gets E0404. Test actual aliasing,
short buffers, storage lifetimes and native representations in a compiled
host. Do not suppress that error or substitute a fake overlap result to
make the check pass.

CPU shader-math checks likewise do not prove identical GPU results.
Compiler math options, precision and device behavior can differ. Keep
exact expectations where required, tolerances where justified, and actual
Metal validation for binding and execution behavior.

Run native tests in debug and optimized modes. Include sanitizers where
the host supports them. For platform effects, test the real platform:
Keychain ownership, audio routes, backgrounding, GPU completion and remote
command outcomes cannot be established by a pure state-machine test.

### A bounded differential experiment

The optional `tools/differential.py` generates valid small-integer programs
with branches, aggregate returns, ordered writes through caller-buffer views
and deferred cleanup. A separate Python integer/state model supplies expected
values. Each generated source checks those expectations with evaluator
`#test` blocks; a separate native `main` exposes the same values and buffer
contents for comparison. The harness emits checked C once and compiles it
with both `-O0` and `-O2`, keeping arithmetic policy fixed.

From the repository root, with an already-built compiler and a new output
directory:

```sh
python3 coax/tools/differential.py --seed 20261028 --cases 16 --output /tmp/coax-differential-run
```

Schema-1 JSON records retain source/compiler/toolchain identities, selected
inputs, process budgets, observations and artifact hashes. A mismatch keeps
the original and a reduced source plus a replayable recipe. Timeouts are
inconclusive; unavailable tools or rejected generated programs are infrastructure
errors. Missing native execution is explicitly recorded, and none of those
outcomes passes the run. Reduction preserves the mismatch's execution stage
and kind; its attempt budget and limits remain visible.

`--seed-defect` deliberately changes multiplication to addition in the emitted
checked helper, after the evaluator has run. The retained regression reduces
to `a * 17 + b * 3` with `(0, 0)`: the model and evaluator expect zero while
both corrupted native builds return 20. This tests the comparison and
reduction path; it is not a defect in the shipped compiler. Python is needed
only for this optional verification tool. The [verification note](https://github.com/OpenAgentsInc/openagents/blob/main/coax/verification-differential.md)
records the exact subset, results and untested behavior.

## Policy observations are not authenticated effects

The Nostr FI candidate library is a concrete example. Coax checks the exact
community, issuer, token class, audience, time and selected public-key shape.
It also rejects unsafe address classes, redirects, oversized responses and
stale snapshots from typed observations. Its native and Wasm fixtures test
those decisions against independent address and time oracles.

Those observations still need an owner. A remote message saying TLS passed
cannot authenticate itself. The actual adapter must resolve and inspect every
address, pin its peer, validate the endpoint, enforce stream/deadline limits
and bind an immutable snapshot to trusted local policy. Ordinary browser fetch
cannot expose all of that evidence. A successful policy candidate is also
separate from checking the original JWS signature and fresh key possession.
A typed proto or a zero-survivor mutation run supplies none of those effects.

The [selected FI profile](https://github.com/OpenAgentsInc/openagents/blob/main/docs/protocol/2026-10-28-fi-rs256-policy-v1.md)
records what currently exists and which integration gates remain. Keeping that
boundary explicit lets the same checked policy serve multiple honest adapters.

## The bootstrap is a trust chain

The lexer, parser, type checker, source-loading policy, descriptor importer,
wire runtime and
fingerprinting logic are authored in Coax. Generated C/header seeds let
the compiler build itself without Python. Requiring regeneration to match
those seeds catches drift. Requiring the final compiler to regenerate
its linked modules catches another class of inconsistency.

Source-unit scope is also part of that chain: the native source owner
annotates the trusted internal checker graph. A physical unit's `#authority`
requirements must survive composition; the source kernel and native
end-to-end suite test that they do.

Neither test proves that the starting compiler or seeds were benign.
The seeds, native graph bridges, evaluator, emitter, C/C++ toolchain and
platform remain trust inputs. Independent native tests and reviewed seed
refreshes strengthen the evidence; a fixed point alone is not a proof of
bootstrap security. Chapter 20 explains the actual build sequence.

## Say what remains untested

A verification report should be specific enough that someone else can
repeat it. For example:

> Request checks passed; 27/27 generated mutants were killed. The emitted
> request and codec libraries passed debug and optimized C-host tests
> under ASan/UBSan. Official `protoc` agreed in both directions. No network
> service, background execution or device GPU was exercised by this test.

That tells a reviewer far more than "verified". Keep limits near the
claim they qualify, and use an actual contract to decide what additional
evidence a release needs.
