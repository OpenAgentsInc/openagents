# Effect Native SwiftUI Renderer Audit

> **Honest-scope header.** This audit answers whether OpenAgents can build an
> owned analogue of Expo UI's SwiftUI bridge inside Effect Native, and how that
> should interact with non-SwiftUI/native host views. It is architecture
> guidance, not an implementation commit. It preserves the current Effect Native
> rule: React Native is the first mobile renderer; true SwiftUI/Compose
> renderers are fidelity upgrades when a real screen demands them.

- **Date:** 2026-07-09
- **OpenAgents dossier read:** `openagents/docs/effect-native/`
- **Effect Native source read:** `/Users/christopherdavid/work/effect-native`
- **Related audit:** `docs/mobile/2026-07-09-expo-ui-swift-ui-audit.md`
- **Local source caveat:** the inspected `effect-native` checkout is on
  `e32b97e0f95b99a6f0547ce74d71056225ead10e`, behind its `origin/main`, with
  local modifications in `packages/core/src/index.ts` and
  `packages/core/test/style.test.ts`. I treated it as read-only source
  evidence and did not change it.

## Answer

Yes, we can build our own version of the Expo SwiftUI idea, but it should not
copy Expo's public shape.

Expo exposes SwiftUI as React components:

```tsx
<Host>
  <Button label="Save" />
</Host>
```

Effect Native should expose SwiftUI as a renderer for the owned typed catalog:

```ts
Stack({ direction: "column" }, [
  Button({ label: "Save", onPress: IntentRef("Save") })
])
```

That difference is the whole architecture. Expo's source of truth is a JSX tree
that calls native views. Effect Native's source of truth is a Schema-validated
data tree plus typed intents, with renderers below it. A SwiftUI renderer should
consume the same `View` tree that the DOM and React Native renderers consume,
then lower each catalog node to real SwiftUI.

The owned version is therefore:

- `@effect-native/core` keeps the catalog, state, intents, style tokens, and
  renderer conformance rules.
- `@effect-native/render-swiftui` becomes a new renderer package or generated
  native module family that lowers that catalog to SwiftUI.
- A small iOS host embeds the SwiftUI root in UIKit when needed, similar to
  Expo's `Host`, but only as the platform mounting boundary.
- Non-SwiftUI views enter through an explicit typed `Host`/foreign-view node,
  not arbitrary children or callbacks.

## What Effect Native already has

The OpenAgents dossier defines Effect Native as "a framework for building
native applications using Effect." The UI layer is only one part of the app:
Effect owns services, state, logic, data, errors, resources, and UI.

The local `effect-native` repo now has concrete scaffolding beyond the original
decision docs:

- `@effect-native/core`
  - closed catalog versioning (`effect-native/v5` in the inspected checkout);
  - component tags: `Stack`, `Text`, `Button`, `Image`, `TextField`, `List`,
    `SectionList`, `Card`, `Spacer`, `Link`, `Modal`, `Sheet`;
  - Schema-backed view data and compatibility decoders;
  - typed `IntentRef` values and an `IntentRegistry`;
  - form state, validation, redaction, and built-in form intents;
  - responsive values and viewport services;
  - typed style objects with platform, breakpoint, and state variants;
  - a renderer adapter interface and conformance tests.
- `@effect-native/render-dom`
  - direct DOM renderer, no React;
  - atomic CSS-style emission from typed tokens;
  - overlay focus handling and virtualized collection support.
- `@effect-native/render-rn`
  - React Native renderer that maps catalog nodes to RN host components;
  - React/RN declared only as peer dependencies;
  - `EffectNativeSurface` as the only React-facing shell;
  - viewport/dimension handling through RN `Dimensions`.
- `@effect-native/platform-desktop`
  - a desktop host adapter that mounts the DOM renderer and models bridge,
    menu, window, deep link, and single-instance services as Effect Layers.

There are also receipts:

- `docs/proof.md`: one signup/activity screen, defined once, rendered through
  web and mobile hosts.
- `scripts/proof-oracle.test.ts`: the same state, intent log, and structural
  snapshots across headless, DOM, and RN.
- `scripts/renderer-conformance.test.ts`: catalog coverage across renderers.
- `scripts/react-dependency-boundary.test.ts`: only `render-rn` may depend on
  React/RN, and only as peers.

That is already the foundation a SwiftUI renderer would need. The missing part
is not "can we represent SwiftUI controls?" It is "how do we lower this catalog
to SwiftUI without weakening the catalog contract?"

