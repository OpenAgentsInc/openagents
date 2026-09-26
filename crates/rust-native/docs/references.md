# Reference architecture review

Use a small, shared Rust semantic UI contract with platform-owned native
controls. Share state transitions, view meaning, and typed events; let each
adapter implement native interaction and presentation. This is an original
design adaptation, not a port of either reference implementation.

This review inspected local source on September 26, 2026. It ran no builds,
tests, models, benchmarks, simulators, or devices. Source tests and retained
reference receipts describe their own coverage; they are not new OpenAgents
verification.

## Inspected revisions

| Reference | Exact revision | Inspection boundary |
| --- | --- | --- |
| [Effect Native][effect-root] | `f53838edd8d38797fb5c8a8fb6768557dc826d64` | Core schema/runtime sections, DOM and React Native adapter sections, mobile services, proof oracle, snapshot/replay helpers, and host-extension contract. |
| [React Native][rn-root] | `d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66` | Fabric tree, component, mounting, event, text-input, and style contracts; iOS and Android mounting implementations. This is not a review of the whole framework. |

Both local checkouts were clean when inspected. Their repository instructions
were not adopted. Both roots contain an MIT license; this work copies no
implementation, prompts, or product assets. Adding dependencies or copied
material later requires its own provenance and license review.

## Effect Native: preserve the separation

| Inspected files and symbols | Useful pattern | Adaptation boundary |
| --- | --- | --- |
| [`packages/core/src/index.ts`][effect-core]: `CatalogVersion`, `View`, `IntentRef`, intent schemas | A versioned component catalog and serializable intent references separate screens from callback code. | Start with Rust enums and validated bounded values. Do not port the large catalog, JSON field-binding language, or Effect runtime. A valid tree still needs identity, resource, and authority checks. |
| [Core][effect-core]: `ViewProgram`, `RendererAdapter`, `MountedSurface`, `makeHeadlessRenderer` | One observable application state produces views; renderers consume views and report events. Surface lifetime owns subscriptions and cleanup. | Rust owns shared application state. Platform objects and handles stay in the adapter. A surface closing must release UI resources without cancelling the durable task. |
| [`packages/render-dom/src/index.ts`][effect-dom]: `makeDomRenderer`; [`packages/render-rn/src/index.ts`][effect-rn]: `createEffectNativeSurface`, `makeReactNativeRenderer` | The same view stream can lower to different hosts. Viewport and reduced-motion changes are explicit inputs. | The actual mobile path uses React elements, React hooks, and React Native host components. It is not a direct SwiftUI renderer or evidence that Fabric can be dropped into Rust as a small library. |
| [React Native renderer][effect-rn]: `ReactNativeHostDriver`, `makeReactNativeHostRuntime`; [`docs/foreign-host.md`][effect-host] | Reviewed imperative hosts have bounded props/events, stable identity, and mount/update/dispose lifetimes. | Reserve such an extension for a demanded editor, terminal, or media surface. Reject unsupported kinds explicitly. Do not admit arbitrary executable widgets through view data. |
| [Core][effect-core]: `mergeStyles`; [`packages/tokens/src/index.ts`][effect-tokens] | Typed component styles, named tokens, and deterministic precedence avoid an implicit global cascade. | Reuse Coder's existing semantic design system. Do not transplant the reference palette, decorative vocabulary, or every platform's style properties. |
| [`packages/platform-mobile/src/index.ts`][effect-mobile] | Lifecycle, keyboard, safe areas, push tokens, and deep links are host services with typed state/events. | These interfaces and test harnesses do not establish operational mobile integration. Keyboard geometry and lifecycle observations do not authorize background execution or remote commands. |
| [`examples/signup-activity/index.ts`][effect-example]; [`scripts/proof-oracle.test.ts`][effect-proof]; [`docs/proof.md`][effect-proof-doc] | One interaction script compares resulting state, intent logs, and structure across renderers. | The oracle uses headless rendering, Happy DOM, and an in-memory React Native shim. The documented browser/iOS screenshots are separate evidence. Neither establishes physical-device accessibility or IME correctness. |
| [`packages/testkit/src/snapshot.ts`][effect-snapshot]; [`packages/testkit/src/replay.ts`][effect-replay] | Versioned deterministic snapshots and recorded intent replay make regressions reviewable. | UI replay must use inert effect fixtures. It must never redispatch Coder commands, grants, payments, or model calls. Private drafts and credentials need explicit recording exclusions. |

## React Native: learn the native boundaries

