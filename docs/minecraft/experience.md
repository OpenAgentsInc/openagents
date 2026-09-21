# The guild foundry experience

Status: proposed. See the [overview](README.md) for scope and baseline.

## The first world

Use a compact, prebuilt arena that fits in one establishing camera shot. Two
guild camps face a shared foundry. A short route reaches iron; a longer route
reaches a scarce diamond deposit. A broken bridge leads to the next quest area.
The map is an experiment fixture with a pinned seed and snapshot, not a promise
of procedural world generation.

The working guild names are Copper and Quartz. Each starts with a scout/miner
and a smith/coder. Roles are task preferences, not permission grants. Both agents
have bodies in the world; the coder can walk to the forge and remain there while
its real worker runs. A later six-agent scene adds a reviewer to each guild.
The neutral verifier remains outside both teams.

Show five things clearly:

| Place | Visible behavior | Underlying fact |
| --- | --- | --- |
| Camp | Guild members and a short conversation | Signed identities and NIP-29 membership |
| Mine | An agent walks to and breaks a registered ore block | A bounded world action and attributed server observation |
| Forge | A compute allocation changes from available to reserved | A durable reservation for one attempt |
| Quest board | Proposed, running, checking, accepted, or blocked | The task's recorded lifecycle |
| Bridge | A gate opens after the repair is accepted | A separate, reconciled world effect tied to the accepted patch |

Keep a permanent indicator for live, replay, or fixture mode. A waiting agent can
idle or face its workbench, but its animation must not imply productive work that
has not happened. On disconnect, show unavailable or stale state.

## One understandable round

1. The board announces a coding quest and its fixed reward. Both guilds can read
   the requirements and the rules of the round.
2. A miner receives a guild request such as “We need enough compute for a repair
   and a review.” Jev matches that request to a permitted task or skill. Rust
   calculates the required credits and available mining opportunities.
3. Guilds choose between nearby iron and a more valuable, contested diamond
   deposit. The coordinator resolves competing claims; it does not ask an LLM
   which simultaneous claim became authoritative.
4. Server observations establish that ore was mined. The referee awards credits
   once. A short guild message makes the transfer of opportunity visible.
5. The coder reserves part of the guild budget, produces a patch, and submits it
   to the neutral verifier. A failed check can lead to a bounded repair attempt.
6. Acceptance awards XP, publishes a categorical achievement label, and opens
   the guild's bridge gate. The inspector shows the patch and independent result.
7. A follow-up quest offers a chance to reuse the recorded skill. Reuse is shown
   only if the second run actually loads that artifact.

For the initial race, give each guild an isolated copy of the same coding quest
and an independent gate. Both can earn the fixed completion reward. Record time
and compute used as secondary results. Shared mining creates competition without
making a single winner prevent the other guild from demonstrating useful work.

Use deterministic rotation between eligible guilds for equal-time contested
claims, with the rotation recorded before the round. Later experiments can
compare allocation policies. Network arrival order alone is a poor fairness
rule when competitors run on different hosts.

## What agents should decide

An agent can interpret a teammate's request, select among relevant quests,
retrieve an applicable skill, propose a repair, and explain a failure. These
choices should change something observable: which agent takes work, which
deposit it pursues, or which skill it reuses.

Do not call a model for every movement tick. Pathfinding, block reach, inventory
arithmetic, claim validity, budget availability, and emergency stops are code.
Do not manufacture disagreements or long conversations just to suggest a swarm.
One useful request, one handoff, and one evidence-backed result are sufficient.

## Coding has a place in the world

The forge represents the place where compute is committed to a coding attempt.
It does not host an editor made of signs. A quest might repair a Rust route
selector whose output identifies a valid bridge path, or a parser that turns a
signed delivery manifest into an admissible gate instruction. The
[coding specification](coding-quests.md) defines the first quest precisely.

The program does not acquire server operator access because its patch passed
tests. A fixed referee adapter interprets its bounded output and performs an
allowlisted world effect. This keeps “code changes the world” tangible without
making a generated program the game administrator.

## The spectator's view

The in-game display needs guild name, active task, available and held credits,
earned XP, and the most recent accepted result. Detailed token usage, signatures,
model identities, probabilities, worktree paths, and test output belong in the
companion inspector. Selecting a unit or card resolves its `source_ref` to those
records.

A useful card reads: “Quartz repaired the bridge planner. 7 credits spent.
Checks passed. 10 XP awarded.” Each number and outcome must resolve to a retained
record. These are illustrative values, not results of an existing run.

Keep system status distinct from game performance. “Worker unavailable” is not
“bad agent.” “Verification unavailable” is not “quest failed.” A small throughput
overlay can show delivered events and decision latency during a separate load
segment; it should not obscure the resource story.

## Add now, add later, leave out

| Feature | Decision | Reason |
| --- | --- | --- |
| Physical agents, mining, shared resources, guild identities | First slice | Makes coordination and opportunity cost visible |
| Jev task matching and skill selection | First slice | Demonstrates useful typed decisions |
| Actual patches and independent checks | First slice | Makes progression mean something outside animation |
| Fixed starter resources and bounded compute | First slice | Prevents a bootstrap deadlock and uncontrolled spending |
| Curated skills with explicit provenance | First slice | Gives the demo a dependable action vocabulary |
| Learned skills and automatic curriculum | After the core loop | Requires execution isolation, evaluation, and retention policy |
| Skill sharing between guilds | Later experiment | Tests collective learning; requires immutable releases and trust rules |
| Auctioned work, credit transfers, resource trading | Later experiment | Introduces collusion, accounting, and incentive questions |
| Human participation and new guilds | After admission controls | The first arena assumes controlled participants |
| Persistent open world, combat, hunger races, griefing | Defer | Adds failure causes unrelated to the first hypothesis |
| Bitcoin, Lightning, transferable tokens, redemption | Leave out of this profile | Game allocation does not need a payment economy |
| XP for tokens, chat, commits, lines changed, or idle time | Reject | Rewards activity that can be increased without useful outcomes |
| Minecraft as the primary code editor or terminal | Reject | Dense text and precise input belong in existing tools |
| A new graphics engine or CoderQuest port | Reject for the demo | Minecraft already provides the world and spectator camera |
| Self-modifying rewards, permissions, or verification | Reject | Competitors cannot redefine what wins or what they may do |

## Progress toward a custom world

Keep world coordinates, block identifiers, and Minecraft transport inside the
adapter. Tasks, attempts, credits, evidence, skills, and achievements must remain
understandable without a Minecraft client. A future CoderQuest surface can show
the same records through its own characters and props.

Do not require visual fidelity between those worlds. Preserve causal fidelity:
the same task identity, budget reservation, patch, result, and award must still
explain what happened.
