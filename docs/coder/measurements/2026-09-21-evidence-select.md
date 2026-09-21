# Evidence selection on a held-out suite

On 2026-09-21, hosted Jev (`jev-1.13.0`) answered the evidence-relevance
question set over an eighteen-item held-out suite, each item sent through
the same `Select::request` a turn's decide step sends — task, observable
candidate records, omissions, and the set's digest — and read back with
`Select::ranking`. The gate picked the labeled candidate on seventeen of
eighteen items (recall@1 0.944), abstained correctly on the one
no-relevant-candidate item, and produced zero confident errors: the one
miss carried a top probability of 0.58, uncertainty rather than
conviction. This is the sixth acceptance item of openagents#9513.

## What was asked

| Set | Digest |
| --- | --- |
| `openagents.evidence-relevance.v1` | `f827ab947d875a714e83cc66a04e5fa51951bfdb30605d73503907f369b1beb0` |

The suite is
[`crates/gym/suites/evidence-select-v1.json`](../../../crates/gym/suites/evidence-select-v1.json),
digest `2f72d3e41cf851b87c49448a89b6efc8cfab4342d530fed1447a1b66c0eabebc`:
eighteen author-labeled task-plus-candidate records the question set was
not authored against — clear picks among distractors, one
no-relevant-candidate abstention, refused path-only candidates, a
truncated read, a duplicated path under two spans, a search hit, a read
with an oversize omission, and two coverage-partials. The door sees only
what `Select::request` renders: path, span, readness, size, and the
withheld reason — never content.

The driver is
[`crates/coder/examples/select_eval.rs`](../../../crates/coder/examples/select_eval.rs);
it builds `Candidates` from the suite with serde, calls `Select::request`
and `Select::ranking` — the production path, not a re-rendered copy — and
records the raw exchange at
[`2026-09-21-evidence-select.raw.jsonl`](2026-09-21-evidence-select.raw.jsonl).
Reproduce with:

```text
TYPESAFE_API_KEY=... cargo run -p coder --example select_eval -- \
    crates/gym/suites/evidence-select-v1.json out.jsonl
```

## Results

| Measure | Value |
| --- | --- |
| Recall@1 (gate picks labeled candidate) | 17/18 = 0.944 |
| Abstention | 1/1 correct, no false abstentions |
| Confident errors (top probability ≥ 0.8 and wrong) | 0 |
| `any_relevant` agreement (≥ 0.5 with label) | 18/18 |
| `coverage` agreement (≥ 0.5 with label) | 14/18 |
| Latency | mean 186 ms, p50 ~175 ms, max 401 ms (first call) |
| Tokens per call | ~860 input, ~95 output |
| Refusals, transport errors | 0 |
| Cost | unmetered — recorded as unknown |

## The miss and the coverage pattern

`es-10` asked for the function computing a file's content digest. The
suite offered a refused search-hit candidate (`src/hash.rs`, one line
named, no content) and a full read of `docs/hashing.md`; the model picked
the document at 0.58 over the search hit at 0.09. The label ranks the
source file first — a search hit on line 88 is the better lead — but the
pick is the uncertain side of a real tension the set's wording creates:
never assume content a candidate does not carry, yet a name with a line
number is still evidence.

Every `coverage` disagreement shares one shape: the model doubts
coverage exactly when the relevant evidence is refused or missing —
0.36 for the refused token file, 0.44 for two refused linter scripts,
0.40 for the search-hit-only set, and 0.58 for the rate-limit task whose
test file was absent. That is calibrated caution, not confident
wrongness: coverage probabilities sit in the 0.36–0.62 band in both
directions, so the noul is informative but not decisive near the label
boundary. `any_relevant`, by contrast, agreed on all eighteen items.

## Limits

Eighteen items is a smoke measurement, not a boundary map — it establishes
that the gate's ranking is reliable on clearly-shaped sets and honest
about uncertainty on marginal ones, and it pins the suite, digests, and
raw exchange so a larger held-out run has a baseline. Labels are the
suite author's. The suite exercises the observable-record path only; it
does not measure downstream task quality from disclosed content, which
remains the one open measure under the issue's last acceptance item.
