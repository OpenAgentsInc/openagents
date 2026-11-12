# 18. Turns become time

> **Status: partly implemented.** A world declares its whole rules profile in
> one call, `rs_set_world`, in
> [`rules/srd/world-profile.coax`](../../rules/srd/world-profile.coax)
> (#13045). The timing profiles are
> [`rules/srd/timing-profile.coax`](../../rules/srd/timing-profile.coax),
> and continuous walking is
> [`rules/srd/walk-budget.coax`](../../rules/srd/walk-budget.coax).
> The design note is [`docs/dnd/timing-profiles.md`](../../docs/dnd/timing-profiles.md).
> Other agents are extending these files now, so this chapter names functions
> and has no source listings.

Everglade aims to follow the SRD 5.2.1's rules faithfully while adapting them
for action combat. The largest adaptation is time. This chapter answers the
player's question: "Whose turn is it?"

In the SRD, combat goes in rounds of 6 seconds, and each creature takes its
turn in order. In Everglade, no one waits. Every creature gets a short turn
every 1.5 seconds, and you move, cast and swing as things happen. The SRD's
rules still decide what each turn may do.

## What this chapter teaches

- How one declaration names every way a world departs from the SRD.
- How the action timing maps a 6-second round onto 1.5-second turns.
- Which events keep the 6-second round, and why.
- How to remove the profile and get strict SRD results back.

## What exists today

A world profile has four fields: timing, areas, press costs and walking.
`rs_set_world` applies all four or none, before the battle's clock is sealed.
`rs_world_deviations` names each departure as one bit:

| Bit | Name | What it means |
| --- | --- | --- |
| `RS_DEV_BEATS` | 1.5-second turns | A turn every 1.5 s, with a quarter-round movement grant |
| `RS_DEV_RECHARGE` | Cooldown recovery | Spent slots, uses and points come back on timers |
| `RS_DEV_SPARE_CASTER` | Caster spared by its own areas | A caster is never in its own area |
| `RS_DEV_FREE_PRESS` | Unlimited presses | Each press refills slots, uses, Focus, Rage and the Action budget |
| `RS_DEV_RATE_WALK` | Unpaid continuous walking | Walking doesn't spend the turn's feet |

Three profiles exist:

| Profile | Worlds | Deviations |
| --- | --- | --- |
| `RS_STRICT_WORLD`, the default | Strict tests, the one-shot table, 5E Lab's SRD rounds | None |
| `RS_ACTION_WORLD` | Everglade 5E single play, 5E MMO, 5E Lab's action mode | 1.5-second turns, cooldown recovery, caster spared |
| `RS_EVERGLADE_WORLD` | Everglade Experimental, the Everglade 5E room, Everglade II | All five |

## What follows the SRD exactly

- **Durations are world seconds.** One minute is 60 seconds in both timings:
  ten rounds of strict turns, or 40 action turns.
- **The 6-second round stays for round events.** Death saving throws, repeated
  saves against a condition and Rage upkeep happen at 6-second round
  boundaries in both timings.
- **Turn boundaries.** Start-of-turn effects, such as the end of Dodge and the
  return of your Reaction, fire only at your own turn's start. "Until the end
  of your next turn" lasts through that turn and ends before anyone else's.
- **The action economy inside a turn.** Each turn has its Action, Bonus Action
  and Reaction, as the SRD says.

## What changes for action combat, and why

| Decision | Strict SRD | Action timing |
| --- | --- | --- |
| A turn and your Reaction's return | Every 6 s | Every 1.5 s (`rs_timing_beat`) |
| Movement each turn | Your Speed, in feet | A quarter of it, to the nearest whole 5-foot square, at least one (`rs_timing_move`) |
| Standing up from Prone | Half your Speed | Half that turn's movement |
| A spent slot, Rage or use | Back on a rest | One back after 9 s (`rs_timing_recharge`) |
| A spent Focus, Sorcery Point or Bardic Inspiration | Back on a rest | One back after 4.5 s |

Why each change:

- **Short turns** let everyone act at once, in real time. A 6-second wait
  between your actions would feel like lag.
- **Cooldowns** stand in for rests in a fight that never stops. They are a
  declared house rule, not a claim that the SRD grants timed recovery.
- **Round events keep 6 seconds**, so that a condition lasts as long as the SRD
  says, and a dying character has the SRD's time to be saved.

In Everglade's own world profile, walking is also unpaid: you walk at a rate
proportional to your Speed (`RS_WALK_RATE`) instead of spending feet from each
turn. You still walk only when you may move. Under strict walking
(`RS_WALK_PAID`), each foot walked costs a foot of the turn's movement, two in
Difficult Terrain or while crawling.

## A worked example: Speed 30 in both timings

```text
strict:   one turn per 6 s        30 ft a turn            = 5.0 ft/s
action:   one turn per 1.5 s      30 / 4 = 7.5 ft → 10 ft = 6.7 ft/s
```

The action grant rounds 7.5 feet to the nearest whole square, 10 feet. Speed 25
gets one square (6.25 feet → 5 feet). The table in `rs_timing_move` is:

| Speed | Under 30 | 30 to 49 | 50 to 69 | 70 to 89 | 90 or more |
| --- | --- | --- | --- | --- | --- |
| Feet a turn | 5 | 10 | 15 | 20 | 25 |

Now a dying character. In the action timing, it gets a turn every 1.5 s, but
it makes a death saving throw only every fourth turn:

```text
turns:       1    2    3    4    5    6    7    8
time (s):    0   1.5  3.0  4.5  6.0  7.5  9.0 10.5
death save:  yes  -    -    -    yes  -    -    -
```

That is one save every 6 seconds, as in the SRD. In both timings, the third
save comes 12 seconds after the first.

## The checks

In `rules/srd/world-profile.coax`:

- `#test "world admission accepts exactly declared fields and only before the
  clock is sealed"`.
- `#test "the declared worlds name every departure and the strict world none"`.
- `#test "a world applies all or nothing, the clock seals it and reset restores
  strict"`.

In `rules/srd/timing-profile.coax` and
`rules/srd/timing-checks.coax`:

- `#test "profile timing declares beat cooldown and movement without changing
  strict defaults"`. It checks `rs_timing_move` against an independent table
  for every Speed from -1 to 100.
- `#test "short action turns renew reactions but death saves and repeat saves
  remain six-second events"`.
- `#test "only the declared action profile recharges spent resources on the
  existing cooldown"`.

In `rules/srd/world-timing-checks.coax`:

- `#test "removing the world profile gives strict beats, grants, recovery,
  areas, presses and walking"`.
- `#test "declared action timing pays its quarter-round grant; the Everglade
  world walks unpaid"`.

In `rules/srd/turn-order-checks.coax`:

- `#test "a one-minute effect lasts ten rounds of turns in both profiles and an
  hour runs on the same clock"`.
- `#test "until the end of your next turn holds through that whole turn and ends
  before anyone else's"`.

The evidence is in
[`docs/verification/srd-timing-2026-11-08/`](../../docs/verification/srd-timing-2026-11-08/README.md).

```sh
coax/bin/coax test --library rules/srd/world-profile.coax
coax/bin/coax test --library rules/srd/timing-checks.coax
coax/bin/coax test --library rules/srd/world-timing-checks.coax
```

## Mistakes

- **Numbers owned by each world.** Before #13045, each world's battle setup
  kept its own beat and recharge numbers. Two worlds could disagree about how
  long a turn is, and nothing named the difference. Now a world chooses one
  declared profile, and the deviations test lists exactly what each profile
  changes.
- **A profile that leaks.** A battle that kept a previous world's action timing
  would give wrong results in a strict test. A profile can only be chosen
  before the clock is sealed, and `rs_reset` restores strict. The admission and
  reset tests check both.
- **A condition that ended four times too soon.** If repeat saves came at the
  end of every 1.5-second turn, a creature would get four tries in the time the
  SRD gives it one. The round-event test asserts that saves stay on the
  6-second round.

## What comes next

Chapter 19, Familiars and Wild Shape, is still to be drafted.
