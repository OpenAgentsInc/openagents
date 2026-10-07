# React or continue: the choice, provisional

Question set:
[`questions/react-or-continue.json`](../../../questions/react-or-continue.json)
(`openagents.react-or-continue.v1`). Consumer: Alice's day plans, through
`crates/coder/src/task/agent_plan.rs`
([generative agents, item 4](../../verse/generative-agents.md#4-visible-day-plans-from-real-work)).

## Status

Unmeasured. No live Jev call was made for this document: the work that
added the set ran under a rule that unit tests and the phase's checks make
no model or network call. The set has no threshold; the owner's live
calibration run, which `NEEDS_OWNER.md` lists, decides whether a low
confidence should fall back to `continue`.

## What the set asks

One Choice question, `reaction`, the gate, with three fixed options:
`continue`, `react_now`, and `defer`. The state names the agent, the local
time, the plan block under way (title, source, and end), the next block,
and the event (what it is and the real work it comes from).

Code decides before asking:

- The owner's request always interrupts (`react_now`), with no question.
- A scheduled standing job fires in the slot the plan already holds; the
  plan moves to that block, with no question.
- When Jev can't answer, the agent continues, and the journal says why.

So the question sees only the rest: a watched issue that came free, the
default branch moving, a check failing while she works a block.

## Calibration to run

Write about 60 events against recorded plans, labeled with what a person
would choose, including events that repeat the current block's work, and
record:

- Agreement with the label, per option.
- The rate of `react_now` on events labeled `continue` or `defer`, which
  costs a re-plan and a broken block; the reverse costs a delay.
- Confidence on correct and wrong answers.
- Tokens per question, against the specification's estimate of 50
  decisions a day at $0.002 in all.
