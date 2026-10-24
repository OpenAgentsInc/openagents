# Platform support and portability

Status: proposed qualification contract. No browser/device combination is yet
qualified. Browser names and nominal hardware are not sufficient evidence.

## Required baseline

The first creator and player product requires a secure context, functioning
WebGPU device, WebAssembly, workers, and the required canvas/input APIs. The
runtime probes capabilities and limits before admitting a profile. It must not
infer support from a user-agent string or silently render an empty world.

| Profile | Required behavior |
| --- | --- |
| Desktop creator | Keyboard/mouse, accessible DOM tools, worker preview, local storage/export, qualified WebGPU |
| Desktop player | Release loading, keyboard/gamepad mapping, bounded runtime, session connection where required |
| Touch creator | Same document and commands, touch targeting, reachable hotbar, software keyboard, gesture-safe dialogs |
| Touch player | Touch controls, orientation/resizing, suspend/resume, thermal/memory quality reductions |
| Offline author | Cached engine and permitted artifacts, local branch, explicit local-save status, later merge/export |
| Accelerated shared-memory | Baseline plus cross-origin isolation and validated shared-memory transport; the primary web performance profile |
| Mobile native (second target) | Same Rust core, renderer, WGSL artifacts, and releases on Dawn; host app supplies surface, input, and lifecycle |
| Desktop native (third target) | Same as mobile native, on desktop surfaces and input |

Chrome, Edge, Firefox, and Safari are qualification candidates, not blanket
support promises. For each release, record browser version, OS, adapter, enabled
features, limits, storage availability, and selected fallback quality. Required
features can vary by profile; optional features cannot become accidental baseline
dependencies. The [WebGPU device API](https://gpuweb.github.io/types/interfaces/GPUDevice.html)
exposes supported features and limits.

## Capability negotiation

1. Probe the canvas/worker path and request an adapter/device using a bounded
   startup operation.
2. Match the selected artifact profile to available GPU limits and supported
   formats. Load the profile's kernel, shaders, and texture variants.
3. Probe persistence separately. A functioning GPU does not imply writable or
   durable browser storage.
4. Report a useful unsupported state with export/read-only project access when
   possible. Never create a partial project or a blank enabled editor.
5. Record the chosen profile in performance and failure evidence.

No WebGL compatibility renderer is selected for the initial engine. A future
fallback must have its own artifact/runtime qualification and explicit feature
restrictions. Cloud video streaming is a separate product mode with a different
latency/cost/privacy contract; it is not used to claim native browser support.

## Memory and threads

The baseline uses transferable buffers with explicit ownership. Shared-memory
paths require an isolated deployment and a tested worker protocol. The
cross-origin-isolated shared-memory profile is the primary performance profile
for player and preview origins
([decision 0006](decisions-history/0006-cross-platform-engine-core.md)); the
transferable profile remains a qualified fallback with identical results. Feature-check
`crossOriginIsolated` and shared-memory availability; never require creators to
weaken browser security to open a project. Browsers use isolation requirements
for shared memory; see the [HTML cross-origin isolation model](https://html.spec.whatwg.org/multipage/browsers.html#cross-origin-isolation).

Mobile and integrated-GPU profiles use lower texture/geometry budgets and optional
passes, with the same game semantics. Quality adaptation can reduce visual detail,
but must not silently remove authoritative colliders, alter puzzle rules, or make
unloaded geometry walkable. A cell lacking required collision remains blocked or
uses its declared conservative proxy.

## Browser lifecycle

- Stop rendering hidden idle previews and reduce non-authoritative background
  simulation. A suspended tab cannot promise continuous multiplayer authority.
- On resume, revalidate session/release identity, elapsed-time policy, and device
  health before resuming input. Never simulate minutes of catch-up in one frame.
- Recover device loss from artifacts and approved state. Report failed recovery
  without losing the editable document.
- Re-measure canvas dimensions, pixel ratio, orientation, and input mappings after
  resize. Dynamic resolution remains within readable UI/accessibility limits.
- Handle unavailable audio autoplay, denied microphone, pointer-lock refusal,
  fullscreen refusal, or gamepad removal through usable alternate controls.

## Portability policy

Worlds and releases identify required engine ABI, behavior VM version, semantic
schema versions, and feature profiles. A loader rejects incompatible required
features with diagnostics; it does not guess at old field meanings. Optional
features require an explicit documented degradation.

Native builds use the same Rust domain, compiler, runtime, and renderer crates,
with Dawn for graphics and wgpu as the fallback backend. They must implement the same command validation and release identity.
Mobile-store code-loading policies, XR, consoles, and native plugin distribution
need separate qualification milestones. A browser specification does not promise
store approval or unrestricted downloaded native code.

## Acceptance

Run startup, first action, shader recovery, input, storage-full, permission-denied,
tab suspend/resume, device loss, and release compatibility tests on every declared
profile. Record exact environment data and skips. A profile with untested required
behavior remains experimental even if a demo launches on it.
