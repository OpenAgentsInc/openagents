# Building Everglade

**Status: proposal.** This page sets out the book's structure and flow. No
chapter is written yet.

Everglade is a painted-forest MMORPG that runs in your browser at
[everglade.world](https://everglade.world). This book builds it from scratch,
one working piece at a time, with the tools we already have:

- [Coax](../../coax/book/README.md), the language. Everything in Everglade is
  written in it, from the land and the rules to the shaders it generates.
- [Aethyr](../../aethyr/book/README.md), the world engine. Everglade uses its
  frame loop, terrain tiles, renderer, lighting, physics, animation and room
  server.
- The Verse, where Everglade is one realm among many.

The Aethyr book builds the engine. This book builds a game on it. Each chapter
starts from a question a player would ask, such as "can I stand on that rock?"
or "why did the guard hit back?", and ends with the code that answers it and
the checks that keep it answered.

## How we build: text first, no editor

Most people will make realms with Aethyrforge's tools and the Verse's maker
flows. We are building Everglade the other way: every tree, path, rule and
light is a Coax function in a text file, built and checked from the command
line, and verified in a headless browser. No level editor, no UI.

We do this for two reasons:

- It shows what the engine can do underneath the tools. Each tool a maker uses
  later wraps something this book builds by hand.
- Some people will want to build this way, as code and checks with no UI. This
  book is their guide.

Where a chapter does something a maker tool will do later, it says so and
links the tool's chapter in the Aethyr book.

## Where Everglade came from

Everglade wasn't built in one pass. We built prototypes first, each to answer
one question:

- **Everglade v1** (`/worlds/everglade`) proved a whole town could run in the
  browser: a gate, walls, townspeople, a raid and SRD 5E combat. It is frozen
  as it is.
- **Everglade Experimental** (`/worlds/everglade-experimental`) is the test
  bench for systems: biomes, hostiles, a dungeon, imported characters and new
  combat. A system proves itself there before it moves into the world.
- **Character Create**
  ([`docs/verse/character-creator.md`](../../docs/verse/character-creator.md))
  built the glade where you make your hero, lit from one reference image: a
  Quaternius forest scene, warm autumn light through the canopy
  ([its reference light](../../docs/verification/character-glade-reference-light-2026-11-07/)).

That glade was the most beautiful thing we had made. So the final version,
**Everglade II** ([`docs/verse/everglade2.md`](../../docs/verse/everglade2.md)),
is rebuilt from scratch outward from it, one piece at a time (#13023): the
glade, one path through dense woods, a clearing, and next a city. The glade's
light became the default for every painterly realm
([`docs/verse/lighting/README.md`](../../docs/verse/lighting/README.md)).

## The flow of the book

The book follows the order we build the game in. A part ends with something
you can walk around in and test.

**Part I. One picture to build toward**

1. A reference image. Why one picture beats a feature list, and how we turned
   the Quaternius forest scene into numbers: light, colour, density, scale.
2. What the prototypes taught us. v1, Experimental and Character Create: what
   each one settled, and what we threw away.
3. A realm with no editor. The text-first workflow: a realm's files, building
   one unit, a headless capture and the checks that run while compiling.

**Part II. The glade**

4. Ground you can stand on. Terrain tiles, seams and why your feet and the
   drawn ground must agree at every sample.
5. Planting a forest as a function. 100,000 pieces placed from the land, with
   tests that keep the path clear.
6. The glade's light. One lighting record, resolved per place, that every
   painterly realm now starts from.
7. Character Create stands in the world. The studio is a place in Everglade,
   and you walk out of it.
8. If you can see it, you can walk to it. Colliders that match what you see:
   rock hulls, trunks, and why an invisible wall is a bug.

**Part III. The path out**

9. A path through dense woods. A road curve, a grade limit and a canopy that
   closes around it.
10. Holding 60 frames a second. Culling, budgets that refuse and measuring on a
    phone.
11. One address. everglade.world, its landing page and `/play`: no character
    means Character Create; otherwise you're back where you stood.

**Part IV. Other players**

12. The room. One server-authoritative simulation that every player sees the
    same way.
13. Coming back. What the server keeps, what the browser keeps and why.

**Part V. The guard: NPC pathing and aggro**

14. A guard walks the path. Patrol routes, turning at the ends and stepping
    around players.
15. Attacking the guard. He doesn't strike first. When you hit him, he fights
    back on SRD rules. What a player sees and hears on every hit.
16. Death and respawn. A death animation, then a respawn that fades in.

**Part VI. The Ranger: action combat**

17. Mouse look and the reticle. A Neverwinter-style camera that follows the
    cursor, and Tab to free it.
18. A bow. Hold right-click to draw, release to loose. An arrow flies on a real
    arc while the SRD attack roll decides hit or miss.
19. Casting while running. Upper-body actions layered over the legs'
    locomotion.

**Part VII. The rules**

20. SRD 5.2.1 as code. Classes at levels 1 to 6, spells, conditions and turn
    timing, run by the room in real time.
21. Familiars and Wild Shape. Creatures you control, and bodies you swap into.

**Part VIII. The city**

22. Walking into a city. From the clearing to a gate, walls and streets, built
    out from the same land and light.
23. Townspeople. Routines, schedules and crowds that hold the frame rate.
24. Interactions. Talking, trading, quests and what the city remembers.

**Part IX. An MMORPG**

25. Many rooms. Shards, capacity and moving between places.
26. Accounts and persistence. Your character on every device.
27. Shipping. One release script, staging checks and rolling back.
28. Running a live world. Watching it, fixing it live and the incidents we
    learned from.

Parts I to III exist today. Parts V and VI are being built now: an armoured
guard on the path to test NPC pathing and aggro, and the Ranger's bow to
attack him. Parts VIII and IX are the plan.

## How a chapter works

Chapters follow the Aethyr book's rules:

- A chapter starts as a design draft with a status marker, and becomes a
  teaching chapter when its code lands.
- Listings are copied from source between `// book:begin` and `// book:end`
  markers, never retyped.
- Each chapter names the checks that keep its claim true, and the headless
  capture that shows it.
- Each chapter keeps its mistakes. When something broke for a player, such as
  an invisible wall, arrows nobody could see or a spell bar with missing
  pictures, the chapter shows what happened and the check that now catches
  it.

## Where Everglade's code lives

Today Everglade's code is spread across the repository:

| What | Where today |
| --- | --- |
| The land, planting, ground and light | `verse/everglade2/` |
| The page, movement, casting, room and Character Create in the world | `apps/openagents-web/everglade2*.coax` |
| The asset pack | `verse/tools/assets/everglade2_pack.coax` |
| Experimental's combat and hostiles | `apps/openagents-web/experimental-*.coax`, `verse/experimental-*.coax` |
| SRD rules | `aethyr/rules/srd/` |
| The domain's pages | `apps/openagents-web/realms/everglade2/domain/` |

We propose moving Everglade's own code into this folder, so the book and the
game live together:

```text
everglade/
  README.md     what Everglade is and how to run it
  book/         this book
  world/        the land, planting, ground, light (from verse/everglade2/)
  game/         the page and its units (from apps/openagents-web/everglade2*.coax)
  tools/        the asset pack and capture tools
  docs/         design notes (from docs/verse/everglade*.md)
```

The line between engine and game:

- **Stays in Aethyr:** anything another realm can reuse. That covers SRD
  rules, the bow system, NPC patrol and aggro, layered animation, the
  mouse-look camera, collision and terrain tiles.
- **Moves here:** only what is Everglade's own: its land, its planting, its
  places, its NPCs and its tuning.

When the guard or the bow lands, its general system goes into `aethyr/` and
Everglade's use of it goes here.

The move needs care. Builds, `realms.json` module paths, `#load` paths and the
release's site layout all name today's paths, and several agents are changing
those files now. We move one directory per commit, after the current work
lands, and each commit keeps every build and check green.
