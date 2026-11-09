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
packed as `glade/weapon4`) in its left hand. The Fighter keeps its greatsword for
now.

Character Create's own preview shows no held gear, so the bow appears only in
Experimental.

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
  shows **MISS**, plays a whiz and flies on through.
- **It meets the ground, a wall or a prop:** it sticks with a puff of dust and a
  thunk. It stays for 4 s, then fades out over 1 s.

The Goblins' shortbow arrows are the same arrows in the same pool. Each Goblin
arrow arcs from its hand to where you stood and arrives as the Goblin's attack
resolves (`verse/experimental-hostiles.coax` `hostile_fly`). One that misses
flies on until it sticks in the ground.

## Rules mapping (SRD 5.2.1)

| SRD | Here |
| --- | --- |
| Longbow: 1d8 Piercing, Ammunition (150/600 ft), Heavy, Two-Handed, Slow mastery | `rs_weapon(W_LONGBOW)`. A hit deals 1d8 + your Dexterity modifier, and the attack roll adds Dexterity and your Proficiency Bonus (`rs_weapon_attack`) |
| The Attack action with a ranged weapon | `FB_SHOOT`, which calls `rs_shoot` (`apps/openagents-web/fivebar.coax`). Extra Attack at level 5 makes its second attack too |
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

The Universal Animation Library has no bow clips. The draw and hold use
`ual1_pistol_aim_neutral`, blended in with the aim through the shared upper-body
layer (`fivelab-bodies.coax` `bd_layer`): standing, the whole body takes the aim;
on the move, only the spine, arms and head do, and the walk keeps the legs. The
release is `ual1_pistol_shoot`'s recoil. Bow clips authored on the UAL rig, with the right hand drawing the string to the
cheek, would replace these two library clips.

## How another realm adopts it

1. Keep one `BowDraw` for each archer and one `Arrows` pool
   (`#load "aethyr/action/bow.coax" as bow`).
2. Input: on press, call `bow_begin(draw, now, false)`. On release, take the
   strength from `bow_release(draw, now)`. Cancel with `bow_cancel(draw)`. A key
   or tap calls `bow_begin(draw, now, true)`, and the shot is due when `bow_due`
   says so.
3. Each frame, call `bow_step(draw, dt)`. Feed `bow_aim(draw)` to your camera
   (tighten, shoulder offset), the reticle (`reticle_strokes`), facing and the aim pose, and
   `bow_pace(draw)` to your walk speed.
4. Loose: `arrow_aim_velocity(hand, crosshair_point, bow_speed(1))`, scaled by
   `bow_speed(strength) / bow_speed(1)`, into `arrow_loose`. A ruleset that
   resolves on arrival uses `arrow_timed_velocity(from, to, seconds)`.
5. Each frame, for every flying arrow, take `arrow_segment`, test it against your
   world (`arrow_capsule` for bodies), and end it with `arrow_stop(..., hit)`.
   Then call `arrow_tidy`.
6. Draw each arrow with `fxp_arrow_at` (`aethyr/fx/puff.coax`) between `fxp_begin`
   and `fxp_close`: in flight at `arrow_pos` and `arrow_dir`, and stuck at its
   `at` and `dir` with `arrow_alpha`.

Experimental's adapter is the bow section of
`apps/openagents-web/experimental-combat.coax` (`xc_bow_*`). Its checks are
`coax/bin/coax test --library apps/openagents-web/fivelab.coax --only "experimental combat"`
and `coax/bin/coax test aethyr/action/bow.coax --library`.
