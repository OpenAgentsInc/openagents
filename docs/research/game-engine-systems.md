# A comprehensive game-engine systems inventory

If you are designing an engine with **Unreal Engine as the scope benchmark**, I would inventory **65 major system families**, covering the runtime, editor, content pipeline, development tools, and surrounding integrations.

The most useful distinction is between:

- **Foundations:** decisions every other system depends on—identity, memory, scheduling, time, serialization, and resource ownership.
- **Runtime capabilities:** rendering, physics, animation, audio, networking, and so on.
- **Authoring and production systems:** everything needed to create, inspect, build, test, and ship content.
- **Reusable gameplay frameworks:** abilities, inventories, quests, combat, and other optional game-building blocks.
- **Specializations and integrations:** XR, robotics, large persistent worlds, online services, and platform-specific functionality.

No finite list can cover every possible game mechanic or simulation domain. The inventory below is intended to cover the engineering surface of a general-purpose engine comprehensively enough that you can explicitly mark areas **required, optional, externally supplied, deferred, or out of scope**.

**The inventory is my architectural synthesis.** The Unreal comparison near the end maps it to documented Unreal systems; it does not imply that Unreal implements every capability listed here. Unreal itself organizes functionality into modules and plugins, so matching its breadth does not require putting everything into one inseparable runtime. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/API?lang=en-US)

---

# A. Engine foundations

## 1. Engine composition, modules, and lifecycle

- **Module organization:** dependency graphs, public/private interfaces, registration, initialization order, shutdown order, optional modules.
- **Execution targets:** editor, standalone game, dedicated server, asset processor, command-line tool, embedded runtime.
- **Service lifetimes:** engine-wide, application-wide, session, world, local player, document, and temporary operation.
- **Backend replacement:** interchangeable rendering, physics, audio, storage, networking, and platform implementations.
- **Lifecycle correctness:** startup failures, partial initialization, cleanup, cancellation of outstanding work, safe reload and unload.
- **Multiple instances:** several worlds, game sessions, preview scenes, or simulation environments within one process.

## 2. Platform and operating-system abstraction

- **Application integration:** windows, event loops, focus, resizing, fullscreen transitions, display enumeration, high-DPI behavior.
- **Operating-system services:** threads, clocks, processes, dynamic libraries, environment/configuration access, clipboard, notifications.
- **Storage access:** files, directories, paths, permissions, file watching, asynchronous reads, memory mapping, sandboxed storage.
- **Device and lifecycle events:** suspend/resume, backgrounding, device removal, low-memory notifications, power and thermal state.
- **Platform capabilities:** supported graphics features, CPU features, input devices, storage limits, account and privilege interfaces.
- **Headless operation:** simulation and server execution without a window, display server, or unnecessary graphics dependencies.

## 3. Core libraries, containers, and memory

- **Foundational types:** strings, interned names, identifiers, arrays, maps, sets, queues, bitsets, optional/result types.
- **Memory allocation:** general allocators, arenas, pools, frame allocators, alignment, allocation tagging, fragmentation management.
- **Ownership models:** handles, generation counters, reference counting, garbage collection, explicit ownership, weak references.
- **Performance primitives:** SIMD, atomics, cache-aware layouts, structure-of-arrays storage, bulk operations.
- **Common utilities:** hashing, compression, checksums, random generators, parsing, formatting, file/archive helpers.
- **Memory diagnostics:** allocation histories, leaks, lifetime violations, high-water marks, per-system memory budgets.

## 4. Concurrency, jobs, and asynchronous execution

- **Job scheduling:** worker pools, task graphs, prerequisites, priorities, work stealing, batching, nested tasks.
- **Thread ownership:** game, rendering, audio, physics, I/O, and background-worker responsibilities.
- **Synchronization:** locks, atomics, fences, concurrent containers, message passing, snapshot exchange.
- **Async operations:** futures/promises, callbacks, cancellation, timeouts, progress reporting, completion guarantees.
- **Safe data access:** declared reads/writes, dependency tracking, deferred mutations, race detection.
- **Work budgeting:** time slicing, background compilation, streaming priorities, avoiding starvation and frame spikes.

## 5. Time, update loops, and determinism

- **Clock domains:** wall clock, simulation clock, rendering clock, audio clock, network clock.
- **Update policies:** fixed steps, variable steps, substeps, interpolation, tick groups, dependency ordering.
- **Time control:** pause, single-step, slow motion, time dilation, accelerated simulation, time warp.
- **Frame pacing:** frame limits, catch-up policies, overload handling, latency measurement.
- **Reproducibility:** seeded random streams, stable iteration order, deterministic scheduling where required, state checksums.
- **Determinism contracts:** same-run reproducibility, same-platform reproducibility, or cross-platform equivalence—explicitly chosen and tested.

A fixed timestep is one part of determinism; it does not by itself establish deterministic behavior across threads, compilers, processors, and GPUs.

## 6. Mathematics, transforms, and coordinate systems

- **Mathematical primitives:** vectors, matrices, quaternions, rotations, planes, bounds, curves, splines, interpolation.
- **Transform hierarchy:** local/world transforms, parent-child propagation, attachment, dirty tracking, nonuniform scale.
- **Coordinate conventions:** handedness, axis conventions, units, angular conventions, conversion between imported formats.
- **Precision strategy:** single/double precision, local coordinate frames, origin rebasing, camera-relative rendering.
- **Geometric operations:** intersections, distances, projections, barycentric coordinates, geometric predicates.
- **Numerical robustness:** tolerances, singularities, degenerate inputs, invalid-number detection, stable normalization.

## 7. Objects, entities, components, and reflection

- **World object model:** object-oriented actors, entity-component systems, or a deliberate hybrid.
- **Identity:** stable IDs, runtime handles, generations, object lookup, identity across unloading and restoration.
- **Composition:** components, archetypes, templates/prefabs, inheritance, variants, instance overrides.
- **Reflection:** runtime type descriptions, properties, methods, enums, attributes, editor metadata.
- **Lifecycle:** creation, activation, deactivation, destruction, pooling, change notifications.
- **Relationships:** ownership, attachment, references, groups, tags, queries, dependency relationships.
- **Data access:** iteration, indexing, change tracking, deferred structural changes, query caching.

## 8. Serialization, schemas, configuration, and versioning

- **Serialization formats:** binary, text, structured data, archives, streaming formats.
- **Schema management:** field IDs, defaults, optional fields, unknown fields, compatibility rules.
- **Version migrations:** asset upgrades, save upgrades, configuration migrations, deprecated types and properties.
- **Reference handling:** hard/soft references, reference reconstruction, external references, redirects.
- **Configuration:** engine/project/platform/user layers, command-line overrides, runtime settings, development versus shipping settings.
- **Data operations:** cloning, diffing, patching, validation, import/export, transaction records.

