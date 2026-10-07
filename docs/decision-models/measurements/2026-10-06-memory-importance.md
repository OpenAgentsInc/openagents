# Memory importance: level mapping, provisional

Question set: [`questions/memory-importance.json`](../../../questions/memory-importance.json)
(`openagents.memory-importance.v1`). Consumer: Alice's scored memory stream,
`crates/coder/src/task/agent_recall.rs` ([generative agents, item
1](../../verse/generative-agents.md#1-a-scored-memory-stream-for-alice)).

## Status

Unmeasured. No live Jev call was made for this document: the work that added
the set ran under a rule that unit tests and the phase's checks make no
model or network call. The mapping below is provisional until the owner's
live calibration run, which `NEEDS_OWNER.md` lists.

## What the set asks

One Score question, `importance`, over one record: a journal row or a memory
entry, with its kind, date, text, and exit status or author. Four levels,
each a concrete situation, as the TypeSafe skill asks:

| Level | Situation |
| --- | --- |
| 0 | Routine work that went as expected. |
| 1 | A failure, or a correction from the owner. |
| 2 | A change that was merged or rejected. |
| 3 | An owner's standing instruction, or an emergency stop. |

## How code reads the answer

Code doesn't threshold the answer. It maps the probability-weighted level
`p = Σ i · p_i` (or the reported `score` when the answer carries no
probabilities) linearly onto the paper's scale: `importance = 1 + 3p`, so
level 0 is 1 and level 3 is 10. Min-max normalization over the candidates
then removes the scale, so only the order of importances matters to
retrieval. The sidecar `agents/NAME/scores.jsonl` records each answer's
probabilities, the model, and the set's digest, so a later calibration can
rescore without asking again.

Jev scores only what the rule table in `agent_recall::rule` leaves open:
requests, reports, plans, failures, rejections, refusals, task changes,
nonzero exits, project entries, and outcomes other than a clean run. Rows
with a known answer (a command that exited 0, a stop or a retirement, an
owner's note, a job firing) never reach Jev. Until Jev scores a record, a
fixed prior by kind stands in (`agent_recall::prior`) and isn't written to
the sidecar. At most 16 records are scored per briefing, newest first.

## Calibration to run

On the phase A fixture (`crates/gym/suites/alice-interview-v1/`), label each
Jev-eligible row with the level an author would give it, ask the set once
per row, and record:

- Categorical agreement with the labels (Gym's selected-level rule,
  [score contract](2026-09-20-score-contract.md)).
- Rank correlation between `p` and the labels, which is what retrieval
  consumes.
- Whether merged and rejected task rows score above routine reports, and
  stops and standing instructions above both.

If the rank correlation is weak, revise the level wording before trusting
the mapping; if levels 1 and 2 swap often, the mapping stays linear but the
set's wording changes and its digest with it.
