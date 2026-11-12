# 33. Let's build a dragon

> **Status: partly implemented.** The model is a script,
> [`scripts/blender/dragon.py`](../../scripts/blender/dragon.py), baked by
> [`verse/tools/assets/dragon_bake.py`](../../verse/tools/assets/dragon_bake.py)
> into the glade's pack. Its clips are
> [`everglade/dragon-clips.coax`](../../everglade/dragon-clips.coax). Flight is
> [`aethyr/action/flight.coax`](../../aethyr/action/flight.coax). The rules are
> [`rules/srd/shapechange.coax`](../../rules/srd/shapechange.coax)
> and [`rules/srd/dragon-actions.coax`](../../rules/srd/dragon-actions.coax).
> The body's change, flight and breath are
> [`everglade/shapeshift.coax`](../../everglade/shapeshift.coax), and the
> field at `/druid` is
> [`everglade/zone-shapechange.coax`](../zone-shapechange.coax).
> Riding the dragon as a mount is a design draft: see
> [`everglade/docs/mounts-spec.md`](../docs/mounts-spec.md).

This chapter answers the player's question: "Can I turn into a dragon and fly?"

Yes. At `/druid` you arrive in an open meadow as a level 17 Druid. Press
Shapechange (Dragon), and a rune circle glows under you, leaves swirl up, a
flash goes off and an Adult Red Dragon stands where you stood. Press Jump to take
off. Hold Fire Breath to aim and let go to breathe.

Each piece of that lives in one place, and every Everglade zone reuses it:

| Piece | Layer | Where |
| --- | --- | --- |
| The dragon's mesh, bones and clips | Asset script | `scripts/blender/dragon.py` |
| Its baked frames in the pack | Asset pipeline | `verse/tools/assets/dragon_bake.py`, `dragon_pack.coax` |
| Walking, takeoff, flight, banking and landing | Engine | `aethyr/action/flight.coax` |
| Shapechange and the dragon's actions | Engine (SRD rules) | `shapechange.coax`, `dragon-actions.coax` |
| The change on screen, the breath's aim, the clip to play | Everglade | `everglade/shapeshift.coax` |
| The change, breath, burning, sweep, buffet and roar effects | Engine | `aethyr/fx/dragon.coax` |
| The druid, the field and the bar | Zone | `everglade/zone-shapechange.coax` |

The zone holds no rules. It places the druid and passes your keys to the
shared layers.

## What this chapter teaches

- How to build a model from a script, with no editor.
- How SRD speeds in feet become a flier's metres a second.
- How Fire Breath's cone, Recharge and Burning work as SRD rules.
- How Shapechange swaps a body and its numbers, and gives them back.
- How the same dragon becomes a mount.

## A model built from a script

The dragon is one Python script that Blender runs headless. Nobody opens an
editor. The script lofts the body, neck and tail as one tube along the spine and
builds the head, legs, horns, back plates and wings from tapered rods and fans.
The faces are flat-shaded, there are 3,260 triangles (the script asserts under
4,000), and each part has one flat colour: a crimson hide, a gold belly, a
wine-dark wing membrane with a warmer underside, and ivory horns and claws.

The skeleton has 27 joints. Twelve are the spine: `body`, `chest`, three neck
bones, `head`, `jaw` and five tail bones. Each wing adds arm, forearm, hand and
four finger bones, and each leg its own chain. The `saddle` bone carries no
mesh; it marks where a rider sits, astride the base of the neck. Then the
script keys each clip on those bones.

The spine and saddle bones, from `scripts/blender/dragon.py`:

