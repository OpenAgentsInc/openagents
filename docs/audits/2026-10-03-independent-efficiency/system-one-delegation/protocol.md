# System One and tighter delegation: prospective protocol

**Draft; scored execution is not registered yet.** The coordinator must fill
the pending bindings in [protocol.json](protocol.json), verify provider access
and isolation, and seal the registration before any scored session. The coordinator saw provisional candidate issue numbers and brief labels
before full policy coding, but has not read the reserve prompts, source,
checkers, or reference fixes. This is partial blinding. Treatment improvements
used only the known development task #10167. Reserve prompts stay withheld
until the treatment freeze; checkers and reference fixes remain isolated.
A capability request that cannot run, including a payment refusal, does not
satisfy that prerequisite. Retain its receipt and any unknown cost.

This round tests four new tasks, six arms, and two repetitions: **48 native
Claude sessions**. It has no external repair turn and no Codex arm. The
accounting and admission ceiling is **$120 in estimated inference cost**.
Completing the study does not require a favorable result.

## Question and retained evidence

Does a bounded handoff with actual System One source selection reduce cost
and time while preserving independently checked quality? Separate that
question from cheaper-model selection and from the tighter workflow itself.

The [latest standing study](../claude-startup-followup.md) shows a useful
lean-Claude product configuration beating raw defaults on seven familiar
tasks, while changing several settings together. Its external checker lies
outside its run timer, and some preparation costs are omitted. The
[earlier factorial](../briefing-model-factorial/README.md) isolates model and
deterministic-brief effects on one historical task. It calls no System One
model, and a later diagnostic finds a defect after its original checker
passes. Those findings motivate this protocol; their task outcomes are not
new independent observations in this round.

## Arms and shared conditions

Both executor models request **medium effort**. Pin their exact served IDs
and the native CLI executable hash before execution. A shared effort label
does not mean equal internal reasoning across models.

| Arm | Executor | Prompt, tools, and source preparation |
| --- | --- | --- |
| A | Opus 5.5 | Native default system prompt and tools; no prepared source pack |
| B | Opus 5.5 | Lean-session prompt, six tools, deterministic AST source pack |
| C | Opus 5.5 | Exactly B, with one Jev 1.13.0 batch reordering the same source candidates |
| D | Sonnet 5.5 | Exactly A except the executor model |
| E | Sonnet 5.5 | Exactly B except the executor model |
| F | Sonnet 5.5 | Exactly C except the executor model |

**F is the candidate, chosen before outcomes.** B/C and E/F isolate the
incremental Jev ranking policy. B/A and E/D measure the combined lean prompt,
tool exposure, and prepared-source workflow. This design cannot attribute
the bundle's effect to an individual prompt or tool change.

The lean arms expose Bash, Read, Edit, Write, Glob, and Grep. They retain
ordinary source exploration and local execution when the brief is incomplete.
No other experimental Jev routing, embedding, recipe, model-selection, or
verification call runs in B/C/E/F. If this cannot be enforced and recorded,
the proposed System One ablation is invalid.

Every arm receives the same complete public task, applicable instructions,
write scope, native budget, and isolated environment. Native controls keep
their default prompt and built-in tools inside that environment; unrelated
connectors and owner settings are absent. Record these shared operational
restrictions when describing a control as a bare harness. No arm has access
to another candidate, the historical fix, the hidden checker, owner files,
or production credentials.

The common environment is offline. Native tool definitions remain intact,
but the broker refuses provider-side WebSearch, MCP, and container tools.
This closes the route by which a native agent could retrieve a future public
fix despite local network isolation. Native tool-search definitions may
remain available. Fast mode, priority service, and non-global inference are
also refused under the common serving policy. Retain refusals as part of
the run; any unsupported billed server-tool use has unknown cost and closes
further admission. This is an offline native-harness comparison, not a
comparison with unrestricted web-connected defaults.

These are experimental configurations derived from the lean-session shape,
not byte-identical shipped routes. There is no matched current-product arm,
so a win cannot establish improvement over the strongest shipped lean
configuration. Existing Codex standing results remain context only.

## Deterministic retrieval and actual System One treatment

