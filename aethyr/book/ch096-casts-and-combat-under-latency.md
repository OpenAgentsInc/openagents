# 96. Casts and combat under latency

> **Status: partly implemented.** The rules are written in
> [the cast rules spec](../../docs/verse/2026-11-04-everglade-cast-rules.md)
> and built (#12790, commit 67b59c14e2): oa-room checks range and line of
> sight at a cast's start and at its release, waits at most half a second
> for the inputs sent before a cast, stops a bar whose target falls or whose
> caster is held, and orders each tick heals before harm
> ([room-casts.coax](../../coax/web/room/room-casts.coax)). The page runs
> the same checks before it sends. `cast-proof.sh` runs four players at
> 250 ms with 2% loss. The rules are Everglade's; a shared `aethyr/net`
> module for other realms is not extracted.

Movement can be predicted and corrected. A hit can't be taken back once
someone saw their HP drop. So combat needs rules that say exactly what is
checked when, and what each player sees before the server answers. This
chapter adds those rules for a target-based game like Everglade.

## What this chapter teaches

- Latency compensation as a game rule, decided per game.
- Two checkpoints for range and line of sight: start and release.
- Bounding a command's age without trusting a client clock.
- What happens when a target falls or leaves during a cast.
- A fixed order of actions within one tick.
- What the page predicts and what waits for the server.

## The rules

The research doc (section 10) lists the questions a design must answer.
Everglade answers them as follows.

**Two checkpoints.** A cast is checked at its start and again at its
release, the end of its cast bar. Only Everglade's own four spells have a
bar; every other cast is released as it starts. Range is measured across the
ground to where the cast lands: its named target while that target stands,
else its aim point. The release allows more slack than the start, because
the target walks during the bar. Line of sight runs from the caster's eyes
to just over and short of where the cast lands, against the room's own copy
of the town; low pieces such as crates and fences don't block it.

<!-- source: apps/openagents-web/everglade-cast-rules.coax cast-rules -->
```jai
// ---- range and sight
CR_SLACK :: 3.0;          // m past a spell's range at its start: where the page and the server have you differ a little
CR_SLACK_END :: 6.0;      // m past it at its release: the target walks during the cast bar
CR_REACH_LEAST :: 4.5;    // m a spell reaches at least (a touch, a 5-foot reach, with the slack)
CR_EYE :: 1.8;            // m over the caster's feet the line of sight starts
CR_SHORT :: 1.0;          // m short of the aim it ends: the wall or roof you aim at is not in the way
CR_AIM_UP :: 0.5;         // m over the aim it ends
CR_LOW :: 1.2;            // m: a piece lower than this (a crate, a fence, a bench) doesn't block the line
CR_NEAR :: 2.5;           // m: an aim this close needs no line of sight (a press aims 2 m ahead of you; the rest is the wire's cm and your pose)

// ---- time
CR_WAIT :: 15;            // ticks (0.5 s, the room's EG_BANK) a cast waits for the inputs sent before it
CR_SHOW :: 0.9;           // s a refused cast's bar shows its reason icon
// Everglade's own four's cast bars (verse/rules/srd5.coax srd_spell bar: Meteor Swarm, Thunderbolt,
// Mega Thunderbolt, Water Orb). Every other cast has none: it is released as it starts.
CR_BAR :: [4] f32.[2.5, 1.2, 0.5, 0.6];
```

**Command age without a client clock.** The page sends no time and the room
reads none. A page's inputs and casts share one sequence counter, and the
socket keeps their order. When the room hears a cast, it waits until it has
stepped every input sent before it, so the cast is checked where the page
was at the press, for at most `CR_WAIT` 15 ticks (0.5 s). No client time
can move a cast.

**A target that falls or leaves.** A named target already gone at the start
falls back to the aim point. One that falls during the bar stops the cast
for everyone near (`e stop SS SLOT gone`). One that falls in flight, after
the release, takes the strike where it stood. A caster held or at 0 HP
during the bar stops it.

**One order a tick.** Starts in the order heard; heals that are due; the
world's harm from this tick; the rules' step; interrupts; releases, which
check range and sight again; harm that is due. So a heal and a killing blow
in one tick land heal first, and an interrupt on the release tick wins.

**Combat is never rewound.** Movement is predicted and corrected (chapter
90). An HP change, a condition or a death is final when the server sends
it.

**What the page predicts.** The cast bar and its fill, the cast animation,
the cooldown sweep, and the range and sight checks before it sends. Damage,
healing, conditions and deaths wait for the room. A refused cast cancels
its bar and shows a reason icon for 0.9 s, never a blocking text warning.

## The latency budget

The research doc's illustrative budget for a 20 Hz server is about 136 ms
from press to result. Everglade's 30 Hz tick adds about 16.7 ms on average
instead of 25 ms:

```text
outbound 50 + tick wait 16.7 + processing 3 + return 50 + draw 8 ≈ 128 ms
```

A cast also waits for the inputs before it, at most 0.5 s. Prediction makes
the press feel immediate; it doesn't make the result arrive sooner.

## Run it

```shell
coax/bin/coax test --library apps/openagents-web/everglade-cast-rules.coax
coax/web/room/build.sh
coax/web/room/cast-proof.sh coax/web/room/bin/room 8881 250 30 20
```

The first command's last line:

```text
4 checks, 4 passed, 0 failed
```

`cast-proof.sh` puts the shaping proxy between four players and the room at
250 ms, 30 ms of jitter and 2% loss. A Warlock casts 2 m inside its range
(taken) and 8 m past it (refused for range), and two druids cast at one wolf
so the first's target falls in its bar (stopped as gone). It prints
`casts hold` and exits 0, or `casts fail` and exits 1.

## Acceptance checks

- Every rule above has a unit check (`casts:` in `room.coax`, the
  cast-rules library, `cast sight` in `everglade5e.coax`).
- `cast-proof.sh` holds at 250 ms with 2% loss.

## Dependencies

Chapters 89 (intents), 90 (prediction) and 94 (effects).