```python
# Bones: (name, parent, head). Spine bones sit on the spine; their weights
# along the loft blend between neighbors. `saddle` carries no mesh: it marks
# where a rider sits, astride the base of the neck.
SADDLE = (0, -1.2, 3.62)
BONES = [
    ("body", None, (0, 0.7, 2.55)),
    ("chest", "body", (0, -0.85, 2.8)),
    ("saddle", "chest", SADDLE),
    ("neck1", "chest", (0, -1.55, 3.1)),
    ("neck2", "neck1", (0, -2.05, 3.6)),
    ("neck3", "neck2", (0, -2.5, 4.2)),
    ("head", "neck3", (0, -2.95, 5.0)),
    ("jaw", "head", (0, -3.1, 4.88)),
    ("tail1", "body", (0, 2.3, 2.36)),
    ("tail2", "tail1", (0, 3.9, 1.9)),
    ("tail3", "tail2", (0, 5.5, 1.4)),
    ("tail4", "tail3", (0, 6.7, 1.12)),
    ("tail5", "tail4", (0, 7.6, 0.98)),
]
```

Each bone is a name, its parent and where its head sits, in metres, with the
dragon facing -Y.

Why a script? You can read it, diff it and review it, and the same script
gives the same dragon on every machine. A change such as "make the wings wider"
is one number, and the preview renders again in seconds.

The previews from three angles are in
[`docs/verification/druid-demo-2026-11-09/`](../../docs/verification/druid-demo-2026-11-09/):
`dragon-model-front.png`, `dragon-model-side.png` and `dragon-model-below.png`.

The clips are idle, walk, fly, glide, bite, breath, roar and sweep for the
druid, and idle_saddled, takeoff, hover, land, bank_left and bank_right for a
mount. The baker evaluates each clip's frames and writes them in that order.
`dragon-clips.coax` holds the same order, so frame `n` of the pack is frame `n`
of the clips:

- `everglade/dragon-clips.coax`, `#test "the dragon's clips cover every baked
  frame once, in order"`.
- `everglade/dragon-clips.coax`, `#test "a loop goes round; a clip played once
  holds its last pose"`.

The rider's place is the `saddle` bone at rest, `DR_SADDLE_X`, `DR_SADDLE_Y`
and `DR_SADDLE_Z` in `dragon-clips.coax`.

## Flight from SRD speeds

The Adult Red Dragon's block gives Speed 40 feet and Fly 80 feet. A realm has
its own pace: how many metres a second a 30-foot Speed walks. The flight
profile converts both speeds against that pace, so the dragon flies twice as
fast as it walks in every realm:

<!-- source: aethyr/action/flight.coax wing-profile-srd -->
```jai
// A profile from SRD speeds in feet: walk_ft on the ground, fly_ft in the air, against the realm's
// pace (pace_mps metres a second for a 30-ft Speed).
wing_profile_srd :: (walk_ft: int, fly_ft: int, pace_mps: f32) -> WingProfile #effects(all) {
  p: WingProfile;
  p.walk = pace_mps * f32(walk_ft) / 30.0;
  p.fly = pace_mps * f32(fly_ft) / 30.0;
  p.accel = p.fly * 0.75;
  p.vertical.on = true; p.vertical.ceiling = 60.0;
  p.vertical.flap_up = 6.0; p.vertical.climb = 4.0; p.vertical.glide = -1.2; p.vertical.dive = -12.0;
  return p;
}
```

A body is in one of four phases. On the ground it walks. A jump starts the
takeoff: lift for 0.8 seconds, then free flight. In the air, a held jump beats
the wings and climbs, letting go glides down slowly, and a dive drops fast and
speeds up. When the body reaches the ground it lands, settles for 0.6 seconds
and walks again. Turning in the air leans the body into the turn. Each phase
picks a clip: walk, takeoff, fly, glide, hover, bank_left, bank_right or land.

The druid's dragon and a ridden dragon both call `wing_step`. There is one
flight module, not one per zone.

## The dragon's actions as SRD rules

Fire Breath is a 60-foot cone. Each creature in it makes a DC 21 Dexterity
saving throw and takes 17d6 Fire damage on a failure, half on a success. A
cone's width at any distance equals that distance, so a point is inside when
it is no further to the side than half the distance along the aim:

