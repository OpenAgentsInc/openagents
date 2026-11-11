# The Archery Range (spec)

Written 2026-11-10. Status: **built, version 2**, as a realm on the web at
`/worlds/archery-range`. Version 1 was a scene written for the Everglade apps
alone, with its own draw key, aim, camera, reticle, hit test and mounts. It
is deleted. Version 2 is built only from the systems the game already has.

The range is a place to practise the Ranger's Longbow: shoot at faces for
points, see how accurate you are, and do it at different distances, on
foot, from horseback and from dragonback.

## Contents

1. [What you do](#1-what-you-do)
2. [The ground](#2-the-ground)
3. [Scoring](#3-scoring)
4. [The bow](#4-the-bow)
5. [Three ways of shooting](#5-three-ways-of-shooting)
6. [The tally](#6-the-tally)
7. [What it is built from](#7-what-it-is-built-from)
8. [The realm and the apps](#8-the-realm-and-the-apps)
9. [What is not checked](#9-what-is-not-checked)
10. [Next](#10-next)

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
| From horseback | E mounts. W and S go and slow, A and D turn, Shift gallops at 14 m/s |
| From dragonback | E mounts. Space takes off and climbs, Shift dives, X lands |

## 6. The tally

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

## 7. What it is built from

Nothing about drawing, aiming, flying arrows, hitting or riding is written
in the range.

| Piece | Where | Layer |
| --- | --- | --- |
| The draw, the arrow's arc, the launch | [`aethyr/action/bow.coax`](../../aethyr/action/bow.coax) | Engine, existing |
| A static target a projectile strikes | [`aethyr/action/target.coax`](../../aethyr/action/target.coax) | Engine, new: any realm can stand targets |
| A zone's own colliders | [`aethyr/world/props.coax`](../../aethyr/world/props.coax) | Engine, new |
| The crosshair, the camera, the walker | `aethyr/cam/`, `aethyr/walk.coax` | Engine, existing |
| The butts' and faces' meshes | [`aethyr/world/build-site-mesh.coax`](../../aethyr/world/build-site-mesh.coax) | Engine, existing |
| The bow in the hand, its string, the draw pose | [`everglade/held-kit.coax`](../held-kit.coax) | Game, moved out of Character Create's file |
| The Ranger and the Longbow's rules | `aethyr/rules/srd/`, `apps/openagents-web/class-kit.coax` | Game, existing |
| The horse and the red dragon | [`everglade/mounts.coax`](../mounts.coax) | Game, existing |
| The HUD panel and hit numbers | `aethyr/ui/hud-menu.coax`, `aethyr/action/floats.coax` | Engine, existing |
| Bow input, the arrows against the world, sounds | `apps/openagents-web/experimental-combat.coax` (`xc_bow_*`) | The shared lab app, existing: it now asks the zone's targets and colliders |
| Where things are | [`everglade/range/layout.coax`](../range/layout.coax) | The range |
| Ring points and the tally | [`everglade/range/score.coax`](../range/score.coax) | The range |
| The pieces' meshes and colliders | [`everglade/range/place.coax`](../range/place.coax) | The range |
| The realm: arriving, standing the range, following each arrow, the tally's panel | [`apps/openagents-web/range-scene.coax`](../../apps/openagents-web/range-scene.coax) | The range |

The range's mesh is one entry in the glade pack (`C glade/range`), added
with:

```sh
coax/web/tool.sh verse/tools/assets/character_glade_pack.coax --append-range IN.pack OUT.pack
```

Check the Coax:

```sh
coax/bin/coax test --library aethyr/action/target.coax
coax/bin/coax test --library aethyr/world/props.coax
coax/bin/coax test --library everglade/range/score.coax
coax/bin/coax test --library everglade/range/place.coax
coax/remote/remote.sh run -- coax/bin/coax test --library apps/openagents-web/fivelab.coax --only "archery range"
```

## 8. The realm and the apps

The Archery Range is a realm of its own in `apps/openagents-web/realms.json`:
its own page (`/worlds/archery-range`), save slot and row. It is unlisted, as
Everglade Experimental and Everglade II are. Its page loads the `fivelab`
module, the same one Everglade Experimental and the Druid realm load, because
that module holds the player, the bow and the mounts.

In the Mac app the world list's row for the Archery Range opens this realm
natively, through Aethyr's page host, with its sound: the same `fivelab`
library Experimental runs ([app scenes spec](app-scenes-spec.md), section 4).
Only one of the two is open at a time. On the phones its row still says it
runs on the web and gives that address.

## 9. What is not checked

- **A second module.** The realm shares the `fivelab` module. Splitting the
  bow's and the mounts' wiring out of that module's state, so a realm can
  have a module of its own, is not done.
- **Phones.** The range was looked at in desktop Chrome sizes only.
- **Mutation.** `coax mutate` has not been run on the new engine files.

## 10. Next

1. Rounds: a fixed number of arrows at one distance with a final score, and
   your best kept.
2. The distance on each post as a number.
3. Moving faces, and a gallop course along the line with faces on both sides.
4. Wind that pushes the arrow.