Paths below are relative to `packages/react-native/` in the pinned revision.

| Inspected files | Relevant observation | Rust implication |
| --- | --- | --- |
| [`ReactCommon/react/renderer/core/ShadowNode.h`][rn-node], [`State.h`][rn-state], [`ComponentDescriptor.h`][rn-descriptor] | Nodes separate props, children, emitter, and component state. Clones preserve family identity; state revisions and weak references avoid competing state and ownership cycles. | Separate stable semantic identity from a particular native object or view revision. Keep native resources out of the shared tree. Do not recreate the C++ class hierarchy. |
| [`ReactCommon/react/renderer/mounting/ShadowTree.h`][rn-tree], [`ShadowTree.cpp`][rn-tree-cpp], [`MountingCoordinator.h`][rn-mount] | Building/committing a revision is separate from mounting it. Mount transactions need sequential application; suspended commits are not mounted UI. | Publish validated immutable views with a revision. Track received versus applied revision. Apply a view coherently on the UI thread; do not advertise a partially applied view as current. |
| [`ReactCommon/react/renderer/core/EventEmitter.h`][rn-emitter], [`EventQueue.cpp`][rn-events] | Event delivery has target lifetime and dispatch ownership. Coalescing preserves ordering between different event types for the same target. | Bind an event to its surface, node, generation, and relevant revision. Reject detached/stale targets. Coalesce only explicitly replaceable observations; never merge submit, approve, cancel, or other durable commands. |
| [`React/Fabric/Mounting/RCTComponentViewProtocol.h`][rn-component]; [`ReactAndroid/src/main/java/com/facebook/react/fabric/mounting/SurfaceMountingManager.kt`][rn-android] | Native mounting owns prop/state/event/layout updates, recycling, invalidation, and UI-thread work. Android teardown stops future mutations and releases native state and emitters. | Define attach/update/detach and idempotent disposal. Invalidate callbacks before releasing native handles. A reused list cell must not inherit the previous record's event target, selection, or private text. |
| [`React/Fabric/Mounting/ComponentViews/TextInput/RCTTextInputComponentView.mm`][rn-input] | A native event counter guards text replacement. The implementation treats selection, marked text, dictation, secure input, and native styling as distinct concerns. | Do not replace the text widget from each Rust snapshot. Specify edit acknowledgement, composition ownership, selection units, and stale-update handling. Unicode string storage alone does not solve native input. |
| [`Libraries/StyleSheet/StyleSheetExports.js`][rn-styles]; [`ReactCommon/react/renderer/components/view/YogaStylableProps.h`][rn-yoga], [`BaseViewProps.h`][rn-view] | Style composition, layout properties, accessibility, and native view props have separate contracts. Yoga includes precedence and logical-edge rules that go beyond a stack layout. | Start with a small semantic layout vocabulary and platform mapping. Do not promise CSS/Yoga pixel parity or import a full layout engine before a specific screen requires it. |

## Ownership rules for Rust Native

These are recommendations for the new contract, not claims about an already
complete renderer.

| Owner | Owns | Must not own |
| --- | --- | --- |
| Durable Coder host and task client | Task truth, grants, command identity/disposition, retained evidence, and reconnect reconciliation. | Widget identity or dependence on a visible screen staying alive. |
| Shared Rust application state | Selected task, validated view model, draft persistence policy, typed action preparation, and loading/stale/unavailable states. | A second task journal or implied permission from an enabled button. |
| `rust-native` semantic contract | Bounded tree, accessibility meaning, stable keys, typed events, adapter capabilities, and revision rules. | Nostr signing, execution, model calls, or product-specific authorization. |
| Platform adapter | Native control instances, UI-thread mounting, focus, scroll anchors, composition, selection, accessibility mapping, and lifecycle callbacks. | Independent business rules or silent changes to action meaning. |

Keep event delivery bounded. Define overflow and disconnect behavior rather
than using an unbounded queue by default. Snapshot updates can replace obsolete
presentation work; user commands require a retained identity and explicit
disposition in the existing task client. A stale action should refresh or
refuse, not acquire authority from an old screen.

UI disposal must remove subscriptions, observers, timers, and callbacks.
Backgrounding is distinct from disposal, process death, and cancelling a task.
On reattachment, recover task evidence through the existing client contract;
do not replay effectful UI events to reconstruct it. Full traces stay in their
existing stores while native lists render bounded pages with original record
identities and explicit missing-data states.

## SwiftUI and native controls

