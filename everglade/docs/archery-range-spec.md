# The Archery Range (spec)

Written 2026-11-10. Status: **built, version 1**, in the Everglade apps
(`apps/everglade-desktop/`, `apps/everglade-mobile/`). It runs on Mac, iOS
and Android from one set of Coax files. The Coax is checked on Linux. No
host build has run it yet: see [What is not checked](#9-what-is-not-checked).

The range is a place to practise the ranger's bow: shoot at faces for
points, see how accurate you are, and do it at different distances, on
foot, from horseback and from dragonback.

## Contents

1. [What you do](#1-what-you-do)
2. [The ground](#2-the-ground)
3. [Scoring](#3-scoring)
4. [The aim](#4-the-aim)
5. [Three ways of shooting](#5-three-ways-of-shooting)
6. [Controls](#6-controls)
7. [The tally](#7-the-tally)
8. [What is reused, and where the code is](#8-what-is-reused-and-where-the-code-is)
9. [What is not checked](#9-what-is-not-checked)
10. [Next](#10-next)

## 1. What you do

You arrive at dusk behind a chalk shooting line, facing five straw butts.
Hold the bow to draw, aim with the reticle, let go to loose. The arrow
flies a real arc and sticks where it lands. A hit shows its points over the
reticle with the distance you shot from. Walk anywhere inside the fence:
closer for an easy shot, further back for a hard one. Take the horse along
the track and shoot from the saddle, or take the dragon up and shoot from
the air.

## 2. The ground

All numbers are in [`everglade/range/layout.coax`](../range/layout.coax).
The rules, the place and its shaders all read that one file.

| Thing | Where |
| --- | --- |
| The ground you may cover | 60 m wide, from 22 m behind the line to 92 m downrange, fenced |
| Five lanes | Butts at 10, 20, 30, 50 and 70 m from the line, 8 m apart |
| Face size | 122 cm across on the near three, 160 cm at 50 m, 200 cm at 70 m |
| Two sky faces | On poles 14 m and 20 m up, leaning back, for a shot from dragonback |
| The riding track | Along the line, 8 m behind it, 52 m long |
| Where you arrive | 3 m behind the line, in the middle |
| The horse | Waits on the track |
| The dragon | Waits behind the track |
| Lanterns | One by each butt and along the line, so the rings read at dusk |

It is dusk, not day, on purpose: the apps draw their text, reticle and
arrows as light added over the picture, which a bright sky would wash out.

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
ring inside it. Off the face scores nothing. An arrow scores only when it
crosses the face from the front. An arrow into the ground, or one that flies
for six seconds without landing, is a miss.

## 4. The aim

The arrow is launched to land on the point the reticle is on, so you do not
aim above a far face by hand. What makes a shot hard is the **spread**: the
angle an arrow may leave the line of sight by. The reticle's ring is the
spread, drawn to scale.

| What | Effect on the spread |
| --- | --- |
| A full draw (0.8 s), held under 2.5 s, standing still | The least: about 25 cm across at 70 m |
| A bow not fully drawn | Wider, up to eight times, and the arrow is slower and may fall short |
| A draw held past 2.5 s | Widens as your arm tires |
| Your own pace, or your mount's | Widens, most at a full gallop or full flight |

So the skill is timing: come to full draw, loose before your arm tires, and
slow the horse or steady the dragon for a far shot.

The spread's dice are seeded, so the same shots give the same arrows.

## 5. Three ways of shooting

| Way | How it moves | Fastest |
| --- | --- | --- |
| On foot | The shared walking controller, as in every realm | A run |
| From horseback | Forward walks, Shift gallops, A and D turn, S slows and backs. It carves arcs and stops at the fence | 14 m/s |
| From dragonback | Space takes off, beats and climbs. Shift dives. X lands. A and D bank it round | 20 m/s, 40 m ceiling |

Choosing Horse or Dragon puts you in the saddle where the mount stands.
Choosing Foot steps you down at its left, and it stays where you left it.
Nobody gets off a dragon in the air.

The camera follows over your right shoulder so the reticle is clear of
your head, and sits further back when you ride.

## 6. Controls

**Keys (Mac)**

| Key | What it does |
| --- | --- |
| W A S D | Move, or steer the mount |
| Hold F | Draw the bow. Let go to loose |
| 1, 2, 3 | On foot, horse, dragon |
| Space | Jump. On the dragon: take off, beat, and hold to climb |
| Shift | Walk. On the horse: gallop. On the dragon: dive |
| X | Land the dragon |
| R | Clear the tally |
| Drag, wheel | Look, zoom |
| Escape | Back to the list of worlds |

**Touch (iOS, Android)**

| Touch | What it does |
| --- | --- |
| Left thumb | Move, or steer the mount. A full push gallops |
| Hold Draw, drag, let go | Draw, aim with the same thumb, loose |
| Foot, Horse, Dragon | Change how you shoot |
| Up, Down (dragon) | Hold Up to take off and climb. Hold Down to dive, and to land when low |
| Reset | Clear the tally |

## 7. The tally

Shown at the left, kept until you clear it or close the app.

- **Overall:** arrows, hits, accuracy (hits in a hundred), points, and
  points an arrow.
- **Runs:** hits in a row now, the longest run, and golds (9s and 10s).
- **By distance,** for the way you are shooting now: the same figures for
  each band you have shot in. The bands are up to 15 m, 25 m, 40 m, 60 m,
  and beyond. A shot's distance is from your hand to the face you shot at,
  so walking back from the 30 m butt moves your shots into a further band.
- Each way of shooting keeps its own figures.

## 8. What is reused, and where the code is

Nothing about archery or riding is written twice.

| Piece | Where | New? |
| --- | --- | --- |
| The draw, the arrow's arc, the launch that lands on a point | [`aethyr/action/bow.coax`](../../aethyr/action/bow.coax) | Existing: Experimental's bow |
| The horse, and a flier's steering | [`aethyr/action/mount.coax`](../../aethyr/action/mount.coax) | Existing |
| Winged flight | [`aethyr/action/flight.coax`](../../aethyr/action/flight.coax) | Existing |
| Walking and the follow camera | [`aethyr/walk.coax`](../../aethyr/walk.coax) | Existing |
| Where things are | [`everglade/range/layout.coax`](../range/layout.coax) | New |
| Ring points, a strike, the spread, the tally | [`everglade/range/score.coax`](../range/score.coax) | New |
| The place's mesh, from Aethyr's architecture kit | [`everglade/range/place.coax`](../range/place.coax) | New |
| Its shaders, emitted for Metal and Vulkan | [`everglade/range/gpu/range.coax`](../range/gpu/range.coax) | New |
| The scene: input, camera, arrows against faces, what is drawn | [`apps/everglade-desktop/range.coax`](../../apps/everglade-desktop/range.coax) | New |

Check the Coax:

```sh
coax/bin/coax test --library everglade/range/score.coax
coax/bin/coax test --library everglade/range/place.coax
coax/bin/coax test --library everglade/range/gpu/range.coax
coax/bin/coax test --library apps/everglade-desktop/scenes.coax --only "range"
```

**The horse and the dragon are drawn as strokes of light,** not models.
Their models and clips are in Experimental's pack, and the apps ship only
Everglade II's pack. The web's `xh_records`
(`apps/openagents-web/experimental-horse.coax`) already makes their draw
records. Once the apps carry that pack, `model.coax` can hand those records
to the host and the strokes go.

## 9. What is not checked

- **No host build has run.** The Swift, Metal and Vulkan changes were
  written on Linux without a Mac or the Android SDK. The Coax checks pass,
  the C compiles, and the compiled scene was driven through `face.h` on
  Linux (the list, the range, a shot, a dragon flight).
- **Nobody has seen a frame of the range.** Its brightness and colours were
  set by arithmetic. Expect to tune them.
- **Feel.** The spread's numbers, the camera's shoulder offset and the touch
  boxes' places are first guesses.
- **Mutation.** `coax mutate` has not been run on the new files.

## 10. Next

1. Build on a Mac, look at it, and tune light, spread and camera.
2. Mount models from the Experimental pack (section 8).
3. A bow in your hands and a draw pose: `aethyr/action/bow-pose.coax` exists.
4. Rounds: a fixed number of arrows at one distance with a final score, and
   your best kept under `~/.openagents/everglade/`.
5. Moving faces, and a gallop course along the track with faces on both sides.
6. Wind, shown by the pennant, that pushes the arrow.
7. Arrow sounds: the loose, the flight and the thump are in Everglade's bank.
8. The same range in the web app, as a zone.
