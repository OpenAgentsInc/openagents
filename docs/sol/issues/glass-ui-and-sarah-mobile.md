# GL: native glass UI standard library and Sarah in OpenAgents mobile

Live issues: #8646 epic, #8647 GL-1, #8648 GL-2, #8649 GL-3, #8650 GL-4.

> **Revision 25 disposition:** #8647–#8649 are closed historical receipts.
> #8646 and presentation/Sarah integration are paused. #8650 may move only for
> an exact correctness, accessibility, platform, or R0–R7 blocker. Do not use
> this document as the mobile product queue; current mobile remote coding lives
> in [`app-mobile.md`](./app-mobile.md) and the
> [`mobile port plan`](../2026-07-10-khala-code-mvp-to-openagents-mobile-port-plan.md).

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

Status: `66d2f7544b` vendors upstream `2918c277`/v27 with typed `IconButton`,
`Toolbar`, semantic `surface: "glass"`, and Sheet detents. That is a catalog
substrate receipt, not the GL-1 exit.

Next, land effect-native#70's Scope-bound `render-rn` host-driver registry and
convert the D-MB-02 app-local Liquid Glass island into a Schema-decoded typed
Host kind, deleting `loadLiquidGlassView` shell wiring. Then add the P0 glass
catalog components and lower them through `@expo/ui` inside `render-rn`. Prove
the view program and every platform/variant lowering, including material
fallback. No `@expo/ui` import is permitted in app code.

### GL-2 — #8648: mobile glass shell

Status: **owner-accepted and closed** on 2026-07-10 after build 111 proved the
typed shell, drawer, composer, under-chrome reply video, and demo Minerals sheet
through simulator, TestFlight, and live-use rungs. Build 112 at `adcf0cca5d`
then fixed the sheet so only explicit user actions dismiss it; that P0 behavior
repair did not reopen GL-2. Demo prices remain presentation-only with no
StoreKit purchase. D-MB-02 catalog conversion and app-island deletion remain
GL-1/GL-4 work.

Complete the owner's target structure in `apps/openagents-mobile`: a left
flyout drawer, selected navigation row, Recents, floating settings, glass Chat
pill and circular controls, layered content, and a floating composer with mic
affordance. Preserve identity, typed-intent, icon, and OTA oracles. Simulator
pixel proof is required before the next TestFlight upload.

### GL-3 — #8649: Sarah conversation in mobile

Status: **closed** at `6647d998ad` / TestFlight build 113. The typed mobile
surface mints and persists the production prospect session, sends composer
turns through the same `/sarah` contracts as web, renders production replies,
survives app restart, and models bounded offline/reconnect states under the
enforced `openagents_mobile.sarah_text_surface.v1` contract. The demo video
remains ambient presentation only.

The current text path renders the POST response while SSE carries typed
liveness/cards, matching the present web behavior. Pure transcript-stream
unification, voice/avatar tiers, authenticated operator posture, and Android
proof remain follow-on work in their owning lanes rather than GL-3 closure
residue. No parallel FleetRun, Blueprint, intent, cursor, or receipt truth was
created.

### GL-4 — #8650: convert and delete the temporary lowering

Replace `@expo/ui` component-by-component with owned SwiftUI/Compose lowerings
as their exact behavior is proven. Exit is zero `@expo/ui` imports, no catalog
contract change for app code, and reconciled vendored divergence.

## Program gates

- GL is paused except for the exact #8650 blocker boundary above. It cannot
  preempt Sync, Fleet, remote-workroom, mobile coding, or release work.
- Every new mobile/renderer leaf must claim its exact scope and coordinate any
  active #8597/#8647/#8650 hot paths before mutation. Elapsed time alone does
  not transfer ownership.
- Each lane reports code-landed, fixture-proven, deployed, live-proven,
  owner-accepted, and closed separately.
- Visual changes require deterministic simulator screenshots and owner device
  acceptance; a successful build or TestFlight state is not visual acceptance.
- Android behavior is real native behavior with an honest material fallback,
  not an iOS screenshot or a platform-name substitution.