SwiftUI can be an adapter to a Rust-owned semantic model; it should not become
a second implementation of Coder state, networking, or policy. Its view
identity and local editing state need the same revision and callback rules as
UIKit or Android Views. The user selected this direction and the repository's
[`AGENTS.md`](../../../AGENTS.md) now scopes the planned SwiftUI exception to
native controls, mounting, and callbacks. This reference review does not prove
that the bridge has been implemented.

The existing [Rust mobile feasibility work][mobile-feasibility] provides the
nearer adoption path: Rust invokes UIKit and Android native text/list controls,
with a Rust-generated HTML observation surface. Extract common semantics
incrementally from that public implementation. The probe's simulator/emulator
observations do not qualify new adapters, and its acknowledged physical-device,
IME, accessibility, and reconnect gaps remain relevant.

Keep native controls for editing, selection, scrolling, system menus, and
accessibility. Share semantic roles and Coder appearance tokens where they
translate meaningfully. Keep font scaling, safe areas, keyboard avoidance,
high contrast, and reduced motion as platform inputs. An adapter should declare
native, adapted, and unsupported behavior rather than silently omitting it.

## Implications for build order

The [build order](build-order.md) is the implementation sequence. The reference
review supports these choices:

1. Define the small Rust contract before an application host: validated
   component values, unique keys, view revisions, event binding, and limits.
   Keep product task behavior outside the crate.
2. Adopt it in one read-only Coder observation surface first. Reuse current
   task views and retained artifact identities; do not rewrite execution or
   introduce a draft field before its native editing contract exists.
3. Add native adapters consuming the same fixtures. Compare semantic
   content, selected record, action meaning, event order, and disposal. Compare
   intended behavior rather than identical pixels or native widget hierarchies.
4. Qualify each adapter separately for composition, selection, screen readers,
   large text, long individual records, lifecycle/reconnect, and secure storage.
   Headless parity is a prerequisite, not a substitute for these checks.
5. Grow the catalog only from a concrete product need. Require typed props and
   events, explicit per-adapter support, lifetime checks, and an upgrade policy.
   Leave editors, terminals, GPU scenes, and cross-platform animation out of
   the initial adoption unless that need is demonstrated.

This sequence starts shared UI adoption immediately while preserving the
existing product and keeping platform qualification separate from source work.

[effect-root]: https://github.com/OpenAgentsInc/effect-native/tree/f53838edd8d38797fb5c8a8fb6768557dc826d64
[effect-core]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/packages/core/src/index.ts
[effect-dom]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/packages/render-dom/src/index.ts
[effect-rn]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/packages/render-rn/src/index.ts
[effect-host]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/docs/foreign-host.md
[effect-tokens]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/packages/tokens/src/index.ts
[effect-mobile]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/packages/platform-mobile/src/index.ts
[effect-example]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/examples/signup-activity/index.ts
[effect-proof]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/scripts/proof-oracle.test.ts
[effect-proof-doc]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/docs/proof.md
[effect-snapshot]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/packages/testkit/src/snapshot.ts
[effect-replay]: https://github.com/OpenAgentsInc/effect-native/blob/f53838edd8d38797fb5c8a8fb6768557dc826d64/packages/testkit/src/replay.ts
[rn-root]: https://github.com/facebook/react-native/tree/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66
[rn-node]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/core/ShadowNode.h
[rn-state]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/core/State.h
[rn-descriptor]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/core/ComponentDescriptor.h
[rn-tree]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/mounting/ShadowTree.h
[rn-tree-cpp]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/mounting/ShadowTree.cpp
[rn-mount]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/mounting/MountingCoordinator.h
[rn-emitter]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/core/EventEmitter.h
[rn-events]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/core/EventQueue.cpp
[rn-component]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/React/Fabric/Mounting/RCTComponentViewProtocol.h
[rn-android]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactAndroid/src/main/java/com/facebook/react/fabric/mounting/SurfaceMountingManager.kt
[rn-input]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/React/Fabric/Mounting/ComponentViews/TextInput/RCTTextInputComponentView.mm
[rn-styles]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/Libraries/StyleSheet/StyleSheetExports.js
[rn-yoga]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/components/view/YogaStylableProps.h
[rn-view]: https://github.com/facebook/react-native/blob/d7ff82ebe4ed5155b955fd60b31e02bbc76a3c66/packages/react-native/ReactCommon/react/renderer/components/view/BaseViewProps.h
[mobile-feasibility]: ../../../docs/coder/design/rust-mobile-feasibility.md