## How Expo's bridge maps to ours

Expo UI has useful implementation lessons:

| Expo UI | Effect Native equivalent |
| --- | --- |
| `@expo/ui/swift-ui` TypeScript wrappers | `@effect-native/core` typed constructors and Schema data |
| JSX children | `View.children` arrays in the typed tree |
| `Host` | SwiftUI renderer root/mount boundary |
| `requireNativeView('ExpoUI', 'Button')` | generated Swift renderer case for `ButtonView` |
| Swift `@Field` props | Schema-decoded catalog props |
| `EventDispatcher` callbacks | typed `IntentRef` dispatched through `IntentRegistry` |
| modifier records with `$type` | typed style values and catalog-specific props |
| `UIViewHost` for React Native children | typed `Host`/foreign-view node with scoped lifecycle |
| `ObservableState` shared object | Effect `SubscriptionRef` / `Atom` / view stream |

Expo's clever bit is its `Host`: React Native lays out one UIKit view, and
inside it a `UIHostingController` renders a SwiftUI subtree. Child SwiftUI
components are virtual nodes rather than one UIKit wrapper per leaf.

Effect Native can reuse that mounting idea while rejecting the JSX-as-contract
part. The Swift renderer root can be a `UIHostingController` that subscribes to
an Effect Native view stream and re-renders a SwiftUI tree derived from typed
catalog nodes.

## Proposed architecture

### 1. Keep the catalog above native code

The `View` union in `@effect-native/core` remains the only public UI contract.
A SwiftUI renderer should not invent a parallel component set such as
`SwiftButton`, `SwiftForm`, or `SwiftTextField`. It should implement the current
catalog tags, then grow only through the catalog growth rule: real screen
demand, typed props/intents/style contract, renderer implementations, version
bump, and conformance tests.

### 2. Add a SwiftUI renderer package

Introduce a renderer lane conceptually named:

```txt
@effect-native/render-swiftui
```

It can start as a native package plus TypeScript glue, but its contract should
look like the existing renderers:

```ts
makeSwiftUIRenderer(options).mount(container, viewStream, report)
```

The implementation can be one of two shapes:

- **Generated native renderer:** generate Swift enums/structs from the catalog
  schema, decode JSON view trees natively, and render with SwiftUI. This is the
  best long-term shape because the Swift side can be statically exhaustive over
  catalog tags.
- **JS-driven bridge renderer:** keep the view tree in JS, send resolved nodes
  over a JSI/native module boundary, and have Swift render them. This is easier
  to bootstrap but weaker: more runtime serialization and fewer native
  compile-time guarantees.

The generated path is the right target. It mirrors React Native Codegen's
spirit without adopting RN's component API: generate native bindings from our
catalog, not from arbitrary JSX props.

### 3. Mount with a SwiftUI root host

On iOS, the renderer root should be a small UIKit-owned host:

- owns a `UIHostingController`;
- subscribes to the Effect Native `viewStream`;
- decodes/resolves the latest `View` tree;
- passes a stable intent reporter into native event closures;
- updates the SwiftUI root view on the main actor;
- closes the Effect `Scope` on unmount.

This is where Expo's `Host` lesson applies. The host is necessary because most
iOS app shells are UIKit/React Native/Electron-style containers somewhere at
the boundary. But inside that root the renderer should build SwiftUI from
catalog data, not host React Native child views opportunistically.

### 4. Lower catalog tags to SwiftUI

Initial lowering can map the existing catalog directly:

| Effect Native tag | SwiftUI lowering |
| --- | --- |
| `Stack` | `HStack` / `VStack` based on `direction`, with spacing/alignment |
| `Text` | `SwiftUI.Text`, applying tokenized type scale, color, weight |
| `Button` | `SwiftUI.Button`, dispatching `onPress` intent |
| `Image` | async/remote image loader or platform image service, with fit/dimensions |
| `TextField` | `TextField` / `SecureField` / `TextEditor` depending on props |
| `List` | `List`, `ScrollView` + `LazyVStack`, or custom virtualized wrapper |
| `SectionList` | `List` with `Section` where behavior matches; otherwise custom |
| `Card` | container view with padding, radius, border/background tokens |
| `Spacer` | `SwiftUI.Spacer` or fixed frame spacer |
| `Link` | `Link` for URL; intent dispatch for route/anchor navigation |
| `Modal` | `.sheet`, `.fullScreenCover`, or overlay host depending semantics |
| `Sheet` | `.sheet` / bottom presentation where platform supports it |

