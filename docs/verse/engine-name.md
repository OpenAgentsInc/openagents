# Naming the engine

The engine core (GitHub #12609: the loop, spaces, state, input, replay,
queries, entities, camera and budgets that every realm shares) needs a name.
It sits beside **Coax** (the language), **Psionic** (the model engine) and
**the Verse** (the world). This doc collects the candidates and how each one
feels; it's a vibes call. No decision has been made yet.

## What the name has to do

- **Name the runtime, not one tool.** The same engine runs games, made
  realms, the Grid, space simulation, robotics and training environments.
  A name that only says "fantasy" or "editor" undersells it.
- **Sit well beside Coax and Psionic.** "Written in Coax, runs on ___"
  should read naturally.
- **Be short and easy to say.** It will be used in docs, code
  (`verse/<name>/`), commit messages and conversation.
- **Leave room for a family.** Engine, editor and realm builder may want
  related names.

## Our own history: Aethyr

Aethyr is already ours. The Verse's player controller, third-person camera and
character cast were ported from **aethyr-cpp**, the C++ engine behind the
first playtest. Its Cloud Run service and load balancer (`aethyr-demo`, see
docs/verse/playtest-cutover.md) still exist. So Aethyr is not a fresh
invention: the Coax engine would be its successor, carrying the name forward.

## The candidates

### Aethyr

Aether, the substance that fills the heavens, spelt the way mystical
traditions spell it.

- **For:** short (6 letters), distinctive on the page, and it sounds like a
  whole platform. It spans magic, celestial mechanics and "the fabric of the
  world", which fits a galaxy of realms and the space-physics plans. It
  continues our own lineage.
- **Against:**
  - **Pronunciation:** EE-ther, AY-ther or AY-theer.
  - **Spelling:** it will often need saying out loud.
  - **Fantasy lean:** it reads more fantasy than science or industry.

### Aethyrforge

Aethyr plus the act of forging something into existence.

- **For:** it says "creation tool" immediately, and it suits a world-building
  environment, procedural generation or a realm builder. Forge stretches
  naturally to compiling, simulating and building worlds.
- **Against:** 11 letters and clumsier to say. It reads more fantasy than
  Aethyr does. "Forge" is very common in developer tools and modding, and is
  also our own `forge` repo's name. It could be mistaken for a game, modding
  kit or asset store, and it fits a general runtime (robotics, science) less
  well.

### Lattice

- **For:** the Grid is the Verse's signature look, and a lattice is the
  structure everything hangs on (spaces, fixed steps, the world grid). It is
  neutral enough for robotics and science. "Built on Lattice" reads cleanly.
- **Against:** a common word, with less mythic pull than Aethyr.

### Orrery

- **For:** a mechanical model of a solar system. It fits the galaxy of
  realms, and it means deterministic clockwork, which is the replay story.
- **Against:** not everyone knows the word, and it leans to space.

### Keel

- **For:** the spine a ship is built on: short, strong, and exactly what this
  layer is. It fits Dead Gears and the space games.
- **Against:** nautical, and plain.

### Others considered

- **Axle:** a play on Coax and axial. Simple, but generic.
- **Strata:** layered foundations.
- **Meridian:** the reference line for frames and time.
- **Not used:** Anvil, Bedrock, Source, Frostbite, Unity, Godot and Bevy are
  existing engines. Coaxial is our web host layer. Forge alone is our repo.

## Comparison

| | Aethyr | Aethyrforge | Lattice | Orrery | Keel |
|---|---|---|---|---|---|
| Length | 6 | 11 | 7 | 6 | 4 |
| Says "engine for every kind of world" | yes | editor more than engine | yes | space-leaning | yes |
| Easy to say and spell | no | no | yes | mostly | yes |
| Mythic or creative pull | strong | strong | modest | strong | modest |
| Our own lineage | aethyr-cpp | through Aethyr | the Grid | the galaxy | — |

## A family of names

One structure keeps both Aethyr names in play without overloading either:

| Layer | Name |
|---|---|
| Language | Coax |
| Engine (runtime: loop, spaces, physics, replay) | Aethyr |
| World creation (realm builder, editor, procedural tools) | Aethyrforge |
| The world people visit | the Verse |
| Model engine | Psionic |

"You forge realms with Aethyrforge, program them in Coax, and run them on
Aethyr."

## Recommendation

- **Engine: Aethyr.** It continues our own engine's name rather than starting
  over, it is the strongest platform-sounding name here, and the Verse already
  gives it context ("Aethyr, the engine behind the Verse"). If its fantasy lean or spelling becomes a real problem, Lattice is
  the neutral fallback.
- **Realm builder and editor: Aethyrforge.** It describes what /realms/new
  does better than "realm builder", and it reads as part of the same family.

## If we adopt it

Renaming touches little, because the engine core is new:

- #12609 and the Verse "Engine core" milestone titles;
- `docs/verse/engine-core.md`, and the module folder if the primitives move
  into `verse/<name>/`;
- AGENTS.md's `verse/` row and the Coax and Psionic book chapters that
  describe the engine;
- the changelog and about copy.

Nothing user-facing has to change until the name appears on the site.
