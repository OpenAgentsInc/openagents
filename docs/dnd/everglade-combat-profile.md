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

**No class has a field of its own on a creature** (#13285). What a class used
to keep there is in four shared places on `RsCreature`
(`rules/srd/engine.coax`):

| What | Where | Read with |
| --- | --- | --- |
| What a class, a subclass or a stat block knows (58 features: Cunning Action, Sculpt Spells, Parry) | `feats`, one bit a feature by `RS_FT_*` | `rs_has_feature(c, RS_FT_SCULPT)`; set with `rs_feature_put` |
| A feature's number (the Martial Arts die, Sneak Attack's dice, Rage's damage bonus, the cantrip an invocation was chosen for) | `rank[RS_RK_*]` | the array |
| Every counted resource. Rages, Focus Points and a Healer's Kit's uses are pools 14 to 16 beside the 14 there were | `uses[RS_USE_*]`, `uses_max`, `uses_back` | An Ability's cost names its pool, so `RS_COST_USE` is the only counted cost besides a spell slot |
| State that an event ends: Rage (with its "kept up this round" and "struck this round" bits), Dodge, Shield, the Open Hand's Addle, a Bardic Inspiration die held | A state Aura: one record no clock ends (`EF_RAGE`, `EF_DODGE`, `EF_SHIELD`, `EF_ADDLED`, `EF_INSPIRED`) | `rs_effect(c, EF_RAGE)`, `rs_state_has`, `rs_state_value`; set with `rs_state_put` |

A state Aura takes a place in the creature's list of 16, so what takes one
asks for room first (`rs_state_room`): Rage, Dodge, Patient Defense, Shield
and Bardic Inspiration are refused before any cost when the list is full. A
state takes the last free place, and a spell's records fill from the front.

A Short Rest gives a Barbarian one Rage and a Monk every Focus Point through
the same table as Channel Divinity and Second Wind (`rs_short_rest_mode`). The
action timing profile brings them back through the one loop over every pool.
A character checkpoint carries pools 0 to 13 as before (`RS_USES_KEPT`);
Rages and Focus come back full on a restore, as they did as fields.

What a feature or a spell keeps between events is `note[RS_NT_*]`: the effect
planned for Cunning Strike, the wish for Stunning Strike on the next hit, a
curse free to move and the turn after which it may, Wild Resurgence's slot made,
and the turn a creature last saved against an Emanation. The spell that
`RS_USE_FREE` casts without a slot is `rank[RS_RK_FREE_SPELL]`. The slot a
Concentration spell was cast with is `conc_slot`, for any spell. The Wild
Shape, familiar and Zone of Truth groups are still fields.

**A spell is asked by its record, not by its name** (#13285). What only one
spell did is a field of `RsSpell` (`rules/srd/spells.coax`) or a step:

| What | Field or step | First spell that sets it |
| --- | --- | --- |
| A creature with a feature casts it with no slot, its dice at their highest | `at_will` (an `RS_FT_*`) | False Life, with Fiendish Vigor |
| A target other than the caster must be willing | `willing` | Guidance, Protection from Evil and Good |
| What the request chooses | `option` (`RS_OPT_*`) | Blindness/Deafness, Hex, Lesser Restoration, Guidance |
| It lasts longer from a higher slot | `lasts_from` | Hex |
| Its casting time beyond the Magic action | `cast_s` | Find Familiar |
| Shield stops it | `shield_stops` | Magic Missile |
| Its hit sets things burning | `ignites` | Fire Bolt |
| An Emanation round its caster | the `RS_S_GUARD` step, read by `rs_emanation` | Spirit Guardians |
| A part needs its parent still there | `part_of`, read by `rs_part_live` | Produce Flame's hurl, Call Lightning's bolts |
| Its Aura moves when its bearer drops | the Aura's record, `rs_aura_moves` | Hex, Hunter's Mark |

Ten `== SP_` comparisons are left outside the data files and the checks. Each
has a comment: five in `class-features.coax` ask whether a built creature holds
the spell its feature names, four are a spell's own procedure asking for its
own spell (Shapechange, Find Familiar), and one is in a check.

**A class feature is a row of steps** (`rules/srd/battle-features.coax`,
`rs_feature`). `rs_feature_use` runs any row: every need first (its turn's
part, the feature known, a use in its pool, a target in reach), and nothing is
spent unless all hold; then what it spends, the line it tells and what it
does, in order. Rage, Second Wind, Action Surge, Wholeness of Body, Lay On
Hands, Sacred Weapon, Steady Aim, Innate Sorcery, Bardic Inspiration, Font of
Inspiration, both ways of Wild Resurgence, Turn Undead, Preserve Life and
Land's Aid are rows. `rs_rage` and the rest are one line each: the row's name.
A feature that uses the steps there is a row and nothing else. Still
procedures: the Attack action and what rides on a strike (`battle-strike.coax`),
and the actions that have a file of their own (Dash, Disengage, Dodge, Hide,
Wild Shape, the dragon's).

**A battle has no room of its own** (#13285). `RsBattle` holds views of its
creatures, squares and lasting areas. Whoever keeps a battle keeps them beside
it: a check or a small world declares an `RsRoom` and gives it with `rs_have`;
a zone gives its own tables with `rs_use` and `rs_use_zones`.

**Events name the rules' own IDs** (#13286). `RsEvent.spell` is an `RsSpellId`
and `RsEvent.by` and `.to` are `RsActorId`. The rules never read a world's
content: a spell's ID is its record's number, which never changes, and
Everglade's registry maps its `spell` rows onto the same numbers. A world
reads `.value` where the event leaves the rules.

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
