# Everglade's combat profile

GitHub #13284 (the profile) and #13285 (the records it reads). This is the
profile Everglade ships: how SRD 5.2.1 combat runs in a world where people
stand anywhere and act in real time. The SRD decides every outcome; the
profile decides only time, measuring and who an area can reach. One call
selects it, `rs_set_world(b, RS_EVERGLADE_WORLD)`
(`rules/srd/world-profile.coax`), before the battle's clock starts.

Everglade's room and Everglade II use it. The single-player zones
(Experimental, frozen with Everglade v1) use it too, with one difference: their
turn is their one-second press cooldown, not the room's 1.5-second pace. Their
presses are paid like everyone's. The grid
action worlds (5E MMO, 5E Lab's action mode, Everglade 5E single play) keep
`RS_ACTION_WORLD`. Strict SRD is the default.

## The decisions

| Decision | Everglade | SRD 5.2.1 |
| --- | --- | --- |
| A turn | 1.5 seconds. It opens on your first press after the last turn's 1.5 seconds are over | 6 seconds, in Initiative order |
| What a turn holds | One Action, one Bonus Action, one spell cast with a slot. The same as the SRD | The same |
| A slot, a use, a Focus Point, a Rage | Paid on every press. It comes back by a Short Rest or a Long Rest, as its class says. No clock gives it back | The same |
| A Short Rest | One hour of the world's clock, uninterrupted (`rules/srd/short-rest-time.coax`) | The same |
| Cast time | An Action or a Bonus Action is its part of the turn: the spell resolves on the press. A Ritual takes 10 minutes more. A longer casting time is that many seconds | The same |
| Recharge | An ability can be pressed again when the part of the turn it takes is back: at most 1.5 seconds. Its icon sweeps for that time. An ability that takes no part of the turn (Action Surge) waits 0.5 seconds | No recharge; one turn a round |
| Range and area | The straight line between two creatures, to the nearest 5 feet, the same in every direction (`RS_MEASURE_STRAIGHT`) | On a grid, a diagonal square counts as a straight one |
| An area that can harm | Every creature in its shape at the aimed point, except the caster and the caster's team | Every creature in its shape |
| An area that helps | Every creature in its shape | The same |
| Player against player | Off. No ability of a player harms another player | Allowed |
| Walking | Continuous, at your Speed, not paid from the turn (`RS_WALK_RATE`) | Paid from the turn's feet |

`rs_world_deviations(RS_EVERGLADE_WORLD)` names the five departures: 1.5-second
turns, caster spared by its own areas, team spared by its harmful areas,
unpaid continuous walking, straight-line ranges.

## What changed, and why

Before this profile Everglade declared `RS_UNLIMITED_PRESS`: each press
refilled every slot, use, Focus Point and Rage, and the turn's Action. Nothing
was ever spent, so nothing in the SRD's resource game mattered. The room named
one creature near the aim and a harmful spell struck only it. One flat gap
(0.5 seconds on the room, 1 second on the page) paced every ability alike.
Range counted the grid, so a diagonal reached 41% farther than a row.

- **`RS_UNLIMITED_PRESS` is deleted.** No profile makes anything free.
  `rules/srd/press-budget.coax` keeps only the snapshot a single-player zone
  uses to undo a refused press.
- **No cooldown recovery.** `RS_BEATS_TIMING` has the action profile's
  1.5-second turns and movement grant, and no timed recovery. A spent slot is
  spent until you rest.
- **Areas are real.** `everglade/shared/cast-rules.coax` casts a harmful area
  at the aimed point through the shared `rs_cast`, which resolves `rs_area`
  against every creature. `RS_AREA_SPARE_ALLIES` leaves the caster's team out.
  A Fireball on a wolf pack hits the pack.
- **Each ability has its own pacing.** `everglade/shared/cast-pace.coax` is one
  arithmetic for the room and the page, in whole ticks. An ability takes the
  parts of the turn its Ability record names. The room refuses a press whose
  part is spent (`cooldown`); the page sweeps that ability's icon for exactly
  the time left. Spells stay in the bar and castable; there is no text warning.
- **Ranges are straight lines.** `rules/srd/measure.coax`. Movement still counts
  squares: a step to any of the eight squares round you is 5 feet either way.
- **Reagents are a stock, not a boolean each.** A seat carries the rules' own
  material stock (`everglade/shared/reagent-stock.coax`): what a cast consumed
  stays consumed across a new socket or a level.

The room allows a turn's first press up to 0.5 seconds early, once, so a press
held up on its way and the next on time are both taken. Over any stretch a
seat still gets one turn each 1.5 seconds.

A bow's arrow takes the turn's Action when it is loosed. Its Attack is rolled
when it lands, with that Action, and the turn the archer is in by then keeps
its own.

## The records the profile reads

`rules/srd/ability-record.coax` holds two records.

**Ability** (`rs_ability(kind, spell)`): what of the turn it takes and gives
back, its cost and pool, its casting time beyond the turn's part, its
recharge, how it is aimed, its range and area, whether it can harm, and the
Aura it leaves. A spell's row is read from its spell record. A class feature's
row is in the table in that file. The hotbar's kinds (`FB_*`) are the rules'
kinds (`RS_AB_*`). The hotbar reads cost and count from the record
(`fb_ready`, `fb_left`), the pacing reads the turn's parts (`xp_parts`), and
the area cast reads harm and area (`e2r_area`).

**Aura** (`rs_aura_record(kind)`): for each lasting-effect kind, where its
conditions come from, how it stacks, what ends it besides the clock, and how
long a feature's lasts. A creature bears up to 16 at once (`RS_EFFECTS`): one
from each seat of a raid. Two stacking rules exist: each source's record
stands (and readers ask "any", so two of one name never add), or a source
renews its one record.

Eight things that were a field of their own on every creature are Auras now:
Sacred Weapon, Innate Sorcery, Starry Wisp's light, Reckless Attack, the Slow
mastery, a saved Stunning Strike's half Speed, Faerie Fire's outline and
Produce Flame's flame. Each is one record a source: struck twice by the Slow
mastery, a creature still loses 10 feet. A weapon attack prepays the records
its riders can leave (`rules/srd/strike-admission.coax`): one for Slow, two for
a Stunning Strike. Faerie Fire's outline is a record a caster, so two casters'
outlines on one creature end separately.

Everglade's content table `ability` (`everglade/content/everglade.content`)
has 83 rows since revision 2, one for each ability the bars hold by content
ID. The tables' revision is a session's content pin (#13381). A room on
revision 2 still seats a page on revision 1 for one release
([the previous pins](../protocol/everglade-previous-pins.md), #13408).

## Checks

- The issue's fight: `everglade/shared/cast-rules.coax`, "a level 5 party
  clears a wolf pack with area spells, runs out of slots, rests and goes on".
- Through the room: `everglade/server/room-checks.coax`, "a Fireball at a
  point takes the whole pack and no player, its slot stays spent, and a turn
  holds one Action and one Bonus Action".
- Equal reach: `rules/srd/measure.coax`, "diagonal and straight reach are
  equal under the straight measure".
- Time to kill for every class and level: [encounter-ttk.md](encounter-ttk.md).

## Not in this profile

- A cast bar that can be interrupted. A spell resolves on the press, so the
  page's prediction and the room agree. The wind-up a page draws
  (`FbAb.delay`) is still presentation.
- Per-ability recharges longer than the turn. The record has the field
  (`RsAbility.recharge`); every row is 0 today.
- A Short Rest shorter than the SRD's hour.