For each tag, the renderer must define where SwiftUI semantics match the
catalog and where they do not. Lists, text input, modal/sheet presentation,
keyboard focus, and accessibility are the high-risk areas.

### 5. Resolve styles before rendering

The SwiftUI renderer should consume the same resolved style model as DOM/RN:

1. Runtime resolves platform, breakpoint, and state variants.
2. Renderer lowers the final flat style object to SwiftUI modifiers.
3. Tokens are resolved through the current theme.

Example lowering:

- `backgroundColor: "surface"` -> `.background(Color(...))`
- `borderRadius: "md"` -> `.clipShape(RoundedRectangle(cornerRadius: ...))`
- `borderColor` + `borderWidth` -> `.overlay(RoundedRectangle(...).stroke(...))`
- `padding: "3"` -> `.padding(theme.spacing["3"])`
- `width` / `height` -> `.frame(width:..., height:...)`
- `opacity` -> `.opacity(...)`
- `typeScale` -> `.font(...)`

Do not copy Expo's open-ended modifier registry as the public contract. It is
powerful, but it would smuggle a second style/component language into Effect
Native. SwiftUI modifiers should be renderer implementation details unless a
modifier becomes a catalog-level typed style or prop with DOM/RN equivalents.

## Non-SwiftUI and native host views

This is the hardest design question, and it is where we should be stricter
than Expo.

Expo allows React Native children inside SwiftUI by wrapping UIKit views as
`UIViewRepresentable`-style children. That is valuable interop, but it is also
an escape hatch: arbitrary host views can enter the SwiftUI tree.

Effect Native should allow foreign/native views only through an explicit typed
node, already foreshadowed by `effect-native/GAPS.md` as the foreign `Host`
node for Monaco/editor/terminal surfaces.

### Proposed `Host` contract

Add a catalog node only when demanded:

```ts
Host({
  kind: "native.map" | "native.camera" | "rn.legacy" | "ios.uikit" | "...",
  props: HostPropsByKind[kind],
  lifecycle: HostLifecyclePolicy,
  onEvent: IntentRef(...)
})
```

Rules:

- Every `kind` is registered in a typed host registry.
- Props are Schema-decoded per kind; no arbitrary prop bag.
- Events become typed intents; no callback closures.
- Lifecycle is `Scope`-owned: mount, update, unmount, cleanup.
- Renderers may support different host kinds, but unsupported kinds fail typed
  conformance instead of silently rendering nothing.
- Host nodes are reviewed and recorded in `GAPS.md` before use.
- Host nodes are leaves or bounded containers only if the driver explicitly
  supports children.

This is the native equivalent of Foldkit Ports: a controlled embed boundary,
not a breach in the component system.

### SwiftUI interop cases

There are three distinct non-SwiftUI cases:

1. **UIKit/AppKit views inside SwiftUI**
   - Implement with `UIViewRepresentable` / `NSViewRepresentable`.
   - Use for map, camera, webview, terminal/editor, or legacy native controls.
   - The driver owns layout limits and cleanup.

2. **SwiftUI inside UIKit/RN**
   - Implement with `UIHostingController` at the renderer root, or at a
     per-component boundary when embedding a Swift-rendered island in an RN app.
   - Useful for incremental migration: one Effect Native SwiftUI surface can sit
     inside a broader React Native screen.

3. **React Native legacy views inside Effect Native**
   - Prefer the RN renderer for the whole surface.
   - If a SwiftUI-rendered screen must include a legacy RN island, treat it as a
     `Host(kind: "rn.legacy", ...)` with strict props/events and clear
     lifecycle. Do not allow arbitrary JSX children to leak into the SwiftUI
     renderer.

The rule of thumb: use SwiftUI for catalog-native controls, use `Host` for
foreign things that are genuinely not catalog components, and keep every bridge
typed and scoped.

## How this differs from Expo UI

The owned renderer should be narrower and more governable than Expo UI:

- **No `@expo/ui/swift-ui`-style component mirror as public API.**
  Authors use the Effect Native catalog, not a SwiftUI-named JSX kit.
- **No open-ended SwiftUI modifier array as public API.**
  Styles and behavior enter through typed catalog props, typed style values,
  and typed intents.
