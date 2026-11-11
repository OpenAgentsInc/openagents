# Bow combat

Hold to draw, let go to loose. Bow combat is an action-combat system inspired by
the SRD: you aim with the camera, the arrow is a real projectile, and the SRD
attack roll decides whether it hits the creature it reaches. Everglade
Experimental is the first realm to use it, and the Ranger is the first class
with a bow.

## Who has a bow

The Ranger starts with a Longbow. Its SRD 5.2.1 starting gear includes a Longbow
and a quiver, and its Weapon Mastery covers the Longbow's Slow
(`aethyr/rules/srd/engine.coax` `rs_ranger`: `ranged = W_LONGBOW`). In
Experimental it holds Quaternius' `Bow_Wooden` (`quaternius/ultimate-rpg/bow-wooden`,
packed as `glade/weapon4` without its string) in its left hand. The string is packed
on its own as `glade/weapon5`, and an `Arrow` as `glade/weapon6`. The Fighter keeps
its greatsword for now.

The bow sits on one attach basis in the left fist, the same in every pose
(`everglade/held-kit.coax` `bow_basis`: the held kit is one game-wide file that
Character Create, Experimental, Everglade II and the Archery Range all hold a
weapon through). The rig binds in a T-pose, so in the hand's
bind frame the arm runs +X and the fist's hole, the thumb's side, runs +Z. The
bow's limbs run through the fist along that hole, and its string side runs back
along the forearm, canted 10 degrees out from the body. Drawn, the arm points at
the aim with the thumb up, so the bow stands upright, and it turns about its
limbs until the string faces your chin.

Carried (standing, walking and running), the bow hangs from the relaxed bow
hand as an archer carries one (`bow_carry_basis`). Its back faces ahead, the
way you face. Its string trails behind the bow along the leg, so the limbs
curve forward. The top tilts forward, and the bow sits
partway into the fist (`CC_BOW_CARRY_HOLD`), which keeps the upper limb off
the forearm and the lower limb and string off the striding thigh. As you draw,
the bow turns from the carry to the drawn hold.

A check (`cc_bow_side_sweep`) keeps the string's middle at least 10 cm behind
the bow's grip, along the way you face, in every standing, walking and running
frame. It measures the bow in the figure's space and as every zone places it
(`bow_carry_side`: from a body record, through the frame's kept hand rows, at
four turns), so a string turned ahead of the bow fails. The overlap sweep keeps
the limbs and string out of the body.

The standalone Character Create page shows no held gear. Everglade II's
Character Create and Experimental show the bow.

## Controls

| Input | What it does |
| --- | --- |
| Hold the right mouse button | Draw. A centred crosshair fades in, the camera moves in over your right shoulder, you turn to face where it looks, and you slow to a walk. Each of these follows one eased blend, so nothing pops. Dragging still turns the camera, which is how you aim. |
| Release the right mouse button | Loose the arrow. The camera and crosshair hold for 0.45 s after the loose, so you watch the arrow fly, then ease back. |
| Left mouse button or Escape while drawing | Let the draw down without loosing. |
| The Attack slot, or its number key | A quick shot: the bow draws itself and looses at full draw. |
| Phone: hold the bow's slot | Draw. Drag to aim, lift to loose. A tap is a quick shot, and a cancelled touch looses nothing. |

You can loose early. The draw builds along one curve, `1 - (1 - t/0.8 s)²`, and
the arrow leaves at `42.4 m/s × (0.25 + 0.75 × strength)`. A full draw takes 0.8 s
and lands where the crosshair points. A short draw drops short. The crosshair
closes in as the draw builds and turns gold at full draw.

The crosshair is the shared reticle (`aethyr/cam/reticle.coax`
`reticle_strokes`), the same centred mark a mouselook camera shows while the
pointer is locked. A drawn bow adds its four arms to that mark. Aim always uses
the camera's centre ray, so the draw works the same with the pointer locked or
free: the right button's press and release arrive as button events either way.

## The arrow

An arrow leaves your bow hand toward the point where the crosshair's camera ray
first meets a monster or the ground, within 150 m. If the ray meets nothing, the
arrow aims 60 m along it. It flies a closed-form arc under gravity
(`aethyr/action/bow.coax` `arrow_pos`), so its position is the same at any frame
rate. Each frame, the segment it flew is tested against the ground, the walls and
every monster's hurt capsule.

- **It reaches a monster:** the SRD rolls the attack. A hit ends the arrow in the
  monster, with sparks, a hit sound, the damage number and a short hit stop. A miss
  shows **MISS**, plays a whoosh and flies on through.
