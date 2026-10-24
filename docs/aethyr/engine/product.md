# Product specification

Status: proposed design, 2026-10-22. No feature or performance claim in this document describes an implemented Aethyrforge engine.

## Purpose and audience

Aethyrforge is a browser-first creation engine for building, playing, and publishing interactive 3D games and connected worlds. An independent creator can start with an empty place, assemble a playable experience, change it while standing inside it, invite collaborators, and publish a link. The world remains the primary workspace throughout that process.

The first audience is independent game creators and small teams making stylized 3D games. Their work includes level construction, interactive props, shaders, gameplay behaviors, multiplayer sessions, and iterative play. Technical artists and programmers need inspectable source and precise controls; creators without programming experience need the same valid operations through direct manipulation and ordinary language.

The primary promise is **change a playable world and see the result without rebuilding your working context**. Owning the engine enables targeted improvements to compilation, simulation, rendering, and distribution. Ownership means Aethyrforge controls its formats and contracts and can replace dependencies; it does not require inventing a browser, cryptographic primitives, or every file decoder.

## Product boundaries

The browser editor, semantic world model, transaction system, incremental compiler, runtime, renderer, asset pipeline, and release format form one product. They use the architecture in [Architecture](architecture.md). Adapters connect OpenAgents, Kiln, decision models, asset providers, and deployment infrastructure without making those services prerequisites for opening an exported project.

Aethyrforge supports both standalone games and places joined through a world directory. A directory entry does not merge simulation, identity permissions, or ownership across projects. Each published place declares its entry points, supported player capabilities, and transition policy. Crossing a portal requires a compatible destination and a recoverable loading state. Destination failure before handoff keeps the player at a safe source location; uncertainty after a prepared transfer shows a pending state until its durable decision is recovered, rather than risking duplicate ownership. Shared economies, arbitrary executable carryover, and universal avatar compatibility require separate approved contracts before implementation.

The first release targets authored stylized worlds, editable buildings, reusable props, gameplay scripts, and bounded multiplayer rooms. The complete building product includes tested multi-storey editing, stairs, ramps, lifts, and accessible alternative routes; the first single-storey acceptance milestone does not remove those requirements. A lift includes platform support, door interlocks, travel state, failure recovery, and a navigation connection rather than a cosmetic moving mesh. Film rendering, a general CAD/BIM suite, a marketplace payment system, a universal engine importer, and unlimited concurrent populations are outside its initial acceptance gate. The underlying formats must permit extensions without making these capabilities launch requirements.

## Priority order

1. Preserve the creator's project and ability to recover work.
2. Keep editing and playing responsive on the supported device profiles.
3. Make the result of each action clear, immediate, and undoable where possible.
4. Keep visible geometry, collision, navigation, and selection consistent.
5. Publish a complete version that another person can actually play.
6. Expand content libraries, advanced graphics, automation, and scale after these gates hold.

Unlimited engineering budget changes how deeply the platform can satisfy these priorities. It does not remove device memory, network latency, browser permissions, or the need to measure correctness. Performance requirements are budgets in [Performance](performance.md), not an assurance that every project fits every device.

## Required creation loop

A new project opens directly into a fullscreen world with a compact hotbar. The creator can choose an empty world or an openly licensed example. Samples are projects using the same public contracts as user work; they are not hard-coded editor modes.

The reference onboarding sequence contains five tasks: place a building, walk through its entrance, add a room, give a prop a behavior, and publish a preview link. Each task is skippable and represented as an optional compact checklist. Creating, saving, undoing, and inspecting never depend on a model request.

The editor distinguishes five states in plain language:

| State | Creator-visible meaning |
| --- | --- |
| Working document | Changes the creator has made and can inspect or undo |
| Candidate build | Outputs currently being checked or compiled |
| Running world | The last complete compatible generation being played |
| Saved revision | A durable project revision, with its local or shared location shown |
| Published version | An immutable release selected by a shareable channel |

The editor may show a newer working document while the running world remains on a last working generation. It must mark that difference. A failed shader edit cannot silently become a published shader, and an unsaved local edit cannot claim to be shared with collaborators. The governing contracts live in [Contracts](contracts.md), [Runtime](runtime.md), and [Publishing](publishing.md).

## Reference experiences

**Medieval Town Lab:** A creator adds a cottage on open ground, walks through an actual doorway, adds a room behind its kitchen, removes the connecting wall, and places two benches outside. A building is an editable structure with compiled kit-derived surfaces, not an opaque decorative shell advertised as editable. A separate Haunted House example can share the engine while keeping its own assets, world data, scripts, and visual style.

**Haunted House shader lab:** A creator edits a material while walking. A valid edit becomes visible at an activation boundary. A syntax error produces a precise diagnostic and preserves the player and last working shader. Ghost movement continues without model requests in the frame loop.

**Small multiplayer game:** Two creators edit on one branch while invited players run a selected test version. Collaborators see authoritative edit results; players receive only a compatible runtime generation and their permitted game state. Closing a creator tab cannot destroy the server session.

## Target capability and acceptance map

The full product goes beyond the first cottage and shader examples. [Authored game systems](game-systems.md) specifies component families and their lifecycle contracts. Each capability below requires a versioned authored component, registered commands, editor controls, runtime semantics, import/export behavior, resource limits, and conformance fixtures before its delivery gate. The first structural wire examples do not yet serialize all of them. [Delivery](delivery.md) assigns milestone dependencies; [Verification](verification.md) records qualification.

