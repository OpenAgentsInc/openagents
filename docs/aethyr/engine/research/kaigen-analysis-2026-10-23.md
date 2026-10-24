# Kaigen research and implications for Aethyrforge

Reviewed October 23, 2026. This is research and a proposed experiment plan, not an
implementation change or an independently reproduced Kaigen benchmark.

A later [C/Wasm Medieval Town assessment](c-medieval-town-experiment-2026-10-23.md)
reviews the subsequently supplied offline archive, including the October ECS
rematch and additional architecture/loading posts. Its collection scope is
separate from the targeted lookup recorded below.

## Main finding

Kaigen's strongest lesson for Aethyrforge is to make a small, useful engine easy
for an agent to understand, exercise, and measure. A mature set of reusable
capabilities can make generated games smaller and development faster without
requiring the agent to invent a renderer, character controller, or asset pipeline
for each project. The evidence does not justify replacing Aethyrforge's language
split or copying another engine's architecture wholesale.

The most actionable opportunities are typed system access, complete playable
templates, reliable visual tests, measured player startup, and a documented agent
iteration loop. These complement the current browser-first design. Parallel
simulation is worth a controlled experiment after profiling; it is not a reason
to make shared-memory Wasm a hidden baseline requirement.

## Evidence and collection scope

- Read the public [documentation landing page](https://kaigen3d.com/docs/),
  [writing-applications guide](https://kaigen3d.com/docs/writing-applications), and
  [setup guide](https://api.kaigen3d.com/getting-started).
- Read the public [setup skill](https://github.com/Kaigen-Technologies/kaigen-plugins/blob/main/plugins/kaigen/skills/kaigen-setup/SKILL.md)
  as reference material. No plugin, engine, or installer was executed. The direct
  Markdown endpoint could not be fetched in this session; the GitHub copy supplied
  the additional setup details.
- Reviewed the existing 952-post index (third-party text, not imported), then made one
  authenticated read requesting 18 relevant posts by ID, including five entries
  in the January 8 thread. All 18 were returned without API errors. Full text,
  long-post fields, references, expanded links, and returned media metadata were
  inspected for analysis. This was a targeted lookup, not an exhaustive new search.
- Inspected the user-supplied `~/Desktop/img.jpeg`, a comparison of Flecs and
  Horizon component/system declarations. The corresponding
  [image post](https://x.com/gdechichi/status/2009314643425329485) was verified.
  The image itself is not committed here.
- No existing local archive was found at the tool's default output path. The bulk
  archive script was not run. This document saves the analysis and source links;
  it does not claim to be an offline copy of every source post, image, or video.
- Compared the committed Aethyrforge documents at base `54d9aaa`. Existing
  uncommitted implementation work in the main checkout was excluded. Runtime
  behavior was not retested for this research.

## What the public docs establish

The welcome page presents a closed beta and an agent-oriented entry path. Most
subsystem reference pages are labeled as forthcoming. Their sidebar presence is
not enough to establish their APIs or implementation quality.
[Source](https://kaigen3d.com/docs/).

The application guide documents three entry functions, application-owned state,
and frame control. Initialization and frame work execute across configured
threads using a single-program, multiple-data model. Its abstraction levels are
ECS systems with declared access, array kernels, direct thread partitioning, and
single-thread game logic. Synchronization and engine update calls still have
rules; choosing single-thread game logic does not remove every barrier obligation.
[Source](https://kaigen3d.com/docs/writing-applications).

The setup guide describes a C engine, version-pinned projects, templates,
separate build/cook commands, and optional platform components. Its agent workflow
includes building, input, debugging, and visual testing. The setup skill adds
structured diagnostic results and explicit failure codes. These are documented
interfaces, not capabilities exercised during this review.
[Setup guide](https://api.kaigen3d.com/getting-started),
[setup skill](https://github.com/Kaigen-Technologies/kaigen-plugins/blob/main/plugins/kaigen/skills/kaigen-setup/SKILL.md).

There is documentation drift: the welcome page offers Linux Studio, whereas the
GitHub skill says Linux is unavailable. Template defaults also differ between the
setup guide and skill. Do not infer current platform qualification from either
alone. Aethyrforge should publish versioned capability results from the same
source as its tooling instead of maintaining conflicting support prose.

## The January ECS thread and supplied image

The [January 8 root post](https://x.com/gdechichi/status/2009314635128951170)
reports a 100,000-boid comparison. Its author gives hardware and configuration in
[the next post](https://x.com/gdechichi/status/2009314637356126686): Windows 10,
Chrome using the high-performance GPU, Ryzen 7 5800H, RTX 3060 Laptop GPU, and
32 GB RAM, with approximately 31 million triangles per frame. A
[follow-up](https://x.com/gdechichi/status/2009314639335837822) says both builds
use WebGPU, Wasm threads, and optimized release configurations.

The crucial distinction comes in the
[timing clarification](https://x.com/gdechichi/status/2009314641567207920):

| Measurement reported by the author | Horizon/Kaigen | Unity | Derived comparison |
| --- | --- | --- | --- |
| Displayed scene FPS in the root post | About 90 | About 17 | About 5.3×; frame intervals roughly 11.1 versus 58.8 ms |
| ECS update throughput in the clarification | About 430 updates/s | About 22 updates/s | About 19.5×; update costs roughly 2.33 versus 45.45 ms |

These calculations use rounded social-post values. The headline's roughly 17×
and the follow-up's roughly 19.5× arithmetic are not an independently established
single result. Neither ratio predicts Aethyrforge's performance. The author's
GPU-bound explanation is plausible, but timing traces, synchronization boundaries,
seeds, quality parity, and repeatability are still needed to reproduce it.

The supplied image illustrates an API issue independently of those measurements.
The Flecs example retrieves component fields by query position; its system
registration must keep the same order. The Horizon example places typed
component parameters in a generated system declaration, with constness describing
read-only access. This removes a duplicated positional agreement from the user's
code. The image alone does not prove scheduler correctness, comprehensive race
safety, or any speed advantage. The current public guide uses different macro
spellings, so the January screenshot should not be treated as today's API reference.
[Image post](https://x.com/gdechichi/status/2009314643425329485).

For Aethyrforge, the transferable idea is one declaration of component identity,
type, and access mode. Generate query bindings and access descriptions together.
Rust's type system can reject some bad accesses, but cross-system conflicts,
stale entity handles, structural mutation, and Wasm/host buffer lifetime still
need explicit rules and tests. The semantic world remains the authoring authority;
a faster ECS representation is a derived runtime representation, not a replacement
for [world-model](../world-model.md) or [transaction](../contracts.md) contracts.

## Additional signals from the posts

| Topic and primary source | Evidence read | Implication for Aethyrforge |
| --- | --- | --- |
| [Visual-test failure, January 11](https://x.com/gdechichi/status/2010433509362680246) | The author describes an AI-written instancing test passing despite a visibly wrong reference. | Inspect reference validity; an image comparison can faithfully preserve the wrong output. |
| [Memory clearing, January 15](https://x.com/gdechichi/status/2011855100328005989) | A repeated frame-time memory clear was reported as a major cost. | Profile initialization, copies, allocation, and buffer reuse before changing renderer architecture. |
| [SSGI, January 23](https://x.com/gdechichi/status/2014739425939620279) | A half-resolution screen-space GI implementation reportedly added about 1.4 ms. | Treat screen-space GI as an experiment with an explicit quality and resolution profile, not a promised budget. |
| [Path tracer, February 18](https://x.com/gdechichi/status/2024162878698234070) | The author reports scene-specific speedups against Unreal and Unity. | No general ranking follows without scene, convergence, samples, denoising, and timing parity. |
| [Clay attribution, February 25](https://x.com/gdechichi/status/2026694766004457969) | The author attributes UI layout performance to Clay. | Ownership of engine contracts is compatible with selective infrastructure reuse. This is not evidence to replace React. |
| [Minecraft demo, September 24](https://x.com/gdechichi/status/2103220847674892299) | A cross-platform demo and high frame rate are advertised. | Measure the same small playable workload across actual target devices; do not generalize a demo's FPS. |
| [Starting template, October 7](https://x.com/gdechichi/status/2107835826269045156) | The author describes a roughly 400-line application using supplied character, sky, and editor systems. | Reduce the amount of engine plumbing an agent must generate before a game becomes playable. |
| [Claude of Duty port, October 11](https://x.com/gdechichi/status/2109360220178989281) | A port reportedly improved speed/loading and reduced application code. | Compare application code and reusable engine cost separately. A smaller game source does not imply less total engine code. |
| [Port optimization, October 16](https://x.com/gdechichi/status/2111155399009370187) | The reported speedup changed after further optimization on an M4 MacBook. | Pin build versions and test conditions; a ratio is a versioned experiment result. |
| [Compressed-size comparison, October 21](https://x.com/gdechichi/status/2112909842075754898) | The author compares an empty Unity build with a Kaigen demo. | Separate engine bytes, bootstrap code, assets, and first-play bytes; different workloads are not equal-size baselines. |
| [Minimal project, October 21](https://x.com/gdechichi/status/2112909844084892125) | A very small stripped empty project is reported. | Establish an owned minimal-player size baseline before adding editor dependencies to player startup. |
| [Optional modules, October 21](https://x.com/gdechichi/status/2112909845963915626) | The author says unused systems can be stripped. | Measure removal at the delivered artifact boundary, including JS imports and Wasm exports. |
| [Code-first positioning, October 22](https://x.com/gdechichi/status/2113304690154951130) | The author argues for reusable engine capabilities rather than regenerating them per game. | Give agents stable composition APIs and executable examples alongside semantic editing. |

Videos, graphical parity, and benchmark traces attached to these posts were not
independently evaluated. The post text supplies claims and experiment ideas,
not measured Aethyrforge acceptance results.

## Fit with Aethyrforge today

Use [implementation status](../implementation.md) for the local prototype's
implemented slice. Several broad design documents retain initial-design status
headers; they describe target obligations and should not erase the more specific
prototype evidence. Conversely, a working town renderer does not establish the
full planned behavior VM, multiplayer, collaboration, or release platform.

| Area | Current committed basis | Proposed response |
| --- | --- | --- |
| UI and frame scheduling | [Frontend](../frontend.md) separates React/Atom/Effect services from runtime frames. The prototype reports an independent preview worker. | Preserve this boundary. Measure UI contention instead of moving all presentation into the kernel. |
| Simulation and parallelism | [Runtime](../runtime.md) starts with coarse Wasm calls and a transferable-buffer baseline; shared memory is optional. | Benchmark serial data-oriented loops first, then a fixed worker pool on qualified shared-memory profiles. |
| Semantic editing | [AI authoring](../ai-authoring.md) routes manual and model actions through validated commands. | Let templates and agent tools compose supported operations without bypassing validation or authority. |
| Graphics iteration | The prototype reports retained working pipelines on shader failure and repeatable captures. | Build stronger independent reference checks on that base; passing shader compilation alone is insufficient. |
| Reusable gameplay | [Delivery](../delivery.md) still proposes the full gameplay/media milestone. | Add one small complete original game before treating a rendering demo as an agent-ready creation platform. |
| Player delivery | [Performance](../performance.md) specifies a compressed first-play target and separate player/editor concerns. | Measure an actual minimal release and its import graph; do not adopt another engine's 16 kB claim as our budget. |

## Recommended experiments, in order

These are proposed tasks, not implementation commitments or completed checks.
Keep experimental results separate from normative contracts until the evidence
supports a decision.

1. **Make one agent iteration reproducible.** Use an original tiny playable room
   with an interaction and restart condition. Document open, inspect, edit,
   capture, test, and close commands with stable JSON results. Record engine and
   artifact identity, camera, world time, input sequence, expected semantic
   outcome, and diagnostic locations. An agent must be able to make a supported
   change and verify it after a process restart without reading unrelated engine
   source. Include a deliberately failing change and show that its failure is
   reported rather than silently accepted.
2. **Protect visual tests against empty success.** Keep expected structure
   independent of generated reference images: object counts/IDs, camera framing,
   foreground coverage, and known occlusion relationships. Mutate a fixture to
   remove its instances, draw black, skip a pass, or capture before readiness.
   Each mutation must fail. Compare pinned images as additional evidence; never
   bless a newly generated reference solely because its producer says it passed.
3. **Create a simulation-only workload.** Use owned seeded boids at 1k, 10k,
   and 100k counts with fixed-step integration and specified neighbor-search
   behavior. Time update work without draw submission, then repeat with the same
   visible rendering workload. Record CPU spans, p50/p95/p99, state checksums,
   allocations, bridge calls, memory, and available GPU timings. A serial
   structure-of-arrays implementation is the baseline. Do not compare different
   neighbor algorithms as if they were merely different ECS implementations.
4. **Evaluate generated typed access.** Prototype one declaration that produces
   system bindings and read/write metadata. Test reordered fields, missing
   components, writes through read-only inputs, conflicting systems, entity
   deletion, and stale handles. Compare generated-code size, compile time,
   diagnostics, and steady update cost. Adopt only if it improves correctness
   or iteration without creating an opaque second schema owner.
5. **Measure optional parallel simulation.** Reuse the serial workload and
   checksums. Record one, two, four, and eight workers, startup and synchronization
   cost, browser isolation headers, total memory, and behavior without shared
   memory. Include a slow worker and cancellation. Parallel scheduling should
   earn its complexity on integrated frame and interaction measurements; fewer
   milliseconds inside one kernel alone are insufficient.
6. **Measure the smallest useful player.** Build three comparable artifacts:
   empty player, one interactable room, and room with character/physics/sky.
   Record raw and compressed JS/Wasm, required assets, cold requests, compilation,
   upload, and time to reliable input. Identify which optional modules actually
   disappear. Keep the editor and agent tooling out of the player import graph.
7. **Trial screen-space GI independently.** Pin resolution, camera path, lighting,
   and quality. Measure effect-on/off timings and inspect disocclusion, screen
   edges, thin geometry, motion, and offscreen lighting. Preserve an effect-free
   fallback. A screen-space method cannot recover arbitrary unseen geometry or
   replace all scene lighting. Do not confuse the prototype's contact attenuation
   with indirect illumination.

## Decisions this evidence does not support

- Rewriting Aethyrforge in C, replacing React with Clay, or adopting Kaigen as an
  engine dependency. The evidence concerns specific designs and workloads.
- Claiming Kaigen's displayed demos prove universal renderer superiority,
  production readiness, race freedom, or complete browser/device support.
- Making single-program, multiple-data execution the only authoring interface.
  Aethyrforge also needs bounded guest execution and editable semantic worlds.
- Removing capability checks, candidate validation, or the separate preview
  origin to make an agent edit look faster.
- Reimplementing every low-level engine feature through generated application
  code. Reusable systems should carry that complexity once and expose a small,
  documented interface.

The practical next step is the tiny playable project plus an honest automated
verification loop. It tests the creator benefit suggested by the posts while
producing evidence useful to every later runtime or rendering decision.
