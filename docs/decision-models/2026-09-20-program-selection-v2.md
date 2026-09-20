# Current program-selection baseline

This suite freezes the production four-option question before scoring:
`none`, `answer-question`, `burn-down`, and `delegate-fan-out`.
`program-selection-v2` preserves the question text already checked against
Coder's runtime. Only its suite pointer and explanatory metadata change.
Historical v1 questions, labels, fixtures, and recorded rows stay intact.

## Inputs and label policy

The new suite contains 80 items: 32 historical open real turns, 12
historical authored development cases, and 36 new authored cases.
Calibration has 28 items, development 40, and the new locked partition 12.
The eight historical locked items are excluded. Each newly authored
partition includes requests for every offered program and ordinary or
negated requests for `none`.

Labels require a request to execute the program's work. `burn-down` means
the checkout's persisted work list with one delegated session per entry;
it does not mean every request to finish issues. `answer-question` requires
explicit delegation of one repository question. Ordinary repository
questions remain `none`, even though the offered summary overlaps them.
The experiment leaves that summary unchanged.

The author reviewed the reused open labels against all four current
options before preserving them. Every label is an author's reading, not
an observed execution outcome. The judgments file records the known
overlap and single-delegation disputes. No independent second annotation
has been performed. New authored cases establish recognition coverage;
they do not estimate how often real users request each program.

Before any model calls, constant `none` is correct on 31 of 32 real turns
and 10 of 36 authored open cases. The new question adds coverage, but it
does not remove the real-turn class imbalance or create room for a large
accuracy gain there.

## Declared measurement policy

Ask hosted Jev and the pinned historical 4B on the same 68 open items,
with no question, label, threshold, or calibration tuning after seeing
responses. Record content and execution identity for Kev and the reported
model name for the hosted reference; a closed model has no independently
verifiable weight identity.

Report spurious selections, missed program requests, and wrong-program
selections separately for real and authored items. Keep refusals and
missing harness results in their original denominators. The probability
gate named by the suite is recorded for reproducibility; its historical
noise floors are not evidence of significance for this program suite.

The 12 new locked cases remain unused. Before a locked read, name the
immutable candidate and record an acceptance rule. The planned 4B update
must first show no additional real-turn spurious selections versus both
open-item references, no additional missed or wrong-program authored
requests, and complete response coverage. With only 32 real turns and
one positive, even passing those checks would support further evaluation,
not automatic production admission. Use the repository's locked-read
ledger for a declared confirmation run; do not inspect it through an eval
flag or repeatedly spend it while tuning.

## Reproduction

```sh
python3 crates/gym/suites/build_program_selection_v2.py > /tmp/program-selection-v2.json
cmp /tmp/program-selection-v2.json crates/gym/suites/program-selection-v2.json
cargo test --locked -p coder --test suite_questions
cargo test --locked -p gym --test program_selection_v2
```

The Coder tests check the exact production wording and admissible catalog.
The Gym test checks labels against those options, preserves every open
historical item, and rejects reuse of a historical state as a new locked
item. The suite, question, and measured model identities will be recorded
with the scored results.

## Historical 4B result

The suite was frozen in commit `2a0c8e226f` before these calls. Its digest
is `378b57c423ebef319e94bfc47e43ab6e8460740230d60d078c9e4e573d596026`;
the unchanged production question digest is
`7a8485ca6904e9d8658d969da6f6ad3d663e19bd9c7516eb2db6256fef97b9ba`.

The historical adapter is `1a0cb0a0c4ea77e259cd215a3fb85d29edcc499e`,
with runtime content digest
`sha256:2559d7a66cd0f4077d7f47b5460eedaf8211a43b917588638cc2da1f459628f9`.
The door reported `kev-4b-historical-1a0cb0a`, Metal bf16, fp32 head, eager
attention, and `fp32-before-cast-v1`. The 128 GiB M5 Max host used a 4,096
packed-token limit and 4,096 MiB forward-memory budget, admitting one
forward for this variant. Calls were serial. All 68 answered, with no
refusals or harness failures.

