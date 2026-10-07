# Insight support: thresholds, provisional

Question set: [`questions/insight-support.json`](../../../questions/insight-support.json)
(`openagents.insight-support.v1`). Consumer: Alice's reflection,
`crates/coder/src/task/agent_reflect.rs` ([generative agents, item
2](../../verse/generative-agents.md#2-reflection-with-checked-citations)).

## Status

Unmeasured. No live Jev call was made for this document: the work that
added the set ran under a rule that unit tests and the phase's checks make
no model or network call. The thresholds below are provisional until the
owner's live calibration run, which `NEEDS_OWNER.md` lists.

## What the set asks

Two Noul questions over one insight and the records it cites, each record
with its reference, kind, date, and text:

| Question | Yes means | Threshold | What code does |
| --- | --- | --- | --- |
| `supported` (gate) | The insight follows from the cited records alone. | 0.7 | Below it, the insight is journaled as unverified with its probability and isn't stored. |
| `preference` | The insight states how the owner wants work done. | 0.5 | At or above it, a supported insight becomes a `preference` candidate, which no briefing carries until the owner accepts it. |

Code runs its own checks before Jev sees an insight: every reference
exists, every reference was among the 15 records shown for that question,
the reflection depth is at most 3, and the secret screen passes the text.
Jev answers only for insights that pass those.

`supported` starts above the midpoint because a stored insight is retrieved
later as if it were fact, so a false yes costs more than a false no, and a
dropped insight stays readable in the journal. `preference` starts at the
midpoint because a false yes costs only a candidate the owner rejects.

## Calibration to run

On the phase A fixture (`crates/gym/suites/alice-interview-v1/`), run
reflections with the live model, label each insight an author would call
supported or not, and preference or not, and record:

- For `supported`: the rate of false yeses at 0.7, which is the
  embellishment the paper measured at 1.3%, and the rate of false noes.
- For `preference`: agreement with the labels at 0.5.
- Insights with planted errors (a wrong date, a wrong count, a habit drawn
  from one record): the share `supported` refuses.

If false yeses on `supported` exceed the paper's 1.3%, raise the threshold
or revise the wording before trusting stored insights.
