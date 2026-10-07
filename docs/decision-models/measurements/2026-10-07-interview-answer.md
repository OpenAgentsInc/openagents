# Interview answers: thresholds, provisional

Question set: [`questions/interview-answer.json`](../../../questions/interview-answer.json)
(`openagents.interview-answer.v1`). Consumer: `coder interview`
(`crates/coder/src/task/agent_interview_judge.rs`), which judges the
self-knowledge, reaction, and reflection answers of the Gym interview
suite `alice-interview-v2` ([generative agents, item
7](../../verse/generative-agents.md#7-gym-interviews)).

## Status

Unmeasured. No live Jev call was made for this document: the work that
added the set ran under a rule that unit tests make no model or network
call. Both thresholds are provisional until the owner's live round and
marks, which `NEEDS_OWNER.md` lists.

## What the set asks

Two Noul questions over one answer, the records the item cites (each with
its reference, date, and text), the item's reference answer, and the
briefing the agent carried (at most 8 KiB of it):

| Question | Yes means | Threshold | What code does |
| --- | --- | --- | --- |
| `supported` (gate) | The cited records support the answer. | 0.5 | Counts toward the arm's supported share. |
| `embellished` | The answer states something neither the records nor the briefing hold. | 0.5 | Counts toward the arm's embellishment rate, which the `interview-v1` gate holds at 1.3% or below. |

Both start at the midpoint because neither reading acts on its own: they
are counts in a report, and the gate counts the embellishment rate only
when the judge agrees with the owner's marks on at least 0.9 of marked
readings.

## Calibration to run

Run one live round (the command is in `NEEDS_OWNER.md`), mark 20 sampled
answers with `coder interview mark`, and record:

- `coder interview agreement`: the agreement rate, and the embellishments
  the judge missed, which would understate the rate the gate reads.
- The gate's verdict on a second round with the same marks.

If the judge misses embellishments the owner marked, lower the
`embellished` threshold or revise the wording before trusting the rate.
