# Audit of recently closed DeepSeek issues

Reviewed on October 22, 2026, by Codex. Issue inventory taken at 08:25 CDT
(13:25 UTC); source inspected at `50b42cce21`. This audit covers **58 closed
issues and 208 comments** from the October 20–22 DeepSeek implementation
campaign, including the R1 serial path, V4.1, its launch, Metal, and the
closed Maya follow-ups. It also reads the six open Maya issues for context.
Flint is outside the audit except where an issue shares Pylon or Microcoder
integration with DeepSeek.

The [issue inventory](2026-10-22-deepseek-closed-issues/issue-evidence-inventory.json)
records every issue, comment count, closing time, and identifiable commit
reference. All 168 resolving commit references in those threads are ancestors
of the inspected main commit. This establishes integration, not correctness.

## Assessment

**Psionic has a substantial native inference implementation and demonstrated
stage serving. DeepSeek-V4.1 is still an unqualified experimental preview.**
The recent closures do not establish the original eight-gate release. Their
closing comments explicitly report an owner-approved preview rescope; this
audit accepts that as the reported scope decision, rather than treating
closure itself as a numerical pass.

The most urgent findings concern recovery and misleading machine-readable
qualification results. The latest qualification report is a genuine failure,
and some of its explanation is inaccurate. Several older audit findings have
been fixed well; those improvements should be preserved.

Read-only checks also caught an operational outage: at 08:30 and 08:31 CDT,
the synchronized production Pylon directory listed six providers and no
DeepSeek group. The old watcher had stopped, while a replacement GPU VM still
existed. During the audit, another operator started a new watcher and
replacement VM. These observations are timestamped below. The group reappeared online at
08:47 CDT during the audit; no new chat was submitted to it.

## Method and limits

I read issue bodies, technical discussion, closing comments, source, and
retained evidence. Large inline checkpoint reports were parsed for their
identity, coverage, and results. I downloaded the 48.8 MB confirmation report
and checked its SHA-256, recomputed its summaries, inspected the retained G2
programs, and rescored those answers. I reran the 13 G2 checker tests and 41
spend-guard tests and executed isolated reproductions of new findings.

I did not allocate a VM, run a new model generation, change a service, create
a production chat, or rerun Rust/CUDA/Metal suites. Historical GPU results below
are retained evidence, not new executions by this audit. The new
`--kv packed` commit `62d86e8a99` landed during the review for open #12471;
its kernels are outside this source snapshot.

The [review metadata](2026-10-22-deepseek-closed-issues/review-metadata.json)
and compact evidence beside this document make those distinctions explicit.

## Findings

P1 means fix before relying on the affected qualification or unattended
service guarantee. P2 means a correctness, evidence, or tracking gap requiring
a bounded follow-up. None of these findings demonstrates that every answer
from the preview is incorrect.

| ID | Priority | Finding | Evidence |
| --- | --- | --- | --- |
| F1 | P1 | A lost cloud-create reply left an unserved replacement VM, blocked relaunch, and escaped the watcher's accounting. | Production directory, watcher state/log, GCE instance IDs, guard records |
| F2 | P1 | Failed release qualification prints `passed: true` and returns success from the CLI. | Actual confirmation stdout and matching failed report; source |
| F3 | P1 | The measured GPU router score error uses an F64 surrogate that selects different sets on 173 rows. | Full confirmation report and score reconstruction source |
| F4 | P1 | The preview watcher's per-launch allowance can exceed its own cap after a paid failed launch. | Isolated execution of the real watcher loop |
| F5 | P1 | The profile named `qualified` does not establish numerical qualification. | NIP-PYLON and routing implementation; legacy preview identity |
| F6 | P2 | The index-check exclusion gives an inaccurate reason for missing coverage. | Pinned worker records selections; oracle manifest contains selection tensors |
| F7 | P2 | G2's new floor still allows a failed reasoning split, and complete forged test output still passes. | Local controls and full-suite counterexample |
| F8 | P2 | Closure summaries misdescribe the failed cohort and overstate the margin verdict. | Confirmation report: one failing prompt has 38 tokens; ten margin verdicts are false |
| F9 | P2 | Closure and documentation leave qualification, live checks, and deployment status inconsistent. | Closed issues, definitive checklist, after-close list, preview receipt |

### F1. Lost-create recovery can strand a billed VM

