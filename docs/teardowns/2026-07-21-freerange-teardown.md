# Freerange Teardown — 2026-07-21

Read-only architecture, product, and process audit of the public
`chenglou/freerange` source tree at an exact commit, plus a bounded local
runtime spot-check (dependency install, one findings run, one audit run, and
the full test suite in the reference clone; nothing tracked was modified).
Freerange is unlike every other subject in this catalog: it is not an agent
harness, a desktop shell, or a control plane. It is a **verification
instrument** — a static numeric range analyzer for TypeScript, explicitly
designed for the agent era — and it lands squarely on the thesis of
[`docs/fable/2026-07-19-verifiable-software.md`](../fable/2026-07-19-verifiable-software.md).

## TL;DR

Freerange shows the possible range of every `number` in a TypeScript codebase
and statically catches `NaN`, `Infinity`, division by zero, and out-of-bounds
array reads. No annotations, no new language, no fork: it consumes the
official TypeScript compiler API, respects the project `tsconfig`, and uses
plain `console.assert` calls as its contract language. Two commands: `fr`
(findings, the CI gate) and `fr --audit` (per-function contracts plus concrete
refactor suggestions, explicitly "great for agents"). [source] [runtime]

```text
fr / fr --audit (Bun CLI)
        |
TypeScript program (official TS 6 API, tsconfig respected, TS errors first)
        |
acceptance-checked lowering  ->  IR (functions, blocks, instructions)
  fail-closed subset boundary
        |
abstract interpretation engine
  interval + integer flag + mayBeNaN + one excluded point per value
  fixed-point loops with widening (16-round backstop)
        |
requirements inference (requires / ensures / assumes / proves)
  created at the operation, discharged by forward analysis, never by solving
        |
findings report (CI gate)      audit report (agent guidance)
```

The architecture is careful, but the architecture is not the headline. Three
things matter more:

1. **The inversion.** Freerange deliberately supports a small TypeScript
   subset and asks *agents to refactor important calculations into that
   subset*, guided by tagged audit codes. The tool no longer meets code where
   it is; the code moves to the tool, because agents make refactoring cheap.
   The README's one-line pitch is the agent-era claim in full: "AI agents can
   now guarantee UI layouts without ever touching the browser." [source]
2. **Honest loss accounting at finding granularity.** Every function is
   `fully analyzed`, `partially supported`, or `unsupported` with the first
   blocker named. Every trust the analyzer takes on faith prints as an
   `assumes:` line. Partial results use a data shape with no contract fields
   so evidence cannot accidentally print as a guarantee. Internal limits fail
   closed, and each one documents which failure it buys. "Freerange does not
   publish a stronger guarantee by pretending that unsupported code was
   understood." [source]
3. **The development process is itself agent-adversarial verification.** A
   67 KB decision ledger records semantics, rejected alternatives with
   measured evidence, and reopen conditions. A tested goal-loop prompt
   template and a checked-in Claude Workflow script run N-lens adversarial
   review rounds whose anti-laundering rules (a dead reviewer agent surfaces
   as `AGENT-DIED`, never as a clean round; every lens must report probes
   run) are a working small-scale instance of the independent-verification
   capacity OpenAgents' VSE program is trying to build. [source]

The central OpenAgents decision: **treat Freerange as a candidate bounded
numeric oracle for analyzable layout/math helpers, and fast-follow its
verifier-shaped-code inversion, printed-trust discipline, fail-closed limit
vocabulary, measured-evidence decision ledger, and anti-laundering review
harness. Reject adopting its Bun toolchain into the monorepo contract,
treating its conditional contracts as acceptance or release authority, and
unpinned dependence on a day-old 0.0.1 with a single maintainer and a
deliberately absent API.**

## 1. Snapshot, provenance, and limitations

### 1.1 Exact source identity

