# Vercel Native SDK, Effect Native, and OpenAgents Desktop audit

- Date: 2026-07-14
- Snapshot: analysis at OpenAgents `843668fd3784ca0901dfd835af5c860a1c6504dc`; initial parity pass `9c65c7a81d080cd877e402cfca0fcab2e6f22969`; typed headed-gate continuation based on `7d5a8720ae8c4b63fd08f98a4bfc22c0fec862e5`; assurance/real-command continuation integrated after React/Tailwind renderer-host cutover `a75d1ceaef739107711f88a6cdf666085c66151b`; Native SDK `f7aa92af6dcece250feba852af4d22e7f5429312` (`v0.5.1`); vendored Effect Native `d82ef135a43420883bacf9580f5b644b40787b23` (`effect-native/v39`)
- Class: architecture and dependency audit
- Status: recommendation with bounded hybrid implementation receipt; no
  migration or release authority
- Dispatch: no; the authorized hybrid spike is complete, and any native
  renderer or product migration still needs a bounded issue and claim
- Owner: OpenAgents Desktop and Effect Native architecture
- Final disposition: retain until Native SDK is adopted, rejected, or superseded by a later renderer/host decision
- Decision: retain Electron as the shipping OpenAgents Desktop host; keep the
  completed hybrid spike as evidence that the two runtimes compose; treat
  Native SDK's component catalog as renderer implementation material, not a
  second Effect Native authoring API; require the next proof to be a real
  native lowering rather than another WebView shell; require targeted child-
  WebView event delivery before treating hybrid native controls as production

## Executive decision