- **No arbitrary child interop.**
  Non-SwiftUI/native views go through `Host` registry entries.
- **Generated native code is preferred.**
  The catalog should generate Swift exhaustiveness and decoder code instead of
  hand-maintained prop mirrors where possible.
- **Conformance gates every renderer.**
  A new component is not "in" until headless, DOM, RN, and SwiftUI either
  implement it or the catalog version explicitly marks renderer support limits.

Expo is a useful proof that SwiftUI can be driven from TypeScript/RN, but its
goal is broad Expo developer convenience. Our goal is a small, closed,
agent-safe component contract.

## Implementation path

### EN-S0: Define the native renderer contract

- Add an RFC in `effect-native` for `render-swiftui`.
- Define the native renderer adapter shape and how it receives:
  - view stream,
  - theme,
  - viewport,
  - intent reporter,
  - host registry,
  - lifecycle scope.
- Decide generated-native vs JS-driven bridge for the first spike.

### EN-S1: Generate Swift catalog types

- Generate Swift enums/structs from the `View` schema:
  - `ViewNode`,
  - per-tag props,
  - style structs,
  - token enums,
  - intent refs.
- Include compatibility version markers.
- Make unknown tags/props decode failures.

### EN-S2: Render the v0 static catalog

Render `Stack`, `Text`, `Button`, `Card`, `Spacer`, and basic `Image` first.
This proves tree recursion, style lowering, token resolution, and intent
dispatch with low platform risk.

### EN-S3: Add text input and forms

Implement `TextField`, `SecureField`, form field change/blur/submit dispatch,
focus handling, and redaction-compatible event logging. This is the first
serious native behavior test.

### EN-S4: Lists and overlays

Implement `List`, `SectionList`, `Modal`, and `Sheet`. These should not be
treated as trivial mappings because SwiftUI presentation and list identity
have different behavior than DOM/RN.

### EN-S5: Typed foreign `Host`

Add one real demanded host kind, probably terminal/editor/camera/map depending
on product pull. Build the registry, scope cleanup, event mapping, and
renderer conformance rule before adding any second kind.

### EN-S6: Conformance and screenshots

Extend `scripts/renderer-conformance.test.ts` with SwiftUI structural snapshots
and add simulator screenshot receipts for the shared proof screen. The exit
receipt should be the same view, state transition, and intent log across
headless, DOM, RN, and SwiftUI.

## Risks

- **SwiftUI layout is not Yoga.** `Stack` can map cleanly for simple cases, but
  exact parity across DOM/RN/SwiftUI will require a constrained layout contract.
  Do not promise arbitrary SwiftUI layout through the common catalog.
- **Text input/focus is always sharp.** Selection, keyboard, submit behavior,
  secure entry, multiline editing, and focus restoration need explicit tests.
- **Lists diverge by platform.** SwiftUI `List` has platform styling and
  identity behavior that may not match DOM/RN virtualized lists. We may need a
  custom `ScrollView` + lazy stack for catalog parity.
- **Open-ended native hosts can rot the contract.** The `Host` node must stay
  reviewed, typed, and rare.
- **Generated code can drift.** Swift generated types must be checked in or
  generated deterministically in CI/local checks, and catalog version bumps must
  fail if Swift output is stale.
- **Effect runtime boundary is real work.** A pure Swift renderer still needs a
  host process story for subscribing to Effect view streams and dispatching
  intents. If the app remains JS/RN-hosted, the bridge is easier; if we want a
  pure Swift app, `platform-native` must provide native service layers and a
  run-main equivalent.

## Recommendation

Build the owned SwiftUI version, but only after the RN renderer remains the
shipping mobile path and a real product screen demands native fidelity.

The first implementation should be a bounded SwiftUI renderer for the existing
catalog, not a SwiftUI component API. It should use Expo's `UIHostingController`
and virtual-child lessons, React Native Codegen's generated-binding lesson,
Foldkit's Ports lesson for foreign views, and Effect Native's current
conformance suite as the guardrail.

The minimum viable proof is:

1. same `signupActivityView` data tree,
2. same typed form intents,
3. same intent log and final state,
4. SwiftUI simulator screenshot,
5. no React/RN imports outside the RN renderer and host shell,
6. no untyped foreign view escape.

If we hold that line, SwiftUI becomes just another renderer under Effect
Native's typed contract. If we copy Expo's public JSX/modifier surface, we
recreate the framework coupling Effect Native exists to avoid.