Asset files, save files, network messages, and editor transactions can share primitives while retaining different schemas and compatibility guarantees.

## 9. Programming, scripting, and live iteration

- **Native programming:** compiler integration, bindings, foreign-function interfaces, ABI boundaries, debugging symbols.
- **Runtime scripting:** interpreters or virtual machines, bytecode, script modules, execution budgets.
- **Visual programming:** node graphs, type checking, graph compilation, reusable functions, graph debugging.
- **Language tooling:** completion, navigation, diagnostics, API documentation, debugger integration.
- **Live iteration:** code reload, script reload, asset reload, state preservation, schema-change handling.
- **Execution control:** coroutines, async functions, latent actions, exception/error handling, script profiling.
- **Programmable tooling:** command-line APIs, editor automation, remote control, structured APIs usable by agents.

---

# B. Assets, resources, and world representation

## 10. Asset identity, registry, and import

- **Asset catalog:** stable IDs, paths, searchable metadata, types, tags, thumbnails, collections.
- **Dependency management:** references, dependency graphs, reference inspection, missing assets, orphan detection.
- **Importers:** meshes, scenes, textures, materials, animation, audio, video, fonts, structured data.
- **Import processing:** units, coordinate conversion, naming, validation, material translation, format normalization.
- **Reimport:** source tracking, import recipes, preservation of manual edits, change detection.
- **Provenance:** source files, generating tools, source revisions, author information, usage/license metadata where relevant.

## 11. Asset processing, cooking, and derived-data caching

- **Build graph:** source inputs, processing stages, dependency tracking, incremental rebuilds.
- **Derived-data cache:** local/shared/remote caches, content hashes, cache keys, eviction, integrity checking.
- **Platform conversion:** texture compression, audio encoding, animation compression, mesh processing, feature stripping.
- **Generated artifacts:** collision geometry, navigation data, lightmaps, shader libraries, LODs, impostors.
- **Execution:** parallel workers, distributed processing, resumable jobs, deterministic outputs.
- **Diagnostics:** why an asset rebuilt, cache-hit rates, processing times, output-size and quality reports.

## 12. Runtime asset loading and resource residency

- **Loading interfaces:** synchronous/asynchronous loading, dependency resolution, priorities, cancellation.
- **Resource ownership:** handles, pinning, reference lifetimes, residency tracking, eviction.
- **Streaming:** textures, geometry, animation, audio, world regions, downloadable content.
- **I/O pipeline:** read scheduling, decompression, deserialization, CPU processing, GPU upload.
- **Readiness:** placeholder resources, fallback assets, partial availability, loading-screen coordination.
- **Budget enforcement:** RAM, VRAM, I/O bandwidth, upload bandwidth, per-frame loading work.
- **Failure handling:** missing/corrupt data, unavailable downloads, insufficient storage, interrupted requests.

## 13. Scenes, worlds, and object organization

- **World containers:** scenes, maps, levels, subscenes, instances, preview worlds.
- **Hierarchy:** object grouping, attachment, nested scenes, reusable assemblies, world ownership.
- **Scene state:** active/inactive objects, visibility, simulation participation, layers, scenario variants.
- **World transitions:** creation, travel, unloading, seamless transitions, state transfer.
- **Multiple representations:** simulation objects, rendering proxies, physics bodies, audio emitters, navigation contributors.
- **World services:** environment settings, gravity, default lighting, global gameplay data, subsystem instances.

## 14. Spatial indexing and world queries

- **Acceleration structures:** bounding-volume hierarchies, grids, quadtrees/octrees, spatial hashes, region indexes.
- **Object queries:** nearby entities, containment, nearest neighbors, range searches, spatial subscriptions.
- **Visibility queries:** line of sight, room/portal connectivity, visibility sets, occluder lookup.
- **Spatial metadata:** zones, volumes, areas, influence fields, semantic regions.
- **Dynamic maintenance:** insertion, deletion, moving objects, bounds updates, index rebuilding.
- **Query execution:** batched queries, asynchronous queries, snapshot consistency, filtering.

Rendering, collision, navigation, and gameplay may share interfaces while using different underlying indexes.

## 15. World partitioning and large-world streaming

- **Partitioning:** cells, regions, sectors, hierarchical partitions, streaming boundaries.
- **Streaming sources:** players, cameras, predicted movement, teleport destinations, mission requirements.
- **Dependency-aware activation:** load order, reference resolution, activation versus mere asset residency.
- **Distant-world representations:** hierarchical LODs, simplified simulation, dormant entities, proxy content.
- **Persistent changes:** modified objects and terrain in unloaded regions, reconstruction after reload.
- **Authoring support:** partial world loading, regional editing, region ownership, streaming visualization.
- **Precision integration:** origin changes and coordinate conversions across rendering, physics, audio, networking, and saving.

## 16. Terrain, vegetation, water, and environment

- **Terrain representations:** heightfields, meshes, voxels, volumetric terrain, caves, overhangs.
- **Terrain authoring:** sculpting, painting, layer blending, erosion, deformation, terrain LOD.
- **Vegetation:** placement, instancing, density, biomes, growth states, bending, wind response.
- **Water:** oceans, lakes, rivers, shorelines, waves, underwater rendering, interaction surfaces.
- **Atmosphere:** sky, clouds, fog, sun/moon, stars, day/night cycles.
- **Weather and seasons:** precipitation, storms, snow, wetness, puddles, accumulation, seasonal variation.
- **Infrastructure:** roads, railways, paths, waterways, fences, spline-driven structures.
- **Shared environmental state:** wind, temperature, moisture, visibility, and surface conditions exposed to other systems.

## 17. Procedural generation and geometry processing

- **Generation framework:** typed graphs, procedural rules, custom nodes, seeds, reproducible generation.
- **Placement:** scattering, density fields, constraints, overlap prevention, biome rules, semantic placement.
- **Structural generation:** buildings, interiors, roads, dungeons, cities, modular assemblies.
- **Geometry operations:** mesh construction, booleans, cutting, subdivision, remeshing, simplification.
- **Surface processing:** UV generation, normals/tangents, material assignment, texture baking.
- **Runtime generation:** asynchronous jobs, chunked generation, collision/navigation rebuilding.
- **Author control:** baked results, manual exceptions, regeneration rules, provenance, incremental invalidation.

---

# C. Rendering and graphics

## 18. Graphics-device abstraction

- **Graphics backends:** whichever APIs you target—for example Vulkan, Direct3D, Metal, or WebGPU.
- **Device management:** adapter selection, feature queries, limits, capability negotiation, device-loss recovery.
- **GPU resources:** buffers, textures, samplers, views, descriptors, heaps, allocation and residency.
- **Command execution:** command buffers, queues, submissions, fences, barriers, synchronization.
- **Presentation:** swap chains, display modes, resizing, refresh synchronization, frame latency.
- **Pipeline management:** pipeline-state creation, caching, precaching, warmup, invalidation.
- **Diagnostics:** validation layers, debug labels, GPU timestamps, captures, crash information.