| Artifact          | Identity                                                                | What it establishes                     |
| ----------------- | ----------------------------------------------------------------------- | --------------------------------------- |
| Public repository | `https://github.com/chenglou/freerange`                                 | Public source and history               |
| Audited commit    | `406ddcf86337a76f3f1c2b3671e56990fdecaa92` on `main`                    | Exact snapshot used here                |
| Commit time       | `2026-07-21T04:02:35-07:00`                                             | Freshness of the audited tip            |
| Commit subject    | `Clarify analysis scope documentation`                                  | Latest audited change                   |
| Product version   | `0.0.1` (`@chenglou/freerange`, published 2026-07-20 per CHANGELOG)     | One-day-old first public release        |
| License           | MIT (copyright Cheng Lou)                                               | Permissive reuse boundary               |
| Author            | Cheng Lou, sole author of all 703 commits since `2026-04-18`            | Single-maintainer provenance            |
| Source scale      | ~11,230 lines across 31 `src/` TypeScript files                        | Deliberately small implementation       |
| Test surface      | ~7,430 lines across 11 test files; 193 tests, 1,506 `expect()` calls    | Dense executable evidence               |
| Toolchain         | Bun runtime; TypeScript 6 compiler API pinned, TypeScript 7 native for checks; oxlint type-aware; knip | Fast solo verification loop |

The runtime spot-check: `bun install` then `bun test` in the reference clone
passed 193/193 tests in ~27 s, `bun fr.ts demo/index.ts` reported 0 findings
with the coverage line `10/14 named top-level function declarations fully
analyzed; 0 partially supported; 4 unsupported`, and `fr --audit` printed
per-function `requires`/`ensures` contracts exactly as the README documents,
including proven ranges like `return.cols is a finite integer number from 1
through 7` for a real spring-animation photo-gallery demo. [runtime]

The first commit is `Initial Freerange extraction` (2026-04-18), and the
decision ledger references a `pre-pivot` branch with a substantially larger
analysis (purity specs, relational reasoning) that the current tool
deliberately abandoned. This public 0.0.1 is a *second, smaller system*
distilled from a bigger private one — the pivot itself is a finding, covered
in §3. [history]

### 1.2 Evidence labels

- **`[source]`** — tracked source, docs, manifests, or config at the commit.
- **`[test]`** — a tracked executable test.
- **`[runtime]`** — observed by running the tool or its suite locally.
- **`[history]`** — Git history at or before the audited commit.
- **`[public]`** — corroborated by a linked public source.
- **`[inferred]`** — reasoned from several observations.
- **`[limitation]`** — a boundary on what this audit can prove.

This audit did not run Freerange against any OpenAgents source tree, did not
benchmark analysis time against `tsc`, and cannot verify the README's
performance claim ("a negligible fraction of TypeScript's analysis time")
beyond the small demo. [limitation]

## 2. What Freerange is

For each TypeScript `number`, Freerange tracks lowest and highest possible
values, integerness, possible `NaN`, possible infinities, and at most one
exact excluded value (e.g. after `value !== 0` it remembers that `value`
cannot be `0`). It follows those facts through control flow, loops, plain
records, tuples, dense arrays, tagged unions, and same-file function calls,
then reports four kinds of statements per function [source]:

- `requires:` — a condition every caller must satisfy (parameter finiteness
  is automatic for every plain `number` parameter; division mints nonzero
  divisor requirements; asserted array reads mint valid-index requirements).
- `ensures:` — a guarantee about the return value, conditional on the
  requires and assumes.
- `assumes:` — trust the analyzer takes without proof, stated per value
  (e.g. `assumes: values is a plain array — its length counts its elements,
  and every index below the length holds an element`).
- `proves:` — a static `console.assert` discharged at compile time.

The `console.assert` mechanism is the standout product idea: assertions at
the very start of a function are caller requirements (checked at every
supported same-file call site); assertions later in the body are obligations
Freerange must prove or report. The contract language is ordinary runtime
JavaScript that already works in production and can be stripped by bundlers —
no decorators, no comments, no sidecar files. Calling `itemColumn(0, 2.2)`
against `console.assert(Number.isInteger(columnCount))` is a compile-time
error. [source] [test]

