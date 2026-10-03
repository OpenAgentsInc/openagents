# Preparation and source-probe results

Jev ranking improved one specific source-coverage measure: beta's 16 KiB pack
contains the complete ATIF `read` function, which the deterministic pack omitted.
Alpha traded a partial implementation for more enrollment tests and fixtures.
Gamma still missed every implementation boundary named in the pre-call diagnostic.
These are source-selection observations on three development tasks, with no native
executor or patch-acceptance comparison.

[Structured results](preparation-results.json) retain candidate IDs, source commits,
blob and span hashes, selected paths, omitted candidates, exact questions, model
identity, costs, and timings. The named-unit diagnostic is fixed to the
[pre-call discovery record](discovery-baseline.json); this review adds no hidden
checker or historical-fix criteria.

## What ran

Each task received one preparation request with a Score for each candidate, a
Choice for each exact task sentence, and a Noul asking whether essential
implementation evidence was missing. Code split sentences; Jev did not generate
a decomposition. The sentence Choices identify a best starting point, not complete
requirement coverage. Independent questions shared state but could not condition
on each other's answers.

Code packed the candidates by their returned Scores. Both ranking policies used
the same candidate IDs, source bytes, and 16,384-byte limit. A second request chose
one additional file from the omitted-source catalog using the task and Jev pack.
Every task received this request; the Noul values did not trigger a threshold.
No threshold was calibrated.

All six calls returned HTTP 200 with valid typed answers, one attempt each. The
requested and returned model was `typesafe-ai/jev` through Vercel AI Gateway;
the reported final provider was `typesafe-ai`. This is an **unversioned gateway
alias**, not evidence for the direct, revision-pinned Jev configuration in the
original delegation study. Costs below are gateway-reported, without invoice
reconciliation.

## Ranking changes within the same budget

| Task | Pool units / omitted units | Deterministic bytes | Jev bytes | Observed change |
| --- | ---: | ---: | ---: | --- |
| Alpha | 14 / 393 | 16,371 | 16,017 | Keeps complete `Host::request_enrollment`; drops partial `Host::execute`; adds the reverse-enrollment CLI test and `Fixture::host`. |
| Beta | 11 / 259 | 14,702 | 15,627 | Adds complete ATIF `read`; `Log::append` and `Log::finish` remain absent from the pool. |
| Gamma | 13 / 397 | 16,238 | 16,302 | Adds `Questions::validate`, `Error`, and `RetryPolicy::validate`; none is the missing response-decoding or typed-client boundary. |

All omitted units in this table exceeded the candidate-state budget. The pools
contained four, one, and two oversized units respectively, retained as labeled
partial spans. There were no file-admission omissions on these tasks. Ranking
cannot select a declaration absent from its pool.

The fixed named-unit measure therefore records one complete-unit gain in beta,
one partial-unit loss in alpha, and no change in gamma. It is not an exhaustive
relevance or quality score. Alpha's added enrollment test is relevant; gamma's
request-input validation is different from response validation, and its retry
validation concerns the already-completed A22 work outside the requested A12 fix.

The sentence maps also need care. Alpha maps the delegation-authority sentence to
`c04`, the partial `Host::execute` that its own ranking omits. Gamma maps the
numeric-validation sentence to a live-test example and the request-aware sentence
to the `Error` type; it maps two other sentences to an omitted blocking test.
These associations do not establish that the delivered pack covers the clauses.
The maps are recorded separately and were not inserted into the packed text.
Reading a live-test example did not execute a provider call.

## Source probes and the repaired window diagnostic

The missing-implementation Nouls were 0.86, 0.87, and 0.89. All three pools had
known gaps, so these three positive cases provide no false-positive estimate or
probability calibration. A high gap probability also did not ensure a useful next
selection: gamma chose the TypeSafe skill with probability 0.71 and Choice
confidence 0.68 instead of an SDK implementation file. The skill is required
background, but the returned introduction does not recover the missing response
validation or typed-client evidence.

The original runner ignored the catalog's `suggested_start_line` and read line 1
of every selected file. Those observations remain unchanged. A later deterministic
diagnostic reused the **same three model selections** with the recorded start
lines. It made no model calls and used the original source reader.

