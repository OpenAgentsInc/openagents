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

The bounded [resource-cost streaming consumer](world/README.md) uses shared
owners on browser and native hosts. Its [adapter/memory inventory](host/stream-adoption.md)
and [qualification index](../docs/verification/aethyr-streaming-12990/README.md)
state the measured profile and remaining consumers; this is not automatic
qualification of every realm or the complete streaming chapter.

The [Physics Lab](lab/README.md) measures the existing rigid/contact/SAT
kernels beside pinned Rapier and Jolt references. It runs six shared seeded
scenes on coderos-4080, with JSON metrics, deterministic Aethyr repeats and
headless captures. Reference engines stay in its ignored lab cache.

## Layers

Loads go down the layers, never up (#13264). `aethyr/` is the engine:
nothing in it loads from `verse/`, `apps/`, `everglade/` or `coax/web/`.
`everglade/` is the game on the engine: nothing in it loads from `apps/`.
Product code in either folder does not load a file in a `tests/` folder.
Support code may load what it exercises: a path with a `tests`, `test`,
`tools`, `examples`, `book`, `fixtures` or `lab` folder, a test or check
file name (`checks.coax`, `X-checks.coax`, `check-X.coax`, `X_test.coax`),
or a program that defines `tool_main`.

`coax/web/tool.sh layer-check` checks this in a few seconds, with no build.
It prints the count of loads for each rule (`aethyr-up`, `everglade-up`,
`tests`). [`layer-allowlist.txt`](layer-allowlist.txt) lists the loads that
broke a rule when the check landed, one `RULE FROM -> TO` line each. The list
only shrinks. A new line fails unless it ends with a reason and an issue
number (`# why (#1234)`). A line that excuses nothing now also fails.

To remove a line, make the load go. Move what the file needs down into its
own layer, or have the caller pass it in. For a `tests` line, move the shared
code out of `tests/` into a module. Then delete the line and run
`coax/web/tool.sh layer-check`.

`coax/web/tool.sh layer-check --consumers` lists the engine modules that no
product loads, directly or through another module (#13272).

| Module | What |
| --- | --- |
| `loop.coax` | The one frame loop: fixed-step simulation, interpolated drawing, one clock |
| `input.coax` | Keys, touch and pads to one `Intent` a frame; levels, presses each taken by exactly one fixed step, and look and zoom split over a frame's steps |
| `space.coax` | Typed spaces (world, local, view, screen, grid) and units (metres, grid units), converted only by name |
| `entity.coax` | Entities: handles (slot and generation) inside a step, `EntityId`s for saves, the network and edits, and spawns and despawns that apply at the step's flush (a despawn before then cancels a spawn). Dead Gears runs on it |
| `query.coax` | One set of colliders a realm (flat/heightfield ground, boxes, capsules, floors) and four queries over it: ground, ray, overlap and sweep; the step height, steep slope and floor pad defined once |
| `visibility.coax` | The visibility service (#13073): whether a point sees a target, for name plates, health bars, damage numbers, targeting and monster sight. `vis_line` tests the colliders through `query_clear` and a terrain profile the realm samples; `vis_depth` tests a read-back depth image; `vis_settle` holds the answer against flicker. [Design](../docs/verse/visibility-service.md) |
| `query-filter.coax`, `query-scene.coax`, `query-fluid.coax` | Semantic queries: stable collider IDs/generations, checked entity references, category and role selection, separate physical surfaces, atomic overlap/volume results and bounded flat/heightfield fluids with holes. [The shared journey](examples/query-world.coax) uses the existing capsule, camera and AI kernels; [qualification and remaining adapters](../docs/verification/aethyr-query-12989/README.md) |
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
| `flyover.coax` | A camera path as text (`?fly=T,EX,EY,EZ,AX,AY,AZ~...`), parsed whole or refused and flown through with `cinematic.coax`'s Catmull-Rom, for benchmarks that cross a realm's heaviest views (#13017) |
| `budget.coax` | Frame budgets: what one frame may use (steps, vertices, draw records, bytes), taken whole or refused and counted; every realm declares one and its check fails on any frame past it |
| `assets.coax` | Asset identity: an asset is the SHA-256 of its bytes; a registry admits a pack whole or refuses it (an entry that doesn't hash to its table's id, new bytes past the declared limit, no free slot), shares bytes already held and frees them with their last use |
| `actors/` | [Compiled actor profile 1](actors/README.md): bounded sensing/weighted behavior, fenced navigation and model intents, scoped lifecycle and shared control; playable Actor Lab (#12992) |
| `state/` | [Declared facts and lifetimes](state/README.md), FST1 migration, retry and atomic six-resource world variants; playable State Lab and HH_Save v1 fact admission (#12991) |
| `world/` | [Cell and resource contracts](world/README.md): historical residency/edge/save primitives, decoder-derived RCM1 manifests and named cost admission; the bounded corridor adopts shared owners, semantic queries and persistent HUD on browser/WebGPU and native/Vulkan hosts, with accepted-history CPU replay. [Qualification and remaining adapters](../docs/verification/aethyr-streaming-12990/delivery-audit.md) |
| `render/frame.coax` | [Bounded CPU frame/pass/binding admission](render/README.md): resource slot/generation/epoch checks, atomic reservation/refusal and cleanup; adopted only at the Grid's native matrix-output boundary so far |
| `render/geometry/residency.coax` | Watertight geometry-page residency (#12999): a page installs only after the pages holding its clusters' parents and leaves only after every page that depends on it, so a partly loaded cut never has holes; parents are fetched before their children, within N slots, a fetch cap and least-recently-wanted eviction, all without the GPU (`#authority`, zero mutants). `pages.coax` keeps bytes and identity |
| `render/sky.coax`, `render/vegetation.coax` | Shared sky/cloud fields and world-space wind, grass, canopy, mask and fade functions; existing shaders consume them through `verse/style.coax` |
| `render/grass-root.coax` | Conservative whole-cell grass rejection against the existing root-clip planes; original instance IDs and density survive, with a bounded 128-range profile and independent terrain/corner oracle |
| `world/environment.coax` | Environment records, version 1 (lighting Phase 1, #13062): `EnvironmentKnot`, `EnvironmentTrack` (1–12 irregular knots, cyclic over a day), `EnvironmentProfile`, `EnvironmentRegion` (sphere/box/prism shape fields, bands, priority), `EnvironmentSet` (replaced whole by `env_set_replace` or refused), `EnvironmentState` and `ResolvedEnvironment`; separate key, direct, ambient, sky, fog, cloud-shadow, water and presentation channels; linear-colour and nlerp interpolation with an antipodal refusal; the glade as `env_glade_profile`, built from the `ENV_GLADE_*` constants that hold the painterly default's numbers once (#13060) (`#authority`, zero mutants) |
| `world/day.coax` | The day adapter for the environment resolver (lighting Phase 1, #13064): `DayClock` (version 1) with explicit fixed, cycle and real modes, a source (authored, authority, local), ticks per second and an epoch tick, read from `verse/style-preset.coax`'s `StyleClock` (`day_from_style`) or the room server's W header (`day_from_room`, anchored at a midnight tick so every seat holds the same clock); `day_at` and `day_state` give `env_resolve`'s `EnvironmentState` from the simulation tick, and a shared world refuses a visitor's local clock (`#authority`, zero mutants) |
| `world/weather.coax` | The weather adapter (#13064): `WeatherCondition` (version 1) from `verse/water-weather.coax`'s schedule and ground, global (`weather_cond_global`) apart from local spell overlays (`weather_cond_local`); `weather_env_apply` is Everglade's existing weather light on resolved values; `light_resolve` resolves day, condition and set, and `light_resolve_cached` keeps one value under its whole key (versions, set generation, profile, tick, world, clock, condition) (`#authority`, zero mutants) |
| `render/painterly.coax` | The painterly default's GPU evaluation (lighting Phase 1 step 0, #13060): Character Create's glade lifted out of the app, reading only the glade profile's `ENV_GLADE_*` numbers. The key (`paint_sun_dir`, `paint_sun_colour`: 4500 K under 6500 K, x2.15), hemisphere ambient and `paint_irradiance` (indirect occlusion and sun visibility on separate terms), the height air (`paint_air_depth`, `paint_phase` g 0.62, `paint_haze_window` clear to 30 m and thicker to 65 m, `paint_atmosphere`), the sky, soil and cloud shadows (`paint_sky`, `paint_soil`, `paint_cloud`: sun x0.45 in shade), and the presentation (`paint_film` at exposure 1.18 through `filmic.coax`'s `film_exposed`, `paint_output` with saturation 1.16). The glade's `cg_*` names are delegates of these. Since #13066 the enabled passes use the `paint_env_*` forms, the same arithmetic reading the frame's resolved environment record at word 208 (`PAINT_ENV_U`) in place of the constants |
| `render/transfer.coax` | The painterly transfer contract: decode display colour once (`transfer_linear`, `transfer_decode`), encode once (`transfer_srgb`, `transfer_encode`), and the HDR shoulder |
| `render/contact-shade.coax` | Octahedral normal packing (`shade_normal_pack`/`unpack`), view-point reconstruction from depth and the normal/depth contact taps the glade's post pass uses (Phase 5 builds on them) |
| `render/environment-frame.coax` | The camera's resolved environment, once a frame (lighting Phase 1, #13066): `EnvFrame` holds the realm's admitted set and region index, its day clock and the last record; `env_frame_write` resolves the environment at the eye (`env_resolve_camera`) at the day the clock gives and writes `env_gpu_pack`'s 108 words into a page's frame uniforms, holding the last record (or the glade's noon) on a refusal; `env_frame_room` takes the room server's day (`day_from_room`). Character Create, Everglade II, Experimental and 5E Lab write it at word 208, and `render/painterly.coax`'s `paint_env_*` read it (`#authority`, zero mutants) |
| `render/environment.coax` | A `ResolvedEnvironment` packed into the fixed 108-float GPU layout 1 (`env_gpu_pack`) and checked back (`env_gpu_unpack`, refused whole on any malformed word) (`#authority`, zero mutants) |
| `world/environment-region.coax` | Environment regions (lighting Phase 1, #13063): a map default plus sphere, box and prism (polygon-with-height) regions with smoothstep inner/outer bands (`inner + outer >= ENV_BAND_MIN`, self-crossing polygons refused); overlaps applied by priority, then the lower id last, so weights sum to 1 and stay continuous; a fixed wrapped 8×8 bucket index (`env_region_index_replace`, replaced with its set whole or refused) and bounded candidate lists (`env_tile_candidates`, at most 8, refused rather than truncated); two entry points, `env_resolve_camera` (grading and sky, at the eye) and `env_resolve_at` (surfaces and fog, from a stored tile) (`#authority`, zero mutants) |
| `world/building.coax` | Buildings placed in a realm: `BuildingFrame` (a centre turned in quarters), points, boxes and kit-piece transforms turned with it, and the 4 m one-room house of 2 m kit modules (`room4_box` walls with an open doorway, `room4_inside`, `room4_goal`: round to the doorway and in). Everglade's player home and Experimental's neighbourhood use it. |
| `world/build-site.coax` | Staged builds: a `BuildPart` (stage, form, material, ends, thickness) for posts, beams, woven or daubed walls, rings, lean-to, gable and cone roofs, slabs, trenches, stones and kit modules. Drawing stage k shows every part up to k. `bs_boxes` and `bs_world_boxes` give colliders that match what is visible: a finished wall blocks you, and you can walk between bare frame posts. |
| `world/build-site-mesh.coax` | Packer-side meshes for `build-site.coax` parts: woven withies, rough daub, fieldstone and dressed stone, leafy boughs, reed thatch with a ridge, and pegged timber. |
| `world/interact.coax` | Interactable spots: `InteractSpot` (a stable id, a place to stand, a facing and a reach) and `interact_pick`, the spot you use where you stand. The game keys the action on the id. |
| `arch/` | [The architecture kit](arch/README.md): buildings made from parts. Its village and flora library generates timber-frame cottages, stone houses, towers, a church, walls, street furniture, trees by species and season, hills and the ground, each from parameters |
| `render/stylized.coax`, `render/stylized-frame.coax` | [Stylized shading](render/README.md#stylized-shading): the hand-painted look as an engine feature (materials painted from metres, banded sunlight, soft shadows, aerial haze, painterly grass, dirt paths, a sky of big clouds); Painted Village draws with it |
| `render/shadow-filter.coax`, `render/post.coax` | Shared shadow sampling, exposure, saturation, black lift, bloom and light shafts; pass orchestration remains with each renderer |
| `action/` | [Actions, contacts and reactions](action/README.md): versioned tick-timed actions whose events fire once at any frame rate, swept blade sensors with attack-instance deduplication, typed attack/response/reaction tables, scoped hit stop, capped presentation, reflection, breakables and attachments; Everglade Experimental's weapon blows land on blade contact (#12993) |
| `cam/` | [Camera modes, attention and hand-back](cam/README.md): parameterized follow modes with inheritance and volume selection, the centred look-up with a ground-clamped eye, one attention query for lock-on/talk/look/action with hold and grace, two-subject framing, cinematic entry and hand-back by outcome; Everglade II's follow camera and its Character Create run, and Everglade Experimental's save-spell aim, use them (#12994) |
| `replay.coax` | Recording and replay: one envelope for every accepted external event, a gate that refuses duplicate, stale and unknown events, a digest every step, and replays that make no remote calls |
| `rand.coax` | Random streams: a draw is a pure function of seed, stream and counter |
| `scatter.coax` | Scatter from a seed: things on a jittered grid of world cells, each cell's draws keyed by its layer, version and grid coordinates, kept clear of keep-outs (book chapter 80) |
| `ai/stimulus.coax` | What a creature senses and knows: sight through `query_ray`, sound and touch as typed stimuli stamped by tick, and a bounded memory that forgets and counts what it refuses (book chapter 75) |
| `ai/pilot.coax` | Bounded external actor actions as typed `Intent`s and `ReplayEvent` envelopes; the Verse OS WoW adapter consumes it, without claiming replay of WoW's simulation (book chapter 75) |
| `talk/` | [Bounded native conversation profile](talk/README.md): authored fallback, UTF-8 request/reply admission, ticketed outcomes, tick deadlines and an owned answer record replayed without send intentions; realm and service adoption remain (book chapter 77) |
| `character/` | [The character runtime](../docs/verse/engine-core.md#characters-aethyrcharacter): a world's pack table (an ECH1 rig and its EFC1 face, sizes under declared maximums) and one `Character` record each, spawned, placed, stepped on the fixed step (locomotion graph, a looped idle clip or a held body; face clips and an idle face cycle) and despawned; it writes the skinning palette, morph weights and capsules the GPU reads. `idle.coax` adds blinks, saccades and breathing. Valley Lab's Echo runs on it (`coax/bin/coax test aethyr/character/checks.coax`) |
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
| `audio/attenuation.coax`, `audio/space.coax` | One checked distance law for world effects and Audio Lab: gain, range fade, propagation delay and lowpass; the camera adapter adds pan |
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
coax/bin/coax test aethyr/tests/cross-realm-made.coax    # made realms: feet and replay on the clock
python3 psionic/tests/book.py --book aethyr --coax coax/bin/coax   # the book and its examples
```

The book's runnable programs are in `examples/`, with their exact output in
`fixtures/`.

The ground/collision consumer gate also includes the preserved native gameplay
programs in `tests/everglade-spells-integration.coax` and the versioned
`tests/deadgears-integration-v2.coax`. The frozen Dead Gears v1 program and
manifest remain intact; v2 substitutes only the renamed `DgStore` backing
type, independently checked against every original body byte. Both retain
their original assertions and finite cases. Run
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

Animation Lab's reusable document readers, project admission, scene sampling,
scene checks and typed directing engine live in `anim/`. CLI and realm editors
share the same catalogue, schema admission and one-component edit writer.
Native version file effects live in `tools/anim-direction-store.coax`; browser
session/effect bindings remain in `verse/animlab/`. The former realm project,
scene and check paths are compatibility loaders. See the
[directing profile](../docs/verse/anim-directing-v1.md).

The component readers' shared closed-key lists live in `anim/field-catalog.coax`.
Those exact lists also generate CLI authoring help; regenerate with
`coax/web/tool.sh aethyr/tools/agent-help-build.coax` after changing a reader
catalogue. The CLI build checks the generated file for drift.