Relevant issues: [#12402](https://github.com/OpenAgentsInc/openagents/issues/12402),
[#12403](https://github.com/OpenAgentsInc/openagents/issues/12403),
[#12409](https://github.com/OpenAgentsInc/openagents/issues/12409), and
[#12411](https://github.com/OpenAgentsInc/openagents/issues/12411).

The guard correctly preserves an uncertain allocation. The watcher does not
finish the recovery around it:

1. At 04:30:59 CDT, the original head session ended. The first preview VM's
   guard record, `facbb012…`, was settled at $50.741023.
2. A replacement attempt began at 04:44:26 CDT. GCE created a replacement
   `oa-v41-preview-m0` in `asia-northeast1-b`, instance
   `5919665083922015646`, at 04:46 CDT.
3. At 05:48:28 CDT, `gcloud` returned a connection-reset error. The guard said
   the VM might exist and kept it counted. Every subsequent zone attempt was
   refused because the VM name was already running in the budget.
4. The watcher's saved `cloud.json` and `ledger_ids` still named the original
   allocation. The replacement's start record, `b065d389…`, was not included
   in its own spend estimate. Its $180.32 booking was a reservation, not an
   observed $180.32 invoice.
5. At 08:29:06 CDT, the watcher stopped "on request". This audit does not
   identify who requested the stop. At 08:30:38 and 08:31:38, production's
   synchronized `/pylons/live` snapshot had no DeepSeek service, and the
   replacement VM was still running.

The implementation of
[`spend`](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/scripts/pylon/v41-live.py#L140-L168)
only recognizes allocation IDs already saved in watcher state or its launch
record. The
[`launch` completion path](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/scripts/pylon/v41-live.py#L217-L232)
reads that same launch record. It does not discover an allocation that exists
under the run's labels after a lost create reply.

This is a real recovery failure, not a failed numerical test. It also explains
why the last successful chat receipt cannot establish current availability.
The larger spend guard still counts the allocation; the faulty claim is the
watcher's recovery and its own sub-cap accounting.

**Change:** reconcile allocations by run and immutable instance identity after
an uncertain reply. Adopt and resume a verified owned allocation, or drain and
delete that allocation safely before retrying. Include it in accounting even
if it never reached readiness or a launch record was not saved. Preserve
#12411's protection against deleting a different instance that reused a name.
Test lost-create replies, head-session loss, stale launch records, and stopping
a watcher with an unadopted allocation.

**Later observation:** a new watcher, run
`v41-live-20261022-083251-e312`, started during this audit. At approximately
08:40 CDT the replacement instance was `5624640619264061266` and was still
staging. At **08:47:23 CDT**, production's synchronized directory listed
**DeepSeek-V4.1-Flash-1017 · Experimental preview** online again. That confirms
restored discovery, not a fresh generation or a fix for the source defect.
The new controller and accounting handoff still need the lost-reply regression.

[Selected live observations](2026-10-22-deepseek-closed-issues/live-observations.json)
retain the initial directory snapshots and allocation records. No service was
started or stopped by this audit.

### F2. The release command announces success for the failed report

Relevant issues: [#12405](https://github.com/OpenAgentsInc/openagents/issues/12405)
and [#12424](https://github.com/OpenAgentsInc/openagents/issues/12424).

The exact retained confirmation log ends with:

```json
{"passed":true,"out":"/mnt/stage/out/confirmation"}
```

The matching report has `passed: true`, **`release_passed: false`**, 74 policy
violations, and ten release refusals. Its SHA-256 matches the one posted in
the issue: `656a3787e30f79bf6cef5453205131f12a73f44c18624cd3136a88c723a53527`.

[`run_qualify`](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/crates/psionic/crates/psionic-v41-stage/src/main.rs#L388-L397)
prints the legacy `passed` field and returns `Ok(())`, regardless of the release
verdict. The
[report builder](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/crates/psionic/crates/psionic-v41-stage/src/qualify.rs#L1750-L1801)
computes `passed` from checks that omit the new router score budget, then
calculates a separate stricter release verdict. The shell runner additionally
logs and continues after a nonzero exit so it can preserve evidence.

The human reviewer read the right field and reported failure. An automated
consumer of the CLI summary or process status can reach the opposite answer.

**Change:** make the primary verdict reflect the requested mode. For
`--release`, print the release result and exit unsuccessfully on a failed gate
while retaining the report. Rename any legacy diagnostic result so it cannot
be mistaken for qualification. Test the actual CLI with a failed release
fixture, not just the report's internal booleans. A collection script may
continue deliberately, but its final result must preserve the failure.

### F3. Router score measurements do not yet bound the served arithmetic

The GPU worker does not expose its actual selection scores to this judge.
[`candidate_router_scores`](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/crates/psionic/crates/psionic-v41-stage/src/qualify.rs#L1236-L1312)
falls back to an F64 calculation on captured `FfnIn` and checkpoint gate
weights. This is a useful diagnostic, and the report labels it `f64`.

The earlier h6 investigation established matching selected sets for this
surrogate only at layers 0 and 1. The fresh confirmation report contains
**173 rows where the F64 selected set differs from the GPU's**, out of 468,840
router rows. All thirteen cases contain at least one such row, including the
three cases that pass the current release checks.

The source counts these as `local_changed`, but neither
[`margin_verdict`](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/crates/psionic/crates/psionic-v41-stage/src/qualify.rs#L1115-L1158)
nor the release evaluator requires that count to be zero or includes a bound
on the actual GPU-to-surrogate score difference.

This is not proof that those GPU selections are wrong. It is evidence that
"the GPU's measured score error" is currently a proxy, and that the h6
layer-0/1 result cannot be extended to all forty layers. Direct oracle-score
regret on the actual selected IDs remains a useful independent measurement.
It does not turn the proxy into the actual device scores.

**Change:** record the actual device scores, or measure and account for the
device-versus-surrogate error over the relevant shapes, layers, and contexts.
If a score-proxy method is retained, name its scope and require the appropriate
selection-consistency/error evidence. Freeze the next calibration only after
this distinction is addressed.

### F4. Zone retries reuse an outdated allowance

In the real watcher
[`serve` loop](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/scripts/pylon/v41-live.py#L235-L275),
spend and remaining hours are calculated before the zone loop. A launch that
incurs cost but fails before readiness returns `refused`; the next zone still
receives the old allowance. Rounding requested hours to the nearest hundredth
can also exceed a strict cap or end time slightly.

An isolated run of that loop, with all cloud operations replaced, gives:

- cap: $300; price: $6.654/hour;
- first launch fails after a simulated $10 of cost;
- second launch is still offered 45.09 hours, about $300.03;
- total admitted cost in the simulation: **$310.03**.

No money was spent by the reproduction. This does not claim the real preview
has exceeded $300. It demonstrates that the advertised sub-cap is not enforced
under this failure sequence. F1's missing allocation IDs introduce an
additional accounting gap.

**Change:** reconcile spend and recompute the time/cost allowance before every
attempt, reserve transfer costs consistently, round down, and stop when the
remaining allowance cannot support another launch. Apply the absolute stop
time during a blocking launch as well as between attempts. Add a failed-paid-
launch/retry test.

[Reproduction output](2026-10-22-deepseek-closed-issues/watcher-cap-reproduce.json).

### F5. `qualified` is a routing category, not a qualification check

Relevant issue: [#12468](https://github.com/OpenAgentsInc/openagents/issues/12468).

The new profile isolation is valuable: a plain model request does not select
a beacon explicitly marked `consumer-q`. The side-by-side fixtures exercise
that distinction.

However, [NIP-PYLON](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/nips/openagents/NIP-PYLON.md#L186-L195)
and [`Group::profile`](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/crates/nostr/src/pylon.rs#L238-L248)
classify an absent profile as `qualified`.
[`serves_want_when_free`](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/crates/pylon/src/group.rs#L106-L128)
checks readiness, freshness, and that profile string. It checks no numerical
qualification status or report identity. The unchanged legacy preview head
omits the new profile and therefore belongs to this category.

Thus `--profile qualified` can mean "the ordinary native-weight configuration"
while the group is explicitly **not G1-qualified**. A name containing
"preview" changes display text, not routing eligibility. I found no evidence
that the explicit consumer-q profile bypasses the new separation; the defect
is the meaning of the qualified category and its missing qualification state.

**Change:** call the arithmetic/deployment category `baseline` or another
neutral name, and represent preview/qualification status separately. If callers
can request verified qualification, bind that state to the relevant artifact,
bundle, manifest/configuration, policy, and report. Missing legacy status is
unknown, not a scientific pass. Preserve backward compatibility deliberately
rather than deriving certification from an omitted field or provider label.

### F6. Some index-selection evidence already exists

Policy 2.3 excludes the index rule with the explanation that the pinned worker
records no index selections and the oracle has none per row. Complete per-row
margin evidence is indeed missing. The broader absence claim is incorrect; the
[index evidence inventory](2026-10-22-deepseek-closed-issues/index-evidence.json)
records the checked sources:

- The pinned worker source `70af56e80c` records
  [`V41Site::Selection`](https://github.com/OpenAgentsInc/openagents/blob/70af56e80c/crates/psionic/crates/psionic-backend-cuda/src/v41_stage.rs#L2298-L2306)
  and exports it through
  [its observation adapter](https://github.com/OpenAgentsInc/openagents/blob/70af56e80c/crates/psionic/crates/psionic-v41-stage/src/cuda.rs#L398-L411).
- The calibration bucket actually contains a 2,048-byte
  `p2-code-0-21.stage2.L20.Selection.u32` object, generation
  `1792604174505567`.
- The committed oracle manifest contains **160 named decode
  `attn.indexer.topk_idxs` tensors**. The
  [oracle hooks](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/scripts/bench/deepseek-v41/oracle_dump.py#L469-L500)
  record selections, index scores, and candidates at captured positions.
  Earlier W5 tests used that evidence.

**Change:** distinguish available captured-position evidence from missing
all-row coverage. Evaluate the comparisons already possible, record their
limits, and identify the additional capture or evaluator work needed for the
full index criterion. An explicit scoped exclusion is more honest than an
unresolved check silently passing, but it still does not establish the
original G1 index requirement.

### F7. G2 is improved but two checker holes remain

The earlier false positives are fixed: the current 13-test suite passes,
empty answers fail, and the simple early-exit functions used in the previous
review now fail. Rescoring the retained real answers reproduces the published
report byte for byte: SHA-256
`49d3a97f6e2ae918f7a1b85b40e63adacf83f73fdb7d29f181f6ad2ffcfec887`.
Both runtime and oracle pass 5/6 and finish all six completions. These actual
answers do not contain the synthetic exploits below.

Two gaps remain:

1. In [`judge`](https://github.com/OpenAgentsInc/openagents/blob/50b42cce21/scripts/bench/deepseek-v41/g2_check.py#L164-L184),
   missing thinking-mode reasoning sets the task's result false but does not
   set its must-pass floor false. A full-suite counterexample removes the sky
   answer's reasoning and fixes the Fibonacci function. It still reports
   `passed: true`, `floor_held: true`, 5/6 versus the oracle's 5/6. A mandatory
   parser failure is compensated by a different task's improvement.
2. Test-result markers are still emitted in the generated program's stdout.
   A function can print every expected `G2-RESULT N ok` line and exit before
   returning. Both Rust and Python versions receive `true, "all cases hold"`.
   The existing forged-line test covers a partial forgery, not a complete one.

**Change:** make required mode/parser checks part of the non-compensating
floor. Use a checking arrangement that verifies returned results rather than
trusting generated stdout as proof of executed assertions. At minimum, add
these complete-forgery controls and describe any remaining trust assumption.

The new suite accurately limits its claim to six greedy completions. It is
not a statistical noninferiority study of coding or general usage. Its
`fib(186)` assertion is explicitly non-must-pass, while still counting the
whole task as a failure; that is a disclosed exemption, not a corrected model
answer. The runtime's accompanying prose even incorrectly says that this
function returns `Some` at 186.

[Rescored real report](2026-10-22-deepseek-closed-issues/g2-rescored.json),
[parser-floor counterexample](2026-10-22-deepseek-closed-issues/g2-parser-offset-report.json),
[complete-forgery controls](2026-10-22-deepseek-closed-issues/g2-forgery-reproduce.json).

### F8. The confirmation failure is not exclusively long-context, and its margin verdict is false

The closing comments say "74 violations in the 10 long prompts" and list
margin/regret among passing output checks. The raw report is more specific:

- There are **74 failing case/layer combinations**, involving **301 router
  rows**, across ten cases. This is not 74 failed prompts or all changed rows.
- Nine failing cases exceed the reference's 263-token maximum. The tenth,
  `c4-engram-variants`, has **38 prompt tokens**. Its layer-15 score error is
  about 0.8626 against a budget of 0.8363.
- `margin_verdict` is **false in all ten failing cases** because score-budget
  validity is part of that verdict. It is not an unrelated internal diagnostic.
- Every changed set has enough oracle scores and passes the separate
  gap/substitution/regret calculations. Those useful observations do not
  establish the score-bound premise that failed.

The [confirmation summary](2026-10-22-deepseek-closed-issues/confirmation-summary.json)
preserves counts and per-case results. Expanding calibration to long contexts
is necessary for a broad launch claim, but the 38-token failure means it is
not an established complete explanation. Diagnose that short Engram case too.
Do not remove the failing score-bound requirement and continue describing the
same evidence as a passing margin-aware qualification.

The output evidence is nevertheless meaningful: 538 compared logit positions
across 26 case/mode units; 14 substituted top-1 decisions, or 97.4% agreement
on those positions; mean TV about 0.0261, maximum TV about 0.3318; and exact
Engram checks plus replay equality pass. These are descriptive measurements
on correlated traces, not independent-sample quality guarantees.

### F9. The work tracker no longer states one consistent completion contract

The closing comments are explicit about preview status, which is good. The
rest of the handoff is inconsistent:

- The definitive completion document still says the epic closes only after
  all eight gates pass. Its preview section says the epic stays open and G2
  is being reworked. #12369, #12403, #12389, #12405, and #12424 are now closed.
- The native document accurately records the failed 2.3 report, while some
  older sections still say v2 is pending or has no frozen policy.
- The only open DeepSeek epic at inventory time is Maya #12462. Its children
  concern serving optimizations and quantization; none owns a replacement
  G1 qualification policy or the failed short-case investigation. A hypothetical
  policy 2.4 in closing comments is not an assigned follow-up.
- After-close checks still require G4, one-machine and cross-machine numerical
  acceptance, pinned-build machine-loss checks, per-device qualification,
  long-prompt acceptance, and some profile integration checks. Closing code
  issues is allowed by the repository contract; it does not complete those
  evidence gates.
- #12403's launch comment says status/stop commands were added to
  `NEEDS_OWNER.md`, but that entry is absent in the inspected main snapshot.
- The preview receipt's production/staging answer headers omit the preview
  warning. The new warning and visualization are implemented in source, but
  the launch comment itself says the web change was not deployed. A local
  capture does not verify the same presentation in production.
- #12421's title concerns repeated answers and the DeepSeek control, while
  its current issue body describes a different Cloud Build optimization.
  Its closing comment and source references, rather than that body, establish
  what was actually fixed.

**Change:** update the top-level status to "preview scope delivered;
qualification not passed," retain the original gates as unmet, and give each
remaining item a named follow-up or explicit retirement decision. Separate
source completion, deployed behavior, and qualification in the tables. Keep
the current deployment/recovery owner and stop commands in the handoff.

## What the closed work does establish

- **R1 serial native serving:** the opt-in Qwen2 CUDA path and managed worker
  recovery were exercised on the pinned 1.5B model. The larger sampled/long
  prompt limitations remain explicit; this is not universal native support
  for every existing profile.
- **Weights and protocol:** the native safetensors loader, exact layouts,
  tokenizer/Engram map, template and token-level parser are real. The retained
  loader report opens 96,085 tensors, verifies all 48 shard hashes, and reports
  19/19 selected dequantization parity cases. Protocol mock tests do not prove
  tool execution or overall model quality.
- **CPU implementation:** the complete transformer, shared state, numerical
  boundaries, and greedy runner exist and have independent tiny fixtures.
  Original-weight G1 v1 remains failed. Its nonzero policy was preserved;
  exhaustive B200 emulation was correctly removed as a launch prerequisite.
- **CUDA and stage transport:** kernel fixtures passed on the documented
  devices; the retained pinned SM120 log has 82 passed, zero failed, and four
  ignored measurement tests. Generation fencing, serial admission, bounded
  handoff, build pins, same-mode comparisons, and two cross-machine placements
  have substantive evidence. Component/transport success does not establish G1.
- **Metal:** 48 G3 checks pass with three ignored measurement tests. The real
  last-stage replay has 51/51 greedy and 19/19 seeded steps with every logit
  bit-equal to CUDA. This verifies the tested tail stage, not arbitrary Metal
  placement or equivalence to the B200 oracle. Full per-device qualification
  and a pinned mixed launch remain after-close checks.
- **Maya exact techniques:** hot-expert caching, host-computed misses, and SSD
  Engram staging are implemented, with bit-equality fixture checks and real
  single-box measurements. `consumer-exact` means equivalent to the ordinary
  Psionic configuration in the tested conditions, not B200-qualified.
- **Consumer quantization:** the separate artifact/profile boundary is the
  correct design. The reported 26.4 tok/s result produces unusable text and has
  no completed quality card. That issue remains open, as it should.

## Gate assessment

| Gate | Current evidence and practical meaning |
| --- | --- |
| G1 | Failed under v1, 2.1, and 2.3; 2.2 could not freeze. Fresh 2.3 confirmation is retained. F2/F3/F6 must be addressed before relying on a successor. |
| G2 | Current finite suite passes its declared comparison at 5/6 versus 5/6, with a disclosed Fibonacci failure. Parser-floor and checker gaps remain; no broad quality noninferiority claim. |
| G3 | Retained per-kernel checks on supported CUDA devices and Metal. Strong component evidence. No fresh GPU execution in this audit. |
| G4 | Executor implemented and measured. Required isolated stage numerical qualification is still on the after-close list. |
| G5 | One-machine full-model serving and runtime behavior demonstrated; numerical acceptance remains dependent on G1. |
| G6 | Four-single and two-dual serving and performance demonstrated; qualified numerical acceptance remains incomplete. |
| G7 | Real crash/freeze/cancel/fencing checks exist. Pinned-build machine-loss rerun remains listed; whole-service lost-create recovery has F1/F4 defects. |
| G8 | Historical launch/chat/capture evidence exists. A fully qualified launch did not occur, and the live recovery state changed during this audit. |

## Issue-by-issue closure audit

The table below distinguishes completed implementation from its remaining
acceptance work. It does not recommend reopening every code-complete issue.

| Issue | Closed work | Evidence and limit |
| --- | --- | --- |
| [#12357](https://github.com/OpenAgentsInc/openagents/issues/12357) | Personal serial plan delivered. | Documentation scope only; implementation followed in later issues. |
| [#12358](https://github.com/OpenAgentsInc/openagents/issues/12358) | Worker-owned admission and strict final-answer checks. | Fixture and targeted checks; native/live checks moved to later R1 issues. |
| [#12360](https://github.com/OpenAgentsInc/openagents/issues/12360) | Real R1 llama.cpp serial relay qualification. | 27/27 retained checks; superseded for the opt-in native path, not all profiles. |
| [#12364](https://github.com/OpenAgentsInc/openagents/issues/12364) | Managed serial hard recovery. | 13 real GPU recovery checks retained; foundation reused by stage groups. |
| [#12367](https://github.com/OpenAgentsInc/openagents/issues/12367) | Explicit R1 profile through pylon up. | 20/20 lifecycle checks retained; unspecified profiles retained llama.cpp. |
| [#12368](https://github.com/OpenAgentsInc/openagents/issues/12368) | Native R1 Qwen2 CUDA and recovery. | Greedy profile passes retained checks; sampled and larger-prompt failures disclosed. |
| [#12369](https://github.com/OpenAgentsInc/openagents/issues/12369) | Native V4.1 stage-serving epic administratively closed as preview. | Original all-gates completion is not met; F1–F9 and the after-close list remain. |
| [#12370](https://github.com/OpenAgentsInc/openagents/issues/12370) | Spend guard, expiry, and alert setup. | Cheap expiry test retained; billing alerts later created. Guard is not an invoice. |
| [#12371](https://github.com/OpenAgentsInc/openagents/issues/12371) | Original-weight B200 oracle and full-checkpoint loader checks. | Pinned captures include same-mode logits and selected activations; chunked p4 absent. |
| [#12372](https://github.com/OpenAgentsInc/openagents/issues/12372) | Tokenizer/template and token-level stream adapter. | Golden and injection/cutoff checks; mock streams do not qualify tool execution. |
| [#12373](https://github.com/OpenAgentsInc/openagents/issues/12373) | Native safetensors and exact packed layouts. | All 48 hashes and 19 selected dequantization cases checked on original weights. |
| [#12376](https://github.com/OpenAgentsInc/openagents/issues/12376) | Corruption refusal, authority, reserves, uncertain settlement. | Code complete; 41 current guard tests rerun here, including later fixes. |
| [#12377](https://github.com/OpenAgentsInc/openagents/issues/12377) | CPU mHC/Sinkhorn. | Independent component fixtures; full-model qualification separate. |
| [#12378](https://github.com/OpenAgentsInc/openagents/issues/12378) | CPU routing and experts. | Selected-row and packed-format fixtures; original connected parity not implied. |
| [#12379](https://github.com/OpenAgentsInc/openagents/issues/12379) | CPU attention/compression/selection. | Boundary and long-selection fixtures; later projection/state integration separate. |
| [#12380](https://github.com/OpenAgentsInc/openagents/issues/12380) | CPU Engram normalization/hash/gather. | Full tokenizer-map hash checked; connected model qualification separate. |
| [#12381](https://github.com/OpenAgentsInc/openagents/issues/12381) | Checkpoint projection and grouped-output wiring. | Independent nonuniform fixtures; original-weight qualification separate. |
| [#12382](https://github.com/OpenAgentsInc/openagents/issues/12382) | Adjacent-pair tail RoPE/YaRN. | Position fixtures through 1M; does not establish a served 1M context. |
| [#12383](https://github.com/OpenAgentsInc/openagents/issues/12383) | BF16/FP8/FP4 activation boundaries. | Conversion fixtures; device and original-weight results followed separately. |
| [#12384](https://github.com/OpenAgentsInc/openagents/issues/12384) | Request-owned shared KV/index state. | Tiny forty-layer attention, resets and rollback; not a complete transformer yet. |
| [#12385](https://github.com/OpenAgentsInc/openagents/issues/12385) | Connected CPU text block. | Independent block intermediates and sparse-read checks. |
| [#12386](https://github.com/OpenAgentsInc/openagents/issues/12386) | Complete forty-layer CPU transformer. | Independent tiny-model logits and rollback; original checkpoint G1 still fails. |
| [#12387](https://github.com/OpenAgentsInc/openagents/issues/12387) | CPU greedy decoder and checkpoint runner. | EOS/limits/cancel/ties and tiny-model tokens; not a served GPU claim. |
| [#12388](https://github.com/OpenAgentsInc/openagents/issues/12388) | W1 comparison and first-divergence reporting. | Valid closure under its failure-report path; G1 v1 failed and remained failed. |
| [#12389](https://github.com/OpenAgentsInc/openagents/issues/12389) | CPU answer diagnostics and finite served-runtime G2 evidence. | Closed as preview; 5/6 equals oracle, Fibonacci failure and F7 remain. |
| [#12390](https://github.com/OpenAgentsInc/openagents/issues/12390) | Measured weights/cache/step/Engram memory and placement inputs. | Real peaks retained; refresh for changed configurations. Not qualification. |
| [#12391](https://github.com/OpenAgentsInc/openagents/issues/12391) | Packed FP4/FP8 CUDA matvec and activation quantization. | SM89 component evidence; SM120 confirmation followed in W8. |
| [#12392](https://github.com/OpenAgentsInc/openagents/issues/12392) | CUDA attention, indexer and candidate kernels. | Real/synthetic component tests; oracle coverage limitations explicitly reported. |
| [#12393](https://github.com/OpenAgentsInc/openagents/issues/12393) | CUDA mHC/Engram/routing/MoE kernels. | Fixed component tolerances and host-model checks; connected drift separate. |
| [#12394](https://github.com/OpenAgentsInc/openagents/issues/12394) | Device-resident CUDA stage. | Executor measured; G4 numerical acceptance remained on the after-close list. |
| [#12395](https://github.com/OpenAgentsInc/openagents/issues/12395) | RTX PRO 6000 target choice. | SM120 suites and bandwidth retained. Hardware choice is not G1. |
| [#12396](https://github.com/OpenAgentsInc/openagents/issues/12396) | Typed stage codec, inbox, manifest and placement. | Synthetic splits and malformed-state refusal; later real binding and pre-mix fix landed. |
| [#12397](https://github.com/OpenAgentsInc/openagents/issues/12397) | Group lifecycle and generation fencing. | Synthetic fault coverage, then real checks in W11/W12; no whole-service create recovery guarantee. |
| [#12398](https://github.com/OpenAgentsInc/openagents/issues/12398) | Full one-machine stage group. | Real serving, pinned rerun and runtime checks; G5 numerical acceptance incomplete. |
| [#12399](https://github.com/OpenAgentsInc/openagents/issues/12399) | Authenticated direct links and two cross-machine placements. | Real fault/performance evidence; qualified numerical G6 and pinned machine-loss follow-up remain. |
| [#12400](https://github.com/OpenAgentsInc/openagents/issues/12400) | One routable group head and model-aware selection. | Fixture-backed stage exclusion/readiness; qualification status is not enforced by F5. |
| [#12401](https://github.com/OpenAgentsInc/openagents/issues/12401) | Extended deadlines, complete-answer refusal and measured profile. | Runtime/deadline evidence; old 3,022-token claim corrected to 1,526; requalification separate. |
| [#12402](https://github.com/OpenAgentsInc/openagents/issues/12402) | Guarded cloud up/down/status and beacons. | Real launches followed; lost-create recovery around the watcher has F1. |
| [#12403](https://github.com/OpenAgentsInc/openagents/issues/12403) | Historical preview launch, chat receipt and Grid source/captures. | Explicit preview rescope; live recovery and production labeling need F1/F9 follow-up. |
| [#12404](https://github.com/OpenAgentsInc/openagents/issues/12404) | Useful CPU arithmetic diagnostics, superseded as launch prerequisite. | Correctly closed without claiming G1 success; old failures preserved. |
| [#12405](https://github.com/OpenAgentsInc/openagents/issues/12405) | Versioned policy/harness, self-checks, G2 fixes, confirmation. | Substantial deliverable; 2.3 release fails. New F2/F3/F6/F7 need follow-up. |
| [#12407](https://github.com/OpenAgentsInc/openagents/issues/12407) | Architecture invalidation, build identity, readiness and arithmetic checks. | Three-build check and pinned SM120 run retained; newer bundle binding completed by #12410. |
| [#12408](https://github.com/OpenAgentsInc/openagents/issues/12408) | Checked request budget, bounded handoff and chat mode. | Exact 2,872 bound, queue and prompt-ID fixtures; chunking followed separately. |
| [#12409](https://github.com/OpenAgentsInc/openagents/issues/12409) | Unified cloud settlement and correction path. | Guard tests rerun; audit records resolve old queue estimate, invoice check remains owner-only. |
| [#12410](https://github.com/OpenAgentsInc/openagents/issues/12410) | Whole build-bundle pins through launch/qualification. | Pinned worker hash is present and equal in fresh confirmation report. |
| [#12411](https://github.com/OpenAgentsInc/openagents/issues/12411) | Instance-identity-safe settlement and replacement protection. | Reused-name cases covered by the current 41-test guard suite. |
| [#12412](https://github.com/OpenAgentsInc/openagents/issues/12412) | Incremental raw text decoding. | CPU parsing measurement and pinned 4K-output follow-up retained. |
| [#12414](https://github.com/OpenAgentsInc/openagents/issues/12414) | Serial bounded long prefill and separate step/context capacity. | Code/synthetic/SM89 group checks complete; original-weight 20K and long W14 still unqualified. |
| [#12418](https://github.com/OpenAgentsInc/openagents/issues/12418) | Per-stage mixed-device identity and mixed development serving. | CUDA architecture and mixed-group evidence; no per-device G1 qualification. |
| [#12420](https://github.com/OpenAgentsInc/openagents/issues/12420) | Metal stage backend and real CUDA-tail replay. | Strong tested-tail evidence; arbitrary Metal placement and G1 qualification deferred. |
| [#12421](https://github.com/OpenAgentsInc/openagents/issues/12421) | History/source-line fixes and explicit DeepSeek control. | Retained browser checks; issue body has unrelated build scope, so repair the record. |
| [#12424](https://github.com/OpenAgentsInc/openagents/issues/12424) | Full-score oracle capture and fresh confirmation run. | Valid pass-or-fail deliverable; failed release retained. Preview closure does not qualify G1. |
| [#12426](https://github.com/OpenAgentsInc/openagents/issues/12426) | Microcoder adapters and three DeepSeek scratch tasks. | Real tasks passed, not a broad agent benchmark; Flint rerun incomplete. |
| [#12434](https://github.com/OpenAgentsInc/openagents/issues/12434) | Approximate turn budget and bounded busy waiting. | Real short-chat checks retained; exact stage admission still supplies the hard token limit. |
| [#12463](https://github.com/OpenAgentsInc/openagents/issues/12463) | VRAM expert cache, host miss lane and measured one-box run. | Fixture exactness plus 19.8/24.3 and 14.3 tok/s measurements; 32K/360GB checks deferred. |
| [#12466](https://github.com/OpenAgentsInc/openagents/issues/12466) | SSD Engram reads and small staged row tables. | Fixture exactness and Hyperdisk timings; consumer NVMe check deferred. |
| [#12468](https://github.com/OpenAgentsInc/openagents/issues/12468) | Profile routing and CLI labeling. | Side-by-side fixtures; live check deferred; qualified naming/status gap is F5. |
| [#12470](https://github.com/OpenAgentsInc/openagents/issues/12470) | Architectural prompt-skip bound and falsification fixture. | 2,414-token structural bound; tiny 72-token test, not a real 16K logit run. CUDA use awaits validation. |

## Open work and next actions

| Open issue | Actual state at review | What should happen next |
| --- | --- | --- |
| [#12462](https://github.com/OpenAgentsInc/openagents/issues/12462) | Single-box optimization epic; separate from qualification. | Keep its experimental profiles separate; do not make it the implicit owner of G1 2.4. |
| [#12464](https://github.com/OpenAgentsInc/openagents/issues/12464) | 2K ten-layer prefill measured at 541 tok/s, 4.45×; single box about 83 tok/s warm. | Finish the missing 8K measurement and real-model logit comparison. Same answer text is weaker evidence than full logits. |
| [#12465](https://github.com/OpenAgentsInc/openagents/issues/12465) | q2q3 artifact exists and measures 26.4 tok/s, but its text is unusable. | Compare q3 and real-shape intermediates; finish the held-out quality card before advertising it for use. |
| [#12467](https://github.com/OpenAgentsInc/openagents/issues/12467) | Single-box profile identity and memory planning landed; serving integration still open. | Finish the declared served-profile check and original-weight evidence for the changed configuration. |
| [#12469](https://github.com/OpenAgentsInc/openagents/issues/12469) | Drafting issue has no completion evidence. | Establish DSpark weights/license and greedy verification. Maya's drafting speed is not a Psionic result. |
| [#12471](https://github.com/OpenAgentsInc/openagents/issues/12471) | Packed-KV code landed during this audit; issue still open. | Preserve the new flag/profile and complete its exact-readback, attention, and memory tests. No kernel verdict is given here. |

Recommended order:

1. **Restore or deliberately stop the preview cleanly.** Verify the new launch,
   reconcile both instance identities and guard records, and confirm there is
   no billed allocation outside the controller's accounting. Fix the lost-reply
   path and sub-cap retry calculation together. An updated live snapshot is
   recorded separately below.
2. **Fix the qualification command's authoritative verdict.** This is cheap
   local work and prevents a future automation from promoting a failed report.
3. **Close the remaining G2 checker gaps.** Rerun the local controls and rescore
   retained answers; no new GPU generation is needed to check those fixes.
4. **Create a specifically owned qualification follow-up.** It needs actual
   GPU score evidence or a justified proxy contract, the short Engram failure,
   matched short/long reference calibration, index coverage, a frozen successor
   policy, and fresh confirmation data. Keep 2.1/2.2/2.3 records unchanged.
   Do not increase budgets after reading the next confirmation set.
5. **Correct the protocol and status terminology.** Separate execution profile
   from qualification status, update the preview completion contract, document
   the service owner/stop command, and verify deployed preview disclosure.
6. **Continue independent Maya work.** Caching and memory improvements need not
   wait for a qualified launch, provided their evidence and profiles remain
   distinct. Qualify each changed arithmetic/prefill schedule before applying
   it to a qualified serving claim.

The earlier spend-guard defects are not evidence of an unpaid or charged
$136.53 invoice. The current unit suite passes, and the old request has
provider lifecycle evidence of no start; actual billing-console reconciliation
is still an owner-only check. The offline guard report is an estimate with
reservations, not a billing export. It is also the $3,000 project guard, not an
automatic enforcement of every issue's separate budget target.

## Reproduction and evidence

Run local controls without a cloud or model call:

```sh
python3 docs/audits/2026-10-22-deepseek-closed-issues/reproduce.py \
  --scratch "$(openagents scratch)/deepseek-audit-reproduce"
```

The script replaces all watcher effects before invoking its retry loop. It
compiles only the tiny Rust control shown in the audit and executes a tiny
Python control. The synthetic parser-offset report is clearly separate from
rescoring the real answers.

Useful retained files:

- [Review metadata and report identities](2026-10-22-deepseek-closed-issues/review-metadata.json).
- [All 58 issues and resolving commits](2026-10-22-deepseek-closed-issues/issue-evidence-inventory.json).
- [Per-case numerical/coverage summary](2026-10-22-deepseek-closed-issues/confirmation-summary.json).
- [Distribution measurements](2026-10-22-deepseek-closed-issues/distribution-summary.json).
- [Initial live observations](2026-10-22-deepseek-closed-issues/live-observations.json).
- [Later recovery observation](2026-10-22-deepseek-closed-issues/live-recovery-update.json).
- [Local reproduction script](2026-10-22-deepseek-closed-issues/reproduce.py).

The full confirmation report is retained at
`gs://openagents-psionic-oracle/deepseek-v41/1bae0e1/g1-v2/policy-2.3/confirmation/confirmation/report.json`,
generation `1792658190056916`; its hash is in the metadata. The audit preserves
compact summaries rather than duplicating 48.8 MB in the repository. Whole
issue/comment responses and downloaded reports remain in this chat's scratch
directory. No source behavior, GitHub issue, or cloud service was changed.

### Follow-up at 17:00 UTC

The corrected SM120 test package passes 121 checks, including complete index
observations and all four qualifier controls; its
[receipt](../psionic/verification/deepseek-v41-gates-2026-10-22/sm120-index-components.json)
retains the source and log. Original-input CUDA measurements now cover all
four stages on C4 and C8 (1,025 tokens). Independent CPU measurements and the
prospective policy remain in progress; these measurements are not G4 passes.

F5 now has a code fix: a numerical claim binds successful G1/G2/G4 evidence to
the running stages' actual configuration and limits, separately from the legacy
`qualified` execution label. Explicit qualified routing refuses unknown or
unqualified beacons. Local routing, cloud-launch, and website-consumer checks
pass. The live preview has not been promoted; final qualified deployment and
G1/G2/G4–G8 acceptance remain open under #12369.


The follow-up checker review also found omissions in G4 summary validation.
Policy derivation and publication now reread raw recorded tensors, enforce
complete stage boundaries, and bind all four stages to matching weights,
oracle hashes, and independently frozen corpus lengths. Missing or duplicated
sections and shortened continuations fail local controls. The rebuilt CUDA
package passes 144 checks (six optional checks ignored), with its source and
log in the [receipt](../psionic/verification/deepseek-v41-gates-2026-10-22/sm120-receipt-components.json).
Full-model gates remain open.


A subsequent independent review reproduced truncated connected G1 publication,
missing intermediate oracle replay logits, and missing post-final reference
tokens. The follow-up requires the frozen lengths, complete unique comparison
labels in both modes, all router rows per layer, valid metrics, and canonical
oracle row mappings and tensors. The prospective corpus requires 1,056
distribution comparisons; no acceptance limits were relaxed. These code fixes
do not close the pending full-model qualification.