The Rust syntax index and the experimental
[preparation adapter](../../../../bench/delegation-study/prepare.py) read an
explicit immutable Git source. Bind the source commit, index bytes, grammar,
extractor, preparer, task, candidate pool, and final pack by digest. Verify
each selected regular file's committed path-to-blob mapping and content
digest. Dirty working-tree bytes and symlink targets are not source evidence.

Preparation policy v2 uses weighted lexical overlap with inverse document
frequency, tuned only on development task #10167. Title terms have weight
three, paths and names ten, code one, and exact backtick names a bonus of
100. The bounded pool contains: at most **24 Rust files**, **32 complete declaration units**,
**6,000 bytes per unit**, and **28 KiB of serialized candidate state**.
The delivered pack, including its labels, is at most **16 KiB**. The total
Jev request is bounded at 60 KiB. A large declaration is omitted whole, with
the omission retained. The parser's resolution limits remain visible.
Selection preserves exact source bytes, with any Markdown separator outside
the source span. Adjacent context and unresolved dependencies are not a
promise of complete coverage.

Use stable ordering and arithmetic across processes. The candidate-pool
digest must match for all prepared arms on the same task and source. Jev
may reorder this pool; it cannot expand it, alter source, change the byte
limit, or add task-specific evidence. Otherwise C/B and F/E would change
retrieval as well as ranking.

C and F issue one real `jev-1.13.0` request per assigned session with a
nonempty pool, totaling up to 16 requests. The state contains the full
public request and bounded candidates. One independent Score question per
candidate uses these ordered levels:

1. Unrelated to the behavior the request changes.
2. Related background, but little direct implementation or regression value.
3. Useful dependency, behavior contract, or regression example for this change.
4. Directly implements or checks the requested behavior and should be read
   before editing.

