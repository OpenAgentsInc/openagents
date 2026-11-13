# Shared combat timing profiles

Implemented for #13045, 2026-11-08. This is the engine's current typed
timing boundary, not the complete world-profile format proposed in README.md.
The timing policy lives in `rules/srd/timing-profile.coax`. No public
wire format is introduced.

## One declaration per world

A world selects its whole rules profile with one `rs_set_world` call before
play (`rules/srd/world-profile.coax`). The profile carries the timing,
area-origin, measuring and walking choices below; it applies all four or
none, the clock seals it, and `rs_reset` restores strict. Shared rules read the
battle's applied fields. No world passes a literal profile any more.

| Profile | Worlds | Named deviations (`rs_world_deviations`) |
| --- | --- | --- |
| `RS_STRICT_WORLD` (default) | Strict tests, the one-shot table, 5E Lab's SRD rounds | None |
| `RS_ACTION_WORLD` | Everglade 5E single play, 5E MMO, 5E Lab action mode | 1.5-second turns; cooldown recovery; caster spared by its own areas |
| `RS_EVERGLADE_WORLD` | Everglade Experimental, the Everglade 5E room, Everglade II | 1.5-second turns with no cooldown recovery; caster and its team spared by harmful areas; straight-line ranges; unpaid continuous walking ([everglade-combat-profile.md](everglade-combat-profile.md)) |

Everglade II's harmless social targeting (`everglade/shared/cast-rules.coax`) is a
targeting rule, not a timing deviation, and stays named in its own file.

## Continuous walking pays the turn's feet

Under `RS_WALK_PAID` (strict, and `RS_ACTION_WORLD`), a physical walker pays
from the same budget as grid steps (`walk-budget.coax`; SRD 5.2.1 p.14
Difficult Terrain, p.179 Crawling, p.180 Dash, p.188 Speed). Each foot walked costs one foot of movement, two in
Difficult Terrain or while crawling, three for both; Dash adds current Speed;
Speed loss keeps spent feet spent. `rs_walk_take` cuts the walker's step where
the turn's movement runs out, so it can't overspend; the carried part of a foot
resets at the next turn. Strict Speed 30 walks exactly 9.144 m a turn;
action timing pays its 10-foot beat. `RS_WALK_RATE` (Everglade) walks unpaid at
the Speed-proportional rate described below; that is the declared deviation.
No current world drives a paid physical walker; the actual walker is checked
against a strict turn in `walk-budget-walker-checks.coax`.

## The battle clock counts whole ticks

GitHub #13287. `RsBattle.clock` is an integer count of ticks, `RS_TICKS`
(1,000,000) a second: a microsecond each, the unit the rest clock already
used. Every deadline and duration in the rules is a count of ticks
(`rs_secs(60)` is a minute). No time in the rules is a float, so a battle a
month old measures a turn, a duration and a recharge exactly as a new one
does. Before this the clock was an `f32` of seconds, whose step grows past
0.01 s after about a day and a half of uptime.

- A world steps the battle with `rs_tick_ticks(b, ticks)`. Everglade's room
  passes `tick * RS_TICKS / EG_HZ`, whole numbers. `rs_tick(b, seconds)` is
  still there for a host that has a float frame time: it admits the frame to
  whole ticks and keeps the part of a tick left over (`rest-clock.coax`).
- `rs_clock_s`, `rs_ticks_s` and `rs_s_ticks` convert for a world that
  draws. The rules never read a float time back.
- A turn boundary is one tick, with no slack on either side. An effect that
  ends before a turn has that turn's tick as its deadline: the tick that
  reaches it drops it before the turn is taken. An effect that lasts until
  the end of a creature's turn carries that creature in `RsEffect.turn_end`:
  the turn's end drops it (`rs_end_turn`), so it holds through that turn
  however long a world's frame is, and is gone before any later turn. The
  clock drops it only when that turn can't come: its owner is dead or gone,
  has already passed that tick, or is a whole beat overdue. The old
  deadlines were 0.01 s before and 0.05 s after the boundary, which a frame
  longer than 0.05 s broke.
- Check: `world-timing-checks.coax`, "a battle 30 days old measures every
  turn, duration, recharge and round exactly as a new one". It runs the same
  fight from tick 0 and from tick 2,592,000,000,000 and compares every
  measure.

## Turn boundaries and durations

Turns are instants on the clock. Start-of-turn triggers (Dodge and Shield end,
the Reaction returns, death saves on a new round) fire only at the creature's
own turn start; repeated saves are end-of-turn rolls. "Until the end of your
next turn" holds through that turn and ends before any later creature's turn.
Durations are counts of the world clock's ticks in both profiles: a one-minute effect lasts
ten rounds of the creature's turns (40 action beats), an hour 3,600 seconds,
and every executable spell's timed duration and Concentration match the pinned
source. SRD p.187 lets the turn's owner order simultaneous effects; the engine
resolves them in a fixed order (effect slot order), which is checked to give
the same result in either order for the covered saves. No order chooser ships.
Checks: `turn-order-checks.coax`.

## Timing profiles