## 19. Renderer architecture and render graph

- **Scene extraction:** transferring simulation state into renderable data without unsafe ownership overlap.
- **Render graph:** passes, dependencies, resource lifetimes, pass culling, transient-resource reuse.
- **Scheduling:** graphics/compute/copy overlap, parallel command recording, asynchronous compute.
- **Rendering paths:** forward, deferred, clustered, tiled, mobile, ray-traced, or hybrid paths.
- **View management:** multiple cameras, split screen, stereo, reflections, scene captures, shadow views.
- **Frame resources:** double/triple buffering, history buffers, per-frame allocations, readback.
- **Extension points:** custom passes, custom renderers, capture outputs, offscreen/headless rendering.

## 20. Geometry processing, visibility, and draw submission

- **Geometry types:** static meshes, skinned meshes, procedural meshes, instances, geometry caches.
- **Visibility:** frustum, distance, occlusion, portal, hierarchical-depth, and small-object culling.
- **Submission:** sorting, batching, instancing, indirect draws, GPU-driven rendering.
- **Detail management:** mesh LOD, hierarchical LOD, billboards, impostors, virtualized geometry.
- **Deformation:** skinning, morphs, displacement, procedural vertex animation.
- **Ray-tracing geometry:** acceleration-structure building, refitting, updating, instance filtering.
- **Special passes:** depth, motion vectors, selection IDs, outlines, debug geometry.

## 21. Shaders and materials

- **Shader toolchain:** source languages, intermediate forms, backend compilation, reflection, binding layouts.
- **Compilation management:** dependency tracking, variants, permutations, caching, distributed compilation.
- **Material authoring:** graphs, functions, instances, overrides, shared parameters, previewing.
- **Surface models:** physically based shading, stylized shading, layered materials, coatings, anisotropy.
- **Specialized materials:** skin, eyes, hair, cloth, foliage, water, terrain, particles.
- **Transparency:** masking, blending, refraction, transmission, sorting, layered transparency.
- **Validation:** unsupported features, shader errors, instruction cost, sampler limits, platform fallbacks.

## 22. Textures and surface data

- **Texture types:** 2D, arrays, cube maps, volume textures, render targets, sparse textures.
- **Import processing:** color-space interpretation, channel packing, mip generation, compression.
- **Streaming:** mip selection, residency feedback, priorities, texture pools, virtual texture pages.
- **Runtime generation:** painting, decals, procedural textures, render-to-texture, surface accumulation.
- **Sampling:** filtering, anisotropy, addressing, derivative handling, normal-map conventions.
- **Surface representations:** lightmaps, masks, displacement, signed-distance fields, volume data.
- **Inspection:** mip/residency views, compression artifacts, memory cost, missing-tile diagnostics.

## 23. Lighting, shadows, and light transport

- **Light sources:** directional, point, spot, area, emissive, photometric profiles, projected patterns.
- **Light management:** culling, clustering, channels, influence bounds, dynamic light budgets.
- **Shadows:** shadow maps, cascades, atlases, virtual pages, contact shadows, ray-traced shadows.
- **Shadow maintenance:** caching, invalidation, filtering, bias, resolution allocation.
- **Baked lighting:** lightmaps, irradiance probes, volumetric lighting data, bake distribution.
- **Dynamic illumination:** indirect lighting, bounce lighting, emissive contribution, radiance/irradiance caches.
- **Reflections:** probes, planar reflections, screen-space reflections, ray-traced reflections.
- **Participating media:** volumetric light scattering, fog, atmospheric scattering, volumetric shadows.

## 24. Image reconstruction, post-processing, and display

- **Anti-aliasing:** spatial, multisample, temporal, and reconstruction approaches.
- **Temporal infrastructure:** motion vectors, histories, reprojection, disocclusion, history rejection.
- **Resolution control:** dynamic resolution, spatial/temporal upscaling, optional frame-generation integrations.
- **Image processing:** exposure, bloom, depth of field, motion blur, lens effects, custom effects.
- **Color pipeline:** linear-light processing, tone mapping, grading, color management, HDR/SDR output.
- **Composition:** transparency, UI, overlays, alpha handling, multiple output layers.
- **Capture:** screenshots, high-resolution stills, cubemaps, panoramas, photo-mode output.

## 25. Specialized rendering capabilities

These depend heavily on your target products.

- **Hair and fur:** strands, cards, meshes, shading, shadows, LOD transitions.
- **Volumetrics:** smoke, fire, clouds, sparse volumes, scientific volume rendering.
- **Alternative geometry:** point clouds, Gaussian splats, implicit surfaces, signed-distance representations.
- **Stylization:** outlines, cel shading, nonphotorealistic rendering, custom lighting models.
- **Advanced output:** path tracing, denoising, render layers, diagnostic/ground-truth passes.
- **Special views:** portals, mirrors, recursive views, multi-display installations, projection mapping.
- **Hardware-specific techniques:** variable-rate shading, foveation, specialized reconstruction, multi-GPU where justified.

---

# D. Physics, animation, and visual effects

## 26. Collision detection and scene queries

- **Shapes:** spheres, capsules, boxes, convex hulls, compounds, triangle meshes, heightfields.
- **Collision preprocessing:** simplification, convex decomposition, cooking, runtime rebuilding.
- **Broad phase:** identify potentially interacting pairs; maintain static and dynamic acceleration structures.
- **Narrow phase:** exact intersection tests, contacts, penetration depths, persistent contact manifolds.
- **Queries:** rays, sweeps, overlaps, closest points, distance queries, batched queries.
- **Filtering:** layers, masks, profiles, per-pair rules, query-only versus simulation geometry.
- **Events:** triggers, enter/exit, contact events, surface/material information.
- **Continuous collision:** handling fast-moving objects and tunneling.

## 27. Rigid-body dynamics and constraints

- **Bodies:** static, dynamic, kinematic; mass, inertia, center of mass, activation.
- **Forces:** gravity, force, torque, impulses, damping, custom force fields.
- **Contact solving:** friction, restitution, stabilization, solver iterations, stacking.
- **Constraints:** fixed, hinge, slider, ball/socket, six-degree-of-freedom joints.
- **Actuation:** motors, springs, limits, servos, break thresholds.
- **Execution:** islands, sleeping/waking, fixed steps, substeps, asynchronous simulation.
- **State management:** snapshots, interpolation, rewind, resimulation, deterministic modes where supported.

## 28. Specialized physical simulation

