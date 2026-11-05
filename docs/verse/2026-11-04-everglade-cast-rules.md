# Everglade cast and combat rules under latency

Date: 2026-11-04. Status: specification, built. Issue: #12790, under the Everglade multiplayer
epic #12718. Background: section 10 of [the MMO research notes](../research/mmo.md).

This spec sets when an Everglade room checks a cast, how old a cast may be, what happens when a
target falls or leaves during a cast, the order of actions in one tick, what your page predicts,
and how a refused cast looks. The room server is the authority. Your page predicts only what it
can take back.

The code:

- `apps/openagents-web/everglade-cast-rules.coax`: the numbers and the small rules both sides use.
- `apps/openagents-web/everglade-cast-sight.coax`: line of sight, and how your page shows a refusal.
- `coax/web/room/room-casts.coax`: how the room resolves a cast, phase by phase.
- `coax/web/room/cast-check.coax` and `cast-proof.sh`: the run against a room under a bad network.

## Contents

1. What happened before
2. Range and line of sight
3. Command age
4. A target that falls, leaves or becomes invalid
4a. Blocked players
5. The order of one tick
6. What your page predicts, and how a refusal looks
7. The wire
8. Tests

## 1. What happened before

- The room checked range once, when it heard the cast, against the aim point. It allowed 3 m of
  slack and at least 4.5 m.
- Nothing checked line of sight. You could cast through a house.
- Nothing bounded a cast's age. The room checked a cast against wherever it had you when the
  frame arrived, which could be ahead of or behind where you pressed.
- Casts resolved in the order of their slots in the room's table, not the order the room heard
  them.
- The strike of Everglade's own four spells went to the town's world at the press, so a cast bar
  that a hold interrupted still broke the town.
- Refusals were text on your screen (`CAN'T · RANGE`).

## 2. Range and line of sight

A cast has two checkpoints: its **start**, and its **release** at the end of its cast bar. Only
Everglade's own four have a cast bar: Meteor Swarm 2.5 s, Thunderbolt 1.2 s, Mega Thunderbolt
0.5 s and Water Orb 0.6 s (`CR_BAR`). Every other cast is released as it starts, so it has one
checkpoint.

**Range** is measured across the ground from the caster to where the cast lands: its named target
while that target stands, else its aim point. Self-targeted spells skip it.

| Checkpoint | Allowed past the spell's range | Least reach |
| --- | --- | --- |
| Start | `CR_SLACK`, 3 m | `CR_REACH_LEAST`, 4.5 m |
| Release | `CR_SLACK_END`, 6 m | 4.5 m |

The start's slack covers the small gap between where your page and the room have you. The
release's larger slack covers a target that walks during the bar.

**Line of sight** is checked at both checkpoints, by the room's own copy of the town:

- The line runs from the caster's eyes (`CR_EYE`, 1.8 m over the feet) to `CR_AIM_UP` (0.5 m)
  over where the cast lands, and stops `CR_SHORT` (1 m) short of it. The wall or roof you aim at is
  not in the way.
- The ground blocks it (a hill), and so does any standing piece of the town at least `CR_LOW`
  (1.2 m) tall. Crates, fences and benches don't. A broken piece doesn't.
- An aim within `CR_NEAR` (2.5 m) of the caster is always seen. A press of an ability you don't
  point with aims 2 m ahead of you, so it never needs a line of sight: the half metre covers the
  aim's centimetres on the wire and the server's copy of your pose.

The room's host (`room-host.c`) answers the sight questions with its world before it ends each
tick. A room with no world, as in unit tests, treats every line as clear.

A failure at the start refuses the cast to the caster alone (`r SEQ range` or `r SEQ sight`). A
failure at the release stops the cast for everyone near (`e stop SS SLOT range`), and its strike
never reaches the town.

## 3. Command age

The page sends no clock, and the room reads none. A cast is ordered against the caster's inputs
by its sequence number: the page's inputs (`I SEQ ...`) and casts (`C SEQ ...`) share one counter,
and the socket keeps their order.

When the room hears a cast, it waits until it has stepped every input the page sent before it,
so the cast is checked where the page was when you pressed. It waits at most `CR_WAIT`, 15 ticks
(0.5 s). After that the cast starts where the room has you. A cast is therefore never older than
half a second of the room's time when it starts, and no client time can move it.

## 4. A target that falls, leaves or becomes invalid

| When | What happens |
| --- | --- |
| At the start, a named target is already gone | The cast falls back to its aim point. |
| During the bar, the named target falls or leaves | The bar stops at the release: `e stop SS SLOT gone`. |
| In flight (released, strike not yet due), the target falls | The strike lands where the target stood at the release. |
| During the bar, the caster drops to 0 hit points | The bar stops: `dead`. |
| During the bar, the caster is incapacitated, paralyzed, petrified, stunned or unconscious | The bar stops: `held`. Restrained doesn't stop a cast. |
| The caster leaves | Casts in their bar stop for everyone (`left`); released casts still strike. |