| Decision | `RS_STRICT_TIMING` (default) | `RS_ACTION_TIMING` (house rule) |
| --- | --- | --- |
| Logical turn/Reaction refresh | 6 seconds | 1.5 seconds |
| Movement grant | Exact current Speed in feet | Existing nearest whole-square quarter-round grant, at least one square for positive Speed |
| Standing cost | Half current Speed, round down in feet | Half that beat's movement grant, round down in feet |
| Slot, Rage, ordinary limited-use recovery | No timer recovery | One spent resource at a time after 9 seconds |
| Focus, Sorcery, Bardic Inspiration recovery | No timer recovery | One point/use after 4.5 seconds |
| Lay On Hands pool recovery | No timer recovery | Up to 5 points after 4.5 seconds |
| Repeat saves, death saves, Rage upkeep | Six-second round boundary | Same six-second round boundary |
| Timed durations | World-clock seconds | Same world-clock seconds |

Cooldowns retain the existing scheduling convention: the first tick observing
a missing resource schedules its recovery. They are a house rule, not a claim
that the SRD grants timed resource recovery. Strict recovery/rest workflows
still require their own implementation and verification.

The action grant preserves the previous adapter: Speed 30 gets two five-foot
squares per beat; Speed 25 gets one. The standing override makes its cost
explicit rather than requiring half a full round's Speed from a short beat.
It does not clear Prone automatically. Strict mode preserves every residual
foot, including Speed 7 granting seven feet and standing at Speed 25 costing
twelve feet. Grid steps still require five feet of actual displacement.

Only these two identities are supported. Unknown profiles reject without
changing state. Begin, turn start or tick seals selection; changing a live
clock would invalidate already scheduled turns and cooldowns. To remove a
profile, reset/recreate the battle: reset restores strict and reopens selection.
This also prevents a previous world's action timing from leaking into strict
tests. Copying a battle/saved seat must preserve its timing and movement fields.

Experimental and the existing Everglade single/room battle initializers select
the action preset. 5E Lab's action/round switch recreates the battle and selects
action/default strict respectively. 5E MMO selects action; the one-shot table
retains strict. These callers no longer own independent beat/recharge numbers.
Constants retained solely in old world assertions are independent expected
values, not core timing inputs.

This timing subset does not authorize or qualify world spell grants, free
casting, material waivers, unlimited-use preparation, monster conversions,
non-grid physics, damage scaling, cast bars or house spell records. The legacy
room's per-press resource reset still needs a declared shared cost profile.
Everglade II adoption and visible standing/input choices remain #13046 work.
No world-profile editor or general profile text parser ships here.

Evidence: [timing verification](../verification/srd-timing-2026-11-08/README.md).

No world refills a press (#13284). `RS_UNLIMITED_PRESS`, which restored
slots, uses, Focus, Rage and the turn's Action on every press in Experimental
and the legacy Everglade 5E room, is deleted. `press-budget.coax` keeps only
the snapshot a single-player zone uses to put back what a refused press
spent. Everglade's profile, with what a turn, a slot and a rest are in real
time, is [everglade-combat-profile.md](everglade-combat-profile.md). The
2026-11-08 press-budget and room-press verification records describe the
deleted policy.

`RS_BEATS_TIMING` (kind 2) is the third timing identity: the action profile's
1.5-second turn and movement grant with no timed recovery. Everglade's world
uses it.

Everglade II's server now commits shared execution under its explicit social
profile: self/ally entries affect the caster, offensive spells pay shared
empty-target costs without harming anyone or creating harmful areas, and empty
swings spend an Action. Unlimited press budgets do not renew Reactions or
movement; failed execution rolls back before cooldown/publication. This is a
named world deviation, not strict target semantics. Page prediction and complete
state/runtime qualification remain open. See
[Everglade II evidence](../verification/srd-e2-shared-cast-2026-11-08/README.md).

Experimental's continuous action locomotion also declares its existing physical
baseline separately from the grid grant: Speed 30 corresponds to player
walk/run 3.5/7.5 m/s (dash 9.2) and AI pursuit/home 2.4/1.5 m/s.
`locomotion-scale.coax` now propagates current walking Speed and additive
crawl/difficult-terrain costs proportionally, including rates above one. The
controller's opt-in subdivision path preserves the paid 1/64-second clock and
original collision travel bounds; it does not multiply world time. This keeps
the continuous action profile explicit and does not claim strict feet-per-turn
payment, special speeds, Haste's program, or complete world-zone/timing behavior.
See [physical locomotion evidence](../verification/srd-physical-scale-2026-11-08/README.md).

## Area origin profile

`rules/srd/area-origin.coax` is a second declared profile with the same
contract (default strict, chosen before the clock is sealed, reset restores
strict). `RS_STRICT_AREAS` follows the SRD: a Cube or Line originating from
the caster leaves it out; an area at a point includes it. `RS_SPARE_CASTER_AREAS`
is a house rule that keeps a caster out of every area it casts. Each world that
selects `RS_ACTION_TIMING` also declares `RS_SPARE_CASTER_AREAS`, preserving its
play; it is a separate declaration, not part of the timing profile. See
[area-origin evidence](../verification/srd-area-origin-2026-11-08/README.md).

