# The Archery Range (spec)

Written 2026-11-10. Status: **built, version 2**, as a realm on the web at
`/worlds/archery-range`, in a module of its own, `archery-range` (section 9).
Version 1 was a scene written for the Everglade apps alone, with its own draw
key, aim, camera, reticle, hit test and mounts. It is deleted. Version 2 is
built only from the systems the game already has.

The range is a place to practise the Ranger's Longbow: shoot at faces for
points, see how accurate you are, and do it at different distances, on
foot, from horseback and from dragonback.

## Contents

1. [What you do](#1-what-you-do)
2. [The ground](#2-the-ground)
3. [Scoring](#3-scoring)
4. [The bow](#4-the-bow)
5. [Three ways of shooting](#5-three-ways-of-shooting)
6. [On a phone](#6-on-a-phone)
7. [The tally](#7-the-tally)
8. [What it is built from](#8-what-it-is-built-from)
9. [The realm and the apps](#9-the-realm-and-the-apps)
10. [What is not done](#10-what-is-not-done)
11. [Next](#11-next)

## 1. What you do

You arrive as a Ranger on an open meadow, behind a chalk shooting line,
facing five straw butts. Hold the right mouse button to draw, aim with the
crosshair and let go to loose. The arrow flies a real arc and sticks where it
lands. An arrow in a face shows its points over the face. Walk anywhere:
closer for an easy shot, further back for a hard one. Take the horse along
the line and shoot from the saddle, or take the dragon up and shoot from
the air.

## 2. The ground

All numbers are in [`everglade/range/layout.coax`](../range/layout.coax).
The rules, the mesh and the realm all read that one file.

| Thing | Where |
| --- | --- |
| The shooting line | 44 m of chalk dashes, its middle at (0, 35) on the meadow. You shoot toward -z |
| Five lanes | Butts at 10, 20, 30, 50 and 70 m from the line, 8 m apart |
| Face size | 122 cm across on the near three, 160 cm at 50 m, 200 cm at 70 m |
| Two sky faces | On poles 14 m and 20 m up, leaning back, for a shot from dragonback |
| Distance posts | One at each lane's left, with a red band for each lane out (one to five) |
| Where you arrive | 3 m behind the line, in the middle |
| The horse | Waits 8 m behind the line |
| The dragon | Waits 15 m behind the line |

The range is a clearing: the meadow scatters no rocks from 34 m either side of
the middle, from 78 m downrange to 24 m behind the line (`RG_CLEAR`). The lanes,
the line and where the mounts wait are open grass and flowers. A few rocks
edge it: a quarter of the meadow's, in a band 9 m wide. Past the band the
range's meadow has no rocks. Experimental's and the Druid's meadows are
unchanged.

Every piece stands on the meadow's own ground under it. There is no fence:
the meadow is open, and its edge is the realm's.

Each butt, pole and post has a collider the size of what you see. You walk
into it, a mount goes round it, and an arrow sticks in it. The chalk has
none.

## 3. Scoring

A face is the standard ten-ring target.

| Rings | Colour | Points |
| --- | --- | --- |
| The two innermost | Gold | 10, 9 |
| Next two | Red | 8, 7 |
| Next two | Blue | 6, 5 |
| Next two | Black | 4, 3 |
| The two outermost | White | 2, 1 |

Each ring is a tenth of the face's radius. An arrow on a line scores the
ring inside it. An arrow scores only when it crosses the face from the
front. An arrow into the straw beside the face, a post, a pole or the
grass, or one that flies for six seconds without landing, is a miss.

There are no dice. Where the arrow goes is the bow's physics alone.

## 4. The bow

The bow is bow combat's ([`docs/verse/bow-combat.md`](../../docs/verse/bow-combat.md)),
unchanged: the same controls, crosshair, over-the-shoulder camera, draw
pose, bow model, string, arrows and sounds as Everglade Experimental.

| Input | What it does |
| --- | --- |
| Hold the right mouse button | Draw. Let go to loose |
| Left mouse button or Escape while drawing | Let the draw down |
| The Attack slot, or its number key | A quick shot at full draw |
| Phone: hold the bow's slot | Draw. Drag to aim, lift to loose. A tap is a quick shot |

A full draw takes 0.8 s and lands where the crosshair points. A short draw
leaves slower and drops short. The crosshair rests on a face it points at,
so a full draw at a face 70 m off or 20 m up lands on it.

## 5. Three ways of shooting

Riding is Everglade's mounts ([mounts spec](mounts-spec.md)), unchanged.

| Way | How it moves |
| --- | --- |
| On foot | The shared walking controller |
| From horseback | E mounts. W and S go and slow, A and D turn, Shift gallops at 14 m/s, Space jumps |
| From dragonback | E mounts. Space takes off and climbs, Shift dives, X lands |

## 6. On a phone

A touch screen has the same range, in a phone's browser and in the iPhone
app. The controls come up at the first touch (the iPhone app shows them from
the start).

| Touch | What it does |
| --- | --- |
| Left thumb, anywhere on the left side between the buttons and the bar | The stick. On foot a small push walks and a full push runs. In the saddle it steers, and a full push ahead gallops |
| Drag anywhere else | Look |
| Hold the bow's slot (Attack), drag, lift | Draw, aim, loose. A tap is a quick shot |
| The tally's line (top left) | On an upright phone the tally is one line: how you shoot, hits and points. A tap opens the whole tally with FOOT, HORSE, DRAGON and RESET under it; a tap on the line again, or on any of them, folds it |
| FOOT, HORSE, DRAGON (under the open tally) | How you shoot. HORSE and DRAGON put you beside that mount and mount it. FOOT gets you off; in the air the dragon lands first |
| RESET (under the open tally) | Clears the tally. On foot it also puts you back behind the line |
| MORE (the bar's last slot, upright) | The bar is one row on an upright phone: the slots that fit, then MORE. MORE opens the rest in a row over it; LESS, or using one of them, shuts it |
| GYRO (right side, over JUMP) | Its menu: the gyro on or off, while aiming or always, its speed (0.5x to 3x) and up-down normal or inverted. Your choice is kept |
| Turn the phone | While the bow is drawn (or always, if you choose), turning the phone turns your aim: left and right, and tilting its top back looks down. Lifting the finger recentres it. On an iPhone's Safari the first touch asks for motion; if you refuse, the menu says MOTION NOT ALLOWED. A phone with no gyro shows no GYRO |
| JUMP (right side, over MOUNT) | Jump, on foot and on the horse |
| UP (right side, on the dragon) | Held, the dragon takes off and climbs |
| DOWN (right side, while the dragon flies) | The dragon lands |
| MOUNT, DISMOUNT (right side, beside a mount) | The same as E |

The stick is the engine's (`metaverse-engine/core/sticks.coax`), the one Everglade II walks
on. Which finger is the thumb's, and JUMP, UP and DOWN, are the zone's
([`everglade/zone-touch.coax`](../zone-touch.coax)), so Everglade
Experimental has them too. FOOT, HORSE, DRAGON and RESET are the range's. A
button only asks: the range then presses the keys a player would (X to land,
E to get off, E beside the mount).

On an upright phone nothing else covers the world: no class plate over the
bar (your portrait says the class), and the graphics, frames-a-second and
place lines show only with `?net`. A phone on its side, a tablet and a desktop
keep the full tally, both rows of the bar and those lines.

A slot's card shows while a finger holds the slot, and goes when it lifts. The
bow's slot shows none, because holding it is the draw.

## 7. The tally

Shown at the top left, where other realms show the class picker. The range
has one class, so it has no picker. It is kept until you leave the page.

- **Overall:** arrows, hits, accuracy (hits in a hundred), points, your
  run of hits in a row and the longest, golds (9s and 10s), and points an
  arrow.
- **By distance,** for the way you are shooting now: hits out of arrows,
  accuracy and points for each band you have shot in. The bands are up to
  15 m, 25 m, 40 m, 60 m, and beyond. A hit's distance is from where you
  loosed to the face it struck, so walking back from the 30 m butt moves
  your shots into a further band. A miss counts at the distance of the face
  nearest where the arrow came down.
- **The last arrow:** its points, or MISS, and its distance.
- Each way of shooting keeps its own figures.

## 8. What it is built from

Nothing about drawing, aiming, flying arrows, hitting or riding is written
in the range. The range's module is an Everglade zone with the range on it.

| Piece | Where | Layer |
| --- | --- | --- |
| The draw, the arrow's arc, the launch | [`metaverse-engine/action/bow.coax`](../../metaverse-engine/action/bow.coax) | Engine |
| The archer: the frame of a draw, the reticle's sight line, each arrow's flight and what it meets, a finger's draw, the reticle on a draw | [`metaverse-engine/action/archery.coax`](../../metaverse-engine/action/archery.coax) | Engine |
| A static target a projectile strikes | [`metaverse-engine/action/target.coax`](../../metaverse-engine/action/target.coax) | Engine |
| A zone's own colliders | [`metaverse-engine/world/props.coax`](../../metaverse-engine/world/props.coax) | Engine |
| The crosshair, the camera over the shoulder, the walker | `metaverse-engine/cam/reticle.coax`, `metaverse-engine/cam/boom.coax`, `metaverse-engine/move/walk.coax` | Engine |
| Mounting, riding and flying | `metaverse-engine/action/mount.coax`, `metaverse-engine/action/flight.coax`, `metaverse-engine/action/ride-pose.coax` | Engine |
| The butts' and faces' meshes | [`metaverse-engine/world/build-site-mesh.coax`](../../metaverse-engine/world/build-site-mesh.coax) | Engine |
| The HUD panel and hit numbers | `metaverse-engine/ui/hud-menu.coax`, `metaverse-engine/action/floats.coax` | Engine |
| **The zone**: one player's world. Its state (`EgZone`), your body's step, the keys and the pointer, the frame, the host's messages | [`everglade/zone.coax`](../zone.coax) | Game: moved out of `fivelab.coax` |
| The bow in a zone: its ground and walls, its monsters as bodies, the SRD Longbow attack, the draw and the loose | [`everglade/zone-bow.coax`](../zone-bow.coax) | Game: moved out of `everglade/zone-combat.coax` |
| Riding in a zone: E, the saddle, the dragon's Fire Breath | [`everglade/zone-ride.coax`](../zone-ride.coax) | Game: moved out of `experimental-ride.coax` |
| The agent driver in a zone | [`everglade/zone-drive.coax`](../zone-drive.coax) | Game: moved from `fivelab-drive.coax` |
| The Longbow's feel: where the arrow leaves, how far the reticle looks, the mouse buttons, the sounds and the dust | [`everglade/longbow.coax`](../longbow.coax) | Game |
| The bow in the hand, its string, the draw pose | [`everglade/held-kit.coax`](../held-kit.coax) | Game |
| The horse and the red dragon, and the rider's keys | [`everglade/mounts.coax`](../mounts.coax) | Game |
| The mounts standing in a zone: E, the saddle, the prompt | [`everglade/zone-mounts.coax`](../zone-mounts.coax) | Game |
| The scene: the meadow, your Ranger, the mounts drawn | `everglade/scene/charactercreator.coax` | App directory (section 9) |
| The Ranger's bar and the Longbow's rules | `everglade/zone-combat.coax`, `class-kit.coax`, `rules/srd/` | App directory (section 9) |
| Where things are | [`everglade/range/layout.coax`](../range/layout.coax) | The range |
| Ring points and the tally | [`everglade/range/score.coax`](../range/score.coax) | The range |
| The pieces' meshes and colliders | [`everglade/range/place.coax`](../range/place.coax) | The range |
| The realm: arriving, standing the range, following each arrow, the tally's panel, the module's frame and messages | [`apps/openagents-web/archery-range.coax`](../../apps/openagents-web/archery-range.coax) | The range |

The range's mesh is one entry in the glade pack (`C glade/range`), added
with:

```sh
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax --append-range IN.pack OUT.pack
```

Everglade Experimental and the range run the same zone: the same walker, bow,
riding, bar and HUD, from the same files. Experimental's module
(`fivelab.coax`) holds an `EgZone` beside the 5E Lab's tabletop and adds what
is Experimental's alone: its labels, film stages and local test addresses.
Everglade II uses the archer's frame and the Longbow's sounds, keeps its own
mouse buttons, and its room resolves the arrow.

Check the Coax:

```sh
coax/bin/coax test --library metaverse-engine/action/archery.coax
coax/bin/coax test --library everglade/longbow.coax
coax/bin/coax test --library everglade/mounts.coax
coax/bin/coax test --library metaverse-engine/action/target.coax
coax/bin/coax test --library metaverse-engine/world/props.coax
coax/bin/coax test --library everglade/range/score.coax
coax/bin/coax test --library everglade/range/place.coax
COAX_NATIVE_CHECKS=1 coax/bin/coax test apps/openagents-web/archery-range.coax   # the zone's checks and the range's four, about 10 s
COAX_NATIVE_CHECKS=1 coax/bin/coax test apps/openagents-web/fivelab.coax         # Experimental on the same zone
```

## 9. The realm and the apps

The Archery Range is a realm of its own in `apps/openagents-web/realms.json`:
its own page (`/worlds/archery-range`), save slot and row. It is unlisted, as
Everglade Experimental and Everglade II are.

It is also a module of its own, `archery-range`:

| File | What |
| --- | --- |
| [`apps/openagents-web/archery-range.coax`](../../apps/openagents-web/archery-range.coax) | The module's root: its state (`Range`: one `EgZone` and the range), `range_frame`, `range_message`, and the range |
| [`apps/openagents-web/archery-range-face.c`](../../apps/openagents-web/archery-range-face.c) | Its C face. It hands the host's calls to the zone and decides nothing |
| `apps/openagents-web/build.sh` | Builds `build/archery-range.wasm` with `coax/web/wasm.sh`, beside `fivelab.wasm` |
| `apps/openagents-web/wasm-budgets.txt` | Its size budget |

The module has none of the 5E Lab's tabletop: no battle of armies, class
turns, siege, Warlock, film or sliders. It draws with Character Create's passes
(`charactercreator.wgsl`, which `metaverse-engine/render/page-render.coax` names for the
page) and the glade's effects (`fivelab-fx.wgsl`), and loads the glade pack, as
Experimental does.

The module's checks run as native code, as every unit's do
([native checks](../../coax/docs/native-checks.md)). They are the zone's checks,
which fivelab also runs, and the range's four.

In the Mac app the world list's row for the Archery Range opens this realm
natively, through the OpenAgents Metaverse Engine's page host, with its sound
([app scenes spec](app-scenes-spec.md), section 4). The app builds the module
as the site does (`verse/native/build.sh archery-range OUT` takes the module,
its face and the libraries `build.sh` links it with) into
`libarchery-range.dylib`, and the realm table (`Realms.swift`) names it. Its
pack is Experimental's, so the app keeps one copy. The range and Experimental
are two libraries now, so both can be open. The iPhone app's build has the same
lines.

## 10. What is not done

- **The scene and the class bar still live in the app directory.**
  `everglade/scene/charactercreator.coax` (the meadow, your figure, the
  mounts drawn) and `everglade/zone-combat.coax` (the class
  bar and the SRD rules against the monsters) are loaded by
  `everglade/zone.coax` from where they are. They belong in `everglade/`. Each
  is tied to a page: the scene to Character Create's studio, the combat to
  Experimental's twelve classes and its monsters.
- **The range carries more than it uses.** Its module holds the whole zone:
  every class's bar, the monsters (the range stands none and marks them dead),
  the village's and the dungeon's walls, Shapechange. The module is about the
  size of fivelab's. It shrinks when the zone's parts can be loaded one at a
  time.
- **Experimental has no module of its own.** It and the Druid's meadow still
  build into `fivelab` with the 5E Lab and Character Create. They can move to a
  module on the zone the way the range did.
- **Fire Breath** is in `everglade/zone-ride.coax` now. **The film stages**
  stay in `apps/openagents-web/experimental-stages.coax`: they are
  Experimental's own.
- **Phones.** The touch controls were driven in headless Chrome at a phone's
  size and in the iPhone simulator. Nobody has held a phone with them yet. On
  a phone on its side the left side is short: the ring that shows where a
  thumb goes is drawn only once a thumb is down.
- **Mutation.** `coax mutate` has not been run on the new engine and game
  files.

## 11. Next

1. Rounds: a fixed number of arrows at one distance with a final score, and
   your best kept.
2. The distance on each post as a number.
3. Moving faces, and a gallop course along the line with faces on both sides.
4. Wind that pushes the arrow.