- **It crosses a static target's face:** it sticks in the face with a thunk, and
  remembers which target and how far from its middle (see [Targets](#targets)).
- **It meets the ground, a wall or a prop:** it sticks with a puff of dust, a soft
  swish into grass or a thunk into wood or stone. It stays for 4 s, then fades out over 1 s.

The Goblins' shortbow arrows are the same arrows in the same pool. Each Goblin
arrow arcs from its hand to where you stood and arrives as the Goblin's attack
resolves (`verse/experimental-hostiles.coax` `hostile_fly`). One that misses
flies on until it sticks in the ground.

## Targets

A realm can stand static targets: round faces an arrow strikes
(`aethyr/action/target.coax`). A face has a middle, the way it looks and a
radius. The engine answers two questions about a realm's set of faces:

- **Where the crosshair rests.** The crosshair's ray stops on the first face it
  meets from the front (`targets_sight`), so a full draw at a face lands on it,
  however far off or high up it is.
- **What an arrow strikes.** Each frame, an arrow's segment is tested against
  the faces (`targets_first`) beside the ground, the walls and the bodies. The
  arrow sticks in the first face it crosses from the front, inside the rim
  (`arrow_mark`), and keeps the target's index and its distance from the middle.

The engine doesn't score. A realm reads the stuck arrow's `mark` and `off` and
applies its own rule. The [Archery Range](../../everglade/docs/archery-range-spec.md)
scores ten rings.

What holds a face up is the realm's collider. A zone lists the boxes round its
own standing things (`aethyr/world/props.coax`), and the walker, the mounts, the
crosshair's ray and the arrows all read that one list.

## Rules mapping (SRD 5.2.1)

| SRD | Here |
| --- | --- |
| Longbow: 1d8 Piercing, Ammunition (150/600 ft), Heavy, Two-Handed, Slow mastery | `rs_weapon(W_LONGBOW)`. A hit deals 1d8 + your Dexterity modifier, and the attack roll adds Dexterity and your Proficiency Bonus (`rs_weapon_attack`) |
| The Attack action with a ranged weapon | `FB_SHOOT`, which calls `rs_shoot` (`apps/openagents-web/fivebar.coax`). Extra Attack at level 5 makes its second attack too. A bow kit's Attack is this shot in every realm and in the room (`class-kit.coax` `xk_rules`), and it aims at the Longbow's range (`xk_weapon`) |
| Which creature is attacked | The one the arrow physically reaches. The roll decides whether the attack hits |
| Range: normal 150 ft, long 600 ft, Disadvantage past normal range | The target stands on the engine's line at its true distance, one 5 ft square to 1.524 m, the same scale as every other range in Experimental, so normal range is 45.7 m. Past it, `rs_strike`'s own long-range rule gives Disadvantage. The arrow's arc reaches about 183 m at 45°, the long range |
| A ranged attack with an enemy within 5 ft | Disadvantage, from the same rule. The other monsters stand on their squares round you |
| Ammunition | Unlimited. The SRD counts arrows, but Experimental's bar has no consumables, so the quiver never empties |
| Cover | A monster with no line of sight to you is behind Total Cover, as for every other attack in Experimental |

## Multiplayer

Experimental has no room. Every roll is the page's own SRD engine, as for every
other attack there, so bow shots resolve the same way. A realm with a room
resolves the shot on the server: the client sends the loose (the hand, the
velocity and the clock), the room steps the same closed-form arc against its own
bodies and rolls `rs_shoot`, and every client draws the same arrow from the same
launch.

## Animation

No library has bow clips for the Universal Animation Library's rig, so the draw
is keyed in Coax on the rig (`aethyr/action/bow-pose.coax`) and baked once with
the other clips, as clip `BD_BOW` (`fivelab-bodies.coax` `bd_bake_bow`). It has
13 frames: 11 draws from strength 0 to 1, then two of the release. Each frame
starts from the idle stance:

- The chest turns the bow shoulder 0.6 radians toward the aim, and the head turns
  back to it.
- The bow arm reaches straight ahead at shoulder height (two-bone IK), and the bow
  hand turns thumb up.
- The draw hand comes to the string, which is the nock at strength 0, and pulls it
  back to the anchor at the jaw as the strength builds. The elbow goes back and out
  at shoulder height.
- On the release, the draw hand follows through back past the ear and the bow arm
  drops a little.

The frame shown follows the draw strength (`bow_pose_frame`), so a partial draw
is a partial pull. It plays through the shared upper-body layer (`bd_layer`):
standing, the whole body draws; on the move, only the spine, arms and head do,
and the walk keeps the legs.

The string is two pieces of the packed string, one from each tip to the nock
point, so it bends into a V at the draw hand's fingers. On the release it snaps
back straight. From the nock until the release, an arrow sits on the string at
the draw hand and lies along the bow arm past the grip.

While you draw, the camera's boom comes level behind your right shoulder, 0.8 m
out and 0.38 m up, whatever the pitch, and the view still looks along the pitch
(the aim).

## Sound

The bow's sounds are synthesized (`aethyr/audio/experimental-sfx.coax`, CC0, made
by OpenAgents). Each plays at the position it comes from, with a small random
pitch and level, like every other combat sound.

| Sound | What it is |
| --- | --- |
| `bow-draw` | The limbs creaking and the string stretching, over the 0.76 s of a full draw |
| `bow-release` | A plucked-string twang (Karplus-Strong, `SFX_PLUCK`) with a low thump of the limbs, louder for a stronger draw |
| `arrow-whiz` | Band-passed air sweeping down, as an arrow leaves you or flies past |
| `arrow-hit` | A low thunk with a click and the shaft's quiver, into wood or stone |
| `arrow-flesh` | A dull meaty thud and a wet tear, into a creature |
| `arrow-grass` | A soft swish and a muffled tap, into the grass |

## How another realm adopts it

The bow is three layers. A realm writes none of the first two.

| Layer | File | What it owns |
| --- | --- | --- |
| Engine | `aethyr/action/bow.coax` | The draw's curve, the arrows' arcs, the launch |
| Engine | `aethyr/action/archery.coax` | One archer against a world: the frame of a draw, the sight line, each arrow's flight and what it meets, a finger's draw on a phone, the reticle on a draw |
| Engine | `aethyr/action/target.coax` | Static targets |
| Game | `everglade/longbow.coax` | Everglade's Longbow: where the arrow leaves, how far the reticle looks, a creature's body, the mouse buttons, the sounds and the dust |
| Game | `everglade/held-kit.coax` | The bow in the hand, its string, the nocked arrow, the draw pose |
| Game | `everglade/zone-bow.coax` | The bow in an Everglade zone (`everglade/zone.coax`): where the zone's ground and walls are, which monsters stand, and the SRD Longbow attack an arrow that reaches one makes. Experimental and the Archery Range share it |
| Realm | its own file | A realm that isn't a zone (Everglade II) says the same three things itself |

Coax has no callbacks, so the engine's two loops hand the realm a point and take
its answer.

1. Keep one `Archer` (`#load "aethyr/action/archery.coax" as arch`). It holds
   the draw, what the body shows, the arrows and the realm's targets.
2. Input: `eg_bow_button(button, phase, archer)` says whether a mouse button
   draws, looses or lets down. To draw, call `bow_begin(draw, now, false)`; a key
   or a tap passes `true`, and the draw looses itself at full. A finger on the
   bow's slot goes through `archer_touch_down` and `archer_touch`.
3. Each frame, call `archer_draw_step(draw, can, dt, now)`. When it answers
   true, loose. Then call `archer_show_step`. Feed `bow_aim(draw)` to your camera
   (`aethyr/cam/boom.coax`), the reticle (`archery_reticle`), your facing and the
   aim pose, and `bow_pace(draw)` to your walk speed.
4. Aim: start a `SightMarch` with `sight_begin(archery_sight(eye, dir, x, z),
   EG_BOW_LOOK)`. While `sight_next` answers true, set `solid` to whether its
   point `p` is inside your ground or a wall. `archery_rest` then gives the
   point, and the body or the target it rests on.
5. Loose: `archer_release` gives the strength. `archer_loose(archer, owner,
   eg_bow_hand(...), aim.at, strength, aim.body, now)` launches the arrow.
6. Flight: while `flight_arrow` answers true, answer `flight_sample`'s points
   the same way (set `grass` when the point is the ground). `archery_land` says
   how the step ended: stuck in a target (`LAND_MARK`), in the ground or a wall
   (`LAND_STUCK`), or at a body (`LAND_BODY`). For a body, your rules decide,
   and you call `archery_body(archer, flight, landing, hit)`. Then call
   `archery_tidy`.
7. Play `eg_bow_land_cue` and `eg_bow_land_dust` for each landing, and
   `eg_bow_draw_cue`, `eg_bow_release_cue` and `eg_bow_leave_cue` at the draw
   and the loose, through your own sound and effects.
8. Hand the `BowShow` to the held kit (`held_pieces`, `held_bow_layer`): the
   string, the nocked arrow and the draw pose.
9. Draw each arrow: `archery_show(arrows, i, eye_x, eye_z, now)` into
   `fxp_arrow_at` (`aethyr/fx/puff.coax`), between `fxp_begin` and `fxp_close`.

A ruleset that resolves a shot on arrival (Experimental's Goblins) launches
with `arrow_timed_velocity(from, to, seconds)` into the same pool.

Three realms use these modules:

- **Everglade Experimental** and **the Archery Range** shoot through one game
  file, `everglade/zone-bow.coax` (`xc_bow_*`): a zone's ground and walls, its
  monsters as bodies, and the SRD Longbow attack. Each realm's module holds an
  Everglade zone (`everglade/zone.coax`), which loads it.
- **The Archery Range** (`apps/openagents-web/archery-range.coax`, a module of
  its own): it adds no bow code, only its targets, its colliders and its tally.
- **Everglade II** (`apps/openagents-web/everglade2-mouse.coax`): its own mouse
  buttons, and the room resolves the arrow. The draw's frame, what the body
  shows and the sounds are the shared ones.

Check them:

```sh
coax/bin/coax test aethyr/action/archery.coax --library
coax/bin/coax test everglade/longbow.coax --library
COAX_NATIVE_CHECKS=1 coax/bin/coax test apps/openagents-web/fivelab.coax --only "experimental combat"
COAX_NATIVE_CHECKS=1 coax/bin/coax test apps/openagents-web/archery-range.coax --only "archery range"
coax/remote/remote.sh run -- coax/bin/coax test --library apps/openagents-web/everglade2.coax --only "e2 mouse"
```
