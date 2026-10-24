# Authored game systems

Status: proposed contracts, 2026-10-22. These component families complete the target creation engine; none is implemented or part of the initial structural JSON fixture. [Contracts](contracts.md) owns registration and envelopes, [runtime](runtime.md) owns ticks and activation, and [asset compilation](asset-compilation.md) owns derived outputs.

## Shared component contract

Each authored component has a stable UUID, registered type and schema version, owner entity or world, immutable dependency references, typed parameters, and a resource profile. Proposed family names below require schemas, migrations, and conformance fixtures before implementation qualification. Unknown required versions block activation. Optional presentation components may degrade only under an explicit profile rule.

Compiled outputs identify source digest, target profile, compiler version, and ABI. Live state separately identifies component, generation, state-schema version, and last accepted tick. Compatible edits migrate state at an activation boundary; failed compilation or migration preserves the prior generation. Removal cancels owned subscriptions and handles. Undo changes source; restoring gameplay requires a separate checkpoint. Profile limits below are initial admission choices, not measured capacity claims.

## Input actions and player cameras

`input.map.v1` defines named boolean, scalar, and two-axis actions, device bindings, contexts, dead zones, and remapping policy. The baseline admits 64 actions and 16 bindings per action. Devices produce normalized samples; the kernel assigns tick and sequence, derives pressed/held/released transitions, and validates multiplayer input. Focus loss synthesizes releases. Text composition and editor shortcuts never become gameplay actions.

`camera.rig.v1` declares first-person, follow, orbit, or authored-path parameters, collision policy, and prioritized control ownership. Runtime state contains pose, smoothing history, and owner. Cinematic control restores the previous valid owner when released. Hot changes preserve pose unless the creator explicitly selects a transition. See [creator experience](creator-experience.md).

**GAME-01:** Keyboard, touch, and gamepad complete the same objective; remapping, disconnection, focus loss, conflicting contexts, and camera handoff produce no stuck input.

## Materials, lighting, and visual effects

`material.graph.v1` contains typed nodes, explicit color-space conversions, immutable texture references, exposed parameters, and a surface model. The baseline uses metallic/roughness shading with normals, emission, occlusion, and declared alpha mode. Initial admission allows 256 acyclic nodes and eight sampled textures per material, additionally constrained by device limits. WGSL extensions use the checked interfaces in [rendering](rendering.md).

`light.v1`, `environment.v1`, and `vfx.emitter.v1` specify lights, sky/exposure, and cosmetic particles. A profile declares active-light, shadow, particle, and upload budgets before allocation. Excess effects are rejected or use an authored quality reduction. Material constants update data; graph-interface changes stage a complete pipeline. Cosmetic GPU effects cannot determine rewards, collisions, or authoritative visibility rules.

The [Medieval Town visual target](references/medieval-town.md) makes environment
authoring concrete: sun/moon direction and radiance, sky/cloud layers,
diffuse/specular environment inputs, fog depth/height and local volumes, exposure,
white balance, and display transform are inspectable settings. Practical lights
declare type, transform, color/intensity convention, range/falloff, emission
association, shadow settings, and resource cost. Register their schemas, units,
migrations, and conformance fixtures before implementation qualification. Imported
camera/light transforms alone do not supply these settings. Day/night presets pin
the complete configuration; editing it uses the normal command/generation path.

**GAME-02:** Imported and authored materials agree on color and alpha conventions; invalid graphs retain working pipelines, and quality reductions preserve game rules.

## Characters, animation, and skinning

`animation.graph.v1` references a skeleton identity, clips, states, blend parameters, transitions, event markers, and root-motion policy. Initial skin admission supports 128 joints and four influences per vertex; a profile extension must qualify larger rigs. Retargeting requires an explicit bone map and bind-pose conversion, not matching names alone.

CPU ticks evaluate state and events; GPU skinning interpolates presentation using conservative animated bounds. Events carry instance, transition, loop, and marker identities for deduplication. Root motion proposes movement to physics. A compatible graph edit maps states and normalized clip time; a changed skeleton needs explicit migration or reset.

**GAME-03:** Blending, looping, interrupted transitions, retargeting, graph reload, and streaming clips preserve valid poses and never duplicate gameplay events.

## Spatial audio and mixing

`audio.scene.v1` defines emitters, listeners, buses, routing, gain, attenuation, occlusion policy, looping, and voice priority. Baseline admission permits 32 buses and 32 simultaneous voices. Voice stealing is deterministic by priority and age; required informational cues also have a text or visual representation. Bus routing is acyclic.

Tick events schedule playback against the audio clock with deduplication IDs. Streaming or missed deadlines follow declared drop/restart policy. Browser consent, suspension, and device changes gate playback. Compatible edits preserve a playing cursor; source replacement requires an explicit crossfade or restart. AudioWorklet DSP uses bounded trusted modules, not arbitrary imported code.

