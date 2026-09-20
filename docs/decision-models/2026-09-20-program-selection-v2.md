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
