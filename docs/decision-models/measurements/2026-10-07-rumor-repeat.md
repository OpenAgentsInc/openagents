# Rumor repeat: level mapping, provisional

Question set: [`questions/rumor-repeat.json`](../../../questions/rumor-repeat.json)
(`openagents.rumor-repeat.v1`). Consumer: `openagents verse town rumor
propose`, which scores each rumor once and stores the score in the rumor's
file ([generative agents, item
5](../../verse/generative-agents.md#5-townsfolk-with-deterministic-routines)).

## Status

Unmeasured. The phase that added the set made no live calibration run. The
mapping below is provisional until the owner's calibration run, which
`NEEDS_OWNER.md` lists.

## What the set asks

One Score question, `repeat`, over one rumor: its fact, its source villager
and role, and the place it starts. Four levels, each a concrete situation:

| Level | Situation |
| --- | --- |
| 0 | A private or dull remark nobody would pass on. |
| 1 | Mild news a few people might mention. |
| 2 | Useful news most people would pass on. |
| 3 | Exciting news the whole town would want to hear. |

## How code reads the answer

Code doesn't threshold the answer. `townsfolk::rumor::repeat_from` maps the
probability-weighted level `p = Σ i · p_i` (or the reported `score` when the
answer has no probabilities) linearly onto a pass probability from 0.1 at
level 0 to 0.9 at level 3. That probability is the chance one villager
passes the rumor to another at one meeting (`townsfolk::diffusion`). The
command records the probability and its basis (`jev SET MODEL`) in the
rumor file, and the roster admits that file's digest, so the score is set
once and every device spreads the rumor alike. Without a Jev key, the
command records the prior, 0.5, with the basis `prior`.

## Calibration to run

Write about 30 rumors across the four levels for Everglade's villagers,
label each with the level an author would give it, ask the set once per
rumor, and record:

- Categorical agreement with the labels (Gym's selected-level rule,
  [score contract](2026-09-20-score-contract.md)).
- Rank correlation between `p` and the labels.
- The share of the town that knows each rumor after one simulated day
  (`openagents verse town rumor preview ID --days 1`), against the paper's
  diffusion figures.

If the rank correlation is weak, revise the level wording before trusting
the scores.