[Native SDK](https://github.com/vercel-labs/native) is technically serious and
architecturally relevant to OpenAgents. It is no longer merely the small
WebView shell described by early `zero-native` coverage. At the audited
`v0.5.1` tag it is a roughly 270,000-line Zig/TypeScript/C++/Objective-C
implementation-and-test source tree with its own retained canvas UI,
declarative `.native` markup,
Model/Msg/update runtime, TypeScript-to-Zig compiler, desktop hosts, experimental
mobile hosts, guarded OS capabilities, deterministic automation, packaging,
and optional WebViews.

It is **not** a viable drop-in replacement for Electron in the current
OpenAgents Desktop application. The blocker is not whether Native SDK can draw
buttons or open windows. The blocker is that OpenAgents Desktop is a large
Effect program with npm dependencies, Effect fibers, streams, schemas,
SQLite, process supervision, workspace services, PTYs, Git/GitHub, encrypted
session custody, Codex and Claude runtimes, and a mature schema-checked host
boundary. Native SDK's TypeScript core deliberately cannot run that program:
it has no npm ecosystem, Promise, `async`/`await`, `JSON`, `Map`/`Set`, or
general JavaScript runtime. It compiles a closed pure subset to Zig.

Effect Native **could target Native SDK**, but only if the integration keeps
the responsibilities straight:

- Effect Native remains the one application, state, intent, component, and
  theme authority.
- Native SDK becomes a renderer and selected platform-services host beneath
  that authority.
- The Effect runtime remains real Effect TypeScript in a JavaScript runtime,
  initially a bundled Node sidecar if the browser renderer is removed.
- A generated, versioned, bounded protocol carries resolved Effect Native
  `View` frames or reconciliation operations to a Zig renderer and carries
  typed intents back.
- Native SDK's Model/Msg/update loop is not allowed to become a second product
  state machine, and product screens are not authored a second time in
  `.native`.

That architecture is viable enough to prototype. It is not production-viable
enough to displace Electron today. The rational near-term action is to learn
from Native SDK's deterministic rendering, accessibility snapshots, automation,
source provenance, explicit capability policy, and headless `NullPlatform`,
while shipping the existing Electron + Effect Native design. The bounded
hybrid spike recorded below proves that Native SDK and a real Effect Native
program can share one window. It does **not** prove a native Effect Native
renderer. A future spike should prove a small
`@effect-native/render-native-sdk` adapter, not begin an OpenAgents Desktop
rewrite.

## Implemented hybrid spike receipt

The follow-up implementation lives at
[`apps/native-sdk-effect-native-spike`](../../apps/native-sdk-effect-native-spike/README.md).
It was created with the pinned Native SDK CLI, not hand-scaffolded:

```text
native init apps/native-sdk-effect-native-spike --template zig-core --full
```

The generated starter was then converted into a deliberately bounded,
product-shaped hybrid:

```mermaid
flowchart LR
  NS["Native SDK macOS window"] --> Canvas["Retained Metal canvas"]
  Canvas --> Chrome["Native 232px session rail"]
  Canvas --> Anchor["Semantic WebView pane anchor"]
  Anchor --> WV["WKWebView child surface"]
  WV --> EN["Effect Native ViewProgram + typed intents"]
  EN --> DOM["@effect-native/render-dom"]
  Chrome --> Pull["sequence + acknowledged intent"]
  Pull --> EN
  EN --> Projection["versioned Effect projection"]
  Projection --> Chrome
```

Native SDK owns the window, Metal-backed retained canvas, opinionated native
list/focus components, layout, accessibility snapshot, child-WebView bounds,
and deterministic automation. Inside the WebView, real Effect v4 owns a
`SubscriptionRef`, typed `defineIntent` definitions, an `IntentRegistry`, a
`ViewProgram`, and the
shared Effect Native catalog rendered by `@effect-native/render-dom`.

The current screen deliberately mirrors the real Desktop MVP shape rather than
a counter demo:

- hidden-inset 1200×800 window and full-height 232px native session rail;
- New chat, Chat, Workspace, three recent fixture sessions, and Settings in
  retained Native SDK controls;
- Effect Native `Transcript`, multiline `TextField`, fixed Codex label, and
  icon-only send/stop control using the shared Khala theme;
- deterministic privacy-safe messages and no provider call;
- typed blank-submit no-op, nonblank append/clear/pending behavior, new-chat
  reset, workspace selection, and session selection.

Effect remains the product authority. A native click only records a bounded
intent and shows a synchronization state. The native model's selected row,
workspace, message count, pending state, and status change only after the
Effect program returns a higher-revision projection. Native SDK may paint its
built-in optimistic press echo during the gesture, but the next source tree is
the Effect-confirmed truth. Its model therefore contains a renderer mirror and
transport bookkeeping, not a second transcript or workroom state machine.

The bridge uses one exact-origin command, protocol version 1, monotonic native
sequence and Effect revision numbers, a three-session fixture ceiling, and an
application-enforced 8 KiB JSON limit. Unknown workspaces, sessions, versions,
oversized messages, stale projections, and out-of-range acknowledgements fail
closed. `js_window_api` stays disabled.

The latest parity increment removes another fixture-only seam: native New
chat, Chat, Workspace, and Settings actions now carry the exact production
Desktop command IDs (`chat.new`, `chat.open`, `workspace.home`, and
`settings.open`). The Effect program uses the matching real Desktop intent
names (`DesktopNewChat`, `DesktopWorkspaceSelected`,
`DesktopSettingsToggled`, `DesktopChatSelected`, `DesktopNoteSubmitted`, and
`DesktopTurnInterrupted`). A source import of the canonical Desktop command
registry checks the mapping at module load and in tests; wrong command/intent
pairs fail decoding. This is meaningful command-contract reuse, but it is only
a partial `CW-AC-12` anchor because the fixture still lacks durable host
dispatch, idempotency, steer/queue/question/approval/review flows, and the real
sidecar services.

One upstream boundary became observable during this pass: Native SDK 0.5.1's
macOS bridge response path can target a child WebView by label, but
`emitWindowEvent` evaluates JavaScript only in the primary window WebView. The
Effect renderer is a declared child WebView, so native-to-Effect events do not
arrive there. The spike consequently uses a 120 ms Effect-initiated
projection/pull with acknowledged native sequence. That is acceptable for a
research fixture and not an endorsed production transport. A native renderer
or production hybrid needs targeted child-WebView events, a primary-WebView
topology, or the proposed sidecar renderer protocol.

The app exact-pins the audited Native SDK commit and archive hash in
`build.zig.zon`; it does not depend on the machine-specific
`projects/repos/native` path. The `native init --full` scaffold emitted a
GitHub Actions workflow, which was removed because OpenAgents forbids GitHub
Actions. Local and repository verification remain the authority.

### Spike verification

The following passed on macOS with Zig 0.16.0, pnpm 11.10.0, and the pinned
Native SDK 0.5.1 source:

```text
pnpm run typecheck             -> exit 0
vp test --run frontend/src     -> 2 files, 10 tests passed
vp build                       -> exit 0; 202 modules transformed
zig build test                 -> exit 0; 6 native tests passed
zig build                      -> exit 0
native validate app.zon        -> manifest.valid
native check . --strict        -> web layer included; manifest valid
pnpm run smoke                 -> production-asset headed host gate passed
```

The native tests prove that a native click does not directly mutate selection,
valid higher-revision projections update the mirror, stale/malformed/oversized
projections fail closed, the WebView stays anchored, expected Native SDK
component descriptors remain present, and the product shell lays out. The
Effect Native tests prove composer submit and blank/new-chat semantics,
Transcript/TextField/IconButton catalog emission, strict bridge intent decoding,
bounded projection size, the three-session ceiling, explicit component
adoption matrix, and bounded reload/restart state encoding.

A first manual `native dev -Dautomation=true` smoke proved the development
composition. The parity increment then added
[`scripts/run-host-smoke.ts`](../../apps/native-sdk-effect-native-spike/scripts/run-host-smoke.ts)
to `pnpm verify`. The new gate builds with automation, launches the actual
binary on the bundled `zero://app` asset source, binds snapshots to the exact
child PID and protocol 6, resolves each short-lived widget id from current
semantic role/name, and produces this nine-step private record:

```text
1  initial production Effect projection
2  macOS composited capture -> native rail + live Effect Native pixels
3  native session click -> higher-revision Effect-confirmed selection
4  native workspace round trip -> higher-revision Effect projections
5  deterministic retained-canvas screenshot
6  Effect WebView reload -> state restored
7  native process restart -> state restored under a distinct PID
8  New chat after restart -> no selected fixture session
9  clean teardown of both attested process generations
```

The passing run had zero dispatch errors, a nonblank 1200×800 Metal surface,
17 retained widgets / 17 semantics, a 19.4 ms first native frame inside the
150 ms SDK budget, a named 968×800 child WebView, an accessibility projection,
and a 3.7 MiB deterministic native-canvas PNG. The continuation also captured
a 2536×1736 macOS window PNG containing the composited native rail and live
Effect Native transcript/composer. Its private, schema-decoded
`openagents.native-sdk.host-gate.v3` JSON binds a run nonce, exact
initial/restart PIDs and termination results, protocol 6, macOS ARM64, Node
24.13.1, Zig 0.16.0, the exact Native SDK commit, command, binary,
frontend-bundle, source-set, and every evidence digest. In assurance mode it
also binds the manifest, Environment Profile, adapter lock, target descriptor,
and target source digests. The artifact plus snapshots, accessibility text,
both PNGs, teardown record, and bounded native log live under
`var/native-sdk-effect-native-spike/host-smoke/`; those are ignored runtime
artifacts, not committed Assurance Receipts.

The run found and fixed a real production-path defect: the hybrid wrapper had
not installed `native_sdk.frontend.productionSource`, so a direct built binary
could paint the native shell but could not resolve `zero://app` and was only
functional when `native dev` injected a Vite URL. The wrapper now supplies the
production asset source and environment-aware development source, and rejects
bridge projections from every WebView label except `effect-native-surface`.

The composited live window and new macOS capture show the Native SDK controls
and the live Effect Native catalog together. Native SDK's deterministic PNG
still covers only the retained GPU surface, so the two artifacts remain
separate: deterministic native render evidence and composited-pixel evidence.
Neither extracts child-WebView DOM semantics. Native catalog lowering,
packaging/signing, accessibility acceptance, provider execution, and Electron
host parity remain unproven.

This receipt changes two conclusions from hypothetical to observed: Option A,
the hybrid WebView shell works, and Effect can remain authoritative while a
Native SDK component mirrors and initiates a real session-selection intent. It
also turns targeted child-WebView event delivery from a theoretical concern
into a measured integration gap. The receipt does not satisfy NS-1 or NS-2
below and does not change the shipping-host decision.

## MVP AssuranceSpec integration audit

The harness now has two exact execution targets:

```text
pnpm --dir packages/assurance-spec run assure:mvp                     # Electron
pnpm --dir packages/assurance-spec run assure:mvp --target=native-sdk # Native diagnostic
```

The second form is intentionally a **complete diagnostic execution**, not a
passing or published Native chain. It compiles a separately named in-memory
AssuranceSpec revision, Environment Profile, two-adapter lock, review,
admission, manifest, and 36 Native-only execution units. It then runs the full
headed Native gate once, produces a private gap report, and publishes none of
the staged spec/review/admission/manifest/receipt/index bytes unless every
criterion pair and the target gate succeed.

The lane is disjoint from Electron:

| Layer | Native binding |
| --- | --- |
| target | `openagents.desktop.native-sdk.mvp` |
| criterion catalog | `apps/native-sdk-effect-native-spike/assurance/mvp-assurance-criteria.test.ts` |
| environment | `ENV-OA-DESKTOP-NATIVE-SDK-MACOS-1`; device harness, macOS ARM64, Node 24.13.1, Zig 0.16.0, Native SDK 0.5.1 |
| criterion adapter | `openagents.native_sdk_assurance.v1` over the shared exact Vite Plus/JUnit mechanism |
| target adapter | `openagents.native_sdk_host_gate.v1` over one headed host run |
| gate | `GATE-MVP-FULL-ASSURANCE-NATIVE-SDK` |
| public namespace | `assurance/openagents-desktop-native-sdk-mvp.*` and `assurance/receipts/openagents-desktop-native-sdk-mvp/` |
| private namespace | `var/assurance/openagents-desktop-native-sdk-mvp/` |

The first full diagnostic run executed all 36 criterion units. All 18
missing-anchor falsifiers were `REFUTED` as expected; all 18 candidates were
also `REFUTED` because the target-specific integration anchors are still
absent. The overall criterion gate therefore remains `INCONCLUSIVE`, with
`confirmed_candidates: 0`, `total_obligations: 18`, and publication
`withheld`. In the same run the separately normalized headed host gate was
`ready` and green. This is the desired false-green behavior: a healthy shell
cannot erase missing workroom evidence.

### Two pre-existing integrity failures found and repaired

The assurance continuation found two problems in the current Electron chain
that are independent of Native SDK:

1. `assurance/openagents-desktop-mvp.session.json` pinned an older
   AssuranceSpec digest. The deterministic session ID and spec digest now pin
   the current already-admitted bytes; `session check` reports `unchanged` and
   the owned runner passes. This operational repair is not new admission.
2. The admitted environment requires Node 24.13.1, while the ambient shell was
   Node 25.8.2. The Vite Plus adapter now rejects OS, architecture, runtime,
   required-command, and dependency-lock drift before execution. It invokes
   the exact Vite Plus entrypoint with the already-attested `process.execPath`,
   so a different Node earlier on `PATH` cannot execute the oracle, and binds
   Node, adapter version, entrypoint, and lock digests into command identity.

The committed Electron session records freshness rather than admission. The
Native target still needs its own committed session pin when its admitted
public document exists; the failed diagnostic correctly does not publish that
document. The Native host adapter now normalizes the observed runtime and exact
target bindings, but the generic v0.1 obligation receipt schema still lacks an
explicit observed-environment digest and the Evidence Index still names its
aggregate field `full_desktop_gate`.

The runner also now stages all spec, review, admission, manifest, receipt, and
index bytes in memory. A failed full host gate leaves the last admitted public
chain untouched; publication occurs only after all 36 units and the complete
host gate are green. This closes the partial-publication failure exposed by a
timing-sensitive Electron smoke retry during this continuation.

### Honest Native target boundary

The new headed gate is a useful **host-smoke substrate**, not a passing MVP
AssuranceSpec. It confirms production asset loading, live native controls,
bounded native→Effect actions, monotonic Effect projections, native canvas
evidence, reload, restart, and teardown. It does not confirm ordinary logged-in
Codex custody, ProductSpec/work packets, real child agents, granted repository
and Git operations, durable provider-turn recovery, update staging, privacy
diagnostics, or signed/notarized lifecycle. None of `CW-AC-01…18` is fully
confirmed end-to-end by the fixture.

A passing Native lane uses these stable identities:

```text
target              openagents.desktop.native-sdk.mvp
environment         ENV-OA-DESKTOP-NATIVE-SDK-MACOS-1
criterion adapter   openagents.native_sdk_assurance.v1
host-gate adapter   openagents.native_sdk_host_gate.v1
manifest namespace  assurance/openagents-desktop-native-sdk-mvp.*
run namespace       var/assurance/openagents-desktop-native-sdk-mvp/
```

The historical Electron artifacts remain separately named. No-arg execution
remains Electron-compatible; unknown or extra target arguments fail before
writes. Every Native unit requires Native integration or release anchors;
shared Node/Effect kernels may support but cannot complete a candidate.

The v3 producer artifact is manifest-aware. A headed assurance run passes five
exact digests into the native process: manifest, Environment Profile, adapter
lock, target descriptor, and target source set. Each of the nine ordered steps
now names nonempty exact evidence, both process generations record actual exit
code/signal and whether SIGKILL was required, and a forced kill fails the gate.
The separately owned verifier does not import the app decoder. It rejects
excess fields, malformed/traversal evidence names, rehashes every evidence
file, binary, frontend bundle, and source input, probes both publisher PIDs,
checks the exact Node/Zig/SDK/platform identity, and only then emits a
public-safe target receipt. Missing, stale, crashed, mismatched, or malformed
state remains `INCONCLUSIVE` with no receipt.

Admission and evidence acceptance remain distinct. The owner's direction
authorizes construction and execution of this exact proof design, not advance
acceptance of future observations. Even after all candidates become green, a
separately bound post-run review is required before Native receipt disposition
can become `accepted` and a final Evidence Index can publish.

### Native automation blockers for parallel assurance

Four upstream behaviors matter before admission:

- desktop runners hard-code `.zig-cache/native-sdk-automation` and the CLI has
  no `--dir`; the spike serializes and refuses a live publisher, but six QA
  lanes need isolated run-relative directories;
- `bridge-response.txt` is global while the Effect pane polls every 120 ms;
  `native automate bridge` can observe a background projection response rather
  than its own request unless the SDK correlates exact request ids;
- Native snapshots and deterministic screenshots omit child-WebView DOM and
  pixels. The gate now adds macOS window-scoped composited capture; a separate
  browser/DOM or accessibility adapter is still needed for semantic controls;
- widget ids are runtime-ephemeral; authored scenarios must select by a unique
  current `(view, role, accessibleName, occurrence)` and resolve the id just
  before each action, as the new smoke does.

Completed steps now include owned-runner freshness, outer/child runtime
fidelity, disjoint target descriptors, a red Native criterion catalog, the
namespaced environment/two-adapter design, all 36 executable units, typed v3
host evidence, independent artifact rehashing, target-bound host normalization,
and macOS composited capture. The next honest sequence is to mount the real
shared Desktop Effect Native renderer, add a bundled Node 24 sidecar around the
Electron-neutral services, replace each empty Native criterion anchor with an
integration observation and falsifier, add browser/semantic evidence, and
finish with signed install/update/rollback/uninstall evidence plus post-run
review.

## How to harness Native SDK's opinionated components

Native SDK 0.5.1 exposes 32 built-in retained-canvas components with house
defaults for neutral surfaces, Geist typography, borders, focus states, color,
radius, shadow, blur, and motion. Its higher-value contribution is broader
than pixels: it has explicit structural identity, controlled input and
selection, virtual-list anchoring, keyboard models, native context menus,
semantic roles, deterministic snapshots, and source provenance.

Effect Native should harness that work at four levels:

| Level | Examples | Effect Native policy |
| --- | --- | --- |
| Direct renderer lowerings | Stack, Text, Button, Card, Badge, Divider, TextField, Toggle, Slider | lower a canonical Effect Native node to one Native SDK primitive and prove props, events, identity, semantics, and tokens |
| Owned renderer composites | List/virtual list, Table, SplitPane, Select/Combobox, Modal/Sheet/Drawer, ContextMenu | keep the Effect Native public contract; implement the composition with Native SDK's behavior and test it against Effect Native conformance |
| Typed `Host` drivers | chart, WebView, future specialist native surfaces | expose bounded serializable props/events and scoped resource ownership; do not add general Native SDK escape hatches |
| Retained existing implementations | CodeEditor, terminal, other unsupported specialists | keep DOM/RN/current host drivers until a real native implementation clears its own acceptance gate |

The spike makes this mapping executable in
[`native-sdk-component-adoption.ts`](../../apps/native-sdk-effect-native-spike/frontend/src/native-sdk-component-adoption.ts):
9 direct candidates, 6 composites, 2 host-only candidates, and one explicitly
unsupported specialist. It is a research matrix, not a parity claim.

The parity pass also exercises one useful subset in context: Native SDK's
retained `list_item`, selected/focus state, dark Geist theme, blue accent,
hidden-inset chrome, semantic snapshot, stable widget identity, and automation
form the session rail around the unchanged Effect Native transcript/composer.
This is the right reuse direction—opinionated native behavior behind a narrow
renderer/host boundary—not a reason to expose Native SDK props to product code.

The key architectural opinion is: **adopt Native SDK's component
implementations and behavioral rigor behind Effect Native; do not adopt its
component API as a second product vocabulary.** In practice:

- Effect Native tags, intents, tokens, accessibility props, and catalog
  versions stay canonical and renderer-independent.
- Native SDK widget options and `.native` attributes are private lowering
  details. Product code never imports them.
- Native SDK's component fixtures, keyboard behavior, identity rules,
  accessibility snapshots, virtual-list mechanics, and deterministic
  automation become implementation references and conformance oracles.
- A Native SDK composite may improve Effect Native's contract only through a
  deliberate cross-renderer catalog evolution, not by leaking a Zig-only prop.
- Unsupported tags fail loudly or use a registered typed `Host`; they never
  silently downgrade.
- Upstream code stays a pinned Apache-2.0 dependency or a studied reference.
  If OpenAgents needs divergence, prefer a narrow upstream patch over copying
  a second component library into Effect Native.

This route captures the best part of Native SDK's current setup—its coherent,
opinionated component behavior—while preserving the reason Effect Native
exists: one application and component sentence across web, React Native,
native, and canvas renderers.

## Direct answers

### What is Native SDK?

Native SDK is a Vercel Labs, Apache-2.0 toolkit for native desktop applications,
with experimental iOS and Android support. The audited default application is
three authored files:

```text
src/core.ts     pure Model / Msg / update logic
src/app.native  declarative native UI
app.zon         app, capability, security, window, and packaging manifest
```

The TypeScript core is checked and transpiled to arena-backed Zig. Release
binaries carry no JavaScript engine, browser, or WebView unless the application
explicitly opts into web content. The runtime rebuilds a retained widget tree,
preserves structural/keyed identity, lays it out, produces accessibility state,
and presents pixels through the platform host. WebViews, native controls, and
native chrome remain available as composable surfaces around the canvas.

### Could OpenAgents use it?

Yes, in four bounded ways:

1. **Hybrid composition now.** The completed spike proves Native SDK native
   chrome and components can coexist with the real Effect Native DOM renderer
   in one window without changing the shipping app.
2. **Reference implementation now.** Borrow its testing, deterministic
   automation, accessibility-audit, capability-manifest, source-provenance,
   component behavior, and canvas lifecycle ideas into Effect Native.
3. **Small standalone native utility later.** A status, diagnostics, or focused
   utility application with modest service needs could be a better first
   production candidate than the full coding workroom.
4. **Effect Native renderer experiment.** Implement a small Native SDK-backed
   renderer for the existing Effect Native `View` contract, with the current
   application runtime outside Native SDK's restricted TypeScript core.

It should not be used to fork the OpenAgents UI catalog, rewrite the Desktop
app in `.native`, or replace Electron before host-service and release parity
exist.

### Can Effect Native target it?

Yes at the renderer boundary; no at the Native SDK TypeScript-core boundary.

Effect Native already exposes the right abstraction:

```ts
interface RendererAdapter<Container, Surface> {
  mount(
    container: Container,
    viewStream: Stream<View>,
    report: IntentReporter,
  ): Effect<Surface, never, Scope>
}
```

A Native SDK adapter can subscribe to that `viewStream`, lower catalog nodes to
Native SDK widget descriptions, and report user actions through the existing
typed `IntentReporter`. Nothing about that contract requires DOM, React, React
Native, Electron, SwiftUI, or a browser.

The existing Effect program cannot be sent through Native SDK's TypeScript
transpiler. Effect v4 and OpenAgents rely on npm modules, higher-order runtime
behavior, asynchronous effects, fibers, streams, schemas, service layers, and
dynamic JavaScript facilities that the Native SDK subset excludes by design.
An adapter therefore needs a runtime/renderer bridge or a future in-process JS
embedding layer; it is not a compiler flag.

### Should it replace Electron now?

No. Electron remains the lower-risk shipping host for the current OpenAgents
Desktop capability set and release program. Native SDK should be revisited
after a renderer spike and a measured host-parity proof, not selected on the
strength of small-binary or native-pixel expectations alone.

## Evidence basis

The upstream reference was cloned at the requested
`projects/repos/native` path and audited at exact tag `v0.5.1`. The Git remote
`vercel-labs/native` and the older `vercel-labs/zero-native` URL resolve to the
same repository. The audit covered source, tests, current documentation,
history, tags, releases, and open upstream issues.

Pinned Native SDK sources:

- [README and quick start](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/README.md)
- [v0.5.1 changelog](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/CHANGELOG.md)
- [TypeScript core contract](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/typescript/page.mdx)
- [where npm packages go](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/typescript/packages/page.mdx)
- [app model](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/app-model/page.mdx)
- [native UI and accessibility contract](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/native-ui/page.mdx)
- [native surfaces](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/native-surfaces/page.mdx)
- [platform matrix](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/platform-support/page.mdx)
- [security model](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/security/page.mdx)
- [capabilities](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/capabilities/page.mdx)
- [automation](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/automation/page.mdx)
- [packaging](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/packaging/page.mdx)
- [signing](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/packaging/signing/page.mdx)
- [update placeholder](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/updates/page.mdx)
- [extension registry and experimental JS seam](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/extensions/page.mdx)
- [embed C ABI](https://github.com/vercel-labs/native/blob/f7aa92af6dcece250feba852af4d22e7f5429312/docs/src/app/embed/page.mdx)

OpenAgents sources inspected:

- [Desktop architecture and capability ledger](../../apps/openagents-desktop/README.md)
- [Desktop package and Electron version](../../apps/openagents-desktop/package.json)
- [Electron main process](../../apps/openagents-desktop/src/main.ts)
- [Forge release configuration](../../apps/openagents-desktop/forge.config.ts)
- [Effect Native renderer boot](../../apps/openagents-desktop/src/renderer/boot.ts)
- [mechanical Electron/Effect Native boundary oracle](../../apps/openagents-desktop/tests/electron-boundary.test.ts)
- [vendored Effect Native pin](../../apps/openagents.com/packages/effect-native-vendor.json)
- [vendored Effect Native core and renderer adapter](../../apps/openagents.com/packages/effect-native-core/src/index.ts)
- [mobile Effect Native host](../../apps/openagents-mobile/src/effect-native/effect-native-host.tsx)
- [Effect Native framework dossier](../effect-native/README.md)
- [one-UI substrate and EN-5/EN-6 plan](../effect-native/2026-07-08-effect-native-one-ui-substrate-analysis.md)
- [SwiftUI renderer audit](../effect-native/2026-07-09-effect-native-swiftui-renderer-audit.md)
- [current component-demand register](../effect-native/DEMAND_REGISTER.md)
- [bounded hybrid spike](../../apps/native-sdk-effect-native-spike/README.md)
- [Native SDK host and native component proof](../../apps/native-sdk-effect-native-spike/src/main.zig)
- [Effect Native program proof](../../apps/native-sdk-effect-native-spike/frontend/src/program.ts)
- [production Desktop command mapping](../../apps/native-sdk-effect-native-spike/frontend/src/production-command-parity.ts)
- [Native MVP criterion catalog](../../apps/native-sdk-effect-native-spike/assurance/mvp-assurance-criteria.test.ts)
- [bounded reload/restart state storage](../../apps/native-sdk-effect-native-spike/frontend/src/state-storage.ts)
- [component-adoption matrix](../../apps/native-sdk-effect-native-spike/frontend/src/native-sdk-component-adoption.ts)
- [headed production host gate](../../apps/native-sdk-effect-native-spike/scripts/run-host-smoke.ts)
- [current admitted MVP assurance runner](../../packages/assurance-spec/scripts/run-mvp-assurance.ts)
- [Native criterion and host-gate verifier](../../packages/assurance-spec/src/native-sdk-assurance-adapter.ts)
- [current MVP criterion anchors](../../apps/openagents-desktop/src/mvp-assurance-criteria.test.ts)

The upstream Native SDK repository had 270 commits from 2026-05-08 through the
audited 2026-07-13 release, 268 authored by one contributor and one each by two
others on the audited branch. GitHub reported 6,254 stars, 254 forks, 30 open
issues, and 40 open pull requests at review time. These are maturity signals,
not quality verdicts:
the project has real momentum and unusually broad implementation, but still
has concentrated stewardship, very high API churn, and no stable 1.0 contract.

Verification run on the pinned source:

```text
zig 0.16.0
zig build test  -> exit 0
```

The run emitted expected negative-test diagnostics for invalid signing,
manifest, replay, font, and effect cases. It also reported that the Android
toolchain was unavailable on this Mac, so Android project generation was
exercised but APK assembly was not local proof. No upstream source was changed.

## How Native SDK works

### One toolkit with three application modes

The current repository has three meaningful modes, not one:

| Mode | UI | Application logic | Runtime cost and purpose |
| --- | --- | --- | --- |
| Native-first default | `.native` markup or Zig widget builder | restricted TypeScript compiled to Zig, or Zig | no browser or JS engine; smallest and most deterministic shape |
| Hybrid | canvas/native chrome plus selected WebViews | Zig/compiled core plus web application where needed | specialist web content without making the entire app a WebView |
| Web frontend | Next, Vite, React, Svelte, Vue, or static assets in system WebView/CEF | JavaScript in WebView plus guarded native bridge | easiest compatibility path; much closer to a web shell |

The first mode is the strategically interesting one for Effect Native. The
third mode could host today's DOM renderer but would mostly exchange Electron's
Chromium/Node integration for system-WebView variability and a new native host.
It would prove shell portability, not a native Effect Native renderer.

### TypeScript is an authoring language, not a JavaScript runtime

Native SDK's `core.ts` contract is unusually explicit:

- `Model` is readonly application data.
- `Msg` is a discriminated union of every event.
- `update(model, msg)` is pure and synchronous.
- asynchronous work is returned as inert `Cmd` data and recurring work as
  `Sub` data;
- dynamic text is UTF-8 `Uint8Array`, with byte-oriented semantics;
- frame and committed-model arenas default to fixed 1 MiB capacities and panic
  loudly on overflow;
- the emitted release code is Zig, with no JavaScript engine or garbage
  collector.

The subset is broad as a language but intentionally closed as an ecosystem.
It excludes npm packages, `JSON`, Promise, `async`/`await`, regular expressions,
`Map`, `Set`, `eval`, module-level mutable state, and ambient time/randomness in
`update`. This is the source of its deterministic replay and small runtime. It
is also why OpenAgents cannot compile Effect into it.

The built-in command vocabulary is useful but bounded: time, delay, whole-file
read/write, buffered fetch, clipboard, subprocess streaming or collection,
audio, host requests, cancellation, and batching. Persistence in the
TypeScript tier is currently file read/write, not an application database.

Node libraries are supported as explicit child-process sidecars. The default
line stream limit is 4 KiB, raisable per spawn to a hard 256 KiB ceiling;
collected stdout is bounded to 512 KiB. That mechanism is appropriate for
bounded commands and agent-event lines, but it is not by itself a production
renderer transport for arbitrary Effect Native trees. A renderer adapter needs
an owned framed protocol with backpressure, version negotiation, bounded
payloads, crash recovery, and lifecycle receipts.

### Rendering is native-hosted retained canvas, not an OS widget tree

The default UI is native in the important binary and presentation sense: it is
compiled code in a real OS window, with no browser engine, and the toolkit owns
input, layout, accessibility, reconciliation, and pixels. Most catalog controls
are retained canvas widgets, not direct AppKit, WinUI, GTK, SwiftUI, or Compose
controls. Native chrome, menus, controls, dialogs, tray items, and WebViews are
separate host surfaces that can be composed around the canvas.

This distinction matters for architecture and product claims:

- Native SDK can deliver consistent, deterministic visuals and screenshots.
- It does not automatically inherit every platform control's behavior or
  visual fidelity.
- Accessibility requires a correct semantic bridge from retained widgets to
  each OS, which the project implements and tests but must be accepted on real
  assistive technologies.
- Input method, font shaping, text selection, focus, drag/drop, large lists,
  and specialist editor/terminal behavior remain renderer work.

After each update, the toolkit rebuilds the view and preserves runtime-owned
state through structural identity, sibling keys, and global keys. Release
markup is compiled at Zig comptime; debug mode can parse and hot-reload markup
while keeping the model and widget identity.

### Platform reality at v0.5.1

| Area | macOS | Windows | Linux | Mobile |
| --- | --- | --- | --- | --- |
| Window host | full desktop | full desktop | full desktop | toolkit-owned single-window host or embed, experimental |
| Canvas presentation | Metal-backed | deterministic software renderer, GDI blit | deterministic software renderer, cairo blit | CPU renderer presented by UIKit/Android host |
| Web engine | WKWebView; optional bundled CEF | system WebView2 | system WebKitGTK | system WebView in host/embed paths |
| Native app menus | supported | supported | supported | unsupported |
| Tray | supported | supported | unsupported | unsupported |
| Packaging | `.app`, DMG, icons | directory artifact; installer future work | install tree; packages/AppImage future work | generated projects; manual store signing |
| Signing | identity/ad-hoc; manual notarization step | no SDK signing tooling | no SDK signing tooling | no SDK signing tooling |
| Multi-window | supported | supported | supported | desktop only |
| Automation | full file protocol | full | full under Xvfb | experimental file/embed protocol |

Mac is clearly the lead platform. OpenAgents Desktop currently ships a macOS
release lane, so that is not fatal for a spike. It is fatal to any claim that a
host replacement already improves cross-platform delivery.

### Security posture

Native SDK has a good security direction:

- `app.zon` declares capabilities and runtime permissions;
- bridge commands are default-deny and require registration, policy, matching
  permission, and allowed origin;
- navigation is origin-allowlisted;
- external links are denied unless action and URL patterns are explicit;
- programmatic child WebViews receive a bridge only when created with
  `bridge: true`; declared shell WebViews are bridge-enabled by the current
  shell layout path;
- dialog, clipboard, credential, and OS commands always require explicit
  builtin bridge policy;
- the public bridge guide describes 16 KiB request / 12 KiB result ceilings,
  but the audited v0.5.1 source currently sets request, response, and result
  maxima to 1 MiB in `src/bridge/root.zig`;
- native-only builds can exclude WebView libraries entirely.

This is compatible with OpenAgents' tokenless-renderer discipline. It does not
automatically reproduce it. The current Electron boundary is enforced by
schema checks and source oracles around `contextIsolation`, sandboxing, fixed
IPC channels, sender validation, CSP, navigation denial, and host-owned
credentials. A Native SDK adoption would need equivalent OpenAgents-specific
oracles over the new renderer protocol and every native extension. Moving away
from Chromium removes one attack surface but introduces a new sidecar/native
ABI and protocol surface. OpenAgents must therefore enforce its own lower
protocol ceiling rather than inheriting the upstream default. The parity spike
uses 8 KiB, exact `zero://app` / loopback-dev origins, protocol version 1, and
monotonic revision/sequence checks.

Credential storage is implemented through Keychain on macOS, Credential
Manager on Windows, and Secret Service/libsecret where available on Linux.
This is a promising replacement for Electron `safeStorage`, but parity is not
just “can store a secret.” OpenAgents must preserve refusal when secure storage
is unavailable, atomic encrypted-record lifecycle, rotation validation,
sign-out/revocation semantics, and the guarantee that credentials never cross
the renderer protocol.

### Automation and agent-facing ergonomics

Native SDK's strongest immediately reusable ideas are in automation:

- a file-based command queue with consumption acknowledgements;
- accessibility snapshots with roles, names, bounds, state, and focus;
- deterministic screenshots through the CPU reference renderer;
- input, focus, menu, shortcut, tray, drag, wheel, resize, and bridge actions;
- rolling p50/p90/max timing by render-pipeline stage;
- record/replay against state fingerprints;
- source provenance from a live widget to file/span/template/iteration keys;
- guarded minimal-diff source write-back followed by hot reload;
- `NullPlatform` and headless application harnesses.

This aligns exceptionally well with an agent-authored Effect Native product.
Effect Native already treats UI as validated serializable data and tests
renderer conformance. Adopting comparable provenance and deterministic
headless rendering may deliver value even if Native SDK never becomes a
shipping host.

### Packaging and updates are not yet Electron Forge parity

Native SDK packages a compelling macOS native-only app and has explicit
WebView-layer audits, generated icons, signing, and a DMG path. The broader
release story is incomplete:

- notarization submission and stapling remain manual documented steps;
- Windows has a directory artifact, no installer or signing tooling;
- Linux has an install tree, no AppImage/deb/rpm tooling;
- iOS and Android are experimental generated hosts with manual production
  signing;
- the `updates` manifest fields reserve a feed URL, public key, and
  check-on-start flag, but the runtime does not provide a silent or complete
  update installer.

OpenAgents' Forge configuration already assembles a hardened Electron package,
unpacks required runtime executables/workers, includes the native audio helper,
applies fuses, signs/notarizes, and makes macOS artifacts. A migration would
have to reproduce that exact payload and acceptance lane before it could be a
release simplification.

## The current OpenAgents architecture

### Effect Native is the application contract

The vendored `effect-native/v39` snapshot defines a large serializable `View`
union, typed `IntentRef` values, an `IntentRegistry`, `SubscriptionRef` state,
`Stream<View>` output, Effect services, and a generic renderer adapter. The
catalog contains far more than basic controls: workbench/navigation structures,
composer, transcript, Markdown, code block, diff, graph, timeline, command
palette, overlays, forms, feedback, marketing, mobile, glass, avatar, copy, and
loading components, plus a typed `Host` escape hatch for editor, terminal,
media, voice, and other specialist widgets.

The current product has two real renderer receipts:

- OpenAgents Desktop now uses a React 19-owned application root and lifecycle,
  with `@effect-native/render-dom/react` subscribing to the Effect Native View
  stream and retaining the proven catalog lowering under that host. Vite and
  Tailwind CSS 4 are build/styling infrastructure, not a second application
  state or schema authority.
- OpenAgents mobile mounts the React Native renderer through an explicit
  React/React Native host binding; application screens remain Effect Native
  data and do not author a second React component system.

The framework dossier deliberately makes renderer replacement possible. It
also makes the component set, intents, services, and Effect runtime—not any
particular renderer—the source of truth.

### Electron is a host, not the UI model

OpenAgents Desktop `0.1.0-rc.12` uses Electron `43.1.0` and Electron Forge.
The renderer boot mounts the React-owned Effect Native DOM adapter over a
`View` stream; Effect Native remains the application contract and the Electron
main process owns runtime and OS authority. The hardened window
uses `contextIsolation: true`, `nodeIntegration: false`, `sandbox: true`,
`webviewTag: false`, `webSecurity: true`, deny-by-default navigation and window
creation, a restrictive CSP, and a fixed preload bridge.

The host currently owns or coordinates capabilities that matter to any
replacement decision:

- Khala Sync SQLite and device-local tables;
- encrypted native-session custody through `safeStorage`;
- OpenAuth loopback/PKCE and token rotation/revocation;
- Codex and Claude executables and provider account probes;
- agent process lifecycle, event streaming, interruption, and teardown;
- selected workspace filesystem, watching, paged search, edit, and save;
- bounded Git and GitHub operations;
- workspace-scoped PTYs and terminal lifecycle;
- audio helper/runtime assets;
- application protocol, deep-link, command, menu, and single-instance paths;
- diagnostics, preferences, updates, and release acceptance;
- closed, schema-decoded renderer projection and intent channels.

Native SDK covers meaningful pieces—windows, menus, shortcuts, dialogs,
clipboard, credentials, file drops, notifications, URL schemes, files, fetch,
subprocesses, audio, and native extensions—but not this combined contract.
The comparison unit must be the complete workroom host, not a counter app.

### Effect Native has already invested in Electron

The upstream Effect Native source at the pinned commit contains an
`@effect-native/platform-electron` package in addition to the generic desktop
contracts. It models hardened preferences, CSP, deny-by-default security,
typed IPC envelopes, sender policy, window/menu/deep-link/single-instance
services, lifecycle, and the DOM mount while depending only on structural
Electron interfaces. The monorepo does not yet vendor that package, and the
Desktop app still carries its larger application-specific host locally, but
the direction is deliberate rather than accidental.

A host replacement therefore has an opportunity cost: it pauses convergence
on a reusable, already-tested Electron platform adapter and opens a second
platform program during a shipping release sequence.

## Where Native SDK and Effect Native align

The two projects independently make several compatible architectural choices:

| Concern | Effect Native | Native SDK | Fit |
| --- | --- | --- | --- |
| UI representation | closed, serializable typed `View` union | closed markup/builder widget tree | strong |
| User actions | typed intent references | typed Msg/command dispatch | strong |
| State | `SubscriptionRef`/Effect program | Model/Msg/update | conceptually strong, but only one may own product state |
| Effects | Effect values, Layers, Scope, Stream | inert commands and recurring subscriptions | conceptually strong; runtimes are not interchangeable |
| Renderer lifecycle | scoped `RendererAdapter.mount` | runtime-owned window/canvas lifecycle | strong adapter seam |
| Identity | catalog keys and renderer reconciliation | structural, keyed, global-key identity | strong |
| Headless proof | renderer conformance/test harnesses | `NullPlatform`, record/replay, deterministic screenshots | excellent |
| Accessibility | typed props and per-renderer lowering | semantic tree plus validation and OS bridges | strong goal; needs platform acceptance |
| Styling | typed tokens and per-renderer lowering | named theme packs and tokens | mappable, not identical |
| Foreign widgets | typed `Host` nodes with scoped drivers | Zig custom widgets/modules, native surfaces, WebViews | viable explicit boundary |

This is enough common ground for a renderer. It is also a warning: the systems
overlap at the catalog, state, event, token, effect, and lifecycle layers. A
careless integration would have two frameworks trying to own the same app.

## The non-negotiable source-of-truth rule

If OpenAgents uses Native SDK under Effect Native, the authority split must be:

```mermaid
flowchart LR
  A["OpenAgents Effect program"] --> B["Effect Native View stream"]
  B --> C["Versioned renderer bridge"]
  C --> D["Native SDK canvas and platform host"]
  D --> E["Typed input intent"]
  E --> C
  C --> F["Effect Native IntentReporter"]
  F --> A
  G["Host-owned workspace, Sync, process, auth services"] --> A
  D --> H["OS window, input, accessibility, menus, dialogs"]
```

Rules:

1. Effect Native `View` and token schemas remain canonical.
2. Native SDK widget descriptions are generated/lowered implementation data,
   not product authoring APIs.
3. Native SDK `Model` may hold renderer-local ephemeral state only—focus,
   hover, selection, layout caches—not application truth.
4. Every native event maps to an existing typed intent or an explicit catalog
   evolution.
5. Unsupported catalog nodes fail conformance or lower through a registered
   `Host`; they never disappear silently.
6. The renderer process receives no access/provider/Pylon credentials, raw
   database handle, arbitrary process command, or unbounded filesystem path.
7. Product screens are never maintained both as Effect Native trees and
   `.native` markup.

The `.native` language remains valuable for upstream examples, isolated native
widgets, or generated fixtures. It should not become a second OpenAgents screen
language.

## Integration options

### Option A: host the current React-owned DOM renderer in a Native SDK WebView

Native SDK can package the current browser output and load it in WKWebView,
WebView2, or WebKitGTK; macOS can optionally bundle CEF.

Advantages:

- lowest UI conversion cost;
- Effect, the React-owned root, and the current Effect Native DOM lowering
  continue unchanged;
- Native SDK can own native windows, menus, dialogs, credentials, and selected
  capabilities.

Costs:

- a JavaScript runtime still exists in the WebView;
- system-engine behavior differs by OS, while bundled Chromium is macOS-only;
- current Node/Electron main capabilities still need a sidecar or rewrite;
- the result gains little from Native SDK's retained native UI;
- it replaces a mature Electron security/release boundary with a younger one.

Verdict after implementation: technically proven on macOS and still
strategically weak as a migration destination. Keep the completed app as a
shell-portability and component-composition fixture, not as evidence that
Effect Native has a native renderer. The native rail round trip also requires
a pull/ack workaround because window events do not target the declared child
WebView at v0.5.1; resolve that transport gap before expanding this option.

### Option B: run Effect in Node and Native SDK as an out-of-process renderer

This is the recommended spike architecture.

The Node process owns the existing Effect program, state, intent registry, and
most current host services. A Native SDK Zig process owns windows, input,
accessibility, canvas rendering, and selected OS capabilities. A generated
protocol carries bounded tree snapshots or keyed reconciliation operations and
typed input events.

Advantages:

- preserves Effect Native instead of reimplementing it;
- reaches Native SDK's native-only canvas path;
- isolates renderer crashes and keeps the renderer tokenless;
- permits incremental catalog coverage and headless protocol tests;
- reuses the existing `RendererAdapter` abstraction.

Costs:

- Node must be bundled and lifecycle-managed after Electron no longer supplies
  it;
- every frame crosses a process boundary unless diffing/coalescing is good;
- protocol versioning, backpressure, crash recovery, focus/IME ordering, asset
  transfer, and shutdown become new correctness work;
- Electron's integrated Node/Chromium packaging is replaced by two-runtime
  packaging;
- specialist WebViews and native `Host` drivers still need explicit ownership.

Verdict: best technical route to a real Native SDK renderer; not automatically
smaller or simpler than Electron for this product.

### Option C: embed a JavaScript engine inside Native SDK

Native SDK has an experimental JS abstraction and `NullEngine`, but not a
shipping embedded engine capable of running the OpenAgents Effect graph. Adding
QuickJS, JavaScriptCore, V8, or another engine could place Effect and the
renderer in one native process.

This conflicts with Native SDK's principal “no JS engine in the binary” value,
creates a large integration/security/GC/debugging program, and risks rebuilding
what Electron or a WebView already supplies. It may become attractive only if
Native SDK upstream adopts a stable engine/module contract with the semantics
Effect needs.

Verdict: reject for a first spike.

### Option D: compile Effect Native views into `.native` or Zig

Static code generation can lower a fixed Effect Native tree or component
fixture into Native SDK markup/Zig. It cannot compile the current live Effect
runtime, services, streams, or arbitrary state transitions into the Native SDK
TypeScript subset. Dynamic applications would still need a runtime bridge.

Code generation is useful for:

- native catalog type definitions;
- exhaustive tag decoders;
- token tables;
- static golden/conformance fixtures;
- accessibility snapshots;
- small renderer-owned composites.

Verdict: use code generation inside Option B, not as an application compiler.

### Option E: rewrite OpenAgents in Native SDK's Model/Msg/update

This would discard Effect Native's whole-app premise and fork the application,
services, catalog, behavior contracts, mobile/web sharing, and current host
proofs. Superficial Elm/MVU similarity does not make the runtimes equivalent.

Verdict: reject.

## Proposed `@effect-native/render-native-sdk` shape

The renderer package should remain conceptually parallel to DOM, RN, and a
future SwiftUI renderer:

```text
@effect-native/core
  View v39 + IntentRef + tokens + RendererAdapter
          |
@effect-native/render-native-sdk
  compatibility matrix
  resolved-style lowering
  keyed reconciliation
  asset registry
  protocol client
          |
native-sdk-renderer
  protocol decoder
  widget/catalog lowering
  Native SDK Runtime + platform host
  accessibility/input/automation bridge
```

### Bridge contract

The first protocol should include:

- protocol and Effect Native catalog versions;
- renderer platform/capability handshake;
- full initial tree plus bounded keyed patches or coalesced latest-tree frames;
- resolved theme tokens and platform appearance;
- stable node keys and host-kind identifiers;
- asset registration by digest, never repeated base64 in every frame;
- intent reports with node ref, intent ref, validated runtime payload, and
  monotonic sequence;
- focus, text-edit, IME composition, selection, scroll, resize, and
  accessibility actions with ordering guarantees;
- renderer-ready, frame-presented, backpressure, unsupported-node, crash, and
  unmount receipts;
- payload/queue ceilings and explicit loss/coalescing counters.

Use an actual framed IPC transport. Do not overload Native SDK's application
`Cmd.spawn` NDJSON line channel for full view trees, and do not expose its
generic WebView bridge to the product renderer.

### Initial catalog slice

Start with a deliberately uninteresting slice:

- `Stack`
- `Text`
- `Button`
- `Card`
- `Spacer`
- `Icon`
- `Divider`
- one controlled `TextField`
- one keyed/virtualized `List`

That slice proves recursion, tokens, intent dispatch, focus, text/IME,
identity, list behavior, accessibility, and asset/icon handling. Do not begin
with Composer, Transcript, Workbench, terminal, diff, graph, Markdown, or
command palette; those hide protocol flaws behind component-specific work.

### Catalog parity risks

Native SDK has a broad component library, but similar names are not semantic
parity. Effect Native v39 contains product-level nodes and behavior contracts
that have no one-to-one Native SDK equivalent. The renderer needs an explicit
matrix for:

- `Composer` and its attachment/voice/send/focus behavior;
- `Transcript`, streaming content, and follow-tail policy;
- `CodeBlock` and `DiffView` selection/copy/virtualization;
- `GraphFigure` and the canvas/Three.js ownership boundary;
- `Workbench`, `SplitPane`, and `NavRail` desktop semantics;
- overlays, sheets, modal focus traps, and command palette;
- responsive/platform/state style variants;
- `Host` kinds such as terminal and editor;
- reduced motion, contrast, keyboard, and assistive behavior;
- secure/redacted fields and automation snapshots.

A renderer is conformant when these contracts match, not when screenshots
roughly resemble each other.

### Host services

Keep product services outside the UI renderer. Native SDK platform capabilities
may implement Effect service Layers over time, for example:

- clipboard;
- safe external URL opening;
- dialogs;
- credential storage;
- notifications;
- menus, tray, windows, and deep links;
- file-drop events.

The existing Node host should initially retain:

- SQLite/Khala Sync;
- OpenAuth/network session lifecycle;
- workspace service, watcher, paged search, and bounded edit;
- PTY and terminal process lifecycle;
- Codex/Claude execution and provider integrations;
- Git/GitHub operations;
- updater/release logic;
- zstd history and worker topology.

Moving a service into Zig is a later independent decision with its own parity
tests. Renderer adoption must not force an all-at-once host rewrite.

## Viability matrix

Scores are decision aids for this snapshot, not evergreen benchmarks.

| Use | Technical viability | Product viability now | Assessment |
| --- | ---: | ---: | --- |
| Keep Electron + Effect Native | 9/10 | 9/10 | shipping path; preserve |
| Learn from Native SDK without adopting runtime | 10/10 | 10/10 | immediate value |
| Small standalone macOS native utility | 8/10 | 6/10 | good first production candidate if demanded |
| Current Effect Native DOM app in Native SDK WebView | 8/10 | 4/10 | possible, low payoff |
| Native SDK-backed Effect Native renderer spike | 7/10 | 3/10 | worthwhile bounded research |
| Full OpenAgents Desktop on renderer + Node sidecar | 6/10 | 2/10 | possible after substantial proof |
| Compile current Effect program with Native SDK TS | 1/10 | 0/10 | incompatible runtime contract |
| Rewrite product in `.native`/Model-Msg-update | 5/10 | 0/10 | violates one-catalog/Effect direction |
| Replace RN mobile host | 3/10 | 1/10 | mobile support experimental; RN is mature |

## Risks and counterevidence

### Pre-1.0 churn and stewardship concentration

Native SDK moved from `v0.4.0` to `v0.5.1` in five days around this audit, and
the TypeScript authoring tier arrived in `v0.5.0` less than a day before
`v0.5.1`.
Fast progress is attractive, but any integration must exact-pin a commit,
vendor/generate its protocol bindings, and budget for breaking changes. The
branch history is overwhelmingly single-author, which raises bus-factor and
review-depth risk for a security- and accessibility-bearing desktop host.

### International text and font coverage

The native UI checker rejects literal characters absent from the bundled font,
and dynamic misses become debug diagnostics/tofu on reference/mobile paths.
The open upstream
[Chinese rendering issue](https://github.com/vercel-labs/native/issues/109)
is direct counterevidence to assuming international text is solved. OpenAgents
must prove shaping, fallback fonts, Unicode scripts, emoji, bidi, selection,
and IME on its actual transcripts before any production decision.

### Canvas performance is workload-dependent

The retained renderer avoids frames while idle and exposes excellent stage
profiling. It also has a documented open issue where unchanged registered
images are replanned at roughly 1.35 ms per drawn 512px image per produced
frame in the reporter's workload:
[image-plan issue](https://github.com/vercel-labs/native/issues/101).
OpenAgents' streaming transcript, avatars, previews, graph surfaces, and editor
islands need measurement; “native” is not itself a performance result.

### Cross-platform fidelity is uneven

Windows and Linux use software presentation, mobile is experimental, several
native affordances vary by host, and signing/install/update support is
macOS-led. The current product should not exchange Chromium consistency for
system-WebView and custom-canvas variability without screen-by-screen,
input-by-input acceptance.

### Two runtimes may erase the size/simplicity win

A pure Native SDK app can be compact because it has no JS/browser runtime. An
Effect Native product still needs Effect TypeScript. If the practical design
bundles Node plus Native SDK—and retains WebViews for editor, auth, preview, or
specialist content—the final artifact and process topology may be different
from Electron rather than simpler. Measure signed package size, installed size,
RSS, process count, cold/warm launch, and first interactive frame.

### Accessibility needs real-host acceptance

Native SDK's semantic model, machine checks, snapshots, and macOS bridge are
substantial. OpenAgents still needs VoiceOver, Windows UI Automation/Narrator,
Linux assistive technology, keyboard-only, focus restoration, zoom, contrast,
and reduced-motion acceptance. Deterministic accessibility text files are
excellent tests but are not the final user-facing proof.

## Bounded proof plan

This plan is deliberately non-dispatching. The completed hybrid app is an
Option A precursor and does not clear any native-renderer phase. If owner
direction and a claimed issue authorize the next spike, use these phases and
stop gates.

The parity pass borrows the NS-3 fixture shape—session rail, transcript, and
composer—but does not clear NS-3: the transcript is DOM-rendered in a child
WebView, is not a native catalog lowering, has no long-running streaming soak,
and uses the research pull/ack transport described above.

### NS-0: pin and contract

- exact-pin Native SDK and Zig, never float a tag range;
- write an Effect Native renderer RFC in the owned `effect-native` repo;
- freeze which side owns state, services, focus, assets, and lifecycle;
- define the protocol threat model and renderer authority ceiling;
- record all upstream patches required by the spike.

Exit: no application code or `.native` product screen exists until the
source-of-truth and protocol contracts are reviewable.

### NS-1: headless protocol proof

- generate protocol types from the Effect Native catalog version;
- drive the nine-node initial slice through a fake/NullPlatform renderer;
- prove identical intent logs and stable keyed identity against headless/DOM;
- fuzz decoders, unknown tags, payload ceilings, reordered/duplicate events,
  cancellation, and renderer restart;
- prove Scope close terminates transport and native resources exactly once.

Exit: one catalog tree and one intent transcript match; malformed renderer or
runtime input fails closed.

### NS-2: real macOS renderer

- render the initial slice in a Native SDK window;
- prove theme/token parity and deterministic screenshots;
- test real keyboard, pointer, focus, VoiceOver, text selection, paste, IME,
  Unicode fallback, drag/drop, and high-DPI resize;
- profile full-tree and patch/coalesced updates under streaming state.

Exit: behavior and accessibility contracts pass, not only visual comparison.

### NS-3: one product-shaped screen

Use a read-only, privacy-safe transcript/workbench fixture. Add only the
minimum components required. Keep execution, workspace mutation, credentials,
and live Sync out of this phase.

Exit: a realistic screen survives 30 minutes of streaming updates, resize,
focus changes, renderer restart, and deterministic teardown with no loss that
is not counted.

### NS-4: one specialist Host

Choose one hard boundary, preferably terminal or code/diff, and decide whether
it is a native widget, WebView, or separately rendered surface. Prove typed
props/events, layout, focus traversal, accessibility, disposal, and crash
containment.

Exit: the foreign-host mechanism is real; the main catalog remains unpolluted.

### NS-5: host and release parity matrix

Prove or explicitly defer every current Desktop capability: SQLite, session
custody, auth, process runtime, workspace, PTY, Git/GitHub, history, audio,
protocol/deep links, single instance, menus, diagnostics, updates, signing,
notarization, smoke, and release acceptance.

Exit: no missing capability is hidden by a demo-only route.

### NS-6: measured decision

Compare the signed products on the same Mac and fixtures:

- compressed and installed size;
- cold and warm process-to-first-present and process-to-first-input;
- idle and active RSS/CPU/process count;
- streaming transcript latency and dropped/coalesced frame counts;
- 100,000-row keyed/virtual list behavior;
- resize, focus, text/IME, and accessibility latency;
- renderer and host crash recovery;
- CI, packaging, signing, notarization, and release time.

Proceed beyond research only if the Native SDK path wins a named product goal
and reaches capability/security/accessibility parity without forking the
Effect Native source of truth. “Native,” “Zig,” or smaller hello-world output
are not sufficient goals.

## Adoption gates

A production host decision requires all of the following:

1. Native SDK is exact-pinned and the required upstream surface is stable
   enough to maintain.
2. Effect Native v39-or-later catalog compatibility is machine-checked and
   unsupported nodes fail loudly.
3. The Effect runtime remains authoritative and no product screen is duplicated
   in `.native`.
4. The renderer is tokenless and its protocol is schema-decoded, bounded,
   sender-authenticated, backpressured, and fuzzed.
5. Unicode/font/IME and assistive-technology acceptance passes on target OSes.
6. The real workroom screen meets measured latency, memory, and stability
   budgets.
7. Every current Desktop host capability has parity, an accepted replacement,
   or an explicit product deletion decision.
8. Signed/notarized install, update, rollback, and release acceptance are
   independent of a developer workstation.
9. The migration can land incrementally without stopping the current Electron
   release lane.
10. A named product benefit exceeds the ongoing cost of a Zig renderer, Node
    sidecar, native host patches, and one more cross-platform acceptance matrix.

## What to borrow even if we never adopt it

Native SDK is useful even under a final “stay on Electron” decision. The best
ideas to carry into Effect Native are:

- provenance from live nodes to authored source spans and iteration keys;
- deterministic cross-host screenshot rendering for catalog fixtures;
- file/ack-based automation that fails loudly when the app is frozen;
- render-stage p50/p90/max profiling in the standard snapshot;
- compile-time and runtime accessibility audits as authoring errors;
- explicit renderer capability handshakes and unsupported-operation errors;
- native-only/WebView-layer binary audits;
- one `NullPlatform` contract for windows, services, input, and lifecycle;
- record/replay with state fingerprints and platform identity;
- minimal-diff guarded source write-back for agent-authored UI;
- fixed resource budgets and visible headroom counters instead of silent
  unbounded growth.

These fit Effect Native's agent-safe, serializable, typed-data premise and can
improve today's DOM/RN/Electron product without waiting for a new renderer.

## Final recommendation

Keep OpenAgents Desktop on Electron + Effect Native through the current release
and capability program. Continue upstreaming the hardened Electron platform
adapter and use the current boundary oracles as the shipping contract.

Record Native SDK as a serious candidate for a future native/canvas renderer,
not as a new application framework for OpenAgents. Preserve the completed
hybrid spike as proof that Native SDK native components and the real Effect
Native program can compose and that Effect can remain authoritative across a
small native session interaction. Do not spend the next experiment repeating
that WebView result or expanding the polling workaround. If capacity and owner
priority permit after the release lane is protected, authorize one disposable
`@effect-native/render-native-sdk` **native-lowering** spike. Run Effect in
Node, use Native SDK for renderer/platform work, generate a versioned bounded
bridge, and stop after a small catalog and one product-shaped read-only screen
until the measurements justify more.

The strategic answer is therefore:

- **Native SDK itself:** promising, unusually ambitious, and worth tracking;
- **OpenAgents host replacement today:** no;
- **Effect Native renderer target:** yes, technically viable through an
  out-of-process renderer bridge;
- **Native SDK component reuse:** yes, behind Effect Native lowerings,
  composites, conformance tests, and typed `Host` boundaries—not as a second
  public component API;
- **compile Effect into Native SDK TypeScript:** no;
- **rewrite product screens in `.native`:** no;
- **best immediate return:** borrow its deterministic automation,
  accessibility, provenance, capability, and headless-renderer patterns.