| Capability | Required creation and runtime behavior | Acceptance stage |
| --- | --- | --- |
| Buildings and traversal | Rooms, shared walls, doors, roofs, floors, stairs, ramps, and lifts compile from semantics into consistent visible and traversable outputs. Vertical routes declare supported controller profiles and accessible alternatives. | Entry slice proves one storey; complete-building gate proves upper floors, lift recovery, and navigation across every declared route. |
| Materials, lighting, and VFX | A typed material graph supports a metallic/roughness baseline, textures, normals, emission, transparency, and exposed parameters; WGSL extends supported interfaces. Lights, environment exposure, sky, and bounded cosmetic particle emitters remain authored data. | Shader slice proves parameter and pipeline replacement; game-authoring gate proves imported materials, graph validation, color correctness, lighting profiles, and particle admission. |
| Characters and animation | Imported skeletons and clips feed versioned blend/state graphs with transitions, events, and controller ownership. Root motion passes through collision; an interrupted clip cannot repeat a gameplay reward. | Game-authoring gate proves locomotion, blending, event deduplication, state-preserving graph edits, and supported retargeting diagnostics. |
| Audio | Authored emitters, listeners, mixer buses, priorities, looping, spatial settings, and event cues have bounded voices and memory. Playback begins after required browser consent; suspension and stream interruption have explicit behavior. | Game-authoring gate proves synchronized cues, voice stealing, pause/resume, device recovery, and captions or text alternatives for required information. |
| Terrain and outdoors | A tiled heightfield with material layers is the first editable terrain primitive. Sculpting and painting update neighboring visual, collision, navigation, and streaming seams together. Holes and caves require separate qualified geometry, not a heightfield trick. | World-scale gate proves seam-crossing edits, undo, stable save/reload, traversal under streaming pressure, and occupied-ground activation rules. |
| Gameplay input and cameras | Named actions map keyboard, mouse, touch, and gamepad onto typed tick inputs, with rebinding, dead zones, held/pressed/released semantics, and focus-loss cancellation. Camera rigs declare ownership and transitions; editor hotkeys cannot leak into play. | Game-authoring gate proves the same core game on keyboard and touch, input replay, controller removal, and accessible remapping. |
| Gameplay UI | Owned declarative UI components support HUDs, menus, inventory, text, focus, navigation, localization, responsive layout, and accessibility. Screen-space UI uses a bounded DOM host; world-space UI uses a qualified render adapter. Both emit typed game actions. | Game-authoring gate proves menu-to-play focus, inventory updates, localization/reflow, safe text, and no editor authority or arbitrary HTML/JavaScript access. |
| Cinematics | Authored timelines control named camera, animation, audio, and permitted event tracks. Preview scrubbing is separate from live simulation; seeking, skipping, and replaying have explicit side-effect rules and restore prior camera/input ownership. | Game-authoring gate proves interrupted playback, seek/skip without duplicated rewards, caption timing, multiplayer authority, and safe hot replacement. |
| Gameplay systems | Behaviors, physics, triggers, navigation, interaction, quests, checkpoints, save data, and configurable game rules use registered components and scoped capabilities. Persistence schemas and migrations belong to the game release. | Game-authoring gate proves a complete playable objective, save/load, failure/retry, behavior debugging, and compatible release migration. |
| Streamed connected worlds | Cells stream by visibility and simulation relevance with conservative traversal boundaries. Portals declare destination, capability, player-state transfer, authority handoff, and failure recovery; source and destination may run different releases. | World-scale gate proves constrained-network travel, disconnect recovery, denied destinations, stable spawn, and bounded residency without dropped collision. |
| Collaboration and release | Shared authoring, offline branches, multiplayer sessions, immutable releases, rollback, and self-hosting preserve distinct authorities and exact versions. | Shared-world gate proves conflict resolution and session isolation; distribution gate proves clean-client play, private access, export completeness, and recoverable publication. |

A capability can be experimental before qualification, but the editor must label it and omit unsupported controls. A complete-engine claim requires all declared full-product gates, not only fast asset placement or a graphical demo. Advanced rendering features such as film-quality effects, XR, and native backends remain separately admitted extensions.

## Acceptance requirements

- **PROD-01:** A new creator completes the five-task loop using direct controls without reading code or using AI.
- **PROD-02:** Buttons, keyboard actions, scripts with editor authority, and natural-language edits produce the same typed transactions and validation outcomes.
- **PROD-03:** A failed edit, compilation, upload, or model request preserves the recoverable working document and last compatible running generation.
- **PROD-04:** A project exports permitted source artifacts, dependency identities, rights metadata, and a reproducibility report. That report identifies unavailable licensed inputs; an export missing required artifacts cannot claim to be self-contained.
- **PROD-05:** Medieval Town and Haunted House remain independent example projects with independent save namespaces and releases.
- **PROD-06:** A published link works without access to the creator's editor credentials or local asset paths.
- **PROD-07:** Representative users, including creators with accessibility needs, complete creation-loop studies before a launch claim of usability. Findings and unmet requirements remain visible in the verification record.
- **PROD-08:** Every capability in the target map passes its named acceptance stage with registered schemas and cross-language/runtime fixtures before it is advertised as qualified. A milestone report lists incomplete target capabilities explicitly.
