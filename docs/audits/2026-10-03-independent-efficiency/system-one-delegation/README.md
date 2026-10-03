# System One and tighter delegation

**Lean delegation has supporting evidence. The added benefit of System One
still needs a controlled test.** The [evidence synthesis](evidence-synthesis.md)
collects positive and negative results and explains which parts of the
thesis each result supports. Tracking:
[#10356](https://github.com/OpenAgentsInc/openagents/issues/10356).

The new panel has **zero scored executor sessions**. One actual Jev request
returned HTTP 402. Its [receipt](jev-capability-refusal.json) records a
deterministic fallback and unknown cost. A subsequent
[gateway capability call](../jev-lifecycle/capability/receipt.json) succeeds
with $0.000020664 reported usage. This resolves access through a different
transport; it does not establish the original pinned model version or settle
the refused request's charge. The separate
[lifecycle pilot](../jev-lifecycle/README.md) uses that gateway alias and now
retains 53 real component calls, costing $0.026275620 in reported usage.
Its evidence-selection and batching gains do not establish a native coding
win; the corrected review misses both tested order-dependence defects.
The single-trial coordinator now connects
preparation, native execution, final acceptance, and the complete endpoint
clock. Its offline tests pass. Linux preflight qualifies the first original
task and three replacements; ordinary-suite failures and a timeout leave
the other three original tasks unqualified. A prospectively registered
transport/model amendment, current native integration checks, and final
registration remain prerequisites.
The protocol remains unsealed.

## The comparison being prepared

The prospective [protocol](protocol.md) and [structured settings](protocol.json)
plan four tasks, six configurations, and two repetitions: **48 native sessions**.
Opus 5.5 and Sonnet 5.5 each run with native prompts and tools, a lean deterministic
briefing, and the same briefing with one Jev 1.13.0 ranking call. Every model
uses medium effort. Independent checks score the final candidate; no external
repair feedback follows.

The prepared candidate is lean Sonnet with Jev ranking. It must improve on
both deterministic lean Sonnet and native Opus while meeting independent
quality gates. This separates a model-choice saving from the incremental
System One effect. The proposed accounting and admission ceiling is $120;
the native budget flag and broker reservations do not guarantee an absolute
spending cap. A fixed panel, all failures, and separately registered future
iterations prevent selecting only favorable trials.

The retained [input provenance](inputs/provenance.json) binds the existing
2,191-byte lean-session preset and the common operational prompts. The latter
are written after public task release and before any scored outcomes. They
name the allowed paths, scratch environment, and the same formatting and crate
checks for every arm. Preparation uses the separate issue JSON whose bytes
match the frozen previews. Controls receive the common task prompt; prepared
arms append two newlines and the exact briefing bytes. This makes the added
context visible without silently changing the public task.

Native defaults run in the same offline historical-source environment as
treatments. Provider-side web, remote MCP, and container tools are refused;
client tool definitions remain available. This prevents later public fixes
from entering a historical replay. It also limits what the comparison can
say about an unrestricted native session.

## New component measurements

The [frozen preparation policy](treatment-freeze.json) uses exact Rust
Tree-sitter declarations, deterministic retrieval, a 16 KiB pack, and an
optional single Jev ranking batch. It freezes before reserve prompts are
released. It does not resolve types or prove dependency coverage.

Five [development measurements](component-timing.json) on known task #10167
complete in **0.6363–0.6602 seconds**, each producing the same 16,130-byte
[briefing](development-briefing.md). These local warm runs include Python
startup, index loading, committed-source validation, and output writes.

After freezing, the same policy runs five times on each of four reserve
snapshots on the owned Linux benchmark machine. The
[20 raw timing records](reserve-preparation-timings.json) include all results:

| Historical task slice | Warm process median | Range | Under 1 second | Separate index build | Preview |
| --- | ---: | ---: | ---: | ---: | --- |
| #10078: wide source tree observation | 0.792 s | 0.777–0.863 s | 5/5 | 6.042 s | [Read](previews/reserved-a/briefing.md) |
| #10228: rebuilt Linux engine discovery | 0.882 s | 0.871–0.935 s | 5/5 | 6.381 s | [Read](previews/reserved-b/briefing.md) |
| #10301: task lock contention | 0.974 s | 0.953–1.058 s | 3/5 | 6.775 s | [Read](previews/reserved-c/briefing.md) |
| #10298: tracked-branch worktree refresh | 0.906 s | 0.888–0.923 s | 5/5 | 6.650 s | [Read](previews/reserved-d/briefing.md) |

All five outputs for a task have identical briefing and candidate hashes.
The packs range from 15,715 to 16,101 bytes. **18/20 warm previews are under
one second; this does not establish a universal subsecond bound.** Trusted
checker compilation also uses the machine during these measurements, so
these are observed times with possible contention. Local development and
Linux reserve timings are separate series.

Cold index construction, live issue fetching, successful Jev inference,
executor work, and acceptance are outside these warm measurements. These
previews use normalized public task title and body; they are source-context
artifacts, not the complete scored-session instruction envelope. Candidate
quality has not been scored. Each preview's adjacent `preparation.json` and
`candidates.json` bind its source, policy, output, and recorded omissions.

The separate [alpha preview](alternative-alpha/README.md) takes 0.665 seconds
after a 4.772-second index build and produces a readable
[15,449-byte briefing](alternative-alpha/preview/briefing.md). This is one
observation, kept separate from the original 20. Exact source spans match, but
two available units are omitted from the delivered pack and one larger function
exceeds the candidate-size limit. The unchanged policy's limits remain visible.

The older, smaller beta and gamma snapshots also receive one unchanged-policy
preview each, after qualification:

| Task | Separate index build | Preview process | Delivered brief |
| --- | ---: | ---: | --- |
| [Beta: trace recovery](alternative-beta/README.md) | 0.565 s | 0.264 s | [16,239 bytes](alternative-beta/preview/briefing.md) |
| [Gamma: SDK validation](alternative-gamma/README.md) | 0.514 s | 0.264 s | [16,278 bytes](alternative-gamma/preview/briefing.md) |

These are single observations with no cache flush, Cargo contention, or model
call. They do not extend the original 20-run latency distribution or establish
coverage or executor improvement. The adjacent receipts retain exact inputs,
candidate pools, source verification, and omissions.

**The new previews expose weak retrieval.** Beta omits the ATIF reader and
writer implementations from its pool. Gamma contains no unit from `jev/src`;
only two delivered units are within `jev`, and both are tests. A semantic
reranker cannot restore those missing implementations. Executors retain file
tools to recover missing evidence, but no measured outcome yet shows whether
the supplied pack helps or adds work. Fast, faithful extraction is insufficient
evidence for adopting this preparation policy.

The independent [component recomputation](recompute_components.py) verifies
the frozen file and preview hashes and reprices the actual native capability
calls. Its [results](component-results.json) reproduce the 18/20 preview
count and $0.288885 known capability usage without importing the runner.

## Native execution and accounting preflight

The [setup findings](setup-notes.md) retain the admission, source-size,
compiler-mount, cache-freshness, and disk-capacity failures and the common
configuration changes made before scoring. The [environment record](environment.json)
describes the owned Linux benchmark machine. The
[independent infrastructure review](infrastructure-review.md) distinguishes
static review, synthetic regressions, and actual namespace probes.

The [runnable infrastructure](../../../../bench/delegation-study/README.md)
includes an isolated single-attempt native runner and a provider broker.
Its [143 local tests](local-validation-features.json) pass, including candidate identity,
accounting, signal cleanup, schedule admission, and incomplete-result handling.
The earlier [129-test](local-validation.json) and
[132-test](local-validation-followup.json) records remain retained. The
[storage amendment](storage-amendment.json) adds a guarded native scratch
release before acceptance, counting that release in the primary endpoint.
This avoids keeping two full build trees at once; it does not impose a disk
quota or establish current native-to-acceptance feasibility.
The [Cargo feature amendment](cargo-feature-amendment.json) binds explicit
features across the seed, common task instructions, native configuration,
and final checks. Configuration alone does not prove the executor runs a test.
These synthetic checks do not substitute for the real Linux and provider
preflights. The [draft schedule](draft-schedule.json) fixes 48 attempt identities
and order; it remains unsealed and grants no execution authority.
The [capability record](../../../../bench/delegation-study/infrastructure-preflight.json)
and [scrubbed call ledger](../../../../bench/delegation-study/infrastructure-provider-receipts.jsonl)
retain the actual test:

- Claude Code 2.1.288 runs Bash and an Agent child that uses Read. Both
  provider requests report `claude-opus-5-5`.
- The broker reprices **$0.288885**, agreeing with native cumulative usage.
  Its totals include the child request and both cache lifetimes. Adding
  the native total again would double-count the same usage.
- The requested $0.20 native budget ends the session with
  `error_max_budget_usd`; the native model does not report successful
  completion. This is a transport and accounting check, not a successful
  coding outcome.
- Two earlier admission probes forward no provider request. The $2 broker
  reservation rejects a native request asking for 128,000 output tokens
  before inference. The prospective scored configuration therefore keeps
  the $2 native budget request and uses a common $8 broker admission target,
  reserving a full six-arm block before launch.
- Namespace checks hide the shared repository, target directory, private
  sentinel, and owner files. Direct external networking is unavailable.
  A synthetic direct-socket request is charged and the next request is
  refused at the configured request limit. That synthetic test makes no
  model call.

Full executor setup is substantially more expensive than a warm briefing.
The selected historical trees contain about 1.53–1.55 GB of tracked files,
including retained benchmark evidence. One seed setup measures 3.63 seconds
for extraction and 14.62 seconds to create its isolated Git snapshot, before
Cargo fails on a missing system-link mount. That is retained infrastructure
evidence, not a completed seed or executor result. The new panel's endpoint
must include its per-run export and snapshot setup; the phase breakdown is
necessary to distinguish study setup from agent work.

After the common compiler-profile and cache-policy amendment, all five
[baseline seeds](../../../../bench/delegation-study/seed-preflight.json) compile
and export successfully: 7.89 GB total and 408.82 seconds of combined setup.
These measurements reuse a shared dependency cache, exclude earlier failed
attempts, and measure compilation and export rather than ordinary-test or
independent acceptance success. The setup report retains the earlier failures
alongside them.

New known executor usage is **$0.288885**, plus one Jev refusal whose billed
usage is unknown. Earlier independent rounds retain $25.4494342 in their
[separate ledger](../experiment-costs.json). The sum of known estimates is
$25.7383192; it is a lower bound, excluding the unknown request, audit-agent
usage, engineering time, and machine charges. None is an invoice.

## Task scope and partial blinding

The later [full acceptance preflight](acceptance-preflight.json) adds a
necessary eligibility check beyond the earlier task-specific calibration:

| Historical case | Full check time | Ordinary checks | Independent check | Final acceptance |
| --- | ---: | --- | --- | --- |
| Reserved A base | 25.61 s | Pass | Fail | Reject |
| Reserved A reference | 25.32 s | Pass | Pass | Accept |
| Reserved B base | 160.79 s | Fail | Fail | Reject |
| Reserved B reference | 176.75 s | Fail | Pass | Reject |

Both B variants fail the same pre-existing artifact-directory test; subsequent
ordinary targets do not run after that library failure. The reference therefore
cannot satisfy the original full-ordinary-test gate. The later
[C/D qualification](eligibility-cd.md) retains a 240-second C timeout and a
173.56-second D reference rejection caused by a stale ordinary integration
expectation. C shares the relevant source and fixture with D, so source review
predicts the same later failure; it is not an observed C integration result.
A proposed longer C retry is canceled before launch.

Only A qualifies under the original complete gate. No check is skipped, no
source workaround is applied, and no verdict is relaxed. Alternative task
eligibility is being screened before scoring. The original task manifest,
previews, draft schedule, and all failed setup attempts remain retained.
These are single feasibility observations, not native coding sessions or
population latency estimates. Cleanup is recorded separately. Eligibility
selection narrows the eventual study to historical bugs with healthy ordinary
suites in the tested Linux environment. It does not sample the full backlog,
and a smaller-library replacement cannot establish a large-workspace win.

[Alternative alpha, #9908](eligibility-alpha.md), qualifies under the same
240-second limit: the reference passes all gates in 55.81 seconds, the original
source fails the independent behavior checks, and two compiled incorrect fixes
are rejected. Its eight retained attempts include two invalid checker-fixture
runs; those do not count as behavioral evidence. The
[second bounded screen](selection-screen-2.md) adds qualified
[beta trace recovery](eligibility-beta.md) and
[gamma SDK validation](eligibility-gamma.md). Each reference passes its full
gate, each original source fails the independent checks, and two compiled
incorrect fixes are rejected. Gamma includes the explicit `jev/blocking`
feature. Its second incorrect fix passes ordinary tests but fails independent
checks, demonstrating additional coverage for that specific omission.

Beta and gamma are older than the original recent-week window, and no matching
fresh Claude launch was established. These are public issue trials, not
reconstructed observations of clean Claude sessions. Calibration errors and
missing offline dependencies remain in their records. Neither qualification
nor a known reference fix counts as a native executor outcome.

The [calibration report](calibration.md) and [24 retained attempt rows](calibration.json)
record four reserve base failures, four historical-reference passes, and
eight rejected negative controls. Each negative control compiles and changes
one relevant behavior. The corrected development task also has a base
failure, reference pass, and two rejected controls. These checks establish
sensitivity to those cases, not exhaustive correctness.

Calibration retains two corrections. An initial development checker adds
an unrequested display-timing condition; that condition is removed. A later
development attempt reuses newer Cargo artifacts when revisiting older
source timestamps; its apparent base pass is invalid. Fresh source timestamps
restore the expected base failure. Cargo records show every reserve variant
recompiled its owning library. The four reserve checkers remain unchanged.
Known calibration subprocess time is 653.686 seconds of building and
486.281 seconds of checking, including invalid attempts. Interrupted time
and preparation, orchestration, and authoring costs remain unmeasured.

The original [public task manifest](public-task-manifest.json) binds four source
commits, normalized public behavior, allowed paths, issue provenance, and
repository instruction hashes. One task is in `coder-boundary`; three are
in `coder`. These are bounded historical issue slices. In particular,
#10301 is still open at the manifest snapshot; this study makes no claim
about resolving the live issue.

The [prospective replacement record](replacement-panel.json) now binds A,
alpha, beta, and gamma in a separate
[public manifest](replacement-public-task-manifest.json) and
[48-run draft](replacement-draft-schedule.json). The draft has a new study UUID;
none of its attempt identities reuse the original draft. The treatment, quality
gates, thresholds, budget, and deadlines remain unchanged. It is unsealed and
does not authorize model execution.

The coordinator sees provisional issue numbers and brief labels before
full policy coding. Policy improvements use development task #10167 only.
Reserve prompts are released after the [treatment freeze](treatment-freeze.json);
checker code and reference solutions remain withheld from executors and
from preparation development. This is partial blinding, not task-identity
blinding. The [checker freeze record](oracle-freeze-safe.json) publishes
identities without exposing private checkers.

Four tasks supply four independent task clusters. Repetition helps measure
run variation but does not turn this into a broad repository sample. A
favorable result with equal observed acceptance would establish lower cost
and time at equal measured quality, not better correctness.

## Remaining work

Register any use of the now-verified gateway alias prospectively; it does not
prove that the original `jev-1.13.0` treatment ran.
Resolve or bound the refused request's unknown charge before claiming complete
accounting. Bind the qualified replacements, current native model capabilities,
cache seeds, executables, and the final schedule before sealing the
protocol. Then run the entire registered panel, preserving
failures, unused preparation, all provider calls, and elapsed time through
independent acceptance. No partial no-Jev panel substitutes for that test.