## 4a. Blocked players

When either player in a pair blocks the other (the page's BLOCK, sent to the room as `K`), the
room keeps them apart in both directions, so no one fights a body they can't see (#12789):

- **What you see:** the room never sends you the other player. That covers their state row, name,
  WANTED level, level, effects, and the casts, stops and hits that name them. It also covers the
  snapshot you get when you join. The rows go away (`x`) as soon as you block.
- **Targeting:** a cast that names the other player as its target falls back to its aim point. A
  heal that names them goes to the caster instead. Neither player counts as "the most hurt ally"
  for the other.
- **Harm and help:** while either player's roll is made, the other player is out of the battle. An
  area spell neither hits nor heals them. PvP doesn't change this.
- **Everyone else:** the other players in the room still see and fight both of them as usual.

The rules live in `everglade5e-room-rules.coax` (`apart`, `e5rr_apart`). The room fills them from
the blocks in `coax/web/room/room-blocks.coax`. The test "interest on the wire" in that file reads
every byte sent to a blocker from its join and checks that none of it names the blocked player.

## 5. The order of one tick

Each room tick runs in this order (`rm_eg_end`):

1. **Starts**, in the order the room heard them.
2. **Heals** that are due, in `cr_before` order.
3. **The world's harm** to seats from this tick (a guard's blow, a falling piece), held until the
   heals are in.
4. **The rules' step**: the creatures' turns and conditions.
5. **Interrupts**: bars whose caster fell or is held stop.
6. **Releases**: bars that are done check range and sight again.
7. **Harm** that is due, in `cr_before` order.

The rules that follow from that order:

- **A heal and a killing blow in one tick**: the heal lands first.
- **An interrupt and a release in one tick**: the interrupt wins. A caster held on the tick their
  bar finishes doesn't cast.
- **Two players hitting one raider in one tick**: the earlier start strikes first, then the one
  heard first. When the first fells the raider, the second strikes where it stood.

## 6. What your page predicts, and how a refusal looks

Your page predicts what it can cancel. It waits for the room for anything that changes another
player's state.

| Predicted on the page | Waits for the room |
| --- | --- |
| The cast bar starting, and its fill | Damage and healing |
| The cast animation | Conditions |
| The cooldown sweep on the hotbar | Deaths and falls to 0 hit points |
| The range and line-of-sight checks, before it sends | Whether a raider's or player's turn happens |

The page runs the same range and line-of-sight rules on its own town before it sends a cast. A
cast it can see is blocked never leaves, and refuses at once.

A refused or stopped cast never shows a blocking text warning:

- Its cast bar cancels, and the Warlock slot's sweep clears.
- An empty meter plate fades where the bar was for `CR_SHOW` (0.9 s), with a short reason icon over
  it: arrows both ways (range), an eye struck through (sight), a cross (gone or left), an hourglass
  (busy or cooldown), two bars (held or dead), a struck ring (anything else).
- Nothing blocks the hotbar. You can cast again at once.

## 7. The wire

| Frame | Meaning |
| --- | --- |
| `C SEQ SLOT AX AY AZ TGT [MODE]` | The page casts. TGT is 1000 + seat, 2000 + creature, or -1. |
| `r SEQ WHY` | The room refused cast SEQ at its start. To the caster only. |
| `e cast SS SLOT AX AY AZ SEED` | Seat SS's cast started. To everyone near. |
| `e stop SS SLOT WHY` | Seat SS's cast stopped before its strike. To everyone near. |

WHY is one of `CR_WORDS`: `range`, `sight`, `gone`, `held`, `dead`, `busy`, `cooldown`, `left`,
`aim` or `no`. A page that hears an unknown word shows the struck ring.

## 8. Tests

Unit tests, one or more for each rule:

- `coax test --library apps/openagents-web/everglade-cast-rules.coax`: reach and slack, command
  age, the strike order, words and icons, the halting conditions.
- `coax test apps/openagents-web/everglade5e.coax --only "cast sight"`: a house blocks the line,
  the wall you aim at and low pieces don't; a refusal cancels the bar and the sweep without text.
- `coax test --library coax/web/room/room.coax --only "casts:"`: waiting for prior inputs and the
  half-second cap; range and sight at the start and the release; a target that falls in the bar
  and in flight; a hold in the bar and on the release tick; leaving; a heal before a killing blow;
  two seats at one raider.

The run under latency, against a local room server built by `coax/web/room/build.sh`:

```sh
coax/web/room/cast-proof.sh coax/web/room/bin/room 8881 250 30 20
```

It puts `net-proxy.coax` between four players and the room: 250 ms round trip, 30 ms jitter, 2% of
reads lost and resent. A Warlock casts Eldritch Blast as it walks, 2 m inside its range (taken)
and 8 m past it (refused for range). Two druids cast at one wolf so the first's target falls in
its bar (stopped as gone). Every cast must be answered within 2 s. It prints `casts hold` and
exits 0, or `casts fail` and exits 1.
