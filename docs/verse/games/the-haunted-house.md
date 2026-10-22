# The Haunted House

Status: built, 2026-10-22 (#12472 to #12476): `haunt-rules`, `haunt-assets`,
`haunt-web`, the `haunt` livehost tenant (`icecream-live --game haunt`), the
Grid's hosted arches, and Game Lab's haunt tools; receipts are under
`docs/verse/verification/2026-10-22-haunted-house*`. The in-zone mic
button is #12485. Spec written 2026-10-21. Owner request: build the Blue Rush flagship as a
small walk-through haunted house: a zone with a portal on the Grid beside
the ICE CREAM MAN arch, made in a vintage-Halloween, late-90s-cartoon
style, with every likely addition prepared in advance. It should be the
easiest zone in the Verse for a coding agent to read and extend. Epic: #12477; issues are
listed in [Delivery](#delivery). Numbers marked *tunable* are starting
values for playtests, not measured results.

The pitch: walk through a short, spooky-cute haunted house in first person.
Then make **one change** ("I wish there was a werewolf here that howls at
you") through [Game Lab](../../game-lab/README.md), and watch it land in the
house live. The Oct 24 creator test gives each tester one change credit.

## Contents

- [Pillars](#pillars)
- [Art direction](#art-direction)
- [Arrival: the portal and the turn](#arrival-the-portal-and-the-turn)
- [The MVP house](#the-mvp-house)
- [The house document, `haunt.v1`](#the-house-document-hauntv1)
- [Movement and view](#movement-and-view)
- [Lighting](#lighting)
- [Audio](#audio)
- [The prepared library](#the-prepared-library)
- [Game Lab: one change per tester](#game-lab-one-change-per-tester)
- [Hosting and code layout](#hosting-and-code-layout)
- [MVP scope and acceptance](#mvp-scope-and-acceptance)
- [Delivery](#delivery)

## Pillars

1. **Spooky-cute, never gory.** Vintage Halloween: grinning cats, friendly
   jack-o'-lanterns, sheet ghosts, cackling witches. Kid-safe, like
   [The Ice Cream Man](the-ice-cream-man.md). Scares are jolts and reveals,
   never injury.
2. **You only see where you look.** A strong vignette and the player's
   lantern leave the edges of the view dark.
3. **Doom walking.** A fast, smooth first-person glide through simple,
   boxy architecture, passing objects close by.
4. **The overprepared mother.** Like a parent turning their house into a
   haunted house, every likely request (werewolf, bats, a coffin that
   opens) already has a finished asset, sound and placement rule in the
   house's style. A wish lands in seconds and looks like it belongs.
5. **Readable by an agent.** Every space states its size, position,
   facing, contents and **intended psychological effect**. Adding a hallway
   or room is one appended entry, checked before it's saved.

## Art direction

Two references from the owner set the style. Neither is checked in: one is
a frame from a commercial game, and the other is a page of vintage prints.
Nothing is traced or copied from them. All art is original.

**Shapes and layout: late-90s N64 cartoon platformers** (the
*Bugs Bunny: Lost in Time* look):

- Low-poly, chunky silhouettes, big flat planes, slightly tilted and
  uneven walls.
- Surfaces painted as if by hand, with **dark cartoon outlines inside the
  texture**: cobblestones as rounded cells with dark mortar lines, wood with
  swirling cartoon grain and ring-marked log ends, iron bars as thick
  strokes.
- Gnarled, dark blue-teal trees drawn as silhouettes, flat triangular
  mountains far off, a lavender dusk sky, murky green ground, still dark
  water.

**Colour and mood: vintage Halloween prints and die-cuts, 1920s–1950s:**

- Cream paper, pumpkin orange, licorice black, aged mustard, faded brick,
  dusty plum.
- Grinning black cats (some in party hats), jack-o'-lanterns with
  friendly or sly faces, witches on brooms, bats, children in sheet-ghost
  costumes, folk-art skeletons.
- A light paper grain and a hint of halftone over the whole view.

**Palette** (tokens in `haunt-assets`; the names are what documents use):

| Token | Hex | Use |
| --- | --- | --- |
| `paper` | `#F1E4C8` | Highlights, ghosts, labels |
| `pumpkin` | `#E07A2E` | Jack-o'-lanterns, accents, warm light |
| `pumpkin_deep` | `#B4501E` | Pumpkin shading, rust |
| `licorice` | `#1B1714` | Outlines, cats, silhouettes |
| `mustard` | `#C99A3B` | Wallpaper, brass, candle bodies |
| `brick` | `#8E3B2A` | Walls, carpet, ribbons |
| `dusk` | `#A9A3C4` | Sky, distant mountains, moonlit stone |
| `plum` | `#4A3F5E` | Shadows, vignette edge, night stone |
| `moss` | `#56664F` | Ground, damp walls |
| `night_teal` | `#2B3346` | Trees, deep shadow, water |
| `candle` | `#FFC56B` | Candle and lantern light |

Rules: shadows fall to `plum` or `night_teal`, never neutral grey. Light
is warm (`candle`, `pumpkin`) indoors and cool (`dusk`) outdoors. Outlines
are `licorice`.

**Rendering:**

- Two-band toon shading on vertex-coloured low-poly meshes.
- Surface patterns (cobble cells, wood grain, wallpaper stripes, halftone)
  made in the shader from a pattern id plus two palette tokens, so
  architecture needs no texture files, like The Ice Cream Man's chunks.
- Characters and decorations are **die-cut cards**: flat cut-outs with a
  thick licorice border that turn to face the player, like Doom's sprites
  and like the paper decorations in the prints. These are the only images
  the zone ships (see [The prepared library](#the-prepared-library)).
- A paper-grain overlay at low strength.

## Arrival: the portal and the turn

**On the Grid.** An arch lettered HAUNTED HOUSE stands beside the ICE CREAM
MAN arch, because both are horror games. The proposed site is
`[27.0, 19.0]` (side, forward, m) in the frame `crates/verse/src/zones/gate.rs`
uses, 7 m outboard of `GRID_HOSTED_AT` (`[20.0, 19.0]`), turned to face the
spawn like the others. The arch tests must show its approach is clear.

Today the Grid has **one** hosted arch: `Option<HostedPortal>` in the zone
runtime, one `GRID_HOSTED_AT`, and one `?hosted=` in
`crates/everglade-web/src/hosted.rs`. A list of hosted arches (tenant, sign,
site) is needed. It is also what deliverable D8 (featured creator portals)
needs. Prefetch, crossing, and **Back to the Grid** work per arch, as they
do for the Ice Cream Man (`docs/verse/ice-cream-man-live.md`).

**In the zone: the reveal is the player's.** The player arrives in the yard
on the cobblestone path **facing away from the house**, looking down a
moonlit road between gnarled trees toward far triangle mountains. About 2 s
after arrival *tunable*, a door creaks and a crow calls **behind** them,
and the music's bass line enters. Turning around reveals the house filling
the view: lit windows, the door ajar, jack-o'-lanterns on the steps.
Nothing forces the turn. The sound invites it.

## The MVP house

The yard, then three hallways and two rooms, then out the back: a 3–5 minute
walk *tunable*. Axes: +x east, +z north. The front door is on the house's
south wall, so walking in means walking north. The arrival point faces
south (yaw 180°).

```text
              N (+z)
   +-----------+----------------+
   | R2 kitchen|  H3 back hall  |---> back door to yard (exit)
   |   7 x 9   |     3 x 10     |
   +-----+-----+--+-------------+
         |  H2 long hall 2.4 x 18 |
   +-----+-------+----------------+
   | R1 parlor   |
   |   8 x 8     |
   +------+------+
          | H1 entry hall 3 x 12
          +--front door--+
             A0 yard 16 x 20 (arrival faces south, away from the door)
```

| Id | Area | Size (w × d × ceiling, m) | Contents (assets) | Intended effect | Intensity 0–5 |
| --- | --- | --- | --- | --- | --- |
| `yard` | Outside, arrival | 16 × 20, open sky | Cobble path, iron gate, gnarled trees, tombstones, jack-o'-lanterns on the steps, moon | Calm unease, then the reveal when the player turns | 1 → 2 |
| `entry-hall` | Hallway | 3 × 12 × 3.2 | Portraits whose eyes follow, a coat stand, a cobweb at the far end | Threshold: "you're inside now" | 2 |
| `parlor` | Room | 8 × 8 × 3.6 | Armchair, gramophone (the bass line louder here), candelabra, a rocking chair that rocks by itself | False comfort, then a jolt: the candles dip | 2 → 3 |
| `long-hall` | Hallway | 2.4 × 18 × 3.0 | Doors on both sides, one of which bangs; a window with lightning | Anticipation: the space narrows | 3 |
| `kitchen` | Room | 7 × 9 × 3.4 | A cauldron bubbling green, a potion shelf, a black cat on the counter, a broom | Relief through a laugh: spooky-cute | 2 |
| `back-hall` | Hallway | 3 × 10 × 3.2 | A hanging sheet that's really a ghost, chains, the back door | The last scare (the ghost crosses), then release outside | 4 → 1 |

The tension curve rises (yard to long hall), dips (kitchen), peaks (back
hall), then releases outside. A new area states where it sits on that curve
(see `intent` below).

## The house document, `haunt.v1`

The whole house is one typed JSON document, the way The Ice Cream Man is
`game.v1`. It is the single source of truth. The shell draws it, the
checks read it, and agents edit it only through typed tools.

**Conventions:** metres; +x east, +z north, y up; `yaw` in degrees,
0 = facing +z (north), 90 = facing +x (east). A rectangle is
`[x, z, width, depth]` from its south-west corner. Assets and palette
colours are named by id, never by file path.

```json
{
  "kind": "haunt.v1",
  "title": "The Haunted House",
  "arrival": { "area": "yard", "at": [8, 4], "yaw": 180, "cue": "creak-behind" },
  "exit": { "area": "back-hall", "door": "back-door" },
  "look": { "vignette": 0.75, "lantern": true, "grain": 0.15, "sky": "dusk" },
  "audio": { "master": 0.8, "music": 0.6, "ambience": 0.7, "sfx": 0.9, "music_track": "bassline-minor" },
  "areas": [
    {
      "id": "parlor",
      "kind": "room",
      "rect": [0, 15, 8, 8],
      "ceiling": 3.6,
      "surfaces": { "floor": "planks", "walls": "wallpaper-stripe", "trim": "wood" },
      "light": { "preset": "candle", "dim": 0.0 },
      "sound": { "bed": "clock-tick", "music_gain": 1.3 },
      "intent": { "feeling": "false comfort, then a jolt", "beat": "the candles dip as you reach the middle", "intensity": [2, 3] },
      "doors": [
        { "id": "parlor-south", "to": "entry-hall", "wall": "south", "offset": 2.5, "width": 1.4 },
        { "id": "parlor-north", "to": "long-hall", "wall": "north", "offset": 1.0, "width": 1.4 }
      ],
      "props": [
        { "id": "armchair-1", "asset": "armchair", "at": [6.2, 6.5], "yaw": 225, "note": "faces the gramophone" },
        { "id": "rocker-1", "asset": "rocking-chair", "at": [1.5, 6.0], "yaw": 135, "state": "rocking" }
      ],
      "triggers": [
        { "when": { "enter_middle": true }, "do": [{ "light": { "dim": 0.6, "for": 1.5 } }, { "sfx": "candle-gust" }] }
      ]
    }
  ]
}
```

A prop's `at` is relative to its area's south-west corner, so moving an
area moves its contents.

**Checks** (run on every edit; a refusal is one plain sentence, as in the
Labs `Refusal`):

- Area rectangles don't overlap. Each door lies on a wall the two areas
  share, and both sides list it.
- Every area is reachable from `arrival`, and `exit` is reachable.
- A clear walking lane at least 1.0 m wide connects every door chain. Props
  may not block it.
- Props lie inside their area, sit on the surface their asset allows
  (floor, wall, ceiling), and fit under the ceiling.
- Every `asset`, `preset`, sound and palette id exists in the library.
- Budgets: at most 12 areas, 40 props per area, 8 lights per area and 6
  playing sounds at once *tunable*, plus the first-play byte budget
  (see [Hosting](#hosting-and-code-layout)).
- Every area has an `intent` with a feeling and an intensity.

**Extending the house.** Adding a hallway or room means appending one
area with a door onto an existing area's free wall. A helper tool,
`haunt_add_area(next_to, wall, kind, size, intent)`, finds the rectangle,
cuts the matching door on both sides, and returns the new document for the
checks. Removing an area refuses if it would cut off the exit.

**Describing the house for an agent.** `haunt describe` (CLI, and the
`haunt_describe` tool) prints, for the current version:

- A top-down ASCII map like the one above, with area ids.
- For each area: size, position relative to its neighbours ("north of the
  parlor, through its north door"), which way a player faces on entering,
  its light, its sound bed, its intent, and each prop with position,
  facing and note.
- The free walls where an area could be added, with the largest size each
  could take.
- The remaining budgets.

An agent reads this before any edit. It is the answer to "what is
anywhere in the game", so nobody has to read code to find out.

## Movement and view

- **First person only**, eye at 1.6 m. A walk of 4.5 m/s and a run of
  7 m/s *tunable*, with a fast ease-in and ease-out so it glides like Doom.
  A small head bob, and collisions that slide along walls. No jump in the
  MVP.
- Look: free turning, with pitch limited to ±35° *tunable*. Keyboard and
  mouse (WASD or arrows), and a touch joystick on phones (the Ice Cream
  Man's touch controls are the pattern).
- Architecture is boxy, with 90° corners and 1.4 m doorways, so the
  player is always passing objects close by.
- **Vignette:** a strong radial darkening to `plum` at the edges
  (`look.vignette`, 0.75 by default), plus the player's **lantern**: a
  warm forward cone that lights only where they look. A `gentle` setting
  in the menu softens both, for younger players and for accessibility.

## Lighting

Every area names a **preset**, with optional overrides. Changing the mood
of a room is one field.

| Preset | What it is |
| --- | --- |
| `moonlight` | Cool `dusk` ambient light with long soft shadows. Default for the yard |
| `candle` | Warm low ambient light, with 1–3 flickering `candle` point lights |
| `lantern-only` | Near-black ambient light, so only the player's lantern lights the area |
| `lightning` | `moonlight` plus timed white flashes paired with thunder |
| `jack-o-glow` | Orange low light from jack-o'-lanterns, flickering |
| `blackout` | Nothing but the lantern, used briefly by triggers |

Overrides: `ambient` (palette token and level), `lights`
(`[{at, height, color, intensity, radius, flicker}]`) and `dim` (0–1).
Triggers can change any of these for a time. The global `look` sets the
vignette, lantern, grain and sky.

## Audio

Web Audio in the page, as in `crates/icecream-web/src/sound.rs`. Five
buses: `master`, `music`, `ambience`, `sfx` and `voice`, each 0–1 in the
document's `audio` section. Sources are positioned in 3D with panner
nodes, so the creak at arrival really comes from behind.

- **Music:** a creepy bass line playing throughout. The MVP track,
  `bassline-minor`, is about 70 bpm in a minor key. It has a walking bass
  with a tritone step, a low organ pad, and a sparse music-box line added
  as intensity rises. It is made in the page, like the Ice Cream Man's
  jingle, so it needs no file. Each area's `music_gain` and `intensity`
  shape it.
- **Ambience beds:** wind, rain on glass, clock tick, fireplace, cauldron
  bubble, distant thunder.
- **Prepared effects:** see the library below. Each has a kid-safe level.

**Volume by voice.** The `audio` section is live: a change applies within a
frame without reloading, like the Ice Cream Man's tuning. A spoken request
("turn the music down", "make it louder") reaches it the way every Lab edit
does: the chat's model chooses the typed `haunt_audio` tool, and words are
never matched (workspace rule). Speech gets there through phone dictation
into the Game Lab chat (deliverable D9) or through the paired-browser
dictation path. An in-zone mic button sends to the same tool. Keyboard `M`
and a menu slider work as well.

## The prepared library

Like the parent who bought every decoration before the party: assets are
made **ahead of time** in the house's style, so a wish needs only a
placement, not new art. Each entry in `haunt-assets` has:

- `id` and `aliases` ("werewolf": "wolfman", "wolf man"). The aliases are
  for the model's reading. Selection is still a typed choice.
- `kind`: `mesh` (low-poly, made in code) or `card` (die-cut image).
- `footprint`: width, depth and height in m, and the surface it sits on
  (floor, wall or ceiling).
- `states` (idle, triggered, and so on).
- The linked sounds.
- A default trigger.
- `intensity` (0–5) and `kid_safe`.

The **MVP set** (all made before the Oct 24 test; *tunable* as Kiki
reviews):

- **Creatures (cards):** werewolf (howls, lunges a step), vampire (rises
  from a coffin), witch (cackles), black cat (hisses), bat swarm, sheet
  ghost (glides across), skeleton (rattles), mummy, zombie hand (from the
  floor), giant spider with web, crow, owl, scarecrow, pumpkin-headed man,
  headless rider (a silhouette in the window), rat.
- **Props (meshes):**
  - Jack-o'-lanterns (three faces), candles, candelabra, cauldron, coffin
    (opens), tombstones.
  - Cobwebs, portraits (eyes follow), grandfather clock (chimes),
    gramophone, armchair, rocking chair (rocks), broom.
  - Potion shelf, skull, chains, cage, chandelier (sways), cracked mirror,
    piano (plays itself), bookshelf (a book falls).
  - Hanging sheet, banging door, lightning window, hay bales, corn stalks,
    iron fence, gnarled tree, moon.
- **Effects:** ground fog, lightning flash, light flicker, dripping,
  glowing eyes in the dark, a floating object, a door slam, footprints
  appearing.
- **Sounds:**
  - Door creak, footsteps upstairs, wolf howl, thunder, crow, owl, cat
    hiss, evil laugh, witch cackle.
  - Rattling chains, heartbeat, wind, rain, organ sting, a kid-safe
    scream, whisper, clock chime, bubbling cauldron, bat flutter, music
    box.

**Making them.**
- **Meshes** are built in code, like `crates/icecream-web/src/scene.rs`,
  with pattern ids and palette tokens.
- **Cards** are original images in the vintage-print style. Each has a
  thick licorice border and a transparent background, uses only palette
  colours, and is about 512 px tall. They are approved by Kiki (Creative
  Director) before they ship.
- **Sounds** are made in the page where possible. Recorded or generated
  sounds must be our own or CC0, and their source is recorded beside them.
- Cards and any recorded sounds ship as **content-addressed chunks**.
  That requires adding image and audio classes to `icecream_zone::chunk`
  (or its generalization), because The Ice Cream Man has neither today.

## Game Lab: one change per tester

The Haunted House becomes Game Lab's second game. Today
`crates/game-lab/src/doc.rs` names one game. Tools offered to the chat's
model:

| Tool | Does |
| --- | --- |
| `haunt_describe` | The text description and map above |
| `haunt_place` | Places a library asset in an area (position or "near the door", plus facing) with its default trigger |
| `haunt_trigger` | Adds or changes a trigger (enter, near a prop, looking at a prop) |
| `haunt_light` | Changes an area's preset or overrides, or the global look |
| `haunt_audio` | Changes bus levels or the music track. This is the volume-by-voice tool |
| `haunt_add_area`, `haunt_remove_area` | Extends or shrinks the house, as above |
| `haunt_words` | The title and the few signs and notes in the house |

Every edit makes a version, runs the checks, and deploys live (below), so a
tester standing in the house sees the werewolf appear without reloading.
For the test, each tester's account gets **one change credit**. Owners
and Kiki have no limit. A refused edit costs no credit.

## Hosting and code layout

The zone is a livehost tenant, `haunt`, built the same way as The Ice Cream
Man (`docs/verse/ice-cream-man-live.md`):

- A zone index.
- One content-addressed chunk per area, plus the asset chunks.
- A first-play budget of the yard, the entry hall, and the assets they use.
  That is 3.5 MB and 1.5 s, the same as the Ice Cream Man *tunable*.
- Areas stream in by distance.
- Live updates fetch only the hashes that changed.

| Crate | Holds |
| --- | --- |
| `haunt-rules` | `haunt.v1` types, the checks, `describe`, collision and triggers. Pure, and builds for wasm |
| `haunt-assets` | The palette, the library catalog, mesh builders, and the card and sound manifests |
| `haunt-web` | The WebGL2 shell: the first-person controller, toon shading and patterns, vignette and lantern, lighting, audio buses, and the live client. It reuses `icecream-web`'s mesh and live-update code instead of copying it |
| Live tooling | Generalize `icecream-live` to serve more than one game, rather than copying it as `haunt-live` |
| `game-lab` | The second game and the `haunt_*` tools |
| `verse` and `everglade-web` | A list of hosted arches, with HAUNTED HOUSE beside ICE CREAM MAN |

## MVP scope and acceptance

In:
- The arch.
- The arrival and turn.
- The six areas.
- First-person movement with the vignette and lantern.
- The six lighting presets.
- The bass line, ambience and effects.
- Live volume by voice.
- The prepared library.
- `haunt.v1` with its checks and `describe`.
- Game Lab tools with one change credit per tester.
- Live deploys.

Out:
- Anything that chases or damages the player.
- Scoring.
- Shared multiplayer runs (each player's walk is their own, as in The Ice
  Cream Man).
- Additional floors.
- New art generated at request time.

Done when:

- From the Grid, a player walks through HAUNTED HOUSE (beside ICE CREAM
  MAN), arrives facing away from the house, hears the cue behind them,
  turns, and walks all six areas out the back door. A recorded run is
  attached.
- The vignette and lantern leave the edges dark, and `gentle` softens
  them.
- Changing an area's light preset or the master volume by a spoken request
  in the Game Lab chat changes the running house within 2 s, without a
  reload.
- "Add a werewolf in the long hall that howls when you get near" becomes
  a checked version with the `werewolf` card, a `near` trigger and the
  `howl-wolf` sound, and appears live.
- `haunt_add_area` adds a fourth hallway and a third room, the checks pass,
  and both are walkable. A refused edit (overlap, blocked lane, unreachable
  exit) returns one plain sentence.
- `haunt describe` lists every area's size, position, facing, contents and
  intent, matching the running house.
- First play stays within budget, and the checks run in tests.
- Every library card is approved by Kiki, and every sound's source is
  recorded.

## Delivery

Epic #12477. In order:

1. #12472 **Grid:** a list of hosted arches, with HAUNTED HOUSE beside ICE CREAM
   MAN.
2. #12473 **`haunt.v1`:** document, checks, `describe`, and the MVP house as
   version 1.
3. #12474 **`haunt-web`:** the first-person shell, the art-style rendering, the
   vignette and lantern, lighting presets, and the livehost tenant.
4. #12475 **The prepared library and audio:** cards, meshes, effects, sounds, the
   bass line, and the audio buses.
5. #12476 **Game Lab:** the second game, the `haunt_*` tools, volume by voice,
   and one change credit per tester.

Targets: a playable house with a few library assets for the Oct 24 test.
The full MVP for the flagship showing by Oct 30.