| Open set | Correct | Spurious selections | Missed requests | Wrong program | Confident errors at p ≥ 0.9 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Real turns | 30/32 | 1/31 negatives | 1/1 positive | 0 | 0 |
| Authored | 23/36 | 3/10 negatives | 8/26 positives | 2 | 0 |

The real-turn constant `none` is correct on 31/32; the model missed the
only real request for fan-out and spuriously selected `answer-question`
for the request to list open issue status. Authored recognition remains
limited despite no confidently wrong answers. These results do not admit
the historical model for program selection.

Constant-answer correct counts on the same open inputs:

| Constant | Real turns | Authored |
| --- | ---: | ---: |
| `none` | 31/32 | 10/36 |
| `answer-question` | 0/32 | 9/36 |
| `burn-down` | 0/32 | 6/36 |
| `delegate-fan-out` | 1/32 | 11/36 |

Calibration raw accuracy/ECE/Brier/NLL are 0.82/0.112/0.075/0.231;
development values are 0.75/0.104/0.129/0.375. Brier measures the selected
answer's probability against its correctness, as Gym defines it. No map
was fitted. The median observed HTTP latency was 194 ms during this run;
it is not the separately required quiet-host serving benchmark.

[All raw rows](../../crates/gym/results/program-selection-v2-historical-4b.jsonl)
retain the complete discovery identity and a verified receipt chain.
[Derived reports](data/program-selection-v2/) retain every error and the
full denominator. Reproduce them with:

```sh
gym eval --suite crates/gym/suites/program-selection-v2.json \
  --door kev-4b-historical-1a0cb0a=http://127.0.0.1:18454 --timeout 300 \
  --record program-selection-v2-historical-4b.jsonl
gym compare --suite crates/gym/suites/program-selection-v2.json \
  --store program-selection-v2-historical-4b.jsonl
python3 crates/gym/suites/score_program_selection_v2.py \
  program-selection-v2-historical-4b.jsonl
```

## Hosted Jev comparison

Hosted `jev-latest` answered the identical 68 open items: 61 correct,
no refusals, and no harness failures. The service supplies no independently
verifiable weight identity. These calls used the existing TypeSafe
credential supplied by the operator; no credential is retained in results.

| Open set | Correct | Spurious selections | Missed requests | Wrong program | Confident errors at p ≥ 0.9 |
| --- | ---: | ---: | ---: | ---: | ---: |
| Real turns | 30/32 | 2/31 negatives | 0/1 positive | 0 | 0 |
| Authored | 31/36 | 3/10 negatives | 0/26 positives | 2 | 0 |

On paired items, Jev alone is correct on nine cases and Kev alone on one.
The real turns contribute one in each direction; the authored cases
contribute eight Jev-only corrections and no Kev-only corrections.

Jev recognizes more authored requests but also selects `answer-question`
for ordinary repository questions. Both models fall below constant `none`
on the real turns. Neither result establishes safe automatic program
execution. The offered summary and intended delegation requirement overlap;
this is a question-contract concern as well as a model-quality concern.

Calibration raw accuracy/ECE/Brier/NLL are 0.86/0.095/0.083/0.256;
development values are 0.93/0.107/0.076/0.245. No calibration map was fitted.
The observed HTTP median was 199 ms, not a controlled latency benchmark.
[Hosted raw rows](../../crates/gym/results/program-selection-v2-jev.jsonl)
and [derived reports](data/program-selection-v2/) preserve the complete
open denominator and verify against the frozen suite and question digests.
The 12 locked cases remain unused. Repeat the commands above with `--jev`
and the hosted result path to reproduce the comparison workflow.

The production drift tests and new suite contract test passed. Rebuilding
the suite reproduces its committed bytes; the result store's receipt chain
verifies, and every result carries the frozen question digest. The manual
gate reached the runtime-feature workspace tests and stopped at the
existing `oversized_http_body_is_a_typed_refusal` connection-reset failure.
Later stages did not run in this attempt; this is not a full gate pass.
