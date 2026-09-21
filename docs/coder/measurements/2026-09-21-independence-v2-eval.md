# Independence v1 against v2 on twelve task lists

On 2026-09-21, hosted Jev answered both independence question sets over a
twelve-item suite of task lists drawn in the shape the fan-out's `decide`
step sends. `openagents.independence.v2` agreed with the labels on eleven
of twelve items against `openagents.independence.v1`'s ten, with no
refusals from either set, so the last v1 binding in
`programs/delegate-fan-out.json` moved to v2. This is the third
acceptance item of openagents#9508 and completes the issue.

## What was asked

Two question sets, each asked over the same twelve states:

| Set | Digest |
| --- | --- |
| `openagents.independence.v1` | `a3d47ae0da35ec1553bc906bc8c702dc7565c8cc28ba52867ce2f5a5f41d4587` |
| `openagents.independence.v2` | `4939e6d3c3f93ba0f8a615cf07e707371962e4c4859259de1346d6b1374a68ee` |

The digests are `Set::digest` over each set's `questions`, the same value
a run records in its provenance. v1's gate wording says "the six tasks";
v2 says "the listed tasks" and adds that no task may depend on another
finishing first.

The suite is
[`crates/gym/suites/independence-eval-v1.json`](../../../crates/gym/suites/independence-eval-v1.json),
digest `7bdf37d9e5b06cb67d25e6f6306340cc5cdc7c450f3e5f5843e5ac21d757ecd6`:
twelve author-labeled task lists at zero, one, two, six, and twelve tasks,
drawn from the colliding, disjoint, ambiguous, and mixed fixture shapes of
`crates/coder-project/src/semantics.rs`. Each state carries `plan` and
`tasks` the way `plan_state` builds them, and `collisions` where a real
lookup would have found them. Six and twelve are the counts the binding
cares about: six is the number v1's wording names, and twelve is the
`max_results` bound the program declares.

The door was hosted Jev (`jev-latest`) reached through
`TYPESAFE_API_KEY`; identity is not verifiable for a hosted door. Each of
the twelve items was asked once per set with the set's full three
questions, the way the `decide` step sends them; the raw exchange is kept
in
[`2026-09-21-independence-v2-eval.raw.jsonl`](2026-09-21-independence-v2-eval.raw.jsonl).
Separately, `gym eval` served each set's gate question alone over the
eleven open items and recorded twenty-two rows in
[`crates/gym/results/independence-eval-v1.jsonl`](../../../crates/gym/results/independence-eval-v1.jsonl)
under question digests `97fe8ea067ab2e93c68cc3dc2e2bbe3c1774186a0ccee8101d07a344ea610586`
(v1) and `2060a041a72097983110082fe44eade10bfb39dd491c1161346573b8a176134a`
(v2), judged by `decision-v1`. The locked item was asked once per set in
the direct run only; the harness never opens the locked partition.

## Per-case answers

The `independent` noul from each set, with agreement at the 0.5 mark the
row's `correct` reads. All twenty-four calls answered; there were no
refusals or abstentions from either set.

| Item | Partition | Label | v1 | v2 |
| --- | --- | --- | --- | --- |
| `n0-empty` | development | no | 0.27 ✓ | 0.83 ✗ |
| `n1-single` | development | yes | 0.13 ✗ | 0.96 ✓ |
| `n2-disjoint-read` | calibration | yes | 0.17 ✗ | 0.97 ✓ |
| `n2-shared-write` | calibration | no | 0.02 ✓ | 0.02 ✓ |
| `n2-ordered` | development | no | 0.06 ✓ | 0.04 ✓ |
| `n6-disjoint-read` | calibration | yes | 0.96 ✓ | 0.97 ✓ |
| `n6-shared-append` | development | no | 0.02 ✓ | 0.01 ✓ |
| `n6-mixed` | calibration | no | 0.08 ✓ | 0.06 ✓ |
| `n6-undeclared` | development | no | 0.32 ✓ | 0.32 ✓ |
| `n6-disjoint-write` | calibration | yes | 0.94 ✓ | 0.96 ✓ |
| `n12-disjoint-read` | calibration | yes | 0.74 ✓ | 0.98 ✓ |
| `n12-one-collision` | locked | no | 0.33 ✓ | 0.04 ✓ |

Aggregate agreement: v1 **10 of 12**, v2 **11 of 12**. On the eleven open
items the recorded run scored, the same split reads 9 of 11 against
10 of 11.

v1's two misses are the failure its wording predicts: a one-task list and
a two-task disjoint list answered as not independent because the count is
not six (0.13 and 0.17). Both are reachable states, and at the program's
`refuse_below` floor of 0.7 both are wrong refusals of a fine plan.
v1's correct "no" on `n0-empty` is the same bug read as a feature — it
says no because the count is not six, not because the list is empty.

v2's only miss is `n0-empty` at 0.83: vacuous independence on a list with
nothing in it. That state cannot reach the question in a run — `select`
refuses `no_tasks` before `decide` is asked — so the miss costs nothing in
the binding and stays on the record as the wording's known soft spot.

The hard reachable cases also moved the right way: `n12-one-collision`,
one shared write inside a large mostly disjoint list, went from a
lukewarm 0.33 to 0.04, and `n12-disjoint-read`, which v1 cleared the 0.7
floor by four hundredths at 0.74, reads 0.98 under v2. `n2-ordered`,
which v2's added dependence clause names, is refused by both sets.

## The flip

v2 is at least as good as v1 on every reachable item and strictly better
on two of them — the only item it loses is the unreachable `n0-empty` —
so `programs/delegate-fan-out.json` now names
`openagents.independence.v2` in its `independence` step. It switched from
`openagents.independence.v1`, digest
`a3d47ae0da35ec1553bc906bc8c702dc7565c8cc28ba52867ce2f5a5f41d4587`.
`questions/independence.json` is untouched; historical digests stay
meaningful. `programs/burn-down.json` already bound v2, so every
independence `decide` step now asks the same wording.

## Limits

- n = 12, and the labels are the author's, read off each state by the
  rule the item carries. Nothing here is a statistical claim; the verdict
  is a comparison judgment on a small suite.
- One door, one day, one call per item per set. The numbers say how this
  door answered on 2026-09-21, not how another door or another day would.
- The gym run serves the gate question alone; the direct run serves all
  three. Questions in one call cannot see one another, and the two
  measures agreed on every item.
- The ambiguous item's "no" is the gate's safe answer, not a claim the
  tasks collide; the suite says so in its `label_rule`.
- `n0-empty` measures wording only. The program never asks it, and it is
  counted here anyway rather than left out, because an eval that hides
  its worst case is advertising.

To reproduce: build `gym` at this revision, export `TYPESAFE_API_KEY`,
and run both sets over the suite:

```sh
cargo run -p gym --bin gym -- eval --jev \
  --suite crates/gym/suites/independence-eval-v1.json \
  --questions independence-gate-v1 \
  --record crates/gym/results/independence-eval-v1.jsonl

cargo run -p gym --bin gym -- eval --jev \
  --suite crates/gym/suites/independence-eval-v1.json \
  --questions independence-gate-v2 \
  --record crates/gym/results/independence-eval-v1.jsonl
```

The full-set run that produced the raw exchange, including the locked item
and all three questions per call, is
[`crates/gym/suites/eval_independence_v1.py`](../../../crates/gym/suites/eval_independence_v1.py):

```sh
python3 crates/gym/suites/eval_independence_v1.py raw.jsonl
```
