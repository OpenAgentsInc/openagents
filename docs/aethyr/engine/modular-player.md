# Standalone modular player

Status: local prototype for [issue #6](https://github.com/OpenAgentsInc/aethyrforge/issues/6).
The static player runs the same Rust kernel, compiled scenes, environment loading,
controller framing, material shaders, and direct WebGPU renderer as the editor
preview. It has a full-screen world and a small floating play bar. React, Effect
Atom, and Effect remain the creation environment; they do not schedule player frames.

## Profiles and delivered files

| Profile   | Features                                                | Content                                                                   |
| --------- | ------------------------------------------------------- | ------------------------------------------------------------------------- |
| Empty     | Clear canvas                                            | No simulation, rules, character, or asset package                         |
| Room      | Materials, movement/physics, collision, proximity rules | Owned courtyard, key, exit, walls, and furniture                          |
| Character | Room features, animated avatar, sky                     | Same courtyard with an owned block explorer and small HDR skies           |
| Town      | Character build with private content                    | Complete Medieval Town geometry, lighting, collision, Druid, and gameplay |

The controller includes the existing Rust fixed-step physics. Room and character
use the same kernel binary; this work does not claim that unused Rust authoring
functions have been removed. The room graphics build excludes character loading,
skinning, animation rendering, sky shader code, and their data. Other shared
material and rendering systems remain in the renderer. This is a measured module
boundary, not a claim that every renderer feature has a specialized minimal build.

Mesh inspection setup is an editor feature. Standalone builds omit its pipeline
and triangle-edge helper modules. Opening a published Room in the editor retains
the editor's mesh-inspection hook while keeping character and sky code disabled.

Player minification shortens an explicit list of 20 owned Renderer members. The
build checks their declarations, all included modules, emitted JavaScript, and
the returned name map. This is a private member convention, not a proof against
arbitrary JavaScript reflection. Public fields, browser interfaces, and saved
formats keep their names. Each build records its actual map beside its graph.

`tools/build-player.ts` creates a separate served directory for each profile.
It disables Vite's blanket public-directory copy, follows emitted imports and
resource references, and copies only the declared cooked closure. Graph and byte
reports live beside the served directories. The audit rejects unreachable served
files, unknown or external edges, missing module records, and editor, agent,
React, Atom, importer, or experiment source dependencies. Tests build actual
room and full renderer graphs and check optional code and WGSL removal.

The empty profile is a separate clear-pass entry. Comparing its size with a
textured Town says nothing about the cost of the same game in another engine.
The three owned profiles have separate results and [byte budgets](../../../apps/aethyr/experiments/kaigen/player-budgets.json). These initial regression ceilings include all delivered files; they are not startup guarantees.

## Artifact and ownership

`aethyrforge.player-artifact.v1` is the implemented local static-player descriptor.
It pins the entry runtime and Rust kernel by ABI, size, and SHA-256; declares
features and their dependency closure; embeds the existing verified cell-package
format; and names immutable authored settings. It is separate from the proposed
signed production release format in [publishing](publishing.md).

The cooker reuses the existing dependency resolver and full-quality cell. It
does not reindex Town geometry, material tables, collision, or GI identities.
Owned room variants remove optional source references before cooking. Every
cooked file belongs to an enabled feature, and rules and collision must be in the
entry closure. Runtime code is deployment-owned; content cannot supply executable
functions or choose another origin.

Both player and editor preview admit the same descriptor through
`preparePublishedWorld`. Before GPU preparation it verifies the package, kernel,
authored settings identity, scene tables, feature compatibility, and object mesh
references. During preparation, asset reads use only verified local bytes. The
temporary cache and fetch override are released on success, failure, or cancel.
The Rust kernel remains the final authority for world initialization.

`WorldSession` owns live player, input, camera, game rules, renderer, and clock.
It uses shared controller framing and fixed-step admission with the preview.
The standalone session currently runs on the main thread; the editor preview uses its worker. Input clears on blur, hidden-page transitions, and close. Asynchronous spell
preparation cannot commit after cancel, reset, camera changes, or disposal.
Captures pause frame admission and restore the live viewport in `finally`.

A standalone player opens one immutable local artifact. It does not contain an
editor, importer, draft store, or agent source-code mutation tool. Its local
diagnostics expose observation, bounded controller actions, capture, and candidate
compatibility validation. Browser tools and tests are not a security boundary.

The existing OpenAgents host protocol and `load_playable` path retain their
adapter when `VITE_OPENAGENTS_HOST_ORIGIN` is configured. That adapter still
loads the creation bridge and is not counted as the minimal standalone build.
This change does not relabel that older deployment as a signed modular release.

## Reproduce and interpret measurements

Run `npm run build:player`; optionally use `npx tsx tools/build-player.ts` with add `--town PRIVATE_SOURCE`.
Use `--out` and `--scratch` to keep generated output and licensed data outside
Git. Serve one emitted profile directory and open `/player.html`.
For editor parity, set `AETHYRFORGE_STREAM_ROOT` to that directory and
`VITE_PLAYER_ARTIFACT_URL=/player-artifact.json`, selecting the matching world.

Run `tools/verify-player.mjs` through `openagents browser run`, with
`AF_PLAYER_ROOT` pointing at the build parent and `AF_CAPTURE_DIR` outside the
checkout. The verifier closes one GPU world before opening the next. It compares
fixed views and quest outcomes for the same artifact, checks real input and
release, and rejects incompatible candidates without changing the active state.

Artifact reports separate raw, gzip, and Brotli sizes from actual response-body
and transfer observations. Startup reports separate descriptor verification,
verified content loading, resource preparation, Wasm compile/instantiate, and
first completed render/control. Renderer resource initialization includes asset
decode, pipeline creation, and upload admission; the initial queue drain is a
separate completion span. Isolated GPU upload time is unavailable and stays null.
Accounted GPU bytes and Wasm linear memory are partial residency counters, not
browser or operating-system memory peaks. Cold means a fresh browser context;
warm means a reload in that context, with observed cache information retained.

## Recorded result

The [recorded evidence](https://github.com/OpenAgentsInc/aethyrforge/blob/71c19bfe8b5de79811de5ca518e783876650d42f/experiments/kaigen/results/issue-6.json) includes exact
player/editor RGBA equality for empty, room, character, and frozen Druid Town.
All playable profiles pass real keyboard movement, blur release, independently
checked quest failures/completion/reset, and incompatible-candidate continuity.
The avatar changes visible pixels in a clear, independently projected region
after real first/third-person transitions. All contexts and preview workers close.

| Build     | JavaScript raw / gzip | Wasm raw / gzip     | Complete served files |
| --------- | --------------------- | ------------------- | --------------------- |
| Empty     | 16,121 / 5,917 B      | 0 / 0 B             | 4                     |
| Room      | 570,998 / 171,443 B   | 878,647 / 266,177 B | 15                    |
| Character | 592,584 / 178,286 B   | 878,647 / 266,177 B | 21                    |
| Town      | Same character code   | Same kernel         | 471                   |

All three owned byte budgets pass. Town content remains approximately 631 MB raw
and is measured separately. On this run Town reports about 2.94 seconds from
entry startup to first reliable control in a fresh context and 2.87 seconds on
reload; these are local observations. Shader and Wasm caches persist in the same
Chrome process, so the faster character result than the first room load is not
evidence that adding an avatar improves startup.

Type checking, 660 TypeScript tests, ordinary production build, configured host
compatibility build, and specification checks pass. Reports retain warnings and
the corrected camera-test failures. No purchased models, textures, or captures
are committed.