The question names its candidate in its instructions, not only in the
caller-side question ID. Rank by the returned score, then deterministic
lexical rank and source location. No confidence threshold, task-specific
question rewrite, extra semantic pass, or model-selected execution authority
is part of this treatment. The official [Score contract](https://docs.typesafe.ai/primitives/score)
defines a graded judgment, not a probability that a patch will be correct.

Validate model and answer identities, numeric distributions, and response
bounds. One failed, refused, invalid, or unavailable request falls back to
the deterministic order. No paid request is automatically retried. Retain
the assignment to C/F, all observable cost, and the failure or empty-pool
reason. Unknown cost is never zero. Socket timeouts and the outer run
deadline must be documented separately; a socket timeout is not a guaranteed
total wall deadline.

Retain request and response digests, provider identity, usage, duration,
selection, fallback, and differences from deterministic selection. A valid
response that picks the same units remains an observation. If no delivered
pack changes, report that the proposed source-selection mechanism did not
change executor input; do not attribute random outcome differences to
better evidence. No held-out task is selected because Jev changes its pack.

The [model documentation](https://docs.typesafe.ai/models), checked October 3,
2026, lists Jev 1.13.0 at $0.042 per million input tokens with free output
tokens. Freeze that price source and all executor cache/input/output rates
before scoring. A future price change requires a dated accounting amendment,
not retrospective selection of the favorable rate.

## Task selection, checks, and freeze

The selector has one `coder-boundary` task and three `coder` tasks across at
least three behavioral families. All four are previously unused in the scored
experiments. This concentration within two crates limits transfer claims:
there is no broad repository-level generalization. Each task needs a clean
pre-fix source, a verifiable original public requirement,
a known reference fix, and offline Linux checks. Exclude earlier development,
held-out, reserve, and standing tasks, including trivial variants. New to
this experiment does not mean absent from model training.

The independent selector freezes eligibility and exclusions without choosing
for retrieval quality. The coordinator already knew provisional issue numbers
and short labels while coding the policy, so task identity was not fully
blinded. Record that exposure rather than claiming an entirely unseen task
selection. Reserve prompts, reserve source inspection, checkers, and reference
fixes stay outside treatment development until the treatment freeze. The
independent checker author may inspect the reference solution; neither it nor
private checker details may enter the task prompt, index, source export, or
brief. Known development task #10167 is the only task source used to improve
the treatment before this freeze.

Before model calls, map every acceptance requirement to public task or source
behavior. Calibrate the base, historical fix, and at least two plausible
incorrect variants per task. Cover connected producer/consumer behavior,
state integrity, unrelated inputs, and boundary conditions where required by
that task. Do not impose private function names or unrequested diagnostic
strings. Retain every calibration attempt; resolve flaky or environment-only
conditions prospectively and symmetrically.

Before sealing the registration, bind the task and instruction bytes, source
and export manifest, checker and verifier, syntax index and grammar,
preparer, lean prompt, tool configuration, CLI version and bytes, served
model IDs, prices, broker, sandbox, schedule, and reporting code. An unresolved
binding or unavailable provider blocks scored execution. No additional live
development harness calls run before this freeze beyond the bounded
capability/accounting probe. Offline development and previously retained
examples remain available.

The offline [schedule validator](../../../../bench/delegation-study/schedule.py)
can preview 48 stable attempt UUIDs in eight seeded, randomized six-arm blocks.
A preview is unsealed and grants no execution authority. Validation requires
a sealed protocol, actual successful Jev and native capability receipts, bound
isolation evidence, source and checker artifacts, the CLI executable, and the
complete reporting bindings. It reserves $48 plus preparation before admitting
another whole block within the $120 ceiling. This tool launches nothing. Paid
execution uses the separately bound
[single-trial coordinator](../../../../bench/delegation-study/trial.py) and
[final acceptance coordinator](../../../../bench/delegation-study/check_candidate.py).
Their offline tests do not replace a successful real provider preflight or a
sealed registration. There is no automatic paid panel loop.

The retained [48-run schedule preview](draft-schedule.json) covers the four
reserved aliases, six arms, and two repetitions with seed 20261003. Its
[provenance](draft-schedule-provenance.json) binds the new study UUID, generator,
and output bytes. It remains unsealed and cannot authorize execution; funded
provider preflights and all final bindings are still required.

The single-trial coordinator holds one local execution slot and uses only the
canonical `runs/RUN_UUID` directory below its registration. It verifies fixed
order, charges earlier retained work, and records admission for the whole
six-arm block before its first trial. Unresolved execution or costs stop later
admissions. It starts the monotonic endpoint before recurring registration and
archive validation, records validation duration separately, runs the frozen
preparation and native executables, then invokes independent acceptance.
It retains launch intent before subprocess creation and captures final
candidate, checks, accounting, and endpoint references durably. A native budget
exit can still produce an accepted candidate; the independent checks decide.

An existing attempt directory is inspect-only, including after an interruption.
Neither a missing native result nor a failed transport proves that no paid
request occurred. Keep its cost unknown and its execution unconfirmed until
evidence resolves them; never relaunch it automatically. Use `trial.py --inspect
--output PATH` to read it without execution. Blinded source review stays a
separate step, and the coordinator does not invent its receipt.

The bound `trial_config` artifact fixes phase deadlines, common native settings,
arm tool settings, source-repository and trusted seed locations, and acceptance
templates. Model, source, prompt, candidate, and run identities are derived from
registration. The native access token is supplied through a separate private
file consumed by the broker. Jev's narrow credential remains outside the
executor namespace. Public receipts contain neither credential paths nor
values; private configuration and process logs require separate publication
review.

A harness-module manifest binds all 12 local Python runtime modules, including
the capture and seed validators imported by the entry scripts. Verify their
hashes again before each phase. A main script's hash alone does not bind its
mutable local imports.

The common build environment sets `CARGO_PROFILE_DEV_DEBUG=0` and
`CARGO_PROFILE_TEST_DEBUG=0` for the base seed, native executor, and acceptance.
This pre-scoring storage amendment removes debug symbols without changing
optimization or assertions. The verified `cargo-reported-libraries-v1` seed
contains baseline libraries; final test executables are relinked. Keep the
failed default-profile setup evidence. After candidate, checks, logs, and
endpoint receipts are retained, remove completed per-attempt target copies and
reconstructible native and acceptance workspaces. Workspace removal additionally
requires the canonical candidate, change manifest, checks, source-archive
identities, and durable private logs. Record cleanup separately, outside the
primary endpoint. Preserve source archives, shared seeds, long-lived targets,
candidate payloads, and logs. Never follow a cleanup symlink or remove a phase's
scratch directories while its execution closure remains unconfirmed.

The [reporter](../../../../bench/delegation-study/report.py) reads static
expectations from `registration.report_bindings`. These include exact primary
and permitted auxiliary model IDs, prices, the original schedule, CLI identity,
per-task source and archive, public issue, index, preparer, policy, candidate
pool, and the fixed operational prompt envelope. Each arm binds its complete
argument tail and, for lean arms, system-prompt bytes. The normalized public
title and body used in preparation stay unchanged; common instructions are in
the separate base prompt. Native controls receive those base bytes. Prepared
arms receive the same bytes, two newline bytes, and the exact recorded source
pack. The native runner prefixes the registered run ID. Dynamic Jev output is
bound after its timed call; it is not represented as knowable before execution.

The reporter verifies preparation, pack, delivered prompt, and candidate-pool
artifacts against those bindings. Candidate identity is the manifest digest,
binding all before-and-after changes, including deletions, to the source and
payload digest. Payload entry types, contents, modes, and symlink targets are
validated without extraction. Final checks, the endpoint, and blinded review
must name that same manifest. Final checks must also confirm
`execution_closed=true`; a worker-only receipt cannot establish completed
acceptance. These are local provenance checks over retained
receipts, not remote attestation that the declared process ran faithfully.
The reporter and runner share the bounded candidate-format validator; cost
and comparison calculations are independently implemented in the reporter.

Report delivery, fallback, empty-pool, and changed-pack counts alongside all
assigned outcomes. An arm whose Jev calls never deliver changed executor input
cannot support the incremental mechanism gate, even if its assigned outcomes
are favorable. Native cumulative cost exceeding the broker's upper bound
leaves accounting incomplete; additional broker cost can reflect direct or
auxiliary requests. Never add the overlapping cumulative estimates together.

## Required isolation and authentication checks

Run these tests with synthetic secrets and a fake provider before transferring
real credentials. They exercise arbitrary shell behavior, not just argv
inspection. Retain their results and the tested infrastructure hashes.

- From the executor, try absolute and relative reads through the owner home,
  environment, process table, `/proc`, symlinks, inherited descriptors, and
  mounted files. Host credentials, hidden checker inputs, reference fixes,
  other runs, and orchestration records must remain inaccessible.
- Try writes and traversal outside the run's allowed source and scratch
  roots. Capture mode changes, deletions, symlinks, and new files as well as
  content changes. Reject archive links and unsafe paths before extraction.
- Try direct DNS, Internet, host loopback, and metadata-service access. Only
  the intended provider bridge and explicit local test transports may work.
- Through the bridge and directly through its mounted socket, try arbitrary
  hosts, redirects, paths, models, headers, malformed bodies, parallel calls,
  and oversized bodies. The broker must refuse disallowed requests and never
  return or forward a real credential to a different origin.
- Send a valid inference request directly from shell. It must be bound to
  the same run/model and included in metering, even when the native CLI did
  not initiate it. Prove that child and direct calls cannot disappear from
  accounting or bypass the registered admission policy. A hidden credential
  alone does not prove bounded authority or cost.
- Terminate the engine, bridge, and transport at controlled points. Preserve
  one attempt identity and its uncertain cost; never recover status by
  relaunching a paid command. Verify that delayed processes cannot mutate
  a final candidate after it is captured and checked.

A mock fixture proves only its exercised contract. Before scored runs, an
isolated capability probe must establish that the native CLI operates
through the same restricted broker and records its actual model and tools.
Provider permission and metering limitations remain explicit. No credential
value, account metadata, or raw owner transcript enters public evidence.

## Execution and timing

Run two fresh sessions for every task/arm pair. The schedule has eight
six-arm blocks, each covering one task and repetition. Freeze task order and
balanced arm permutations before outcomes; reverse matched orders where
possible. Publish the exact schedule only after provider feasibility and
all source bindings are established. Do not reorder based on preview quality.

Each session has one initial task input. It owns its exploration, internal
tests, and fixes. **There is no external repair feedback, automatic formatting
rewrite, second input, or coordinator patch.** After native completion, bind
the final candidate and run the common formatting check, affected ordinary
tests, and independent acceptance. The checker remains outside the executor's
tree. Retain nonzero exits, refusals, timeouts, incomplete patches, and failed
checks. Never choose the best intermediate patch after inspecting outcomes.

Use isolated scratch homes, clean exports, the same warm dependency setup,
and the same reserved compute resources. Keep build targets outside source
trees and bind checks to candidate bytes. Record cache state and shared
resource contention. A provider prompt cache is observed through usage;
it is not assumed to reset between runs.

The proposed executor deadline is ten minutes, followed by at most four
minutes for final external checking. The final registration must confirm
these are feasible for all tasks and set the same limits across arms.
Ordinary budget or time exhaustion is a task outcome.

Primary elapsed time starts before per-run source export/preparation and
ends when the final independent checks and candidate artifacts are durably
recorded. It includes preparation, Jev, engine work, internal fixes, final
checks, polling, and capture. Record each phase. Queue time, common cold
compilation/indexing, final cleanup, and later blind review are separate.
This is a warm workflow endpoint, not time through deployment, merge, or
issue closure. Report batch elapsed time separately from summed run time.

## Cost and admission

| Allocation | Estimated inference budget |
| --- | ---: |
| Capability/accounting probe | $2 |
| 48 main sessions, each requesting a $2 native budget | $96 |
| At most one prospective whole six-arm infrastructure replacement | $12 |
| Preparation, metering uncertainty, and other reserved overhead | $10 |
| Total accounting and admission ceiling | **$120** |

Six-arm blocks have a nominal $12 native budget. The common broker admission
target is **$8 per session**, so reserve **$48 plus preparation overhead**
for a full six-arm block. Native sessions still request a $2 budget. Two
retained preflights admit no inference: the former $2 broker target refuses
the native default first request (66,784 bytes with `max_tokens=128000`),
whose conservative reservation is about $3.76. Preserve that native output
default and apply the $8 broker target symmetrically. The capability probe
requests a $0.20 native budget and uses the same $8 broker target. Its actual
cost remains in the probe allocation or reserve if it exceeds the forecast.

The broker reservation uses a byte-based input estimate; it is a heuristic,
not an attested tokenizer upper bound or a hard invoice guarantee. This is
not a claim that native limits prevent every overshoot. The actual per-run total includes
Jev, direct broker calls, child calls, unused preparation, native recovery,
and any overrun. The coordinator must conservatively reserve a complete
six-arm block and its preparation overhead before launching its first arm.
Do not admit a new block if recorded spend plus the conservative reservation
exceeds $120. Unknown in-flight liabilities require reservation or a stop;
they cannot be treated as zero. Do not use the remaining budget to complete
only promising arms.

Meter every admitted provider request, including direct socket requests.
Native final cumulative estimates are counted once and cross-checked against
disjoint broker usage. Never add both overlapping totals. When cache lifetime
is missing but token counts are known, publish lower and upper cost bounds;
do not report a point estimate. A cost gate passes only when treatment upper
cost divided by comparator lower cost establishes its threshold. Otherwise
the cost comparison remains unevaluable unless the bounds prove a failure. Retain input,
output, cache-write and cache-read quantities, request identities, served
models, and price provenance. A strict broker reservation may be required
to bound arbitrary calls; publish the mechanism and any remaining limitation
before real execution. Report actual overshoot rather than clipping it.

The program ledger includes unsuccessful probes, failures, invalidated
blocks, replacement work, and preparation. Missing cost blocks a complete
cost claim. These are list-price estimates unless independently reconciled
with bills. Engineering, orchestration, machine charges, and historical
conversation cost are separate and may remain unmeasured. No net return on
engineering investment follows from this ledger.

## Analysis and prospective decision rules

Preserve all 48 assigned rows. Report each final autonomous acceptance,
internal test/fix cycle, limit outcome, cost, and elapsed time. Publish
per-task/per-arm means over the two repetitions, paired repetition ratios,
and variability. Four task clusters do not become 48 independent tasks.

For cost and time, the primary ratio is the sum of treatment task means
divided by the sum of comparator task means. Report the equal-task geometric
mean of positive task-mean ratios, raw totals, medians, and paired differences
as secondary summaries. A zero or unknown denominator is undefined; do not
drop that row. Cost per accepted run includes all assigned costs in its
numerator, including failures. Zero accepted runs never means zero cost per
acceptance.

Economic gates require both compared arms to have complete identities and
costs, and all **eight** final independent acceptances. Independently review
every final patch with arm/model labels hidden using the same frozen rubric:
public-contract defects, regressions, weakened tests, and prohibited scope.
A demonstrated material defect blocks practical recommendation even when
the frozen checker passes. Preserve the original checker verdict; label any
diagnostic invented after viewing a patch as retrospective. No model repair
follows this review, and review time is outside the automated endpoint.

| Comparison | Question | Panel threshold after the quality gate |
| --- | --- | --- |
| F/E | Incremental System One effect on Sonnet | Cost ratio at most 0.90; time ratio at most 0.95; cheaper and faster on at least three of four task means |
| F/A | Whole configuration versus native Opus | Cost ratio at most 0.80; time ratio at most 0.90; cheaper and faster on at least three of four task means |
| C/B | Incremental System One effect on Opus | Same thresholds as F/E; report separately |
| B/A and E/D | Tighter workflow at a fixed model | Same thresholds as F/A; report separately |
| E/B and F/C | Executor model at a fixed workflow | Report cost, time, and quality without attributing model savings to System One |

The combined panel decision requires **both F/E and F/A**. A cheap F/A
result with no F/E gain does not establish that System One helped. A missed
C/B comparison cannot be hidden by pooling it with F/E. No result in this
round is a matched Codex comparison or a shipped-product comparison.

Use 10,000 paired hierarchical bootstrap draws with seed 20261003: sample
task clusters, then paired repetition indices within each selected task.
Intervals are descriptive. Four clusters give weak information about
between-task variation, and individual 95% intervals across contrasts do
not provide a familywise guarantee. Report intervals spanning no effect even
if the point-estimate gate passes.

Distinguish favorable direction, a registered engineering gate on these four
tasks, and convincing generalization. The last needs a new prospective
confirmation with substantially more unseen task clusters and a sample-size
rule informed by this panel's variation. Do not extend this panel until an
interval becomes favorable. Equal acceptance supports no observed loss on
these checks, not better correctness. Claim “cheaper and faster at the
measured acceptance level” when quality ties; report an observed quality
improvement only when the acceptance/defect evidence actually improves.

## Amendments, stopping, and completion

Before scored execution, a dated infrastructure amendment may append to the
protocol chain. It must bind the previous protocol, changed files, reason,
tests, and exact scope, without silently replacing prior evidence. Treatment
changes need a new treatment freeze before task disclosure or an explicit
loss-of-blinding disclosure. The coordinator fills pending CLI, source,
task, check, tool, and schedule bindings before sealing the initial scored
registration.

After scoring begins, a confirmed infrastructure defect permits **at most
one whole six-arm block replacement**, prospectively registered before its
launch and within the reserved budget. Preserve and charge the old block.
Do not replace individual unfavorable arms. Ordinary model refusal, timeout,
failed patch, and frozen Jev fallback are outcomes, not replacement reasons.
An unresolved binding, unknown cost, or exhausted reservation leaves an
incomplete retained panel and no positive gate.

Complete all assigned valid rows regardless of apparent wins or misses.
Any later iteration must be separately preregistered with new tasks, its own
fixed budget and limits, and all earlier failures retained. Continued iteration
does not authorize adding favorable rows to this panel or stopping its
comparisons when they win. Completion means
the fixed rows, receipts, candidates, checks, blind review, bindings, complete
ledger, all planned comparisons, and independent recomputation are available.
A completed report may conclude that the hypothesis is unsupported. It does
not establish universal System One benefit, general model superiority,
production safety, or net economic return.
