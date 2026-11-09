# Building Everglade

**Status: proposal.** This page sets out the book's structure and flow. No
chapter is complete yet.

Everglade is an MMORPG built on the
[D&D 5E System Reference Document](https://www.dndbeyond.com/srd) (SRD 5.2.1).
It aims to follow the SRD's rules faithfully: classes, spells, conditions,
attack rolls, saving throws and turn timing. It changes only what real-time
play needs. You aim, move and act as things happen, and the SRD's rules decide
the outcome. You play it free in your browser at
[everglade.world](https://everglade.world).

This book shows you how to build Everglade from scratch, one working piece at a
time, with these tools:

- [Coax](../../coax/book/README.md), the language. You write everything in it:
  the land, the rules, the network code and the shaders it generates.
- [Aethyr](../../aethyr/book/README.md), the world engine. Everglade uses its
  frame loop, terrain, renderer, lighting, physics, animation and room server.
- The Verse, where Everglade is one realm among many.

The Aethyr book builds the engine. This book builds a game on it. Each chapter
starts from a question a player asks, such as "Can I stand on that rock?" or
"Why did the guard hit back?" It ends with the code that answers the question
and the checks that keep the answer true.

## The SRD and action combat

Everglade has two goals that pull against each other:

- **Follow the SRD.** A Fireball deals 8d6 fire damage in a 20-foot-radius
  sphere, with a Dexterity saving throw for half. A longbow deals 1d8 piercing
  damage plus your Dexterity modifier. Conditions, spell slots, rests and class
  features work as the SRD says.
- **Play as action combat.** No one waits for a turn. You hold right-click to
  draw a bow, aim with a crosshair and release to shoot. You cast while you
  run.

The book treats every difference from the SRD as a design decision with a
reason. For example:

- **Turns become time.** An SRD round is 6 seconds. Everglade spends your
  action, bonus action, reaction and movement over the same 6 seconds, in real
  time.
- **Aim picks the target, and the roll decides the hit.** Your aim chooses what
  an arrow or spell can hit. The SRD attack roll against Armor Class still
  decides whether it hits.
- **Range stays in feet.** The SRD's ranges and areas convert to the world's
  metres, so a 150-foot longbow shot is about 45 metres.

Each rules chapter lists what follows the SRD exactly, what changes and why,
and the checks that keep each rule true.

## How you build: text first, no editor

Most people will make realms with Aethyrforge's tools and the Verse's maker
flows. In this book, you build Everglade without them. Every tree, path, rule
and light is a Coax function in a text file. You build it from the command line
and verify it in a headless browser. You don't use a level editor or any other
UI.

The book works this way for two reasons:

- It shows what the engine does underneath the tools. Each maker tool wraps
  something this book builds by hand.
- Some people prefer to build as code and checks, with no UI. This book is
  their guide.

When a chapter does something a maker tool will do later, it links to that
tool's chapter in the Aethyr book.

## Where Everglade came from

Everglade started as prototypes, each built to answer one question:

- **Everglade v1** (`/worlds/everglade`) showed that a whole town can run in a
  browser: a gate, walls, townspeople, a raid and SRD combat. It stays frozen
  as it is.
- **Everglade Experimental** (`/worlds/everglade-experimental`) is the test
  bench. New systems, such as hostiles, a dungeon, imported characters and
  action combat, prove themselves there before they move into the world.
- **Character Create**
  ([`docs/verse/character-creator.md`](../../docs/verse/character-creator.md))
  is the glade where you make your hero. Its look and light come from one
  reference image: a Quaternius forest scene with warm autumn light through the
  canopy
  ([reference light](../../docs/verification/character-glade-reference-light-2026-11-07/)).

The glade became the starting point for the final version, **Everglade II**
([`docs/verse/everglade2.md`](../../docs/verse/everglade2.md)). It is rebuilt
from scratch outward from the glade, one piece at a time (#13023): the glade,
one path through dense woods, a clearing and next a city. The glade's light is
now the default for every realm in that art style
([`docs/verse/lighting/README.md`](../../docs/verse/lighting/README.md)).

## The flow of the book

The book follows the order in which you build the game. Each part ends with
something you can walk around in and test.

### Part I: One picture to build toward

1. **A reference image.** Turn one picture into numbers: light, colour,
   density and scale.
2. **What the prototypes taught.** What v1, Experimental and Character Create
   settled, and what you throw away.
3. **A realm with no editor.** A realm's files, building one unit, a headless
   capture and the checks that run while compiling.

### Part II: The glade

4. **Ground you can stand on.** Terrain tiles and seams, and why your feet and
   the drawn ground must agree at every sample.
5. **Plant a forest with a function.** Place 100,000 pieces from the land, with
   tests that keep the path clear.
6. **The glade's light.** One lighting record, resolved for each place, that
   every realm in the art style starts from.
7. **Character Create in the world.** Make the studio a place in Everglade that
   you walk out of.
8. **If you can see it, you can walk to it.** Make colliders match what you
   see, from rock hulls to tree trunks, and treat an invisible wall as a bug.

### Part III: The path out

9. **A path through dense woods.** A road curve, a grade limit and a canopy
   that closes around it.
10. **Hold 60 frames per second.** Culling, budgets that refuse and measuring on
    a phone.
11. **One address.** everglade.world, its landing page and `/play`. A new
    player goes to Character Create; a returning player goes back to where
    they stood.

### Part IV: Other players

12. **The room.** One server-authoritative simulation that every player sees
    the same way.
13. **Coming back.** What the server keeps, what the browser keeps and why.

### Part V: The SRD as code

14. **Ability scores, proficiency and the d20.** The core roll and its
    modifiers.
15. **Classes, levels 1 to 6.** Twelve classes, their features and their
    choices.
16. **Spells and slots.** Casting, concentration, areas and saving throws.
17. **Conditions.** Each condition's effect in real-time play.
18. **Turns become time.** Actions, bonus actions, reactions and movement
    across a 6-second round.
19. **Familiars and Wild Shape.** Creatures you command and forms you take.

### Part VI: The guard

20. **A guard walks the path.** Patrol routes, turning at each end and stepping
    around players.
21. **Attack the guard.** He doesn't strike first. When you attack him, he
    fights back on SRD rules, and you see and hear every hit.
22. **Death and respawn.** A death animation, then a respawn that fades in.

### Part VII: The Ranger and action combat

23. **Mouse look and the crosshair.** A camera that follows your mouse, and
    Tab to free the cursor.
24. **The longbow.** Draw, aim and release. The arrow flies on a real arc, and
    the SRD attack roll decides whether it hits.
25. **Cast while you run.** Upper-body actions layered over the legs'
    movement.

### Part VIII: The city

26. **Walk into a city.** From the clearing to a gate, walls and streets.
27. **Townspeople.** Routines and crowds that hold the frame rate.
28. **Interactions.** Talking, trading, quests and what the city remembers.

### Part IX: Run an MMORPG

29. **Many rooms.** Shards, capacity and moving between places.
30. **Accounts and persistence.** Your character on every device.
31. **Ship it.** One release script, staging checks and rollback.
32. **Run a live world.** Monitor it, fix it while it runs and learn from
    incidents.

Parts I through V exist today. Parts VI and VII are in progress: an armoured
guard on the path to test NPC pathing and aggro, and the Ranger's longbow to
attack him with. Parts VIII and IX are planned.

## How a chapter works

Chapters follow the Aethyr book's rules:

- A chapter starts as a design draft with a status marker. It becomes a
  teaching chapter when its code lands.
- Listings are copied from source between `// book:begin` and `// book:end`
  markers. They are never retyped.
- Each chapter names the checks that keep its claims true, and the headless
  capture that shows them.
- Each chapter keeps its mistakes. When something broke for a player, such as
  an invisible wall, arrows that couldn't be seen or a spell bar with missing
  pictures, the chapter shows what happened and the check that now catches it.

The book uses the
[Google developer documentation style guide](https://developers.google.com/style):
second person, active voice, present tense, short sentences and
sentence-case headings.

## Where Everglade's code lives

Everglade's code is currently spread across the repository:

| What | Where it is now |
| --- | --- |
| The land, planting, ground and light | `verse/everglade2/` |
| The page, movement, casting, room and Character Create in the world | `apps/openagents-web/everglade2*.coax` |
| The asset pack | `verse/tools/assets/everglade2_pack.coax` |
| Experimental's combat and hostiles | `apps/openagents-web/experimental-*.coax`, `verse/experimental-*.coax` |
| SRD rules | `aethyr/rules/srd/` |
| The domain's pages | `apps/openagents-web/realms/everglade2/domain/` |

Everglade's own code moves into this folder, so the book and the game live
together:

```text
everglade/
  README.md     what Everglade is and how to run it
  book/         this book
  world/        the land, planting, ground and light (from verse/everglade2/)
  game/         the page and its units (from apps/openagents-web/everglade2*.coax)
  tools/        the asset pack and capture tools
  docs/         design notes (from docs/verse/everglade*.md)
```

The line between the engine and the game:

- **Stays in Aethyr:** anything another realm can reuse, such as the SRD rules,
  the bow system, NPC patrol and aggro, layered animation, the mouse-look
  camera, collision and terrain tiles.
- **Moves here:** what belongs only to Everglade, such as its land, planting,
  places, NPCs and tuning.

Builds, `realms.json` module paths, `#load` paths and the release's site layout
all name the current paths. The move happens one directory per commit, after
the work in progress lands, and each commit keeps every build and check
passing.