| Task | Original lines / bytes | Corrected lines / bytes | Corrected read time | Actual added evidence |
| --- | --- | --- | ---: | --- |
| Alpha | `host/mod.rs:1–109` / 4,075 | `564–685` / 4,070 | 0.146 s | Complete `Host::book`, `Step`, `refused`, and `principal`; the start of `devices`. Includes persisted bounds and grant admission checks. |
| Beta | `coderbench/src/lib.rs:1–102` / 4,077 | `869–965` / 4,082 | 0.109 s | A path-label test, `outcome_word`, complete `Task::load` and `Task::judge`, and the start of `judge_program`. Includes manifest validation and fault aggregation. |
| Gamma | `SKILL.md:1–63` / 4,064 | `1–63` / 4,064 | 0.111 s | Exactly the same introductory guidance; no SDK implementation. |

None of the original or corrected reads intersects any of that task's fixed named
units. Alpha and beta gain relevant neighboring implementation, without closing
the measured gaps. The catalog chooses the first omitted declaration in a file
before considering a partial admitted declaration; honoring its suggestion does
not guarantee the desired behavior lies in that window.

The [alpha](probe-window-followup/alternative-alpha.json),
[beta](probe-window-followup/alternative-beta.json), and
[gamma](probe-window-followup/alternative-gamma.json) follow-ups retain the complete
returned text, immutable source identity, line ranges, hashes, and timings. Both
original and corrected source windows were independently matched to Git bytes.
The corrected reads total 0.366 seconds, outside the original run timers. This is
a runner-repair diagnostic, not a new model-selection result.

The extra read has its own 4,096-byte bound and is retained separately from the
16 KiB pack. There is no matched deterministic-probe arm or native handoff here,
so the combined evidence is not a same-total-budget executor comparison.

## Cost and latency

| Task | Deterministic preparation | Jev preparation call | Jev probe-choice call | Original total | Gateway cost |
| --- | ---: | ---: | ---: | ---: | ---: |
| Alpha | 2.579 s | 0.518 s | 0.441 s | 3.751 s | $0.000948906 |
| Beta | 1.497 s | 0.662 s | 0.479 s | 2.801 s | $0.000884436 |
| Gamma | 2.281 s | 0.676 s | 0.432 s | 3.572 s | $0.001062222 |
| Total | 6.357 s | 1.856 s | 1.353 s | 10.124 s | $0.002895564 |

The six calls asked 73 typed questions and reported 68,942 input tokens and 5,296
output tokens. Their total wall time was 3.209 seconds. Per-call costs, timings,
request sizes, and question types are retained in the structured record.

Deterministic preparation includes manifest/index loading, assembly, packing, and
initial artifact writes. Original total time also includes both calls, the original
source read, and intermediate output writes; it ends before the final result file
is written. These are one local observation per task, not a controlled cold-cache
benchmark. They do not measure executor startup, engineering effort, or net savings.

## Deterministic latency hypothesis

Static inspection of [the source assembler](../../../../bench/jev-lifecycle/context.py)
identifies repeated process startup: each Cargo manifest and source file uses one
`git cat-file -s` and one `git cat-file blob` subprocess. The three tasks read 18,
13, and 25 admitted source files, plus package manifests. Each targeted probe also
reconstructs the pinned tree. Candidate admission repeatedly serializes growing
state, and assembly hashes the complete syntax index. These are hypotheses about
where time goes; this record does not profile their individual contributions.

A small follow-up can batch immutable blob reads while preserving the existing
path-to-blob, size, UTF-8, index, and hash checks. Its acceptance criterion should
be byte-identical candidates, order, catalog, omissions, and both packed outputs
on these three inputs, plus warm timing observations on the same host. It must
preserve the original implementation and observations and leave selection rules
and budgets unchanged. Any measured speed gain would be a deterministic
implementation improvement, not a Jev benefit.

## Limits and next decision

Keep the beta ranking gain as evidence that semantic ranking can help after
candidate admission. Keep alpha's tradeoff and gamma's failure visible. The next
retrieval experiment should distinguish candidate discovery, span choice, and
ranking: a better ranker cannot restore missing declarations, and a correct file
choice can still return the wrong portion of the file.

These three tasks were already development evidence, with one response per phase.
No native executor ran, no accepted-code rate was measured, and no engineering
return on investment is established. The retained question definitions and local
TypeSafe skill govern this interpretation; live primitive-document fetches were
unavailable during this review.