- **Characters:** ragdolls, active ragdolls, physical animation, recovery.
- **Flexible structures:** cloth, ropes, cables, chains, hair, fur, tendons.
- **Deformables:** soft bodies, flesh, elastic materials, plastic deformation.
- **Destruction:** fracture, structural connections, clusters, damage propagation, debris.
- **Vehicles:** tires, suspension, drivetrain, traction, modular assemblies.
- **Fluids and fields:** liquids, gases, buoyancy, hydrodynamics, wind, heat, combustion.
- **Ground materials:** soil, snow, sand, mud, granular materials, deformable terrain.
- **Solver coupling:** rigid–soft, rigid–fluid, cloth–body, thermal–mechanical interactions.

Visual fluid effects, gameplay-authoritative fluid interactions, and scientifically validated fluid simulation should have separate requirements.

## 29. Animation data and pose evaluation

- **Animation assets:** skeletons, clips, curves, events, compression, streaming.
- **Evaluation:** clip sampling, blend trees, blend spaces, state machines, transitions.
- **Layering:** additive animation, masks, per-bone blending, synchronization groups.
- **Motion selection:** pose databases, motion matching, trajectory queries, selection debugging.
- **Movement integration:** root motion, movement alignment, motion warping, stride/orientation warping.
- **Event timing:** animation notifies, synchronization markers, event ordering.
- **Performance:** parallel evaluation, pose caching, animation sharing, update throttling, animation LOD.

## 30. Rigging, deformation, and character presentation

- **Rigging:** controls, constraints, procedural rigs, inverse kinematics, full-body solving.
- **Retargeting:** skeleton mapping, proportion differences, runtime retargeting.
- **Contact adaptation:** foot placement, hand placement, look-at, aim constraints.
- **Deformation:** skinning, morph targets, corrective shapes, deformation graphs, learned deformers.
- **Face and speech:** facial rigs, expressions, eye movement, lip sync, blendshape driving.
- **Character assembly:** modular bodies, clothing, equipment attachment, customization.
- **Capture interfaces:** body/face/hand tracking, calibration, retargeting, recording.
- **Physics integration:** pose targets, physical readback, blending, cloth/hair binding.

## 31. Visual-effects systems

- **Effect structure:** systems, emitters, particles, attributes, modules, parameters.
- **Execution:** CPU/GPU simulation, spawning, lifetimes, events, sub-emitters.
- **Behavior:** forces, fields, collisions, constraints, scene sampling.
- **Rendering:** sprites, meshes, ribbons, beams, lights, volumes.
- **External data:** animation, geometry, audio, physics, gameplay, textures, custom data interfaces.
- **Authoring:** graphs, timelines, reusable modules, previews, parameter exposure.
- **Playback:** warmup, caching, seeking, deterministic seeds where useful.
- **Scalability:** pooling, significance, bounds, culling, simulation LOD, effect budgets.

---

# E. Gameplay, input, cameras, and AI

## 32. Gameplay framework and session structure

- **Game structure:** application instance, worlds, game rules, match phases, game state.
- **Player structure:** local users, controllers, controllable entities, possession, player state.
- **Lifecycle:** joining, leaving, spawning, respawning, reconnecting, spectating.
- **Gameplay communication:** events, commands, messages, delegates, subscriptions, tags.
- **Data-driven gameplay:** definitions, tables, registries, tunable parameters, feature activation.
- **Session behavior:** world transitions, local multiplayer, bots, pause rules, restart.
- **Development support:** gameplay inspection, debug commands, cheats, test hooks.

## 33. Reusable gameplay mechanics — optional frameworks

These are useful engine packages, but their rules usually belong to the game.

- **Abilities and attributes:** actions, costs, cooldowns, charges, modifiers, effects, stacking, cancellation.
- **Combat:** targeting, hit detection, damage, healing, projectiles, melee, reactions, death.
- **Items:** inventory, equipment, loot, crafting, durability, item definitions, ownership.
- **Progression:** experience, skill trees, unlocks, achievements, reputation, difficulty scaling.
- **Narrative:** quests, objectives, dialogue, conversations, branching conditions, relationship state.
- **World interaction:** usable objects, affordances, reservations, construction, destruction, harvesting.
- **Strategy and simulation:** resources, production, logistics, research, settlements, formations, fog of war.
- **Genre packages:** racing, sports, survival, farming, rhythm, puzzles, cards, board games, turn-based rules.
- **Presentation coordination:** gameplay cues connecting animation, VFX, sound, camera, and UI.

## 34. Input devices, mapping, and haptics

- **Devices:** keyboard, mouse, gamepad, touch, pen, motion controllers, wheels, flight controls.
- **Device lifecycle:** discovery, hotplugging, reconnects, assignment to users, capability detection.
- **Input representation:** raw events, timestamps, logical actions, axes, contextual mappings.
- **Processing:** dead zones, sensitivity, curves, inversion, smoothing, gestures.
- **Triggers:** press/release, hold, tap, double tap, chords, combinations, input buffering.
- **Routing:** gameplay versus UI, focus, capture, modal contexts, priority.
- **Customization:** rebinding, presets, conflicts, persistence, device-specific prompts.
- **Output/testing:** rumble, haptics, adaptive triggers, input recording and injection.

## 35. Character movement and traversal

- **Movement modes:** walking, running, crouching, jumping, falling, swimming, flying.
- **Ground interaction:** slopes, steps, ledges, grounding, penetration recovery.
- **Traversal:** climbing, ladders, vaulting, mantling, wall movement, grappling.
- **Reference frames:** moving platforms, moving vehicles, arbitrary gravity, zero gravity.
- **Motion sources:** input, AI, animation/root motion, impulses, physical forces.
- **Transitions:** mode changes, knockback, landing, ragdoll recovery, constrained movement.
- **Integration:** path following, collision, animation, prediction, reconciliation.

## 36. Cameras and viewing behavior

- **Camera types:** first person, third person, orbit, chase, top-down, free, orthographic.
- **Rigs:** spring arms, rails, cranes, splines, target tracking.
- **Framing:** look-at, composition, aim offsets, multiple targets, visibility preservation.
- **Obstructions:** camera collision, clipping prevention, obstruction fading.
- **Transitions:** blends, cuts, priorities, camera zones, cinematic handoffs.
- **Effects:** shake, recoil, procedural motion, lens/FOV changes.
- **Multiple views:** split screen, spectators, photo mode, replay cameras, XR views.

## 37. AI perception and decision-making

- **Perception:** sight, hearing, touch, damage stimuli, awareness, memory, forgetting.
- **Knowledge:** blackboards, world-state models, beliefs, goals, shared team information.
- **Decision frameworks:** state machines, behavior trees, utility systems, planners, task networks.
- **Environment queries:** cover, visibility, threats, tactical positions, usable objects.
- **Coordination:** teams, factions, squads, roles, task assignment, reservations.
- **Scheduling:** time slicing, budgets, sleeping agents, behavioral LOD, offscreen simulation.
- **Inspection:** decision traces, stimulus history, goal visualization, reproducible scenarios.

## 38. Navigation, pathfinding, and crowds

