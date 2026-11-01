# Aethyr

Aethyr is the engine every Verse realm runs on, written in Coax. This folder
holds its core: a small set of primitives each realm uses instead of keeping
its own copy (GitHub #12609). Aethyrforge, the realm builder and
world-creation tools, is built on top of it. `verse/` keeps realm content.

The [Aethyr book](book/README.md) builds the engine from its smallest
primitives, one checked piece per chapter. Each module below has a chapter,
named by its task slug in the chapter's status block; fill the chapter in when
the module lands.

Each primitive is a module with its own checks and, where it decides
behaviour, zero surviving mutants. One short section per primitive is in
[docs/verse/engine-core.md](../docs/verse/engine-core.md).

| Module | What |
| --- | --- |
| `loop.coax` | The one frame loop: fixed-step simulation, interpolated drawing, one clock |
| `input.coax` | Keys, touch and pads to one `Intent` a frame; levels, presses each taken by exactly one fixed step, and look and zoom split over a frame's steps |
| `space.coax` | Typed spaces (world, local, view, screen, grid) and units (metres, grid units), converted only by name |
| `entity.coax` | Entities: handles (slot and generation) inside a step, `EntityId`s for saves, the network and edits, and spawns and despawns that apply at the step's flush |
| `query.coax` | One set of colliders a realm (ground, boxes, capsules, floors) and four queries over it: ground, ray, overlap and sweep; the step height, steep slope and floor pad defined once |
| `follow.coax` | The follow camera: look and zoom, a spring on the point it looks at, room from the colliders (pulled in at once, backed out gently), the eye drawn between steps, and its third- and first-person shots; it steps after the player and only reads it |
| `shot.coax` | The `CameraShot` every camera makes (an eye and the point it looks at): top down, the free orbit and the dive from a far shot down to a live one |
| `replay.coax` | Recording and replay: one envelope for every accepted external event, a gate that refuses duplicate, stale and unknown events, a digest every step, and replays that make no remote calls |
| `rand.coax` | Random streams: a draw is a pure function of seed, stream and counter |
| `math.coax` | Sine, cosine and powers with the same bits natively and in the browser, for simulation steps |
| `refusals.coax` | Checks that what must not compile doesn't: the fixtures in `tests/` |

## Audio

`audio/` synthesizes sound from integers: a sample is Q16, a phase a 32-bit
fraction of a cycle and time a count of samples, with no `libm` and no clock,
so the same score and seed give byte-identical samples on every build and
target ([book chapters 65–68](book/ch065-sound-from-numbers.md)).

| Module | What |
| --- | --- |
| `audio/synth.coax` | Oscillators (saw, square, sine), seeded noise, ADSR envelopes, decays, a one-pole filter, constant-power pan, soft saturation, Schroeder reverb lines |
| `audio/score.coax` | The beat grid, keys and triads, patterns and the sequencer, stereo buses with a reverb send, gain automation and the master |
| `audio/wav.coax` | 16-bit PCM WAV bytes and the FNV-1a hash of rendered samples |
| `audio/epic.coax` | The `promo-epic` score: 120 BPM in D minor, its section times as typed seconds |
| `tools/score.coax` | Renders a score to a WAV file (it never plays one) |

```sh
coax/bin/coax test aethyr/audio/epic.coax --library      # every audio check (it loads the rest)
coax/bin/coax mutate aethyr/audio/synth.coax --library
coax/bin/coax mutate aethyr/audio/score.coax --library --root-only
coax/web/tool.sh aethyr/tools/score.coax promo-epic OUT.wav [--seconds S] [--home S] ...
```

The older engine modules (`verse/player.coax`, the overview, dolly and orbit easing in `camera.coax`, `figure.coax`,
`contact.coax`, `terrain.coax` and the rest) move here with the realm
migration (ec-migrate).

```sh
coax/bin/coax test aethyr/loop.coax --library
coax/bin/coax mutate aethyr/loop.coax
coax/web/tool.sh aethyr/refusals.coax      # mixing spaces fails to compile
python3 psionic/tests/book.py --book aethyr --coax coax/bin/coax   # the book and its examples
```

The book's runnable programs are in `examples/`, with their exact output in
`fixtures/`.
