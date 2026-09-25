# Archive confirmation: executed checks and grader disagreement

The frozen detector catches **6 of 11 official failures**, versus **1 of 11**
for the existing scenario checks. It also flags three officially passing
circuit candidates. Its official-label failure precision is **6/9 (67%)**,
below the scenario baseline's **1/1 (100%)**. This does **not** meet
[#9584](https://github.com/OpenAgentsInc/openagents/issues/9584)'s preregistered
requirement to improve both point estimates. The issue remains open.

Those three disagreements are real specification defects. A separate,
post-label replay confirms that each Luna circuit passes the grader's 28 cases
but produces wrong answers on larger inputs in the public task's domain.
All three Astra circuits pass those additional checks. The original official
labels remain unchanged. This finding exposes a limitation of using a benchmark
grade as the sole truth label; it does not retroactively turn the primary study
into a win.

This is **archived Terminal-Bench component validation**, not TB4, a Fable
comparison, a same-model Coder ablation, or an admission of a new runtime policy.

## What was frozen

The [protocol](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/archive-confirmation-protocol.md)
declared 12 previously unused archived task groups, with three Luna and three
Astra attempts per task. The original v13 executor, policies, immutable public
images, review binary, literal citation prompt, Jev questions, and 0.8 cutoff
were fixed before generation. A deterministic file-check difference or a
supported reproduced defect means fail; otherwise the rule abstains. It never
certifies a pass.

All 72 predictions and their evidence digests were pushed in **`94458c65f3`**
before any official grade was opened. The
[seal](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/records/archive-sealed.json)
has SHA-256 `143aafd40d4e1d117e2f4ea9d29ef0180882a3944e452dc2eea0d2fbc7c38b6d`.
The join verified that published commit and every bound candidate, check, plan,
and baseline record before reading outcomes. All 72 restarted trials have
official grades; none required a regrade. The original 72 setup refusals remain
separate infrastructure attempts, with no agent, model call, or verifier.

The running checkout stayed at `1ac2d59f29`. Review access fixes, reporting work,
and verification ran in another checkout. In particular, the later `owner-exec`
profile was not used in the frozen detector.

## Official-label measurement

| Signal | Correct failure calls / calls | Failure precision, Wilson 95% | Detected / 11 failures | Recall, Wilson 95% |
| --- | ---: | --- | ---: | --- |
| Existing scenario checks | 1/1 | 100% (21–100%) | 1/11 | 9% (2–38%) |
| Original combined verdict | 1/9 | 11% (2–44%) | 1/11 | 9% (2–38%) |
| Public file checks alone | 0/0 | Undefined | 0/11 | 0% (0–26%) |
| Frozen executed combination | 6/9 | 67% (35–88%) | 6/11 | 55% (28–79%) |

The executed combination improves both point estimates over the **original
combined verdict**, but that is a secondary comparison. The declared primary
baseline is the scenario checks, whose sole failure call is correct. Changing
the primary baseline after opening grades would overstate the result.

All 72 reviews run: the cheap file arm establishes no failure, so it skips none.
The detector abstains on 63 candidates. Existing scenario checks abstain on 71.
All 11 officially failed candidates remain in recall, including the five misses.
The original combined verdict falsely rejects eight official passes and accepts
four official failures. Its historical calibration does not transfer reliably.

The [full measurement](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/records/archive-measurement.json)
includes 10,000 paired whole-task bootstrap resamples with seed 9584. Against
scenario checks, the executed detector's precision difference has a 95%
percentile interval of **−81.8 to 0 percentage points**; 3,451 resamples have
undefined precision because the baseline makes no call. Recall difference spans
**0 to +100 points**, with 73 undefined resamples containing no failures. Against
the original combined verdict, both difference intervals span **0 to +100
points**. Twelve clusters, one scenario-baseline failure call, and concentrated
defects leave substantial uncertainty. These intervals do not establish a
population-wide advantage.

## Candidate completion and within-task separation

| Archived task | Luna official passes | Astra official passes | Executed detector |
| --- | ---: | ---: | --- |
| `bn-fit-modify` | 3/3 | 3/3 | Abstains on all |
| `circuit-fibsqrt` | 3/3 | 3/3 | Flags all three Luna candidates; see the specification audit |
| `constraints-scheduling` | 3/3 | 3/3 | Abstains on all |
| `distribution-search` | 3/3 | 3/3 | Abstains on all |
| `financial-document-processor` | 0/3 | 2/3 | Catches one Luna failure; misses three failures |
| `model-extraction-relu-logits` | 1/3 | 3/3 | Catches both Luna failures |
| `multi-source-data-merger` | 3/3 | 3/3 | Abstains on all |
| `openssl-selfsigned-cert` | 3/3 | 3/3 | Abstains on all |
| `polyglot-c-py` | 3/3 | 3/3 | Abstains on all |
| `regex-log` | 3/3 | 3/3 | Abstains on all |
| `sparql-university` | 1/3 | 3/3 | Misses both Luna failures |
| `write-compressor` | 0/3 | 3/3 | Catches all three Luna failures |
| **Total** | **26/36** | **35/36** | **6/11 official failures detected** |

Four tasks have both official passes and failures. Ranking failures above passes,
with ties worth one half, gives **78.8% across 33 dependent within-task pairs**
for the executed detector, versus 53.0% for scenarios and 50.0% for the old
combined verdict. The mean over the four task-level rates is 78.1%. These pairs
are descriptive and not independent samples.

The pooled figure partly compares models. Within Luna alone, the two
mixed-outcome tasks supply only four pairs: 75% concordance, with perfect
separation on matrix extraction and ties on SPARQL. Within Astra, the document
task supplies two pairs, both ties. This is evidence of useful candidate-level
signal on some tasks, not a validated selector across all tasks.

## What the six detections did

- **Matrix extraction, two Luna candidates.** The reviewer loads the saved
  matrix and the fixed public network, permits row permutation and independent
  scaling, and compares directions. One candidate's worst relative residual is
  about 0.006319; another has a best possible row residual of about 0.012662.
  Fresh execution reproduces the saved results. The official matrix-equality
  test fails for both. A passing Luna candidate is not rejected.
- **Compression, three Luna candidates.** One 2,400-byte submission satisfies
  the size bound but the stated decompression pipeline segfaults and produces
  zero bytes. Another is 2,711 bytes against a 2,500-byte maximum and also
  segfaults. The last decompresses correctly but is 5,000 bytes. These are direct
  executions and byte counts tied to literal public requirements; no invented
  input or expected constant is needed.
- **Documents, one Luna candidate.** The reviewer decodes a retained PDF and
  establishes that a stock report was placed in the invoice directory and
  entered in the invoice summary. The official classification and summary
  assertions also fail. The record retains the exact PDF evidence and commands.

## Why five official failures were missed

Two Luna document candidates classify PDFs but move all eleven JPGs into
`other`, omitting invoices from the summary. The reviewer verifies some PDF
totals and CSV arithmetic, but cannot inspect the JPG contents without image or
OCR tooling. Some PDF command output is also truncated. It correctly records
incomplete coverage rather than declaring success, but detects no defect.

Both failing Luna SPARQL candidates are inspected only as text. The review image
has no available Python, graph-query engine, Java, or Node runtime. The reviewer
cannot execute the query against the supplied graph and does not establish the
actual result mismatch. More reasoning over the same source does not replace
the missing execution.

The sole failing Astra candidate is more revealing. It leaves all 17 source
documents in place and creates neither destination directory nor the required
CSV. The reviewer reproduces the omissions, but Jev scores the two findings
**0.79 and 0.72**, below the frozen 0.8 cutoff. The deterministic extractor
produces an empty plan despite explicit output paths in the instruction. A
missing mandatory file is a fact code should establish directly; the model score
should not veto a proved literal contract violation. Lowering the cutoff on
these now-opened outcomes would be a fit, not confirmation.

## Circuit audit: real defects behind official false alarms

The public task requires `fib(isqrt(N)) mod 2^32` from a circuit whose input has
32 bits. The
[pinned grader](https://github.com/harbor-framework/terminal-bench/blob/3b5caaa4863d64dda7f0957bf4fc2d4f019202d4/archive/circuit-fibsqrt/tests/test_outputs.py)
uses 28 inputs, with a maximum of **48,401**. Every submitted simulator source
matches the public and grader source byte-for-byte, SHA-256
`700645ac6156cdc88ec1e210f7c7d8d9969bb42aec969c410597e8efca796173`.

[#9645](https://github.com/OpenAgentsInc/openagents/issues/9645) independently
replays all six unchanged gate files with that simulator. Expected values come
from both an iterative recurrence and fast doubling, which must agree. The audit
uses the 28 original inputs plus nine explicit additional inputs, including zero,
square boundaries, one million, `2^24`, and `2^32−1`. It makes no model calls,
changes no candidate or official reward, and retains every command and output.

| Candidate suffix | Executor | Original-case mismatches | Additional-case mismatches |
| --- | --- | ---: | ---: |
| `7YduQi2` | Luna | 0 | 3 |
| `GsDeSsu` | Luna | 0 | 6 |
| `X9xhXrv` | Luna | 0 | 2 |
| `AsrhbfB` | Astra | 0 | 0 |
| `cbpJmSh` | Astra | 0 | 0 |
| `iXo3BvS` | Astra | 0 | 0 |

All three Luna circuits return zero for `N=16,777,216`; the independently
computed required result is **1,501,401,659**. These are reproducible public
specification violations, not invented test expectations. Passing the additional
cases does not prove the Astra circuits correct for every input.

This audit explains all three primary false alarms, but it is a **post-label
specification audit**. It neither replaces the original grade table nor licenses
selection of a new threshold. Even treating those counterexamples as additional
failures would only tie the scenario baseline's observed precision, not meet
the frozen requirement to improve both point estimates.

## Cost and time

| Measurement | Luna executor | Astra executor |
| --- | ---: | ---: |
| Recorded list-price cost lower bound, all 36 attempts | $0.355388 | $33.143967 |
| Calls with unknown final cost | 3 | 0 |
| Recorded cost lower bound per official pass | $0.013669 | $0.946970 |
| Median trial time | 219.4 s | 256.4 s |
| Median agent-execution time | 193.6 s | 226.9 s |
| Total trial time / official passes | 394.2 s | 329.9 s |

All failed attempts contribute to these costs and times. Luna's known cost per
official pass is about 69 times lower, but its completion rate is lower and its
total trial time per pass is about 20% higher. Three prices are unknown, and
three Luna circuit passes have the specification defects above. This cannot be
presented as a complete cost ratio, equivalent quality, a Fable win, or proof
that adding Coder makes an identical configuration cheaper.

The separate reviews cost **$12.467562** in 364 native responses plus
**$0.001619** in Jev list-price estimates. No retained review response has an
unknown price. Review process time totals 3,714.3 seconds across two workers,
with a 49.8-second median. That is about $2.08 per officially failed candidate
detected. Review cost greatly exceeds this cohort's Luna execution cost. These
reviews diagnose the signal; they did not control, repair, or select any attempt.
All prices are estimates, not subscription invoices. Shared-host work limits
interpretation of wall-time comparisons.

## Evidence and reproduction

The [experiment directory](../../bench/terminal-bench/experiments/2026-09-25-candidate-review/)
contains the frozen protocol, seal, verified join, task-cluster measurement,
cost ledgers, and reproduction commands. The published evidence includes:

- **Luna:** 1,400 files in 886 blobs, archive SHA-256
  `2f0473dcb0fa25163ec199eba953ed26ef0d6b051c524430b9b7bd6a4f896b67`.
- **Astra:** 1,381 files in 945 blobs, archive SHA-256
  `00c112429ff09b9e649cd430a4a83bb0a2e29a7817d63230f700040ccfa3c2ef`.
- **Cohort and checks:** 2,401 files including official outcomes, verifier logs,
  all review commands/replies, plans, task provenance, and measurement
  verification. Archive SHA-256
  `69ec26eb324af9d3155eba9da657d2c1f5fa742ab5a2e1090b0a4d9e9af65d9d`.
- **Separate circuit audit:** nine files, archive SHA-256
  `44207878ac2c7c23f3a6e540e83a88510af465743f873a395344898567360c8a`.

Every bundle was scanned for exact local credential values, restored, and
hash-verified. The analysis tools pass 17 focused regressions. The previously
retained scoped Gym gate passes, and all three real-report views (pooled, Luna,
and Astra) render successfully. Inspect the actual measurement without inference:

```sh
gym coder truth --confirmation \
  bench/terminal-bench/experiments/2026-09-25-candidate-review/records/archive-measurement.json
```

## Next work under #9584

Keep this cohort as development evidence for every later change. Retain executed
counterexamples even when a grader misses them, and keep specification audits
distinct from benchmark-label agreement. Expand the deterministic extractor's
literal output obligations so missing files, directory contents, size limits,
and stated CSV structure can be checked without a probabilistic veto. Validate
those extensions first on synthetic contrasts and these retained candidates.
Review runtimes need explicit, pinned tooling and coverage records before another
confirmation; an unavailable runtime must remain unknown. A later rule needs a
new reserved task cohort and a frozen comparison before inference. Do not promote
the current rule, change this study's baseline, or close #9584 on these results.