<!-- source: rules/srd/dragon-actions.coax breath-cone -->
```jai
// Whether point (dx, dy) from a cone's origin is inside a cone of `length` aimed along (fx, fy),
// any units so long as they're the same: no further than length along the aim, and no further to
// the side than half the distance along it.
rs_in_cone :: (dx: f32, dy: f32, fx: f32, fy: f32, length: f32) -> bool #effects(all) {
  n := sqrt(fx * fx + fy * fy);
  if !(n > 0) || !(length > 0) { return false; }
  ux := fx / n; uy := fy / n;
  along := dx * ux + dy * uy;
  side := abs(dx * uy - dy * ux);
  return along >= 0 && along <= length && side * 2 <= along + 0.0001;
}
```

The same test in three dimensions, `rs_in_cone3`, lets a flier breathe down
onto the ground. A world measures which creatures are in the cone and passes
them to `rs_fire_breath_on`. The engine rolls the saves and the damage.

After a breath, Fire Breath is spent. It has Recharge 5–6: at the start of each
of the dragon's turns, a d6 roll of 5 or 6 brings it back. Burning is a house
rule: a creature that fails the save burns for 1d4 Fire at the start of each of
its turns for three rounds. Both run from the engine's turn start, so any world
that steps turns gets them:

<!-- source: rules/srd/dragon-actions.coax recharge-burning -->
```jai
// The start of i's turn: Burning, then the Recharge roll.
rs_dragon_turn_start :: (b: [] RsBattle, i: int) #effects(all) {
  if b[0].c[i].burning > 0 { _ := rs_burning_tick(b, i); }
  if rs_breather(b[0].c[i]) && b[0].c[i].breath_spent && rs_recharges(rs_roll(b[0].dice[..], 6)) { b[0].c[i].breath_spent = false; }
}
```

The dragon's other actions are Rend (Multiattack: three Rends), and three
house actions: Tail Sweep, Wing Buffet and Frightful Roar. They use the SRD's
save DC formula, 8 plus the ability modifier plus the Proficiency Bonus. The
[compliance ledger](../../docs/dnd/srd-compliance-ledger.md) lists each house
rule.

## The druid's Shapechange

Shapechange is a 9th-level spell, so the druid is level 17. Casting it takes an
Action, the 9th-level slot, Concentration and a jade circlet. The form must not
be Undead, and its Challenge Rating must be no higher than your level:

<!-- source: rules/srd/shapechange.coax shapechange -->
```jai
// The acting i casts Shapechange with a 9th-level slot and takes block m's form (its Action, the slot,
// Concentration). A Wild Shape form ends first.
rs_shapechange :: (b: [] RsBattle, i: int, m: int) -> bool #effects(all) {
  if i < 0 || i >= b[0].n || !rs_shape_eligible(b[0].c[i], m) || rs_shape_keep_at(b, i) < 0 { return false; }
  if b[0].c[i].wild_form != 0 { rs_wild_shape_end(b, i, " leaves its Beast form"); }
  if !rs_cast(b, i, SP_SHAPECHANGE, 9, b[0].c[i].x, b[0].c[i].y, i) { return false; }
  rs_shape_take(b, i, m);
  b[0].c[i].temp = rs_max(b[0].c[i].temp, rs_monster_block(m).hp);
  rs_say(b, RS_EV_CONDITION, i, i, m, " takes a new shape with Shapechange");
  return true;
}
```

You keep your Intelligence, Wisdom and Charisma, your Hit Points and your saving
throw proficiencies. You take the dragon's Armor Class, speeds, Strength,
Dexterity, Constitution, size and Fire immunity, and its 256 Hit Points as
Temporary Hit Points. When Concentration ends, or you press Return to Form,
everything comes back.

Your own numbers wait in the battle's side table, `shape_keeps`, not on every
creature. Only the few creatures that cast Shapechange need them, and a battle
holds `RS_SHAPE_KEEPS` (4) at once. A fifth caster's Shapechange is refused
before it spends a slot:

- `rules/srd/shapechange.coax`, `#test "Shapechange keeps the caster's
  own numbers in the battle's side table, and refuses when the table is full"`.

The zone doesn't decide which body to draw. It reads the engine's creature and
draws that:

