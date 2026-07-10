# OpenAgents mobile — v0.4.3 build 106: invisible-island + black-button P0 fix, simulator pixel proof (2026-07-09)

Owner escalation on build 105: the SwiftUI Liquid Glass island was invisible
and the Effect Native "Dispatch a typed intent" button rendered black-on-black.
Both bugs were **reproduced on the iOS simulator first** (per the new upload
gate), root-caused, fixed, and pixel-proven before this build.

## Root causes (simulator-reproduced, not guessed)

Reproduce evidence: `docs/mobile/receipts/2026-07-09-build105-broken-simulator.png`
(Release build of the 105 code on iPhone 17 Pro sim — identical to the owner's
device screenshot: ghost button label, no island).

1. **Invisible island — shell layout bug.** The EN Home root Stack is
   `height:"full"` (100% of its parent). The shell mounted `EffectNativeHost`
   directly under the `SafeAreaView`, so "full" meant the ENTIRE safe area:
   the EN surface consumed the whole screen and the island (and even its
   fallback notice) were laid out below the fold. The island was mounted and
   healthy — just off-screen.
   **Fix:** wrap the EN host in a `flex: 1` view so "full" means "the space
   above the island" (`src/screens/home-screen.tsx`).
2. **Black-on-black button — vendored renderer bug.** `render-rn`'s
   `renderButton` applied NO background and NO label color: RN `Text` does not
   inherit color, so the label fell back to default black on the `#05070d`
   theme, and `variant: "primary"` was ignored entirely on RN.
   **Fix (upstream, then re-vendored):** Button variant theme lowering in
   `OpenAgentsInc/effect-native` commit `fd1ccc5` — primary = accent surface +
   `textPrimary` label, secondary = surface + border, ghost = accent text;
   padding/radius from token scales; app `style` overrides still win.
   Regression tests (`packages/render-rn/test/button-variants.test.ts`) pin
   all three variants on the khala theme. The monorepo vendored copy carries
   the same hunk; vendor pins bumped to `fd1ccc5`.

## Vendoring honesty note

The re-vendor recipe says "re-copy upstream src/**", but a byte comparison
showed the vendored `render-rn`/`render-dom` copies already carry material not
present at their pinned upstream commit (e.g. mobile host-kind surfaces,
atomic-stylesheet changes). A wholesale copy would have REGRESSED them, so
only the Button hunk was applied and the pins were bumped (all four packages +
`effect-native-vendor.json` → `fd1ccc520b9c8d92f9fd0df5b571a936e4d84fc7`; the
guard test passes). Reconciling the vendored set with upstream deserves its
own lane — flagged in #8597.

## Simulator pixel proof (the new upload gate)

Device: iPhone 17 Pro simulator, Release build of this commit, screenshots via
`xcrun simctl io booted screenshot`; taps via `cliclick` at mapped
point-accurate window coordinates.

- `receipts/2026-07-09-build106-fixed-initial.png` — Home renders: accent-blue
  readable "Dispatch a typed intent" button; the SwiftUI Liquid Glass card
  ("Liquid Glass", subtitle, "Typed intents from SwiftUI: 0") and the glass
  button VISIBLE below the EN surface; "Bundle: 2026-07-09.embedded-106".
- `receipts/2026-07-09-build106-after-glass-tap.png` — after ONE simulated tap
  on the SwiftUI glass button: **"Glass intents received: 1"** (EN tree
  re-render) and **"Typed intents from SwiftUI: 1"** (island prop projection)
  — the typed SwiftUI → `GlassPinged` → state → two-renderer round-trip on
  real pixels.
- `receipts/2026-07-09-build106-after-en-tap.png` — after one simulated tap on
  the EN button: **"Typed intents dispatched: 1"**.

## Tests

- App: 20 pass; typecheck clean.
- `clients/khala-mobile` (same vendored renderer): 482 pass.
- Upstream `effect-native`: render-rn 32 pass (incl. new variant regression
  tests) + conformance scripts 32 pass.

## Build 106 release receipt

- Version `0.4.3`, iOS build `106`, `com.openagents.app`; BUNDLE_TAG
  `2026-07-09.embedded-106`.
- Same local recipe (prebuild → archive → manual-signing export → altool);
  upload gated on the pixel proof above. Result recorded in #8597
  (UPLOAD receipt + ASC `processingState`).
- Build 105's device rung: **FAILED owner verification** (recorded in the 105
  receipt correction header).