- **Navigation representations:** navmeshes, grids, waypoint graphs, voxel/volume navigation.
- **Generation:** static baking, dynamic updates, incremental rebuilding, streaming.
- **Agent profiles:** dimensions, movement capabilities, allowed surfaces, traversal costs.
- **Pathfinding:** asynchronous searches, hierarchical paths, cancellation, path invalidation.
- **Special traversal:** doors, elevators, ladders, jumps, moving platforms, off-mesh links.
- **Local movement:** steering, avoidance, formations, queuing, congestion, bottleneck handling.
- **Recovery:** stuck detection, replanning, unreachable targets, fallback behaviors.
- **Special domains:** flying, swimming, space navigation, large crowds, traffic.

---

# F. Audio, interfaces, accessibility, and localization

## 39. Audio playback and mixing runtime

- **Device layer:** outputs, formats, switching devices, channel layouts, latency.
- **Source playback:** decoding, streaming, buffering, seeking, looping, resampling.
- **Voice management:** priorities, concurrency, virtualization, voice stealing.
- **Spatial sound:** attenuation, panning, binaural rendering, Doppler, multiple listeners.
- **Acoustics:** occlusion, obstruction, reverb zones, reflection/propagation models.
- **Mixing:** buses, submixes, sends, sidechains, ducking, dynamic ranges.
- **DSP:** filtering, EQ, compression, limiting, convolution, delay, synthesis.
- **Diagnostics:** meters, underrun detection, voice inspection, audio-thread profiling.

## 40. Interactive audio, music, and audio authoring

- **Sound design:** reusable events, parameter graphs, randomization, layers, variations.
- **Procedural audio:** synthesis graphs, physical/event-driven sound generation.
- **Interactive music:** states, stems, transitions, tempo, beat/bar scheduling.
- **Dialogue:** voice assets, localization variants, lip-sync data, subtitle timing.
- **Gameplay integration:** animation events, impacts, surface-dependent sounds, environmental states.
- **Audio capture:** microphones, recording, voice-chat routing, analysis.
- **Pipeline:** import, conversion, cooking, previews, loudness checks, middleware integration.

## 41. UI, HUD, and text rendering

- **UI model:** widget trees, ownership, lifecycle, reusable components, themes.
- **Layout:** constraints, anchors, grids, scrolling, scaling, safe areas.
- **Rendering:** batching, clipping, caching, invalidation, world-space UI.
- **Text:** shaping, fonts, fallback fonts, glyph atlases, rich text, bidirectional layout.
- **Interaction:** hit testing, focus, keyboard/controller navigation, drag/drop, modal layers.
- **State:** data binding, view models, notifications, menu stacks, transitions.
- **Large interfaces:** virtualized lists, tables, search, filtering, asynchronous data.
- **Text entry:** selection, editing, IME, virtual keyboards, clipboard.

## 42. Accessibility, localization, and internationalization

- **Input accessibility:** full remapping, alternate controls, hold/toggle options, timing accommodations.
- **Visual accessibility:** text sizing, contrast, color-independent information, reduced motion.
- **Audio accessibility:** subtitles, captions, speaker labels, visual equivalents, separate mixes.
- **Assistive interfaces:** accessibility trees, screen readers, speech input/output integrations.
- **Localization pipeline:** stable text IDs, gathering, translation exchange, validation, language packaging.
- **Language behavior:** pluralization, grammatical variants, number/date formatting, collation, fallback cultures.
- **Localized content:** fonts, imagery, video, voice, layouts, runtime language switching.
- **Testing:** pseudolocalization, text expansion, missing translations, navigation and readability checks.

---

# G. Networking, persistence, and online operation

## 43. Network transport and connections

- **Transport abstraction:** sockets, platform transports, datagrams, reliable streams, relay adapters.
- **Connections:** discovery, handshakes, authentication, negotiation, timeouts, reconnects.
- **Packet mechanics:** framing, sequencing, acknowledgments, ordering, duplicate detection.
- **Delivery policies:** reliable/unreliable, ordered/unordered, fragmentation, reassembly.
- **Traffic efficiency:** serialization, bit packing, quantization, compression, bandwidth budgets.
- **Network conditions:** congestion, jitter, loss, latency estimation, clock synchronization.
- **Connectivity/security:** NAT traversal, relays, encryption/integrity interfaces, rate limits.
- **Diagnostics:** packet inspection, captures, simulated loss/latency/reordering.

## 44. State replication and interest management

- **Network identity:** entity IDs, reference mapping, creation/destruction, ownership.
- **Synchronization:** initial state, late joins, property/component replication, dirty tracking.
- **Delta updates:** baselines, change masks, incremental collections, state compression.
- **Remote commands:** RPC routing, validation, ordering, execution authority.
- **Interest management:** distance, visibility, teams, ownership, subscriptions, information hiding.
- **Scheduling:** priorities, update rates, dormancy, bandwidth allocation.
- **World integration:** streaming boundaries, travel, resynchronization, ownership transfers.
- **Compatibility:** protocol versions, schema evolution, supported client/server combinations.

## 45. Prediction, reconciliation, rollback, and networked physics

- **Authority model:** server authority, peer authority, deterministic lockstep, rollback, or hybrids.
- **Prediction:** local inputs, input history, speculative state, predicted actions.
- **Correction:** authoritative snapshots, reconciliation, smoothing, error thresholds.
- **Interpolation/extrapolation:** remote movement, buffering, missing updates.
- **History:** snapshots, rewind windows, resimulation, state checksums.
- **Lag compensation:** historical queries, hit validation, fairness policies.
- **Physics networking:** state replication, predictive interpolation, resimulation, interaction consistency.
- **Side-effect handling:** avoiding duplicated audio, particles, rewards, or gameplay events during rollback.

## 46. Save/load, persistence, and replay

- **Save architecture:** profiles, slots, checkpoints, autosaves, persistent/transient state.
- **World restoration:** stable IDs, reconstruction order, reference repair, streamed-region changes.
- **Reliability:** consistent snapshots, asynchronous writes, atomic replacement, backups, corruption handling.
- **Compatibility:** save migrations, content dependencies, missing mods, removed features.
- **Cloud persistence:** synchronization, conflicts, offline changes, quotas.
- **Replay models:** input recording, state recording, event recording, deterministic resimulation where supported.
- **Replay tools:** checkpoints, seeking, scrubbing, playback speed, cameras, annotations.
- **Long-lived state:** server snapshots, event logs, recovery, archival.

**Save/load, replay playback, and rollback simulation are distinct capabilities.** For example, Unreal’s standard Replay System records data through its replication infrastructure using `DemoNetDriver`; that does not establish a general whole-engine deterministic rewind system. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/using-the-replay-system-in-unreal-engine)

## 47. Online-service integrations

