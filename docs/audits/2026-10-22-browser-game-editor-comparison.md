# Browser game editors for OpenAgents: PlayCanvas, Babylon.js, and Three.js

Date: 2026-10-22. Repository snapshot: `74a47f2258`.
Status: research and recommendation; no engine migration or implementation.
Upstream links describe the sources reviewed on this date, not pinned releases.

## Recommendation

**Updated owner direction: build our own editor, runtime, and services; assume browser-first and no Rust-only constraint.** The [owned-renderer, WebGPU, and Effect assessment](2026-10-22-owned-renderer-webgpu-and-effect.md) selects a TypeScript editor/runtime, Effect orchestration, direct WebGPU, and editable WGSL, with selective Wasm CPU kernels. It replaces the earlier preference for starting from the Rust renderer.

- **Our project model and authoring commands come first:** do not make an upstream editor's history, services, or scene objects the product foundation.
- **Three.js can supply a reference or optional adapter:** the selected from-scratch rendering direction is our own direct WebGPU renderer. `three-effect` supplies useful patterns and a comparison path.
- **Babylon and PlayCanvas remain useful engine/tool references:** their available authoring interfaces no longer determine the recommendation when we intend to build those systems ourselves.
- **Rust remains viable for fast iteration:** our existing local demos already update data and art without rebuilding the game, and replace a small Rust rules module without restarting the run. Shader text can follow the same principle.

The comparison below retains the capabilities and tradeoffs of reusing existing editor tooling. Its conditional assessments of editor reuse do not override the owner's clarified requirement. No candidate was benchmarked.

The owner explicitly removes the Rust-only constraint for the current architecture assessment. The Rust reuse inventory below describes existing investments, not requirements for the new browser application's state or UI. This audit changes documentation only and leaves the repository's [agent contract](../../AGENTS.md) unchanged.

## What owning the editor means

Ownership has to cover more than the source license of the renderer:

| Layer | What OpenAgents must control |
| --- | --- |
| Creator experience | Selection, transforms, scene hierarchy, asset import, material editing, behavior editing, preview, errors, and undo |
| Project data | Stable object IDs, editable scene and asset sources, versions, migrations, and export without losing the ability to edit |
| Services | Identity, permissions, project storage, asset processing, collaboration, backups, and publishing under our control |
| Running game | State ownership, safe updates, diagnostics, resource cleanup, and recovery from failed edits |
| Agent access | The same permitted edit operations for Kiln and people, with reviewable results and conflict handling |

An exported game can run on our domain while its authoring workflow still depends on a vendor. An open editor frontend can still depend on services that we have not replaced. An embedded Inspector can change a scene without providing a complete project editor.

Kiki's UX direction remains relevant: the first experience should have a clear path from opening a project to making a change, seeing it, and choosing to publish. A long list of available engine tools should not become the initial screen. The editor should expose the controls needed for that flow, then reveal advanced tools when useful.

## Comparison at a glance

The effort assessments below are relative judgments. No candidate was installed or benchmarked for this audit.

| Question | PlayCanvas | Babylon.js | Three.js |
| --- | --- | --- | --- |
| What comes closest to a finished browser editor? | Its hosted Editor: integrated visual authoring, assets, collaboration, scripts, and publishing | Separate browser tools to assemble; the full Babylon Editor is a desktop application | Included browser scene editor; a broader game-authoring product remains our work |
| Can we own the source? | Engine and Editor frontend are MIT licensed | Core engine is Apache-2.0 licensed; inspect separately adopted projects and dependencies | Core repository, including editor source, is MIT licensed |
| Can we run the complete authoring service ourselves from the reviewed setup? | Not established; local frontend setup still uses hosted services | Custom editor and our services are feasible; no complete browser service is supplied by choosing the engine | Custom editor and our services are feasible; no complete service is supplied by choosing the library |
| Main integration burden | Replace hosted service dependencies in a frontend fork, or build a new editor over the engine | Combine authoring tools into our project, history, asset, and collaboration model | Choose and integrate more game systems and build the authoring workflow around them |
| Script changes during play | Documented per-script lifecycle for swapping instances and copying state | Build an explicit behavior replacement lifecycle | Build an explicit behavior replacement lifecycle |
| Shader and material editing | Runtime custom shaders and material parameters | Runtime materials plus a visual Node Material Editor | Runtime materials; WebGL and WebGPU custom-material paths differ substantially |
| Kiln starting point | Existing Editor API and MCP integration provide a strong reference | Inspector and engine APIs provide building blocks; our tool bridge remains to be built | Engine objects and editor commands provide building blocks; our tool bridge remains to be built |
| Biggest ownership risk | Mistaking frontend source or game export for ownership of the entire platform | Mistaking several powerful tools for a finished creator product | Underestimating the work between a rendering library and a game editor |

