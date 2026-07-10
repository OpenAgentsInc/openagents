# GL-1 (#8647) — glass chrome through the real render-rn seam (island deleted)

- **Date:** 2026-07-10
- **Lane:** GL-1 residue (#8647): Scope-bound render-rn host driver, real
  internal `@expo/ui` lowering, D-MB-02 island convert/delete.
- **Upstream (effect-native, all on `main`):** `83f1bde` host-driver registry
  (#70 ask 2), `68893b3` v30 glass-chrome icons + internal `@expo/ui` Liquid
  Glass lowering, `64f211f` sizing/menu-theming fixes, `1abef88` + `f825137` flexed-button
  label frame/contentShape hit-testing (the frame must live INSIDE the
  SwiftUI Button label). Full upstream check green (typecheck, 380+
  tests, doc snippets, catalog reference v30).
- **Vendored at:** `f825137` (hunk-level onto the documented monorepo
  divergence — see `apps/openagents.com/packages/effect-native-render-rn/VENDORING.md`).

## What changed

1. **Scope-bound host driver seam (residue 1).** `render-rn` now exposes
   `ReactNativeHostDriver` + `makeReactNativeHostRuntime` — the typed `Host`
   mounting seam mirroring `render-dom`'s MediaVideo driver contract:
   Schema-decoded props fail closed to loud `en-host-error` markers, driver
   events dispatch the Host node's `onEvent` as typed intents (bound to the
   latest emission), and instance lifecycle is owned by the surface Scope
   (mount / render-per-emission / sweep-on-removal / dispose-on-close), with
   the same lifecycle bound to the React component instance on the
   `EffectNativeSurface` entrypoint.
2. **Real internal `@expo/ui` lowering (residue 2).** `surface: "glass"` on
   `IconButton` / `Button` / `Toolbar` / all-lowerable `Stack` containers
   lowers INSIDE render-rn through `@expo/ui/swift-ui` on iOS 26+: SF Symbols,
   `glassEffect` circle/capsule/roundedRectangle, one SwiftUI subtree per
   glass container, typed intents preserved. Everywhere else (Android,
   iOS < 26 where `@expo/ui` glass no-ops dishonestly, missing runtime, tests)
   the documented RN material approximation stays. `@expo/ui` is an optional
   render-rn peer; the app declares it only as the native-module installation
   vehicle and **never imports it** (oracle in
   `tests/component-sharing.test.ts`).
3. **D-MB-02 island deleted (residue 3).** `modules/openagents-liquid-glass`
   (Swift ExpoView islands + `requireNativeViewManager` bindings) is gone.
   The chrome is typed catalog data in `home-core.ts`: glass menu IconButton,
   glass mode pill Button, glass compose IconButton, the composer bar as ONE
   glass Toolbar (plus / flexed muted "Ask anything" / mic), the pill's mode
   menu as a typed `DropdownMenu`, and the minerals sheet as ONE glass Stack
   panel. Demand register row D-MB-02 closed.

## Simulator pixel proofs (iPhone 17 Pro, iOS 26.5, Release; idb HID taps —
no host cursor; committed under receipts/)

1. `2026-07-10-gl1-glass-chrome-home.png` — the full chrome rendered through
   the NEW seam: SwiftUI Liquid Glass circles (SF Symbols
   `line.3.horizontal` / `square.and.pencil`), glass "OpenAgents" pill, and
   the one-capsule composer with muted "Ask anything".
2. `2026-07-10-gl1-drawer-via-glass-menu.png` — **typed intent round-trip**:
   idb tap on the SwiftUI glass menu button dispatched `DrawerToggled`
   through the program registry; the EN drawer projection opened (bundle tag
   footer visible).
3. `2026-07-10-gl1-typed-mode-menu.png` — pill tap dispatched
   `ChatPillPressed` → the typed EN `DropdownMenu` opened (themed rows,
   checkmark on the active mode) — the SwiftUI Menu that lived inside the
   deleted island, as catalog data.
4. `2026-07-10-gl1-sarah-mode.png` — menu row tap dispatched
   `SurfaceModeMenuItemSelected("sarah")`: pill label flipped to "Sarah",
   demo video under the glass chrome, live restored Sarah session.
5. `2026-07-10-gl1-minerals-sheet.png` — composer tap dispatched
   `ComposerPressed` (ask-video takeover), and the minerals sheet rendered as
   ONE SwiftUI glass VStack (title + four typed pack Buttons + ghost
   "Not now") through the container lowering.
6. `2026-07-10-gl1-after-pack-select.png` — pack row tap dispatched
   `MineralPackSelected` → sheet closed, surface resumed (sheet lifecycle
   stayed user-owned across the video boundary).

## On-simulator bugs found by the proofs (fixed upstream + re-vendored)

1. **Zero-height glass containers hit-test nothing** — a glass Toolbar
   without an app height *renders* (glassEffect overdraw) but the RN wrapper
   collapses to zero height, so taps fall through. Containers without an
   explicit height now host via `matchContents`; the chrome composer passes
   `height: 54`.
2. **SwiftUI buttons hug their labels** — the middle of the composer was a
   dead zone the island's full-capsule dispatcher never had. `style.flex` on
   a glass leaf Button now lowers to `frame(maxWidth)` + `contentShape` ON
   THE LABEL content (a frame on the Button itself does not extend its
   tappable area — verified on-simulator).
3. **Ghost label color** — the composer placeholder rendered accent-blue;
   glass Buttons now respect the typed `style.color` token (`textMuted`).
4. **DropdownMenu rows were unthemed** (#71-class: RN Text does not inherit
   color — default-black rows on the dark panel); menu rows now theme
   glyph/label/keybinding and dim when disabled.

## Fingerprint / release note

- iOS runtime fingerprint changes from `b0211cc7bdb65d42495ad2e0639db5eb16da721f`
  (builds 112/113) to `ac264017d5a1ec983d9ae021ebd3ff76c3802c75`: `@expo/ui`
  native module added, `openagents-liquid-glass` local expo-module removed.
- **No TestFlight upload in this lane** (owner-coordinated native-build
  policy): the native change accumulates for the next coordinated build.
  Until that build ships, the OTA channel must NOT publish bundles from this
  tree onto the 112/113 embedded runtime (the fingerprint gate would refuse;
  do not force-seed).

## Residue (GL-4 #8650 and later)

- Owned SwiftUI lowerings replace `@expo/ui` component-by-component
  (convert-and-delete), per the hybrid decision.
- Pill sparkles glyph + trailing chevron affordance (typed Button icon slot —
  catalog demand, goes upstream first).
- DropdownMenu anchored placement on RN (currently the honest centered-modal
  lowering).
- Sheet detent presentation for the minerals sheet (the shell still owns the
  fly-up animation; `Sheet.presentationDetents` lowering is available but the
  shell keeps its layering).