- **Identity:** login, account linking, session renewal, logout, platform users.
- **Social:** friends, presence, invitations, parties, blocking, reporting.
- **Session discovery:** lobbies, server browsers, matchmaking interfaces, region selection.
- **Communication:** voice, text chat, muting, channel management.
- **Player records:** achievements, statistics, rankings, leaderboards, cloud files.
- **Commerce:** catalogs, entitlements, purchase flows, ownership verification.
- **Platform privileges:** crossplay settings, communication restrictions, service availability.
- **Resilience:** retries, idempotency, timeouts, offline fallback, maintenance states.

## 48. Server hosting, backend operation, and live updates

These generally include systems outside the engine executable.

- **Server fleets:** allocation, startup, health checks, scaling, draining, shutdown.
- **Routing:** regions, instances, shards, session placement, reconnect routing.
- **Live configuration:** tuning data, feature flags, cohorts, scheduled events, emergency disable switches.
- **Content delivery:** downloadable bundles, manifests, compatibility, staged rollouts.
- **Telemetry:** event schemas, sampling, dashboards, gameplay and operational metrics.
- **Administration:** player support, moderation tools, server controls, audit trails.
- **Release operation:** deployment coordination, canaries, rollback, client/server version overlap.
- **Backend interfaces:** progression, inventories, economies, transaction processing, durable storage.

---

# H. Editor, development tools, and production pipeline

## 49. Editor application framework

- **Workspace:** windows, docking, tabs, layouts, menus, commands, keyboard shortcuts.
- **World interaction:** viewports, selection, outliner, gizmos, snapping, pivots, measurement.
- **Inspection:** reflection-driven property editors, custom property controls, multi-object editing.
- **Transactions:** undo/redo, command history, dirty state, copy/paste, duplication.
- **Document handling:** opening/saving, autosave, recovery, multiple documents, preview scenes.
- **Play workflows:** play-in-editor, simulate-in-editor, standalone launch, multi-client launch.
- **Live inspection:** pause, single-step, runtime editing, explicit apply/revert behavior.
- **Automation:** scriptable commands and structured APIs sharing the editor’s transaction and validation rules.

## 50. Specialized authoring tools

- **World tools:** level design, terrain, foliage, water, splines, partition management.
- **Asset tools:** mesh inspection, LODs, collision editing, UVs, materials, textures.
- **Character tools:** skeletons, rigs, animations, retargeting, physics assets, cloth, hair.
- **Graph editors:** gameplay, materials, animation, AI, VFX, audio, procedural generation.
- **Timeline tools:** curves, keyframes, sequences, events, takes.
- **Gameplay tools:** abilities, items, quests, dialogue, spawn data, navigation, balancing.
- **Shared infrastructure:** graph widgets, property panels, previews, validation, search, diffs.
- **Bulk workflows:** batch changes, mass renames, scripted fixes, migration tools.

## 51. Collaboration, source control, and change management

- **Source-control interfaces:** status, checkout, locking, changelists, history, revert.
- **Asset-aware changes:** rename/move tracking, dependency updates, reference repair.
- **Diff/merge:** objects, scenes, properties, graphs, text, binary-asset inspection.
- **Concurrent authoring:** presence, shared sessions, edit ownership, transaction synchronization.
- **Conflict handling:** conflicting edits, stale state, session recovery, review.
- **Workspace management:** large files, partial synchronization, shared caches, project setup.
- **Upgrade workflows:** deprecation reports, migrations, compatibility checks, branch/version transitions.

## 52. Build, package, deployment, and patching

- **Code builds:** targets, module dependencies, code generation, compiler/linker configuration.
- **Toolchains:** SDK discovery, version pinning, installation, platform configuration.
- **Automation:** incremental/distributed builds, build graphs, CI workers, artifact management.
- **Release outputs:** debug/development/shipping builds, dedicated servers, symbols, build IDs.
- **Packaging:** cooking, staging, containers, compression, signing, dependency closure.
- **Content distribution:** chunks, install order, DLC, downloadable assets, differential patches.
- **Deployment:** device discovery, install, launch, debugger attachment, log retrieval.
- **Update reliability:** resumable downloads, verification, disk-space planning, rollback, executable/content compatibility.

## 53. Debugging, profiling, and observability

- **General diagnostics:** logs, assertions, console commands, object inspectors, debug drawing.
- **CPU analysis:** frame timelines, tasks, threads, locks, stalls, scheduling.
- **GPU analysis:** passes, timestamps, memory, draw calls, captures, device-loss diagnostics.
- **Memory:** allocations, leaks, ownership, fragmentation, resource budgets.
- **Loading/build analysis:** streaming hitches, asset dependencies, cache misses, cook/build times.
- **System-specific tools:** physics, animation, navigation, AI, audio, UI, replication.
- **Failure reporting:** crash dumps, symbolication, bucketing, hangs, watchdogs.
- **Remote operation:** live traces, saved sessions, comparison tools, production performance telemetry.

## 54. Automated testing and validation

- **Code tests:** unit, integration, property-based, fuzz, concurrency and lifetime tests.
- **Content tests:** asset validity, broken references, import/reimport, budget violations.
- **Gameplay tests:** functional scenarios, bots, input-driven tests, progression checks.
- **Visual/audio tests:** screenshots, rendering comparisons, audio captures, synchronization.
- **Networking tests:** multiple clients, loss/latency, reconnects, desynchronization, version mismatches.
- **Persistence tests:** migrations, corrupted saves, interrupted writes, restoration.
- **Performance tests:** benchmarks, regressions, stress, soak, memory growth.
- **Shipping tests:** packaged builds, installation, patches, platform/device matrices, failure-artifact collection.

## 55. Plugins, extensions, mods, and user-generated content

- **Plugin packaging:** descriptors, dependencies, versions, discovery, load order.
- **Extension APIs:** new asset types, importers, components, render passes, tools, script bindings.
- **Feature packages:** activation/deactivation, downloadable features, cleanup, dependency resolution.
- **Mod workflows:** SDKs, project templates, packaging, installation, compatibility testing.
- **UGC distribution:** publishing, discovery, updates, ratings/reporting, content-service interfaces.
- **Compatibility:** removed mods, save dependencies, conflicts, overridden assets, API migration.
- **Untrusted execution:** sandboxed scripting, capabilities, quotas, watchdogs, process isolation where needed.

Native plugin loading and secure execution of untrusted code require different designs.

## 56. Security, integrity, and trust boundaries

- **Input validation:** network messages, asset formats, save files, scripts, remote commands.
- **Authority enforcement:** server-side checks, command permissions, ownership validation.
- **Resource protection:** message limits, decompression limits, execution budgets, request throttling.
- **Content integrity:** signatures, hashes, manifests, trusted sources, update verification.
- **Credential handling:** authentication tokens, platform credentials, server secrets, log redaction.
- **Anti-cheat interfaces:** detection integrations, evidence capture, enforcement hooks.
- **Tool isolation:** import workers, build workers, remote editor access, debug facilities excluded or restricted in shipping.