Primary sources: [PlayCanvas Editor](https://github.com/playcanvas/editor), [PlayCanvas engine](https://github.com/playcanvas/engine), [Babylon browser tools](https://www.babylonjs.com/tools/), [Babylon Editor](https://github.com/BabylonJS/Editor), [Three.js editor source](https://github.com/mrdoob/three.js/tree/dev/editor). License sources: [PlayCanvas Editor MIT](https://github.com/playcanvas/editor/blob/main/LICENSE), [Babylon engine Apache-2.0](https://github.com/BabylonJS/Babylon.js/blob/master/license.md), [Three.js MIT](https://github.com/mrdoob/three.js/blob/dev/LICENSE).

## PlayCanvas: the strongest complete workflow, with a service boundary to resolve

### Three distinct adoption paths

1. **Use the hosted Editor.** This adopts an existing creator workflow while OpenAgents supplies surrounding identity, agents, and game services. It also makes the authoring account, project service, and collaboration workflow a vendor dependency.
2. **Fork the Editor frontend.** This gives control over its interface and code. The [documented development setup](https://github.com/playcanvas/editor#local-development) loads a local frontend through a project on `playcanvas.com`; its tests use hosted login, editor, and launch hosts. We would need to inventory and replace the service contracts before treating this as self-hosted.
3. **Use the engine with our own editor.** This avoids depending on the hosted Editor, but gives up much of the immediate advantage of adopting its finished workflow.

For a frontend fork, the ownership investigation must cover project creation, scene persistence, asset upload and conversion, collaboration transport, history, build generation, account assumptions, and recovery. Some pieces may be reusable or replaceable; their replacement effort is unmeasured. The reviewed material does not justify declaring the entire backend available, or declaring self-hosting impossible.

[Downloading a project](https://developer.playcanvas.com/user-manual/editor/projects/downloading/) produces a self-hostable application package. The documentation distinguishes that package from an editable project backup and its history. This is a useful publishing route, but it does not solve authoring-service ownership.

### Live iteration and agents

[PlayCanvas script hot swapping](https://developer.playcanvas.com/user-manual/editor/scripting/hot-reloading/) explicitly creates replacement script instances. A script opts in through `swap(old)`; declared attributes transfer automatically, while other state and event listener cleanup require developer work. This is a valuable lifecycle to learn from. It does not establish that arbitrary edits preserve every running game.

The [Editor MCP integration](https://developer.playcanvas.com/user-manual/editor/mcp-server/) can edit scenes and assets and inspect a launched application. Its documented bridge is a local Node server connected to an open, signed-in Editor session. It directly changes project data. This is the strongest existing agent-authoring reference in this comparison, but browser Kiln cannot run that Node process through its WASI tools. A browser bridge or a separately authorized native-host integration would still be needed.

**Assessment:** prefer PlayCanvas if operating its hosted workflow is acceptable, or if an early service inventory shows that adapting the frontend is cheaper than building our editor. Do not budget a frontend fork as a cosmetic rebrand.

## Babylon.js: the provisional foundation for our own editor

Babylon supplies an engine and a broad set of browser tools: Playground, asset Sandbox, Node Material, Node Geometry, Node Particle, and Node Render Graph editors. These give us more authoring components to evaluate before building equivalent tools ourselves. They remain separate tools, with integration work between them and an OpenAgents project.

[Inspector V2](https://github.com/BabylonJS/Documentation/blob/master/content/toolsAndResources/inspector.v2.md) provides scene hierarchy and property inspection, real-time manipulation, and an extension API. Its documentation distinguishes extensible ESM packages from UMD bundles with more limited extensibility. Package choice therefore matters if we plan to add project-specific panels. The Inspector's diagnostic interface still needs a creator-focused UX around it.

The [full Babylon Editor](https://github.com/BabylonJS/Editor#introduction) is an Electron desktop application for Windows, macOS, and Linux, with native dependencies. Its templates and source are references; shipping that application inside a browser is a separate port. The recommendation here is **Babylon engine plus selected browser tools plus our own editor**, not an assumption that the desktop Editor already meets the requirement.

The [Node Material system](https://doc.babylonjs.com/features/featuresDeepDive/materials/node_material/nodeMaterial/) supports visual construction and programmatic construction, with GLSL or WGSL shader generation. Nonconstant inputs can be adjustable uniforms. This is a useful route for creators who should not need to write shader source. We still need to keep a candidate material separate from the active one until validation succeeds, then replace it without disturbing the player.

What remains ours:

- A coherent hierarchy, properties, assets, behaviors, and preview experience.
- Project persistence, imports, source assets, and version migrations.
- Undo transactions that also work for agent edits.
- Collaboration through our existing session system.
- Script replacement, error recovery, publishing, and player-state preservation.

**Assessment under an editor-reuse strategy:** Babylon offers a useful balance between reusable authoring tools and a first-party service architecture. Under the clarified requirement to build those tools ourselves, this advantage carries less weight than control of the rendering boundary.

## Three.js: maximum design freedom, more systems to assemble

Three.js gives us direct control over the rendering layer and lets us design a small editor around the exact creation flow. That can be preferable to adapting a large existing editor if the initial product only needs a limited set of scene and material operations.

Its [included editor](https://github.com/mrdoob/three.js/tree/dev/editor) is real source we can study or fork. The [editor entry point](https://github.com/mrdoob/three.js/blob/dev/editor/index.html) wires scene changes, history, serialization, and browser autosave together. We should assess that implementation as an application we would maintain, rather than assume it is a stable, independently packaged editor SDK.

For a broad game editor, OpenAgents would still choose or build behavior conventions, physics integration, game-specific animation workflows, project assets, collaborative edits, and publishing. Three's [physics guide](https://threejs.org/manual/pages/physics.html) illustrates the separate simulation integration. Existing add-ons can reduce work, but an ecosystem of packages is not a single maintained authoring workflow.

There is also a material-platform choice. The [WebGPU renderer guide](https://threejs.org/manual/pages/webgpurenderer.html) describes WebGPU with WebGL2 fallback through node materials and TSL. It explicitly excludes the old `ShaderMaterial`, `RawShaderMaterial`, and `onBeforeCompile` custom-material paths, and requires porting those materials. It also describes a different postprocessing stack. Haunted House's existing GLSL should therefore not be treated as a drop-in WebGPU migration.

**Assessment:** choose Three.js when a distinct, constrained editor UX is the main advantage, and we accept responsibility for the surrounding game systems. It is less compelling if the goal is to acquire a broad game engine editor with the least assembly work.

## Live iteration: separate the three loops

| Loop | Trigger | Required result |
| --- | --- | --- |
| Creator preview | Change a property, shader, asset, or behavior | Update a private running game; preserve compatible state; report errors beside the edit |
| Editor development | Change OpenAgents editor or engine integration code | Rebuild or reload developer tooling as needed |
| Publishing | Choose to release a checked project snapshot | Upload changed objects and update the selected published version |

A development server's hot module replacement does not automatically implement the first loop. For example, [Vite's HMR API](https://vite.dev/guide/api-hmr) provides disposal and state-carrying hooks and can invalidate modules; our integration must still decide what state survives and when a reload is necessary. A creator editing a deployed project also needs a preview transport that exists outside a developer's local Vite server.

### What Rust already demonstrates

These are recorded local measurements from different workloads, not comparative engine benchmarks:

| Existing path | Recorded time | What it establishes |
| --- | --- | --- |
| Ice Cream Man tuning | 191 ms median | Data edits can reach the same run without Rust compilation |
| Ice Cream Man mesh recolor | 181 ms | One baked mesh chunk can change; this is not a general texture-import benchmark |
| Ice Cream Man small Rust rules edit | 1.43 s median, including a 1.18 s warm build | A separately compiled rules module can replace compatible logic while the shell keeps state |
| Ice Cream Man rollback | 32 ms | A cached version can restore the paused run |
| Grid module staging rebuild | 5.5–5.6 s warm; 183 s cold | Whole-module build latency differs from content editing and depends on build settings |
| Kiln self-edit | 11.0 s warm; 25.1 s after a newer commit | Updating Kiln itself builds, checks, publishes, and reloads the page; it is not a game shader swap |

Sources: [Ice Cream Man live demo](../verse/ice-cream-man-live.md#measurements), [web deploy measurements](2026-10-21-web-deploys-in-seconds.md), [Kiln self-edit](../kiln/self-edit.md). Ice Cream Man keeps runs local to each player; it does not demonstrate authoritative multiplayer updates. Its shell updates save state and reload the page, which differs from replacing only a rules module in the same runtime.

The [deploy stack audit](2026-10-21-web-deploy-speed-and-stack.md) also identifies packaging and rebuild scope as major costs. Replacing Rust with JavaScript would not by itself fix an architecture that rebuilds, compresses, validates, and publishes unrelated modules on every edit.

### Shader editing across the candidates

| Runtime | Practical path | Remaining work |
| --- | --- | --- |
| Existing Rust Haunted House | Load GLSL as editable data; compile a replacement WebGL2 program | Asset transport, interface checks, last-working-program retention, and cleanup |
| PlayCanvas | Update parameters or build a replacement custom material | Map engine shader interfaces; validate and replace safely |
| Babylon | Update uniforms or build a replacement shader/node material | Keep graph/source drafts separate from the active material and its state |
| Three.js | Update uniforms; replace a WebGL material or use the WebGPU node/TSL path | Choose the backend and material model; validate and replace safely |

[Haunted House's program creation](../../crates/haunt-web/src/gpu.rs) already passes shader strings to WebGL and checks compile/link results. The strings currently come from Rust constants. Making those strings editable assets removes the Rust compilation step from shader edits; no renderer migration is required for that benefit.

[PlayCanvas custom shaders](https://developer.playcanvas.com/user-manual/graphics/shaders/) support GLSL for WebGL2 and WGSL for WebGPU; GLSL on WebGPU requires an additional transpilation path. This is useful portability machinery, with its own startup and compilation costs. None of these engines makes custom shader interfaces interchangeable with our current lighting and postprocessing.

For all four paths, the desired replacement lifecycle is:

1. Keep rendering with the last working program while a new revision is checked.
2. Compile and link the candidate; validate required attributes, uniforms, and material bindings.
3. Discard results from older revisions if a newer edit has arrived.
4. Replace the active program at a frame boundary; retain player position, view, and game state.
5. On failure, retain the working program and show the diagnostic; release rejected GPU resources.

GPU compilation still takes time. [Parallel WebGL shader compilation](https://registry.khronos.org/webgl/extensions/KHR_parallel_shader_compile/) can avoid blocking completion checks where supported; it does not eliminate compilation or guarantee protection from a lost GPU context. Syntax failure and device failure need different recovery paths.

Behavior changes need a similarly explicit lifecycle. Store player and world state outside disposable behavior instances; transfer compatible state and remove old listeners, timers, and subscriptions. If an edit changes state layout or initialization assumptions, provide a deliberate restart or migration rather than silently resetting the game. This remains application work in JavaScript as well as Rust.

## Kiln integration: share edit operations, not just files

### Existing foundation

[Kiln](../kiln/README.md) already supplies a Rust agent loop, a browser workspace persisted in OPFS, and WASI tools in workers. Its [remote runner](../kiln/rust-tests.md) checks configured Rust packages in a sandbox. These are not a general browser Node environment or an unrestricted npm build service.

The new [native Kiln host](../../crates/kiln-host/README.md) supplies a persistent Rust agent and bounded tasks through the existing runner and patch bridge. It can support a future native integration, but its documented workflow proposes checked repository changes; it does not supply a live scene-editor connection. The [workstation specification](../kiln/workstation.md) separates built capabilities from planned ones.

The missing connection is between a workspace edit and the particular running game preview. Updating Kiln itself, opening a PR, or publishing the site is a different operation.

### Proposed integration boundary

The following operations describe a proposed contract. They are not existing Kiln tools or a new protocol implementation.

| Operation | Why Kiln and the manual editor both need it |
| --- | --- |
| Read project and selection | Identify the correct scene, objects, assets, and base revision |
| Apply scene transaction | Group property and hierarchy changes into one reviewable, undoable edit |
| Write asset draft | Update shader, behavior, or material source without replacing the working runtime version yet |
| Validate and preview | Compile or import changed content and update only the target private preview |
| Inspect and capture | Return errors, relevant runtime values, and an image of the actual result |
| Accept or undo | Keep or reverse an agent edit through the same history as a manual edit |
| Publish snapshot | Release an explicitly chosen version through existing publishing services |

Use stable project, scene, object, and asset identities, with a base revision on each mutation. Kiln must not overwrite a person's concurrent changes by saving an old whole-scene snapshot. Kiln's current worker snapshot model also needs care before parallel tools or a manual editor write the same workspace; see [Kiln findings](../kiln/findings.md).

An engine adapter maps these operations to its scene and material APIs. The editor owns the project document and history; the running preview owns temporary simulation state. Neither is reconstructed from GPU objects. Engine serialization can be an adapter format, but should not silently become our only editable project record.

Keep live collaboration in **Loro documents on `coder-sync`, with NIP-HOST grants**, as the repository contract requires. Choosing PlayCanvas's collaboration workflow would require resolving that difference; running two independently authoritative project histories would create conflicts. Adapting a change-observer library does not automatically adapt its collaboration semantics.

For browser Kiln, a bounded browser message bridge can connect the Rust/Wasm agent to the preview. For native Kiln, an explicitly configured service connection is another option. In both cases, project scope and allowed operations must be checked by the receiving editor/service. An unrestricted “evaluate JavaScript” tool is a poor substitute for project edit operations.

Imported images, audio, and meshes need asset processing and content storage. Keep their original sources and derived outputs; do not force binary content through the small text-patch workflow. Repository contributions can continue through [Kiln's existing maintainer bridge](../kiln/maintainers.md), independently of private game drafts.

Use [livehost's existing object and manifest model](../../crates/livehost/README.md) for published game versions, with an adapter for the selected runtime's package. Ordinary preview edits should not require a merge or a public version update. A Pylon model response, asset conversion job, and GPU shader compilation are separate contributors to latency and should be reported separately.

Creator scripts also need isolation from account and wallet authority. A separate preview origin with restricted capabilities is a candidate boundary; details such as messaging validation, resource limits, and worker use still need design. Preserve the editor when game code throws.

## What Rust we can keep

| Rust investment | Reuse assessment | Boundary or cost |
| --- | --- | --- |
| Identity, authorization, Nostr, wallet, inference, receipts, and server services | Keep existing domain crates and services | Expose bounded operations; do not recreate domain logic in editor scripts |
| Kiln agent, tools, native host, runner, and patch workflow | Keep | Add editor operations and preview transport; a renderer choice does not replace these |
| Livehost storage, manifests, and publishing | Keep the infrastructure | Add runtime packaging and update adapters |
| Existing browser-compatible Rust modules | Reuse where already supported | Verify each crate's browser target and capabilities; server Rust is not automatically browser-ready |
| Game rules and procedural generation | Strong candidates for Rust/Wasm | Use explicit inputs, outputs, state versions, and batched calls; avoid a cross-language call for every property of every object |
| Physics or other CPU-heavy simulation | Can remain Rust/Wasm | Choose one owner of simulation state and synchronize render transforms; avoid two competing simulations |
| Rust Native cards and existing app shells | Keep their application and domain roles | A JavaScript viewport needs a host adapter; adopting JavaScript editor panels requires a policy decision about that UI boundary |
| Current renderers and GPU resource objects | Keep for existing games; not directly reusable inside a JS renderer | Port material interfaces, geometry uploads, lighting, postprocessing, and resource lifetimes where migrating |
| Existing native Verse presentation | Remains a separate runtime | JavaScript scenes and material graphs do not become native Rust scenes automatically |

The [Rapier JavaScript bindings](https://rapier.rs/docs/user_guides/javascript/getting_started_js/) demonstrate the general Rust/Wasm simulation plus JavaScript rendering pattern. That is evidence that a hybrid is practical, not a recommendation to replace our physics implementation.

UI reuse needs a separate decision from renderer reuse. PlayCanvas's frontend uses PCUI, while Babylon's Inspector documentation describes React and Fluent dependencies. Keeping every editor panel in Rust Native would reduce how much of these upstream interfaces we could adopt directly. The original Babylon preference assumed that selected authoring tools could be adopted. Building our own interfaces removes that assumption; compare the engines mainly as rendering and scene-manipulation libraries.

Browser Rust here uses ordinary WebAssembly modules and JavaScript/Web APIs. The runner's native Wasmtime/WASI component hosting is a different environment; a server component cannot be assumed to load directly into a browser without adaptation.

Likewise, glTF is a useful asset interchange choice, but game scripts, editor history, custom material graphs, and runtime state require additional formats and conventions. Exporting a mesh does not preserve a complete editable game. Version original assets and engine-specific output separately so that a later renderer change has a defined migration path.

For desktop and phone playback, a web view is one possible way to reuse the browser runtime, with its own embedding, input, graphics, and performance work. Native rendering adapters are another, larger investment. Babylon also has [Babylon Native](https://github.com/BabylonJS/BabylonNative), but that is a separate integration to evaluate, not evidence that our Rust/GPUI/UIKit hosts already support Babylon scenes. Cross-platform playback should be an explicit requirement in the decision.

## Evidence needed before selecting an engine

Use the existing Rust demos as reference measurements. [The Haunted House shader/Kiln issue, #12500](https://github.com/OpenAgentsInc/openagents/issues/12500), now specifies the first WebGPU/WGSL slice in the [browser editor implementation plan](../verse/browser-editor-implementation-plan.md). Its scope has been updated from the earlier WebGL2 demo; no implementation has started.

The next evaluation should use one small Haunted House slice, the same assets, and the same target devices. It should demonstrate these tasks for each serious candidate:

1. Open, save, close, and reopen an editable project under our control.
2. Place and move an asset, adjust lighting, and undo the edits.
3. Edit shader source or a material graph during a walk; keep position and view. Introduce an error, then repair it; retain the last working material throughout the error.
4. Change one behavior while preserving its compatible state; demonstrate what happens when the state becomes incompatible.
5. Have Kiln change the same scene through editor operations, inspect the running result, and undo its change. Include a concurrent manual edit.
6. Import an asset, retain its source, and reproduce its derived output.
7. Publish a chosen snapshot independently of private preview edits, then restore a prior version.

Test ownership early: for a self-operated solution, project creation, editing, assets, saving, and reopening must work without a vendor account or hosted project service. For PlayCanvas, perform the dependency inventory before committing to a frontend fork. For Babylon, prove that selected browser tools can participate in our history and project model. For Three.js, estimate the missing authoring and game systems rather than comparing only the first rendered scene.

Measure edit-to-first-visible-frame separately from agent response time, remote compilation, and publishing. Record warm and cold paths, median and p95, frame stalls during replacement, first-load bytes, memory after repeated edits, error recovery, and touch-browser behavior. Compare equivalent functionality; a uniform slider should not be compared with a full Rust build or an LLM request.

The acceptance bar is uninterrupted compatible edits, useful errors, reversible agent changes, and independent project ownership. No side-by-side results exist yet, so this audit does not assign numerical performance rankings or migration schedules.

## Decision record

- **Selected browser architecture:** own the editor and runtime contracts; use TypeScript + Effect with direct WebGPU and WGSL. See the [follow-up assessment](2026-10-22-owned-renderer-webgpu-and-effect.md).
- **Rust reuse:** existing services and Kiln can remain behind bounded APIs; use browser Wasm kernels where measured CPU benefits justify them.
- **First implementation slice:** #12500 calls for a bounded Haunted House WebGPU/WGSL shader-editing demo connected to Kiln, with retained player state and last working pipeline on failure.
- **Unresolved implementation decisions:** full editor versus guided creator flow, supported browser capabilities, module replacement details, and performance budgets. Browser-first and language freedom are explicit assumptions; hosted authoring services are outside the product direction.
- **Validation of this document:** repository docs and targeted source inspection, plus upstream primary documentation and source. No prototype, engine benchmark, production change, or product-code change was made.
