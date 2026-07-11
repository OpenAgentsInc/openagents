# CUT-01 Effect Native mobile surface receipt

- Issue: [#8681](https://github.com/OpenAgentsInc/openagents/issues/8681)
- Date: 2026-07-11
- Status: closed on `bab737f565`
- Surface: `apps/openagents-mobile`

## Outcome

The app-local `openagents-liquid-glass` Expo/SwiftUI module is deleted. One
Effect Native view program now owns the home toolbar, drawer, transcript,
composer, draft, and typed actions. `home-screen.tsx` retains only safe-area,
keyboard-avoidance, platform selection, and a single `EffectNativeHost` mount.

`@effect-native/render-rn` owns both native paths. On iOS 26+ a semantic glass
`Composer` uses renderer-internal `@expo/ui` TextField state and an explicit
typed send control. Android, older iOS, tests, and missing-module hosts use an
accessible RN `TextInput`/material fallback with the same change/submit intents,
44-point send target, controlled clear, keyboard submit, and busy/disabled
state. App source never imports `@expo/ui`.

## Deterministic evidence

- `bun run --cwd apps/openagents.com/packages/effect-native-render-rn typecheck`
- `bun test apps/openagents.com/packages/effect-native-render-rn/src/index.test.ts`
  — 8 pass, including injected iOS-26 SwiftUI change/submit/clear and Android
  accessible-fallback proofs.
- `bun run --cwd apps/openagents-mobile typecheck`
- `bun run --cwd apps/openagents-mobile test` — 62 pass, 279 expectations.
- `apps/openagents-mobile/tests/component-sharing.test.ts` recursively rejects
  direct app imports of `openagents-liquid-glass` or `@expo/ui`, the deleted
  module/package, app-owned `Pressable`/`TextInput` controls, and duplicate
  composers.
- `bun run --cwd apps/openagents-mobile prebuild:ios` — passed; Pod resolution
  includes ExpoUI 57.0.4 and contains no `OpenAgentsLiquidGlass` target.
- `xcodebuild ... -sdk iphonesimulator -destination <iPhone 17 Pro, iOS 26.5>
  ... build CODE_SIGNING_ALLOWED=NO` — `BUILD SUCCEEDED`.
- The installed iOS 26.5 simulator rendered the Effect Native glass top toolbar
  and bottom composer with visible menu, mode, compose, plus, input, and send
  controls. This is a pixel sanity check, not the required physical receipt.

## Deferred installed-product rung

The paired physical iPhone was offline and no Android device was attached when
CUT-01 ran. CUT-27 #8707 already requires installed physical iOS and Android
acceptance for the complete coding cutover, so that non-agent-completable rung
stays there instead of serially blocking the architecture leaves. CUT-27 must
still demonstrate native glass/material pixels, text entry, send/clear,
keyboard avoidance, safe areas, accessibility labels, new-chat, drawer/surface
controls, and reconnect behavior on both platforms. This receipt does not
claim those physical outcomes.
