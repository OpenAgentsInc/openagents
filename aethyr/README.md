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
| `entity.coax` | Entities: handles (slot and generation) inside a step, `EntityId`s for saves, the network and edits, and spawns and despawns that apply at the step's flush (a despawn before then cancels a spawn). Dead Gears runs on it |
| `query.coax` | One set of colliders a realm (flat/heightfield ground, boxes, capsules, floors) and four queries over it: ground, ray, overlap and sweep; the step height, steep slope and floor pad defined once |
| `ground.coax` | Checked sampling of actual rounded mesh triangles, metre inputs and exact stored vertices; shared with legacy terrain |
| `ground-query.coax` | Bounded exact cell traversal for terrain rays, with explicit exhausted-work refusal |
| `ground-contact.coax` | Sphere overlap and continuous sphere contact against triangle faces, edges, vertices and constant edge continuation |
| `collide.coax` | Checked kinematic capsule resolution, declared passes/travel and atomic refusal diagnostics |
| `collision-profiles.coax` | Explicit supported realm identities and their collision profiles; unknown identities refuse |
| `walk.coax` | The fixed-step walking controller and camera-state adapters; declared locomotion tuning; an ephemeral trace of the actual paid steps; `aethyr/player.coax` retains its C facade |
| `locomotion.coax` | Finite, bounded walking and grounded-route tuning, separate from capsule collision profiles |
| `flight.coax` | The walker's per-step flight hook: a realm's `FlightProfile` (Fairy Forest's is on) sets the vertical speed of each step in the air: a wing beat, the held climb, the glide, the ceiling and the dive |
| `walk-frame.coax` | Shared post-step targets and velocity records, with no C exports or persistent clock |
| `follower.coax` | Grounded route followers and fetch on the realm's paid steps: checked terrain, declared tuning, strict directions, atomic refusal and previous-pose drawing, with no second clock |
| `follow.coax` | The follow camera: look and zoom, a spring on the point it looks at, room from the colliders (pulled in at once, backed out gently), the eye drawn between steps, and its third- and first-person shots; it steps after the player and only reads it |
| `shot.coax` | The `CameraShot` every camera makes (an eye and the point it looks at): top down, the free orbit and the dive from a far shot down to a live one |
| `budget.coax` | Frame budgets: what one frame may use (steps, vertices, draw records, bytes), taken whole or refused and counted; every realm declares one and its check fails on any frame past it |
| `assets.coax` | Asset identity: an asset is the SHA-256 of its bytes; a registry admits a pack whole or refuses it (an entry that doesn't hash to its table's id, new bytes past the declared limit, no free slot), shares bytes already held and frees them with their last use |
| `render/frame.coax` | [Bounded CPU frame/pass/binding admission](render/README.md): resource slot/generation/epoch checks, atomic reservation/refusal and cleanup; adopted only at the Grid's native matrix-output boundary so far |
| `replay.coax` | Recording and replay: one envelope for every accepted external event, a gate that refuses duplicate, stale and unknown events, a digest every step, and replays that make no remote calls |
| `rand.coax` | Random streams: a draw is a pure function of seed, stream and counter |
| `scatter.coax` | Scatter from a seed: things on a jittered grid of world cells, each cell's draws keyed by its layer, version and grid coordinates, kept clear of keep-outs (book chapter 80) |
| `ai/stimulus.coax` | What a creature senses and knows: sight through `query_ray`, sound and touch as typed stimuli stamped by tick, and a bounded memory that forgets and counts what it refuses (book chapter 75) |
| `frame-report.coax` | A realm's frames, measured: frame statistics and the 60 FPS rule every realm is held to (`coax/web/realm-perf.coax` judges each case by it), and one frame's report by stage (book chapter 83) |
| `voice.coax` | Spoken requests (HOLD TO TALK): a press is a question on the replay gate, the voice host's final transcript its answer, applied once, only in its epoch; duplicates, answers after a cancel and unasked answers are refused, and the record replays every heard request with no recognizer (`examples/hold-to-talk.coax`; local recognition is resident Parakeet, `docs/voice/dev.md`) |
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

The engine modules that were in `verse/` moved here in one scripted commit
(ec-migrate, #12609), keeping their module names and C symbols:

| Module | What |
| --- | --- |
| `player.coax` | The walking controller's C surface over `walk.coax` |
| `camera.coax`, `cinematic.coax`, `arrange.coax`, `spacecam.coax` | The overview, dolly and free orbit; authored flights; the close-up arranging view; the floating-origin space camera |
| `points.coax` | The points and arithmetic the world modules share (was `verse/space.coax`; `space.coax` here is the typed spaces) |
| `stand.coax`, `terrain.coax`, `throw.coax`, `flock.coax` | Floors to stand on; heightfield ground, paths and followers; thrown and carried things; a following flock |
| `sticks.coax` | The touch controls: move stick, look finger, pinch, tap, the hotbar's slot |
| `figure.coax`, `motion.coax`, `skin.coax` | Soft figures, keyframed motions, skinned clips |
| `rigid.coax`, `contact.coax`, `gravity.coax`, `thrust.coax`, `vehicle.coax`, `orbit.coax` | Rigid bodies, contacts, gravity fields, thrusters, wheeled vehicles, orbits |
| `particles.coax`, `particle-motion.coax` | Particle emitters and their stateless motion |
| `command.coax` | Selection, groups and order queues for commanding units |

```sh
coax/bin/coax test aethyr/loop.coax --library
coax/bin/coax mutate aethyr/loop.coax
coax/web/tool.sh aethyr/refusals.coax      # mixing spaces fails to compile
coax/bin/coax test aethyr/tests/cross-realm.coax         # every walking realm: camera, feet, replay
coax/bin/coax test aethyr/tests/cross-realm-floor.coax   # the floor grid from any position
python3 psionic/tests/book.py --book aethyr --coax coax/bin/coax   # the book and its examples
```

The book's runnable programs are in `examples/`, with their exact output in
`fixtures/`.

The ground/collision consumer gate also includes the preserved native gameplay
programs in `tests/everglade-spells-integration.coax` and
`tests/deadgears-integration.coax`. They keep their original assertions and
finite cases while avoiding the interpreter's resource limits. Run
`python3 aethyr/tests/check_native_gameplay.py --realm REALM --mode MODE`
separately for each realm (`everglade`, `deadgears`) and mode (`debug`,
`release`, `sanitized`). The [chapter 8 commands](book/ch008-ground-and-collision.md#check-the-implementation)
show how sanitizer builds reuse admitted debug C. These are required companion
checks when their source closures change; ordinary imported authority checks
still run before emission.

The separate original-weight Linux CUDA streaming consumer,
[`psionic/examples/transcribe-incremental.coax`](../psionic/examples/transcribe-incremental.coax),
feeds real partials from recorded audio and admits one final through the same
voice replay gate. Partials never enter the realm record; cancellation revokes
late answers. The default voice host remains final-only.
[Qualification](../docs/verification/incremental-asr-2026-11-03/README.md).
