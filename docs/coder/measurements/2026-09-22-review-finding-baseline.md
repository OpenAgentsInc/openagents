# Wording baseline for the per-finding review question

On 2026-09-22, hosted Jev (`jev-1.13.0`) answered
`openagents.review-finding.v1` over a twelve-item labeled suite of
synthetic diffs and reviewer findings. Per-finding agreement with the
author's truth labels is **18 of 20** (0.90), with zero confident errors
and zero unanswered findings. This is the wording baseline
openagents#9503's third acceptance item requires before a review
question reaches production: it says what the shipped wording does on
labeled findings, and where its remaining errors sit.

## What was asked

| Artifact | Digest |
| --- | --- |
| `openagents.review-finding.v1` | `567027733b5a40e7d2ab52c8ba5785f51eb0fb82d0b8ed8041c7d3553cd2b50c` |
| `review-finding-v1` suite | `f9939f55486e5b2ab64108252e88959a562fe967c61bfa00a577d8d5df340ba8` |

The suite is
[`crates/gym/suites/review-finding-v1.json`](../../../crates/gym/suites/review-finding-v1.json):
twelve synthetic Rust diffs carrying twenty findings — genuine defects
the diff contains (unwrap panics, SQL injection, executor-blocking
sleep, lock-ordering reads) beside anchored false positives (misreads of
the diff, invented preconditions, nitpicks, correct behavior complained
about, and one finding that names pre-existing context lines as the
change's own). Each item ran once through
[`finding_eval`](../../../crates/coder/examples/finding_eval.rs), which
builds exactly the state a `per_finding` decide step sends — revision
pins, captured scope, diff sections, findings under `f`-style names —
and asks with `Fill::Findings`. The raw exchange is retained at
[`2026-09-22-review-finding.raw.jsonl`](2026-09-22-review-finding.raw.jsonl).

## Results

| Kind | Findings | Agreed |
| --- | --- | --- |
| Genuine (truth yes) | 8 | 7 |
| False positive (truth no) | 12 | 11 |
| **Total** | **20** | **18** |

The wording separates the two classes: genuine defects came back at
0.74–0.94, false positives at 0.04–0.43, and the two misses are the
only readings between 0.44 and 0.73 — the band where an honest
disagreement belongs.

## The two misses

- `rf-06 f1` (0.51): the finding claims `balance += entry.amount` can
  underflow an unsigned balance. The diff shows neither the balance's
  type nor the amount's sign — the claim invents a precondition, so the
  label is false, and the model's 0.51 is maximal uncertainty on an
  unverifiable claim rather than a wrong answer. The suite keeps it:
  this is the case the question is honest about not resolving.
- `rf-10 f1` (0.25): the finding demands a timing-safe comparison for
  an API-key check done with `==`. The label is true — the leak is real
  — but the claim sits in a class reasonable reviewers weigh
  differently, and the model dismissed it. This is the baseline's one
  substantive wording gap: the question does not tell the judge how to
  weigh severity, only genuineness, and a severity-contested genuine
  problem reads as dismissible.

## Suite correction during the run

The first pass carried `rf-08 f1` as truth yes — a `read_dir().unwrap()`
panic the finding attributes to the change. On audit the unwrap sits in
diff **context lines**: the change adds the depth bound, the panic is
the base's. That is exactly the misattribution the question's
"this change's rather than the base's" clause exists to catch, and the
model caught it at 0.09. The label was corrected to false and the suite
digest re-pinned — the item now stands as a labeled context-lines false
positive the wording already defeats.

## Headroom

A 0.90 baseline with both misses inside the ambiguous band is a wording
that works. Headroom is in the severity dimension — the question asks
whether a problem is genuine, not whether it matters — and in the
invented-precondition class, where the model abstains toward 0.5 instead
of dismissing. Neither miss argues for rewording: tightening the
severity clause would ask the question to carry a policy judgment the
disposition thresholds already own, and the 0.51 reading is the honest
answer to an unverifiable claim. The baseline records both so a later
wording revision has something to beat.