Findings mode is the CI gate; audit mode is informational and adds tagged
refactor suggestions (`[guard-derived-value]`, `[encode-input-rule]`,
`[use-direct-operands]`, `[handle-missing-element]`, `[guard-array-index]`,
`[write-explicit-condition]`, `[use-loop-for-aggregation]`) that tell an
agent exactly which rewrite would make an unanalyzable function analyzable.
Since `fr` runs TypeScript first and reports its errors in the familiar
format, it can literally replace a `tsc --noEmit` CI step. [source]

## 3. The inversion: agents move the code to the verifier

The most consequential design decision is recorded plainly in the README and
the decision ledger. Freerange used to support a much larger TypeScript
subset — and deliberately shrank it: "Those patterns often made numeric
inference and proofs much harder and slower... Now that AI agents write code,
we strongly recommend asking agents to refactor important calculations into
shapes that Freerange analyzes well, guided by `fr --audit`." [source]

Historically, static analyzers had to meet code where it was, because asking
humans to restructure working code for a tool's benefit was a losing
proposition. Infer (credited in the README) spent enormous effort analyzing
arbitrary code. Freerange inverts the economics: when agents write and
refactor code at near-zero cost, the verifier can define a small, predictable,
fast subset and let agents move the important calculations into it. The
engineering guide states the calibration rule for that boundary: "Real usage
is evidence, not the feature specification. Do not overfit by adding a
collection of rules for the exact expressions found in the current corpus...
prefer a small written subset whose behavior is complete and predictable."
[source]

The subset that survived the pivot reads like a functional core: named
synchronous top-level functions, immutable records/tuples/dense arrays,
tagged unions with exhaustive switches, explicit conditions, direct loops.
The rejected constructs are exactly where the TypeScript checker's word is
void or where analysis blows up: `any` values are carried claim-free rather
than trusted, `as` casts erase to opaque values (three adversarial review
rounds defeated every attempted type-level carve-out, each attack a
diagnostic-clean TypeScript program reaching an internal crash), `eval` and
`@ts-ignore`/`@ts-expect-error`/`@ts-nocheck` reject the whole file because
one occurrence voids the checker's word everywhere, `var` rejects because
hoisting gives one variable several declaration sites. [source]

Everything else in the design follows from refusing the state-space
explosions that killed the pre-pivot analysis: no theorem prover, no general
relational domain ("solving is where the pre-pivot analysis blew up" —
`total / (a + b)` requires `(a + b) is nonzero`, never "a is not -b"), one
merged abstract state per CFG block, branch refinement capped at
pattern-matching depth, requirements simplified only where the simplification
is *exactly* true for IEEE floats. The ledger pins the float facts in
regression tests rather than trusting memory of the standard: `5e-324 / 2
=== 0` (positive ratios can underflow to zero), `1e-200 * 1e-200 === 0` (a
small constant factor cannot be peeled from a requirement), and strict
comparisons refine to the adjacent representable double by bit-stepping
(`nextUp`). Freerange reasons about floats, not reals, because "real-number
algebra would produce false guarantees." [source] [test]

## 4. Honest loss accounting, all the way down

Freerange's reporting discipline maps one-to-one onto the laws the
verifiable-software essay derives, which is remarkable for an independent
project with no connection to OpenAgents:

- **No silent surfaces.** Every named top-level function is fully analyzed,
  partially supported (first blocker named), or unsupported (first blocker
  named). Module-level statements that cannot be analyzed print `skipped:`
  lines, and every binding a skipped statement could write is demoted so
  later analysis cannot trust a stale value. The coverage line always prints,
  and the README instructs: "Always read the coverage line. No findings does
  not mean an unsupported file is safe." This is the anti-coverage-cap move —
  the exact failure class the essay's revision added (green gates silent
  about what they do not measure). [source]
