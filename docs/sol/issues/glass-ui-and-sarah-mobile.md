# GL: native glass UI standard library and Sarah in OpenAgents mobile

Live issues: #8646 epic, #8647 GL-1, #8648 GL-2, #8649 GL-3, #8650 GL-4.

## Outcome

OpenAgents mobile can consume Sarah immediately through the same text-first
conversation and typed-intent grammar as the retained web surface, presented
inside a native glass application shell. Application code remains Effect
Native code: it does not import `@expo/ui` or grow an iOS-only state model.

## Binding architecture

- `@expo/ui` is an MIT-licensed lowering target inside `render-rn`, never an
  application-facing component API.
- The Effect Native catalog owns semantic glass components: glass surfaces and
  pill buttons, circular icon buttons, detented sheets, and floating toolbars.
- `surface: "glass"` means Liquid Glass on iOS 26+, an honest material fallback
  below iOS 26, and the native Compose material equivalent on Android.
- New catalog demand is registered upstream first, then the reviewed package is
  re-vendored. Local app-only component forks are not the destination.
- As effect-native#70 and the owned SwiftUI renderer mature, each `@expo/ui`
  lowering is replaced and deleted without changing the application contract.

## Ordered lanes

### GL-1 — #8647: typed glass catalog and `render-rn` lowering

First land effect-native#70's Scope-bound `render-rn` host-driver registry and
convert the D-MB-02 app-local Liquid Glass island into a Schema-decoded typed
Host kind, deleting `loadLiquidGlassView` shell wiring. Then add the P0 glass
catalog components and lower them through `@expo/ui` inside `render-rn`. Prove
the view program and every platform/variant lowering, including material
fallback. No `@expo/ui` import is permitted in app code.

### GL-2 — #8648: mobile glass shell

Compose the owner's target structure in `apps/openagents-mobile`: a left
flyout drawer, selected navigation row, Recents, floating settings, glass Chat
pill and circular controls, layered content, and a floating composer with mic
affordance. Preserve identity, typed-intent, icon, and OTA oracles. Simulator
pixel proof is required before the next TestFlight upload.

### GL-3 — #8649: Sarah conversation in mobile

Deliver the text availability floor first: authenticated/prospect session,
bounded SSE transcript, composer turns, and typed Sarah cards over the same
server contracts as `/sarah`. Operator posture remains server-authorized;
linking an account must not let UI tone manufacture authority. Voice/avatar
tiers remain optional #8610 enhancements and cannot block text.

GL-3 may establish and test this shared Sarah seam in parallel with GL-1/GL-2,
then compose it into the glass shell. It must not create another transcript,
FleetRun, Blueprint, intent, cursor, or receipt truth.

### GL-4 — #8650: convert and delete the temporary lowering

Replace `@expo/ui` component-by-component with owned SwiftUI/Compose lowerings
as their exact behavior is proven. Exit is zero `@expo/ui` imports, no catalog
contract change for app code, and reconciled vendored divergence.

## Program gates

- GL is P1 parallel. It cannot take the #8639 steering, authority, projection,
  or receipt hot contracts or delay #8640 Phase A.
- #8597 has an unreleased Fable claim scoped to initial greenfield setup, while
  later mobile work exceeded that published scope. A GL implementer must get an
  explicit re-scope/release or perform the complete claim-protocol audit, then
  claim the relevant GL issue and coordinate the same mobile hot paths before
  mutation. Elapsed time alone does not transfer ownership.
- Each lane reports code-landed, fixture-proven, deployed, live-proven,
  owner-accepted, and closed separately.
- Visual changes require deterministic simulator screenshots and owner device
  acceptance; a successful build or TestFlight state is not visual acceptance.
- Android behavior is real native behavior with an honest material fallback,
  not an iOS screenshot or a platform-name substitution.