**GAME-04:** Device recovery, voice exhaustion, reload, pause/resume, and repeated network events cause neither stale sound bursts nor repeated cues.

## Terrain and outdoor structure

`terrain.heightfield.v1` defines a world-anchored grid, tile coordinates, integer-millimeter heights, material layers, collision profile, and holes. The initial tile has 64 by 64 quads and 65 by 65 samples with declared uniform sample spacing. Neighboring tiles share boundary sample identities. Brush operations are explicit bounded source edits; an instant brush affects at most four tiles, while larger work uses a reviewable job.

The compiler rebuilds affected geometry, collision, navigation, and seam halos together. Occupied ground follows safe-activation rules. Distant visual LOD never substitutes inconsistent near collision. Heightfields cannot represent caves or overhangs; those require qualified separate geometry. Water initially uses a non-authoritative visual surface plus explicit gameplay volumes.

**GAME-05:** Sculpt, paint, undo, save/reload, and cell crossings preserve seams, traversal, and unchanged tile identities under cold streaming.

## Declarative gameplay interface

`ui.screen.v1` contains typed layout nodes, localization keys, focus order, style tokens, accessibility names, and bound view-model fields. Initial admission permits 256 nodes per screen and depth 16. The trusted player host renders screen-space UI as DOM; world-space UI uses a separately qualified adapter. Projects cannot inject raw HTML, CSS URLs, JavaScript, or editor DOM handles.

UI reads bounded projections of permitted runtime state and emits registered game actions. Inventory authority remains in simulation. Hot replacement preserves matching node focus and typed local form state; removed nodes return focus to a declared fallback. [Security](security.md) governs cross-origin hosting and broker capabilities.

**GAME-06:** Menus, inventory, responsive layout, localization, screen-reader navigation, and reload operate without hidden authority changes or keyboard traps.

## Cinematics and timelines

`timeline.v1` stores integer-microsecond key times, target IDs, tracks, easing, and event policy. Initial admission permits 64 tracks and 4,096 keys per timeline. Playback declares its clock, authority, camera/input leases, and interruption behavior. Multiplayer timelines follow the server's start tick; client playback does not award progression.

Editor scrubbing evaluates presentation without gameplay effects. Runtime seeking explicitly chooses whether permitted markers are replayed, skipped, or reconciled; rewards use independent idempotent game events. Skip returns camera/input ownership and lands at a validated destination. Incompatible edits hold the previous timeline or require a declared restart.

**GAME-07:** Scrub, seek, skip, reload, and disconnect preserve ownership, caption timing, and exactly-once accepted rewards.

## Game rules and saved progress

`game.rules.v1` declares typed state, events, objectives, interactions, checkpoints, and behavior attachments. `save.schema.v1` partitions local preferences, local progress, and authoritative shared progress. Saved records identify release compatibility, state-schema version, revision, and integrity metadata. They contain data, never executable source or credentials.

Writes are transactional and quota-bound. Loading validates before migration; migrations run without external side effects on a copy. Failure preserves the previous save and offers export. Multiplayer services own shared rewards and inventory; clients submit intent. Distribution rollback does not automatically roll back progress databases.

**GAME-08:** Complete, fail, retry, save, load, duplicate reward, corrupt save, and release rollback scenarios preserve declared progress semantics.

## Vertical traversal and world portals

`connection.vertical.v1` describes stairs, ramps, or lifts with endpoints, clearance, traversal profile, and collision/navigation outputs. Lifts additionally declare door interlocks, platform ownership, travel state, emergency landing, and recovery. Publicly accessible storeys need qualified accessible routes or an explicit limitation, not a cosmetic lift.

`portal.world.v1` pins destination project/channel policy, entry point, allowed transferable player fields, and authorization requirements. Prepare destination admission before leaving the source. Progress transfer requires an explicit authority/trust agreement; without one, travel starts an independent destination identity and transfers no inventory. A durable coordinator records one commit-or-abort decision per transfer ID. Prepare freezes transferred source fields and stages destination state. Commit makes destination acceptance idempotent and retires source ownership; only a durable abort restores the source checkpoint. An uncertain decision stays pending until recovery, sacrificing availability rather than duplicating progress. Prepared reservations expire only before a commit decision. The record binds source session/checkpoint, destination release/protocol, state digest, and trust agreement. Prepare permits no irreversible broker effects. Decision and participant preparations require zero-loss durability within the declared fault scope; ordinary project-backup recovery is insufficient. Recovery cannot unfreeze source state on timeout after an uncertain commit. This transfer requires reachable session services. No raw capability or arbitrary guest code crosses worlds.

**GAME-09:** Stair landings, lift interruption, occupied edits, and alternate routes remain traversable. **GAME-10:** Portal timeout, denied access, incompatible release, and duplicate acceptance never create duplicate player ownership or progress transfers; uncertain outcomes remain pending until recovery. [Verification](verification.md) must fault-test both families before general connected-world qualification.