- **Evidence cannot masquerade as guarantee.** "Partial results use a data
  shape with no contract fields, so evidence from analyzed paths cannot
  accidentally print or propagate as a guarantee." Structural typing enforces
  the rung separation that OpenAgents states as law ("no rung implies the
  next") — a compile-time version of refusing to let `completed` absorb
  `verified`. [source]
- **Trust is printed, never held silently.** When the engine takes something
  on faith — an external array being dense, a declared kind surviving `any`
  writes, a nonzero divisor it cannot name over parameters — the report
  prints the assumption on every function whose result rests on it. The
  ledger's reasoning for printing per-value trust is exemplary: type-clean
  JavaScript (`[1, , 3]`, `new Array(5)`, a `push`-grown tuple) can violate
  the trust, so the trust must be stated where a caller can see it. Folded
  quantified lines exist purely to keep reports readable, and the fold rules
  are carefully scoped so a folded sentence never blesses values the engine
  treats as impossible. [source]
- **Limits fail closed and name their price.** "Limits must fail closed, and
  each one states which failure it buys." A loop gets 16 fixed-point updates
  and hitting the limit records a stop, never a wrong stabilized state; type
  walks stop at depth 8 (removing that boundary made five corpus functions
  produce tens of thousands of assumption lines); requirement expansion gets
  one instruction visit per instruction; an abstract number keeps at most one
  excluded point, and a second exclusion can drop precision but cannot
  strengthen a claim. "No limit may silently strengthen a result." [source]
- **Conditional greens are conditional.** An `ensures` line assumes its
  `requires` and `assumes`; violating an assumption makes downstream claims
  vacuous, not false. The README warns that a requirement is not
  automatically a bug and an assumption is not automatically a limitation —
  "Decide what the program should do before changing code to remove either
  one." [source]

One more structural discipline deserves quoting because it generalizes far
beyond static analysis: "Prefer correctness by construction over correctness
by review: when a rule must hold across many places... reshape the code so
one place enforces it and forgetting is impossible... Reviews stay as the
safety net, not the mechanism." [source]

## 5. The development process is the second product

Freerange ships four process artifacts that are as instructive as the
analyzer:

**`current-decisions.md` (67 KB).** A decision ledger recording implementation
semantics, rejected alternatives, measured evidence, and deferred work — with
`owner-locked` markers on decisions agents may not reopen, a "Maybe
Reconsider" section whose every entry names the evidence that would reopen it
("Reconsider when a real property is materially clearer as..."), and a
"Punted" section. Features are deleted with receipts: the recursive
structural-union comparison was removed because it "had no successful gallery
use, slowed lowering by about 9% in a five-run median, and required about 100
lines"; a call-result cache prototype was rejected because its deep cache key
made a 500-field-record workload grow from ~1 ms to 162 ms; four memo tables
survive because independent measurements showed 45–80% lowering slowdowns
without them. This is the Sol challenge-ledger discipline ("a rejected or
deferred challenge must be as easy to revisit as an accepted one") applied at
per-feature granularity by a solo developer. [source]

**`goal-prompt.md`.** A tested prompt template for verification-heavy tasks:
turn every requirement into a concrete example with its exact observable
result before editing; pair every required `unknown` or rejection with a
positive control through the same path; treat performance requirements as
behavior with named workloads; give one fresh reviewer only the original task
and the frozen diff and have it reconstruct requirements and run
counterexamples; reread the engineering guide against the finished diff and
revert if the result is not worthwhile. [source]

**`.claude/workflows/review-round.js`.** A checked-in Claude Workflow script
running one adversarial review round: N finder lenses attack a commit in
parallel, optional per-finding adversarial verification follows, and the
header comment records "the standing disciplines, learned the hard way": a
died agent returns null and `?? []` "would launder that into a clean round —
a review process whose failure mode is indistinguishable from success," so
every lens returns `probesRun` and null agents surface as `AGENT-DIED`
instead of green; findings require reproduced runtime contradictions or
crashes, because "honest stops and imprecision are not findings." The
decision ledger adds review-lens calibration: soundness reviews aim at code
people actually write, "or the reviewers drift into hunting exotic spellings
as if they were security exploits." The README's robustness claim —
"Adversarially tested by agents against thousands of edge cases" — is backed
by this machinery and by ledger entries recording multi-round attacks that
defeated three successive cast-handling designs. [source]

**`AGENTS.md`.** Beyond the usual contract lines (no monkey-patching, no
legacy compatibility during refactors), it contains a writing-style contract
for agent-authored prose: concrete banned-word lists ("Do NOT use pseudo-
jargon... Bad: `earn`, `win`, `seam`, `source-backed`"), a rule to pair every
general point with an example, and a rule against context-dependent variable
names in docs. Governance of agent *communication* quality, not just agent
code quality. [source]

## 6. Mapping onto the verifiable-software thesis

The essay's economics say generation cost is collapsing while verification
cost stays linear, and that whoever bends the verification curve owns the
category's margin. Freerange is a pure instance of the curve-bending moves
the essay names [inferred]:

- **Downshift.** It is the cheapest sufficient checker made real for a whole
  claim class: "this layout math cannot produce NaN" moves from human review
  or browser QA to a deterministic compile-time oracle with near-zero
  marginal cost per run. The README's browser line — guarantee UI layouts
  without ever touching the browser — is the "Eliminate" limit case for
  layout verification.
- **Decompose.** The supported subset *is* the verification quantum made
  concrete: small named synchronous functions with typed inputs admit
  deterministic checks that a monolithic component never will. Freerange
  operationalizes "shrink the verification quantum" as an authoring style
  with tool support (`fr --audit` names the rewrite).
- **Amortize.** A discharged `requires` line is checked once at the callee
  and reused by every caller; a proven `console.assert` is proof that never
  re-runs in a browser.

It also independently converges on the essay's honesty laws: evidence-gated
status (coverage lines, first-blocker reporting), loss accounting (`assumes`
as printed trust), no rung collapse (contract-free partial shapes),
falsifier-driven review (counterexample-required findings), and the
challenge-ledger discipline (reopen conditions on every rejected feature).
When an outside project with different roots arrives at the same laws, that
is evidence the laws are load-bearing rather than house style. [inferred]

There is also a market signal here. Cheng Lou is a prominent frontend/
languages engineer (React Motion, ReasonML/ReScript community), and his first
public project of the agent era is not another harness — it is a verifier
whose README leads with agents as both audience and workforce, crediting
Infer and AlphaProof. The scarce good he chose to build is exactly the one
the verification-gap economics predicts: trustworthy, cheap, mechanical
proof. [public] [inferred]

## 7. What OpenAgents should adapt

These are Fast Follow candidate lessons in the `docs/teardowns/` evidence
lane. Nothing here is dispatch authority; implementation requires the normal
admission path.

**7.1 Pilot Freerange as a bounded numeric oracle.** The natural first
target is analyzable numeric/layout helpers: Effect Native renderer layout
math, virtualized-list windowing, spring/animation parameters, grid sizing in
the desktop and mobile apps — the exact domain (UI sizing) the tool was built
against. A pilot would run `fr --audit` over candidate files, refactor one or
two important calculations into the subset, and check the contracts into the
normal test sweep as a designed oracle. In ASSURE-REPO vocabulary, `fr`
contracts on a surface are oracle refs — index entries, not verdicts — and a
green `fr` run is conditional on its printed `assumes` lines. Frictions to
resolve honestly in the pilot: the CLI is Bun-only (`#!/usr/bin/env bun`)
while the monorepo contract is Node 24 + pnpm, so it must run as an isolated
pinned dev tool (or wait on upstream Node support); it pins the TypeScript 6
compiler API, which must be checked against the repo's TypeScript version;
analysis is same-file only, so helpers must be co-located with their
callers to get call-site checking; output is human text with no JSON mode
yet, so machine consumption (inventory integration, receipts) needs either
parsing or an upstream feature request. [source] [limitation]

**7.2 Adopt verifier-shaped authoring for important calculations.** Whether
or not the tool is adopted, its authoring guidance is worth folding into
agent contracts for numeric code: put important calculations in small named
synchronous functions; name a calculation before checking it; decide how
invalid inputs are handled (assert or normalize) before using them; choose
arithmetic order deliberately for floats; prefer explicit conditions and
loops; keep records immutable and unions tagged. Most of this is already the
house style; the delta is treating *analyzability by a mechanical oracle* as
an explicit goal of code shape, so future oracles (Freerange or owned ones)
get cheap. [source] [inferred]

**7.3 Import the discipline formulations.** Three sentences deserve to enter
the assurance vocabulary nearly verbatim: "Limits must fail closed, and each
one states which failure it buys"; "No limit may silently strengthen a
result"; and printed trust — every assumption a claim rests on renders where
the consumer can see it. OpenAgents holds these laws at product scale;
Freerange demonstrates them at per-finding granularity, including the
structural trick of giving partial evidence a type with no contract fields.
[source]

**7.4 Adapt the anti-laundering review harness.** The `review-round.js`
disciplines are immediately applicable to this repository's own Workflow
review patterns: require every finder lens to return a positive control
(`probesRun`) so an empty findings list proves a sweep happened; surface died
agents as explicit failures, never as clean rounds; require reproduced
contradictions for findings; calibrate lenses toward code people actually
write. The goal-prompt's "pair every required `unknown` or rejection with a
positive control through the same path" is a test-design rule worth adopting
wholesale. [source]

**7.5 Consider per-package decision ledgers.** `current-decisions.md` shows
that the challenge-ledger discipline scales down: owner-locked semantics,
rejected alternatives with measured evidence, and reopen conditions kept
beside the code they govern. For packages with heavy agent iteration
(assurance-spec, behavior-contracts, the harness contracts), a scoped ledger
in this format would prevent re-litigating settled semantics and make
"evidence, not the feature specification" the default calibration for scope
requests. [source] [inferred]

**7.6 Watch the static-assertion contract idea.** `console.assert` as a
zero-dependency, runtime-true, statically-discharged contract language is a
genuinely good interface. A long-horizon candidate: numeric behavior
contracts written inline in source and discharged by an oracle in the sweep,
rather than only registry-side oracle tests. The "Maybe Reconsider" entry on
procedure specifications (sidecar `requires`/`ensures` for imported
functions, with trusted-versus-proven visibility) is the germ of a
cross-module contract system worth tracking. [source] [inferred]

## 8. What OpenAgents should reject

- **Toolchain adoption.** Bun, oxlint, and knip serve Freerange's solo fast
  loop well, but the monorepo's Node 24 + pnpm + Vite Plus contract is
  settled. Consume the CLI as an isolated pinned tool; do not let a
  dependency's runtime leak into the toolchain contract. [source]
- **Contract greens as acceptance authority.** An `fr` pass is a designed,
  conditional oracle result — conditional on printed assumptions, scoped to
  the analyzed subset, silent about unsupported functions beyond the coverage
  line. It never substitutes for behavior contracts, AssuranceSpec
  obligations, or owner acceptance. The tool's own docs say this; keep it
  true in any integration. [source]
- **Unpinned or load-bearing adoption now.** Version 0.0.1, published the
  day before this audit, one maintainer, an explicitly absent API ("There's
  no API =)"), and a subset that may widen or narrow. Pin the exact version,
  keep it dev-only and advisory until it has weeks of stable history, and
  file upstream issues rather than forking or vendoring. [source]
  [limitation]
- **Rebuilding it in-house.** The analyzer embodies hundreds of adversarially
  tested float-exact decisions (subnormal underflow, NaN propagation,
  bit-stepped refinement). Reimplementing that surface for ownership's sake
  would repeat exactly the mistake the decision ledger warns against —
  speculative support without corpus evidence. Adapt the disciplines; consume
  the tool. [inferred]

## 9. Watch items

- **JSON/machine-readable output** — the blocker for receipts-grade CI
  integration and ASSURE-REPO inventory rows. [limitation]
- **Node compatibility** for the CLI, removing the toolchain friction.
- **Cross-module contracts** — the procedure-specification sidecar in "Maybe
  Reconsider" would let imported numeric helpers carry trusted-or-proven
  contracts, the main scaling limit today.
- **Subset evolution** — the ledger's reopen conditions (collection
  callbacks, bitwise ops, purity) show where the boundary will move if
  corpus evidence appears.
- **Adoption traction** — whether other agent-era codebases accept the
  refactor-into-the-subset bargain, the live experiment behind the whole
  inversion thesis.
