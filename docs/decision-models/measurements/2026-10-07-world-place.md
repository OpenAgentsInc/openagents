# World place: the choice, provisional

Question set: [`questions/world-place.json`](../../../questions/world-place.json)
(`openagents.world-place.v1`). Consumers: Alice's day plans and Everglade's
townsfolk, through `crates/coder/src/task/agent_place.rs`
([generative agents, item 3](../../verse/generative-agents.md#3-a-world-tree-generated-from-the-layout)).

## Status

Unmeasured. No live Jev call was made for this document: the work that
added the set ran under a rule that unit tests and the phase's checks make
no model or network call. The set has no threshold yet; the owner's live
calibration run, which `NEEDS_OWNER.md` lists, decides whether it needs
one.

## What the set asks

One Choice question, `place`, the gate. The state names the agent, its
activity in its own words, and the place it is in (name and kind). The
options are the children of that place in the world tree
(`world_tree::choose::options`) that the agent knows, that offer what the
activity needs or hold a place that does, and that aren't an exclusive
object someone occupies, each described by its name, kind, and
affordances. `none` is always an option.

Code walks down the tree one question at a time, from the zone to a
district, a building, a room, and an object, and stops at an object or on
`none`. Restricting the options by affordance and occupancy is what keeps
an agent out of odd places; the question only ranks sensible ones.

## Calibration to run

On Everglade's tree (`crates/world-tree/data/everglade.json`), write about
60 activities with the place a person would pick at each level, including
activities no option fits, and record:

- Agreement with the labeled place at each level.
- The rate of `none` when a fitting option exists, and of a place when
  none fits.
- Confidence on correct and wrong answers, to decide whether a low
  confidence should stop the walk.
- Tokens per question, against the specification's estimate of about
  1,000.
