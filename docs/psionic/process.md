# Development process

Status: M0 implements this workflow through [psionic/check.sh](../../psionic/check.sh).
The runner checks the shared library, two examples, native boundaries, and
source-linked book listings. Later target and performance work below remains
proposed until its workload and evidence exist.

## The unit of progress

A chapter is accepted with a working library increment, a runnable example,
and evidence for the claims it makes. A long explanation or a collection of
green unit checks alone does not complete it. Keep the current chapter fully
worked and later chapters as short outlines until their dependencies exist.

Each chapter should answer:

- What can the reader build by the end, and what earlier pieces does it use?
- What equation or behavior are we implementing, and on what finite domain?
- What are the shapes, numerical policy, storage owners, and refusal cases?
- Which authored library file and named procedure implement each listing?
- How is the result checked independently, and which command reproduces it?
- What does the evidence establish, and what is still planned or unsupported?
- What small exercise can the reader change and then check?

Teach with small numbers, diagrams when useful, hand-computable cases, and
actual diagnostics. Explain the operation before its conventional ML name.
Cross-reference Coax syntax and memory chapters instead of teaching a second
version of the language.

## The change loop

1. **State the contract and chapter result.** Define shapes, capacities,
   ownership, numerics, supported profiles, and failures. Choose the smallest
   workload that makes the behavior observable.
2. **Write checks first.** Put `#test`/finite nonempty `#forall` expectations
   before implementation. Derive expected values independently. Include
   boundaries and failures, and identify native-only properties separately.
3. **Implement in the shared Coax library.** Use explicit capacities and
   caller storage. Keep host code to effects and observations. An example may
   derive a simpler algorithm, but must name that scope and eventually call
   the actual library for the completed chapter result.
4. **Exercise the compiled boundary.** Compare evaluator expectations with
   emitted C in debug and optimized modes. Run focused native oracles and
   sanitizers for actual aliasing, lifetime, representation, and ABI behavior.
5. **Challenge the checks.** Run mutation on authority files, require zero
   survivors, and retain regression witnesses. Do not mark mutants equivalent
   to pass. Investigate weak contracts and unreachable checks.
6. **Finish the prose from the running result.** Link listings to source,
   capture the real transcript, and update capability/status records. Run the
   earlier examples affected by the changed operations.
7. **Review and land together.** Review code, expected values, policy,
   tolerances, source references, and retained evidence in the same change.
   Preserve failed experiments when they explain an outstanding claim.

If the compiler is missing a feature, first try the implemented subset. When
a language change is necessary, retain a minimal consumer case, land the
compiler/spec/book/conformance work with its own evidence, then resume the
Psionic chapter. Do not make future syntax look runnable or hide ML math in
the native host to bypass a compiler limitation.

## A small local verification entry point

M0's `psionic/check.sh` uses an explicit [example manifest](../../psionic/book/examples.json):
every runnable example has a source path, supported build
modes, required tools, expected exit status, and comparison method. Every
chapter says `planned`, `partial`, or `runnable` for its own contents;
execution support is recorded separately by backend and profile.

Adapt the useful parts of [the Coax book runner](../../coax/book/check-examples.sh):
source-linked listings, authored checks, native hosts, debug/release behavior,
and local link/structure checks. Use exact `.out` transcripts for stable
teaching output and explicit expected statuses for deliberate failures.
Use typed numeric fixtures and tolerance-aware comparisons for floating-point
results; never normalize away unexpected failures or non-finite outputs.

The M0 runner combines these kinds of checks. As the book grows, add
targeted selection for the changed chapter or library module:

```text
coax check/test        syntax, finite behavioral expectations, source graph
coax mutate           each authority owner; zero survivors
native debug/release  the actual generated library, ABI and numerical oracle
native sanitizers     short storage, aliasing, bounds and lifetime witnesses
book checks           manifest, source/listing identity, links, output, status
```

`coax/bin/coax` is the existing compiler entry point; no package manager or
new runtime language is required. Tests may use Python as tooling for an
independent high-precision oracle or fixture generation, with explicit
provenance. Normal runtime/build behavior must not acquire that dependency.
Read and review external reference code; do not execute downloaded repository
scripts on this Mac. Checks run locally or on owned infrastructure, with no
GitHub Actions.

A required unavailable target is recorded as unverified, not successful.
An empty example set, missing oracle, timeout, unexpected exit status, skipped
required device case, or uncovered authority procedure cannot pass the gate.
Do not automatically bless new output as its own expected result.

## Evidence and benchmarks

Keep tiny fixtures and machine-readable observations in version control.
Large checkpoints and datasets stay external with pinned origin, license,
hash, size, and explicit retrieval instructions. Examples must work offline
with small fixtures unless their documented purpose is a larger-model run.

A numerical or performance report should record:

| Field | Purpose |
| --- | --- |
| Source, compiler, native toolchain and command | Reproduce the exact implementation and build options. |
| Input/model/tokenizer identity and seeds | Distinguish workload changes from implementation changes. |
| Target, backend, dtype/accumulation and numerical profile | Bound the device and arithmetic claim. |
| Shapes, context/batch/step limits, warmup and repetitions | Define the work performed. |
| Expected rule, tolerance, raw observations and exit status | Make failures and comparison policy reviewable. |
| Persistent/scratch/peak bytes and measurement scope | Expose hidden copies, stack use, and allocator limits. |
| Remaining unsupported cases and failed budgets | Keep capability claims accurate. |

Set comparisons and budgets before evaluating the candidate. Separate cold
load, warm computation, device transfer/synchronization, and eventual
queue-inclusive delivery. Report actual completed work. Preserve raw samples
before deriving medians or tails; measure on an idle target and record the
conditions. A numerical pass cannot be reported as meeting a failed speed
budget. Simulator results cannot qualify a physical phone.

Use three complementary oracles as they become relevant: hand/exact tiny
cases, an independently implemented numerical reference, and pinned historical
Psionic/format fixtures for compatibility. Explain disagreements instead of
assuming the old implementation is correct. Gate changes to tolerances,
fixtures, verification manifests, and accepted profiles as contract changes.

## Integration and repository hygiene

Stay within this repository's [agent contract](../../AGENTS.md) and
[invariants](../../INVARIANTS.md). Use a fresh worktree from `origin/main`
when the shared checkout is dirty, commit only task files, rebase on current
`origin/main`, and push the result to `main`. Keep the model's `Co-Authored-By`
trailer. Do not restore the retired workspaces.

Before a chapter is called complete, list what ran and what did not. Put
required owner taps, credentials, paid compute, device checks, or sign-offs
in the workspace's `NEEDS_OWNER.md` with a concrete action and acceptance
condition. Work that is entirely local and credential-free should continue
without an owner dependency. M0 needs no owner action.

Integrate with the app, voice engine, or a future provider only through a
real consumer's explicit boundary. Introduce cross-surface contracts in root
`proto/` and its spec first. Keep wallet custody and settlement policy with
their existing owners, and never include secrets or private training inputs
in transcripts or fixtures.