---

# I. Specialized targets and broader engine capabilities

## 57. Cinematics, media, and virtual production

- **Sequencing:** tracks, channels, keyframes, nested sequences, blending, property animation.
- **Gameplay interaction:** triggering, interruption, skipping, branching, state restoration.
- **Production workflow:** shots, takes, camera rigs, recording, editorial interchange.
- **Offline rendering:** render queues, distributed jobs, sampling, render passes, image sequences.
- **Media playback:** decoding, streaming, buffering, playlists, synchronization, media textures.
- **Media I/O:** capture devices, video inputs/outputs, recording, compositing, keying.
- **Virtual production:** tracking, lens calibration, timecode, genlock, multi-machine synchronization, LED-wall workflows.

## 58. Dedicated 2D and 2.5D systems

- **Sprites:** atlases, sprite sheets, flipbooks, batching, sorting.
- **Tile worlds:** tilemaps, tilesets, terrain rules, autotiling, chunk streaming.
- **2D animation:** skeletal animation, mesh deformation, frame animation, attachments.
- **2D physics:** shapes, joints, sensors, collision, movement.
- **2D presentation:** orthographic cameras, parallax, pixel snapping, pixel-perfect scaling.
- **2D lighting:** shadows, masks, normal-mapped sprites, layered effects.
- **Hybrid integration:** 2D elements in 3D worlds and 3D content in 2D games.

## 59. Virtual, augmented, and mixed reality

- **XR runtime integration:** sessions, devices, capabilities, tracking spaces.
- **Tracking:** head, hands, controllers, eyes, body, anchors.
- **Rendering:** stereo, multiview, foveation, late updates, compositor layers.
- **Interaction:** grabbing, pointing, gestures, hand poses, spatial UI.
- **Mixed reality:** passthrough, depth, occlusion, scene understanding, real-world alignment.
- **Locomotion:** teleportation, continuous movement, comfort settings, seated/standing modes.
- **Spatial persistence:** anchors, shared spaces, relocalization, room/world mapping.
- **Testing:** tracking loss, recentering, device performance, motion-to-photon latency.

## 60. Mobile, browser, console, and streamed execution

- **Mobile:** touch interfaces, suspend/resume, thermal budgets, battery use, variable display sizes.
- **Browser:** WebAssembly/JavaScript integration, graphics capabilities, browser storage, download strategy, worker constraints.
- **Console:** platform users, storage, device disconnects, suspend/resume, platform service integration.
- **Cloud streaming:** video encoding, interactive streaming transport, input return paths, session provisioning.
- **Adaptive quality:** hardware profiles, automatic settings, dynamic budgets, graceful degradation.
- **Deployment environments:** app bundles, browser caching, launchers, platform manifests, updates.
- **Platform diagnostics:** device labs, remote logs, performance profiles, crash collection.

These are target choices for your engine, not a claim that Unreal natively supports every listed execution path.

## 61. Machine learning, agents, and training environments

- **Inference:** model assets, runtime backends, tensor layouts, CPU/GPU execution.
- **Scheduling:** batching, asynchronous inference, resource sharing, latency budgets.
- **Model lifecycle:** import, optimization, quantization, versioning, replacement, fallback behavior.
- **Training interfaces:** observations, actions, rewards, episode resets, terminal conditions.
- **Parallel environments:** isolated worlds, batched stepping, accelerated/headless execution.
- **Data generation:** images, depth, segmentation, motion, annotations, ground-truth state.
- **Experiment control:** domain randomization, scenario generation, seeds, provenance.
- **Agent interaction:** semantic world queries, action APIs, tool permissions, recorded decisions, automated playtesting.

## 62. Robotics and digital-twin capabilities

- **Robot representations:** articulated bodies, joints, links, inertial parameters, limits.
- **Actuators/controllers:** motors, servos, control loops, trajectories, actuator dynamics.
- **Sensors:** cameras, LiDAR, radar, IMU, GNSS, contact, force/torque.
- **Sensor modeling:** noise, latency, calibration, distortion, rolling shutter, synchronization.
- **Interchange:** robot descriptions, CAD/scene imports, robotics middleware bridges.
- **Simulation integration:** external solvers, co-simulation, hardware-in-the-loop.
- **Validation:** measured-reference comparisons, accuracy budgets, uncertainty, reproducibility.
- **Operational state:** equipment telemetry, live synchronization, historical playback, scenario testing.

## 63. Scientific, planetary, space, and geospatial simulation

- **Coordinate frames:** planetary, geographic, inertial, rotating, local, vehicle-relative.
- **Planetary worlds:** curved terrain, geographic datasets, large-scale streaming, atmosphere transitions.
- **Orbital mechanics:** gravity models, propagators, trajectories, reference-body changes.
- **Spacecraft:** propulsion, fuel, staging, docking, attitude control, thermal/electrical models.
- **Time scales:** accelerated time, event detection, adaptive stepping, transitions between simulation modes.
- **Multiphysics:** specialized fluid, thermal, electromagnetic, structural, or chemical solvers.
- **Scientific workflow:** units, error estimates, validation datasets, experiment configuration, result export.
- **Visualization:** scientific fields, volumes, trajectories, sensor outputs, uncertainty displays.

## 64. Massive persistent and distributed worlds

- **World allocation:** shards, zones, instances, region assignment.
- **Authority distribution:** ownership of entities/regions, handoff, cross-boundary interactions.
- **Cross-server communication:** messaging, routing, interest propagation, consistency policies.
- **Persistence:** durable entities, event logs, recovery, snapshots, schema migration.
- **Shared services:** inventories, economies, guilds, social systems, world events.
- **Failure handling:** server loss, duplicate requests, split ownership, reconnect restoration.
- **Scale management:** load balancing, simulation LOD, capacity planning, operational observability.
- **Live evolution:** rolling upgrades, version overlap, regional migrations, world maintenance.

World streaming inside one process and simulation distributed across multiple servers are separate engineering problems.

## 65. Engine distribution, SDKs, and developer ecosystem

- **Distribution:** source/binary releases, installers, engine versions, project association.
- **SDKs:** stable APIs, generated bindings, extension templates, sample applications.
- **Documentation:** architecture guides, API references, tutorials, examples, migration instructions.
- **Project creation:** templates, starter content, feature selection, target configuration.
- **Dependency management:** third-party libraries, middleware, plugin compatibility, notices and provenance.
- **Upgrades:** project conversion, deprecation tooling, compatibility reports, automated fixes.
- **Supportability:** diagnostic bundles, reproducible bug projects, issue templates, release notes.

---

# Unreal Engine comparison map

These are the most useful Unreal reference points to study for the corresponding areas. Some are core facilities; others are plugins or separate production tools. Their names should be treated as implementation references rather than required names or boundaries for your own engine.

