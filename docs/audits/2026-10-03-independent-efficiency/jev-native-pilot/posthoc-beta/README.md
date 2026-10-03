# Post hoc trace-integrity diagnostic

The original pilot scores remain **2/4 for each arm**. This later diagnostic found that four of the six beta patches still let a specific damaged trace receive a successful CoderBench grade. Both original successes in arm C fail one added case. The historical reference passes all three cases; the unchanged base fails the step-before-session case.

## Results

| Arm | Original panel passes | Original beta passes | Beta diagnostic passes | Original checks plus applicable diagnostic |
| --- | ---: | ---: | ---: | ---: |
| A: bare Sonnet | 2/4 | 2/2 | 1/2 | 1/4 |
| B: lean deterministic spans | 2/4 | 2/2 | 1/2 | 1/4 |
| C: lean Jev-selected spans | 2/4 | 2/2 | 0/2 | 0/4 |

The final column is a retrospective conjunction, **not a replacement primary score**. The extra diagnostic applies only to beta (trace integrity); gamma retains its original checks. The [derived summary](summary.json) binds this table to the [original report](../report.json) and [diagnostic receipt](diagnostic.json).

| Revision | Diagnostic cases passed | Failed case | Recorded check time (s) | Output |
| --- | ---: | --- | ---: | --- |
| Historical reference | 3/3 | None | 56.800 | [Log](logs/historical_reference.log) |
| Unmodified base | 2/3 | Step before session | 55.393 | [Log](logs/unmodified_base.log) |
| A beta 1 | 2/3 | Form-feed prefix | 56.185 | [Log](logs/A-beta-1.log) |
| B beta 1 | 3/3 | None | 60.328 | [Log](logs/B-beta-1.log) |
| C beta 1 | 2/3 | Step before session | 61.061 | [Log](logs/C-beta-1.log) |
| B beta 2 | 2/3 | Form-feed prefix | 57.793 | [Log](logs/B-beta-2.log) |
| C beta 2 | 2/3 | Form-feed prefix | 58.048 | [Log](logs/C-beta-2.log) |
| A beta 2 | 3/3 | None | 57.411 | [Log](logs/A-beta-2.log) |

The failed test names are `step_before_session_is_not_successful_evidence` and `form_feed_before_an_interior_record_is_not_successful_evidence`. In every failure, the consumer returned `Passed` for the damaged trace. `clean_trace_remains_successful_evidence` passes on all eight revisions. Scope, formatting, ordinary tests, and diagnostic compilation also pass on all eight; these are observed behavioral failures, not compile or setup failures.

## Method and limits

The diagnostic was written after static review of anonymized candidate packets. Those packets omitted arm labels and execution outcomes; the coordinator had the results. This was **post hoc**, not a prospective or fully blinded experiment. Review notes remain static findings; these executions provide separate evidence for the two named beta cases.

The [unchanged diagnostic](posthoc_trace_integrity.rs) uses public ATIF and CoderBench APIs. It checks a clean trace, moves a valid step before the session header, and prefixes a valid interior JSON record with form feed (`0x0C`). A later valid step and end remain in the latter case. Each damaged case first confirms its clean control; the consumer may reject the trace or return any non-`Passed` verdict. The cases follow the public requirements for session ordering and malformed interior records, without requiring a particular implementation.

The [runner](run_posthoc.py) waits for the completed native panel, verifies candidate/source identities, and reuses the isolated acceptance harness, frozen source/seed/toolchain settings, and 240-second per-revision deadline. It runs the historical reference, unchanged base, and all six beta candidates once, without repairs or retries. Only the independent checker changes. The existing harness also repeats formatting and ordinary tests. The reference positive control passes 3/3; the base negative control passes 2/3 and fails the expected ordering case.

Recorded driver wall time was **465.980803 seconds**, covering control preparation, eight check sequences, retention, and cleanup. Its clock starts after the initial panel, module, and candidate validation and stops before the final receipt write. The per-revision times above exclude the separately recorded cleanup. This is separate from the original pilot cost/time comparison. There were **0 model calls**. All eight executions closed, and all eight cleanup receipts confirm removal of their scratch workspace and copied target; the shared Cargo target was retained. Passing three cases does not establish general correctness, and these exposed, retrospectively selected cases do not establish a model ranking.

## Retained evidence

- [Evidence archive](evidence.tgz): 196 files, 127,747 bytes; SHA-256 `298620ee3c0bf110a5316f4beac76f90e0cd5af2ae3f1744f6f43e35f6ad4eac`.
- [Retained-file manifest](retained-manifest.json): each archive member's length and SHA-256, reverified before this copy.
- [Complete diagnostic receipt](diagnostic.json): source, plan, candidate, check, timing, and cleanup bindings.
- [Checker](posthoc_trace_integrity.rs): SHA-256 `d91083ae434b72265e688b0a2061d626efb6f701337503fa85e79b294e83f062`.
- [Runner](run_posthoc.py): SHA-256 `8e45be9045d67844ee909d27e5ea48f7b599c82b39c165eee0e7a2f47957e958`.

The archive, checker, runner, and receipt are copied byte for byte. They retain sandbox paths and run identifiers. The checker's “not compiled or run” comment and the archived draft README describe the pre-execution state; the completed receipts now supersede that status. Their original bytes remain intact for provenance. The eight linked test logs are also unchanged copies.