<!-- source: everglade/zone-shapechange.coax druid-form -->
```jai
// The engine's body, as the shapeshift layer's kinds.
xd_form :: (c: RsCreature) -> int { return ifx c.shape_form == MON_ADULT_RED_DRAGON then SH_DRAGON else ifx c.wild_form != 0 then SH_WOLF else SH_DRUID; }
```

So the picture can't disagree with the rules. If a monster breaks your
Concentration, the dragon becomes a druid again on the next frame.

## Riding it as a mount

> **Design draft.** The mounts are being built now.

A dragon in Everglade Experimental is a mount as the horse is. You mount it,
sit at its `saddle` bone and fly it with the same `wing_step`. Your request to
mount, to fly or to breathe goes through the deed pipeline like any other
action: the world asks the rules first, then acts, then shows the result
([`docs/verse/action-pipeline.md`](../../docs/verse/action-pipeline.md)). Fire
Breath from the saddle is the same `rs_fire_breath_on`. The rider aims, and the
mount's creature breathes. [`everglade/docs/mounts-spec.md`](../docs/mounts-spec.md)
has the SRD's mounted-combat rules that the mounts follow.

## The checks

- `aethyr/action/flight.coax`, `#test "SRD speeds: an Adult Red Dragon walks 40
  ft and flies 80 ft against a 30-ft pace"`, `#test "flight is faster than
  walking: the dragon covers much more ground in the air"`, `#test "climb,
  glide and dive: a held jump climbs, letting go glides down, a dive drops
  faster and speeds up"` and `#test "it lands: gliding or pressing land reaches
  the ground, settles, then walks"`.
- `rules/srd/battle.coax` runs `dragon-actions.coax`'s `#test "the cone:
  60 feet long, a half-angle of atan(1/2); in front and inside, never behind or
  wide"`, `#test "Fire Breath: DC 21 Dexterity, 17d6 Fire to each foe in the
  cone, half on a success; then it's spent"`, `#test "Recharge 5–6: a 5 or a 6
  at the start of its turn brings Fire Breath back"` and `#test "Burning: 1d4
  Fire at the start of each turn for three rounds; an action Prone puts it
  out"`.
- `rules/srd/battle.coax` runs `shapechange.coax`'s `#test "Shapechange:
  a level 17 Druid becomes an Adult Red Dragon with its 9th-level slot, Action
  and Concentration"` and `#test "Shapechange ends with Concentration and gives
  everything back; its Temporary Hit Points go"`.
- `everglade/shapeshift.coax`, `#test "the fire cone: inside 18 m and its angle
  it hits, outside it misses; a flier aims down"`.
- `apps/openagents-web/fivelab.coax` runs `everglade/zone-shapechange.coax`'s `#test "druid:
  Fire Breath held and let go burns the monsters in its cone and none behind"`
  and `#test "druid: the dragon takes off with Jump, flies faster than it walks,
  and glides down to land"`.
- `aethyr/fx/puff.coax` runs `dragon.coax`'s `#test "dragon: the breath's fire
  stays inside its cone, half as wide as it is far"`.

```sh
coax/bin/coax test --library aethyr/action/flight.coax
coax/bin/coax test --library rules/srd/battle.coax --only "Shapechange"
coax/bin/coax test --library everglade/shapeshift.coax
coax/bin/coax test --library apps/openagents-web/fivelab.coax --only "druid"
```

The headless captures are in
[`docs/verification/druid-demo-2026-11-09/`](../../docs/verification/druid-demo-2026-11-09/).

## Mistakes

- **A full spell list.** The engine's level 17 Druid already had as many
  prepared spells as it may, so Shapechange couldn't be prepared and the first
  cast failed. `rs_shapechange_ready` now lets the highest prepared spell step
  out for it.
- **A push that went through a body.** The first Wing Buffet check expected a
  creature to slide 15 feet back. Another creature stood behind it. The engine
  was right: a push stops at an occupied square, and the check now says so.
- **Fire on an Imp.** Imps are immune to Fire, so the first breath check burned
  nothing. The check now picks creatures that fire harms.

## What comes next

- A Wild Shape body: today the druid keeps its own body as a wolf.
- Riding the dragon in Experimental, with Fire Breath from the saddle.