| Area | Unreal reference points |
|---|---|
| Modules and object metadata | Unreal Modules, `UObject`, reflection, reflected properties and functions. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engine-modules) |
| Parallel work | Tasks System and its task dependencies, nested tasks, synchronization, and task chains. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/tasks-systems-in-unreal-engine) |
| Asset discovery and loading | Asset Registry, Asset Manager, asynchronous asset loading. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/asset-registry-in-unreal-engine) |
| Content production | Derived Data Cache, cooking, staging, packaging, Zen-related storage tools. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/setting-up-your-production-pipeline-in-unreal-engine) |
| World organization and streaming | World Partition, Data Layers, Level Instancing, One File Per Actor, hierarchical LOD. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/world-partition-in-unreal-engine) |
| Procedural content | PCG Framework and its procedural graph/tooling model. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/procedural-content-generation-overview?lang=en-US) |
| Graphics infrastructure | Render Hardware Interface and Render Dependency Graph. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/graphics-programming-for-unreal-engine) |
| Geometry, materials, and lighting | Nanite, Substrate, Lumen, Virtual Shadow Maps, Path Tracer. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/designing-visuals-rendering-and-graphics-with-unreal-engine) |
| Physics | Chaos rigid bodies, destruction, cloth, vehicles, fields, networked physics, and debugging. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/physics-in-unreal-engine) |
| Animation and rigging | Animation Blueprints, Control Rig, Motion Matching through Pose Search. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-engines-systems-and-workflows-overview-for-unity-developers) |
| Visual effects | Niagara systems, emitters, modules, parameters, and data interfaces. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/overview-of-niagara-effects-for-unreal-engine) |
| Gameplay framework | Actors, Components, Controllers, GameMode, GameState, player state, Gameplay Ability System. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/gameplay-systems-in-unreal-engine?lang=en-US) |
| Input | Enhanced Input actions, mapping contexts, modifiers, and triggers. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/enhanced-input-in-unreal-engine) |
| AI and navigation | Behavior Trees, StateTree, EQS, AI Perception, Smart Objects, Navigation System, MassEntity. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/artificial-intelligence-in-unreal-engine) |
| Audio | Audio Mixer, MetaSounds, Audio Modulation, Quartz. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/audio-in-unreal-engine-5) |
| UI and localization | Common UI for layered interfaces and input routing; localization/internationalization facilities for language and culture behavior. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/common-ui-plugin-for-advanced-user-interfaces-in-unreal-engine) |
| Multiplayer | Replication, Iris, Replication Graph, Replay System. [Epic Developer Community](https://dev.epicgames.com/documentation/unreal-engine/networking-and-multiplayer-in-unreal-engine) |
| Online integrations | Online Services and Online Subsystem interfaces. These provide access to services; they are not themselves an operated backend. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/overview-of-online-services-in-unreal-engine) |
| Editor automation | Editor scripting, Blueprint/Python automation, editor utility and remote-control facilities. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/scripting-and-automating-the-unreal-editor) |
| Build automation | Horde and BuildGraph for distributed build workflows and artifact movement. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/horde-build-automation-for-unreal-engine) |
| Profiling and testing | Unreal Insights, Automation Test Framework, functional and screenshot testing. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/unreal-insights-in-unreal-engine) |
| Modular features | Plugins, Game Features, and Modular Gameplay. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/plugins-in-unreal-engine) |
| Cinematics | Sequencer, Take Recorder, Movie Render Pipeline. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/cinematics-and-movie-making-in-unreal-engine) |
| Specialized targets | Paper 2D; XR development facilities; Neural Network Engine for inference-runtime abstraction; Georeferencing and Large World Coordinates. [Epic Developer Community](https://dev.epicgames.com/documentation/en-us/unreal-engine/paper-2d-overview-in-unreal-engine) |

---

# The checklist that makes each subsystem complete

A feature inventory becomes much more useful when every entry has the same completion criteria.

For **each subsystem above**, I would track the following:

| Dimension | Questions to answer |
|---|---|
| **Purpose and scope** | What does it guarantee? Which cases are deliberately unsupported? |
| **Ownership and lifetime** | Who owns its state? What happens when a world, object, plugin, or session disappears? |
| **Data model** | What is the source representation, runtime representation, and stable identity? |
| **Authoring** | How do developers create, edit, preview, and validate it? |
| **Serialization and migration** | How is it stored? How do old assets and saves survive changes? |
| **Scheduling** | When does it run? Which threads may access it? What are its synchronization points? |
| **Streaming** | Can it load incrementally, cancel, unload, and recover from missing dependencies? |
| **Networking** | Which state is authoritative? What replicates? What may be predicted or hidden? |
| **Persistence and replay** | What must be saved or recorded? Can it restore or resimulate correctly? |
| **Performance** | What are its CPU, GPU, RAM, VRAM, I/O, and bandwidth budgets? |
| **Scalability** | What can degrade gracefully? What must remain behaviorally correct? |
| **Debugging** | Can you inspect its state, explain its decisions, and reproduce its failures? |
| **Testing** | What verifies correctness, compatibility, stability, and performance? |
| **Platform coverage** | Which targets and feature combinations work? What are the fallbacks? |
| **Extensibility** | How can a game or plugin customize it without modifying unrelated internals? |
| **Shipping support** | Does it work in cooked, packaged, stripped, headless, and production configurations? |

## Cross-system contracts to decide early

Several decisions affect almost the entire inventory:

1. **Identity and references.** Decide how an object remains identifiable across saves, streaming, networking, editor changes, and reloads.

2. **Transform authority.** Define when movement, animation, physics, networking, and cinematics may write an object’s transform.

3. **Time and update order.** Establish the relationships between simulation ticks, physics steps, animation evaluation, audio scheduling, and rendering.

4. **Thread and memory ownership.** Define how systems exchange state and when resources can be destroyed.

5. **Source versus derived versus runtime data.** Keep editable source content, rebuildable caches, and shipped assets conceptually distinct.

6. **State-change semantics.** Specify how spawning, destruction, property changes, and events behave during undo, replay, rollback, and live reload.

7. **Capability and fallback rules.** Make supported feature combinations explicit across platforms, rendering paths, and network modes.

8. **Headless and automated access.** Give asset workers, tests, dedicated servers, simulations, and agent tools usable APIs independent of interactive editor operation.

A good integration test is a **destructible wall**. Its destruction may need to update geometry, collision, navigation, visibility, sound propagation, particles, AI knowledge, replicated state, saved state, and editor previews. If those systems disagree about identity, timing, authority, or lifetime, implementing more features will amplify the inconsistency.

For a new engine, I would therefore use this inventory as a **coverage matrix**, then build a small playable vertical slice through the foundations, asset pipeline, runtime, editor, testing, and packaging. That gives you a working architecture into which the remaining capabilities can fit.

