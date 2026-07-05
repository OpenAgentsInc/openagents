# Khala Mobile vs. Ignite Structure Audit - 2026-07-05

Status: structure audit only. This document compares the current
`clients/khala-mobile` Expo React Native app against the local Ignite
reference at `/Users/christopherdavid/work/projects/repos/ignite`. It does not
change a product promise, claim new device proof, or recommend adopting Ignite
as a framework wholesale.

Scope:

- Khala Mobile: `clients/khala-mobile`
- Ignite app reference: `projects/repos/ignite/boilerplate`
- Ignite CLI/reference mechanics: `projects/repos/ignite/src` and
  `projects/repos/ignite/docs`

Guardrail: `clients/khala-mobile/AGENTS.md` says Khala Mobile must build and
submit locally only, must use OpenAgents Updates instead of Expo hosted update
service, and must keep bearer material in the secure-store/keychain adapter.
Any Ignite pattern that conflicts with those rules is explicitly out of scope.

## One-Line Verdict

Khala Mobile already has the hard product-specific pieces Ignite cannot give
us: Khala Sync, secure-store policy, OTA policy, Expo Router routes, native
module seams, and a serious Bun test harness. Ignite is useful as a mature
React Native app-structure reference: it shows successful patterns for a
boring reusable screen shell, typed theme/provider ergonomics, dependency
architecture checks, generator templates, Maestro flows, i18n/copy discipline,
error boundaries, and development diagnostics.

The highest-leverage borrow is not a dependency. It is a small "mobile app
spine" layer around Khala's existing product code.

## Side-by-Side Shape

| Area | Khala Mobile today | Ignite pattern | Audit read |
| --- | --- | --- | --- |
| App entry | Expo Router `app/_layout.tsx` owns `GestureHandlerRootView`, `StatusBar`, `KhalaAuthProvider`, auth gate, and signed-in `Stack`. | `app/app.tsx` owns `SafeAreaProvider`, `KeyboardProvider`, font/i18n readiness, `ThemeProvider`, navigation persistence, and `ErrorBoundary`. | Keep Expo Router. Borrow the explicit provider spine and error/loading boundaries. |
| Routes | File routes in `app/(drawer)` and `app/thread/[threadId].tsx`; reusable code in `src/*`. | Classic React Navigation stack/tab files in `app/navigators`, plus an Expo Router conversion path that moves reusable code out of the router namespace. | Khala's route/source split is already right. Preserve it as route count grows. |
| UI primitives | Product-specific primitives (`ArwesButton`, `BackgroundGradient`, `Frame`, `SwipeableItem`, `Toggle`, `ChatComposer`) plus direct RN `Text`, `Pressable`, `View` in many places. | Reusable `Text`, `Button`, `Screen`, `Header`, `ListItem`, `TextField`, `EmptyState`, `AutoImage`, `Icon` with common accessibility/theming behavior. | Borrow the primitive architecture, not the visual style. Khala needs OpenAgents-flavored `Text/Button/Screen` wrappers. |
| Theme | `src/theme/tokens.ts` bridges shared `@openagentsinc/ui` NativeWind tokens and `tailwind.config.cjs`. | `app/theme` has typed colors, spacing, typography, timing, light/dark themes, provider, and `themed()` helper. | Khala should keep shared tokens. Borrow typed theme/context affordances only if they reduce duplicated classes or unlock safe native styles. |
| Config | `app.json` has self-hosted updates, local native module plugins, and public `extra.khala` endpoints. | `app/config` separates base/dev/prod and documents that bundled config is public, not secret. | Borrow the "bundled config is public" documentation and maybe a typed public config module. Do not put secrets there. |
| Networking/errors | Sync/auth code returns or throws `messageSafe` strings and typed Khala Sync states. | `services/api/apiProblem.ts` normalizes transport/status failures into a typed union. | Borrow typed problem classification for mobile HTTP boundaries, adapted to Effect/Khala Sync instead of `apisauce`. |
| Storage | SecureStore for API keys, SQLite for sync cursors/projections, explicit invariant banning secrets outside secure-store. | `utils/storage` wraps MMKV for nonsecret local state and has unit tests. | Borrow the wrapper/test pattern only for nonsecret preferences, if needed. Do not borrow MMKV for bearer material. |
| Tests | `bun test`, pure-core tests, behavior-contract registry, and a custom RN component mount harness for `ChatComposer`. | `jest-expo`, React Native Testing Library example, i18n missing-key test, Maestro flows. | Khala's Bun harness is stronger than Ignite's sample for current needs. Borrow Maestro flow structure next. |
| Architecture checks | No dependency-cruiser rule in this package today. | `.dependency-cruiser.js` checks circular deps, orphan modules, test imports, missing package deps, dev-dep leakage, and platform extensions. | Strong borrow candidate. Add a Khala-specific `depcruise` check with monorepo exceptions. |
| Generators | No local mobile scaffolder. New screens/components are hand-shaped. | `ignite/templates/*` EJS templates with front matter, destination dirs, patches, route/type anchors, app-icon and splash helpers. | Borrow a small local template idea for Khala screens/components/contracts. Do not adopt Ignite CLI as an app dependency. |
| Device proof | Pending in the Khala UX contract: launch/sign-in/thread/send and native STT/FM proof. | `.maestro/flows` has shared startup/login flows and env-provided app id. | Highest-confidence borrow for closing the pending device-smoke gap. |

## Borrow First

### 1. A Khala `Screen` Primitive

Ignite's `Screen` component is the best structural pattern to borrow. It
centralizes safe-area edges, keyboard behavior, status/system bar styling, and
fixed/scroll/auto layout presets. Khala currently has `ScreenShell`, but it is
mostly a scroll wrapper with a title/subtitle. That is fine for first-pass
screens, but the thread/composer surface needs stronger keyboard and safe-area
behavior than a generic `ScrollView`.

Recommended Khala shape:

- Add `src/components/screen.tsx` or extend `src/components/shell.tsx`.
- Keep NativeWind/OpenAgents tokens as the styling authority.
- Support presets that map to Khala needs: `fixed`, `scroll`, and
  `keyboardAware`.
- Include safe-area edge selection and a consistent content-width/padding
  contract.
- Use it from settings, sign-in, thread list, and future native-status screens.

Why borrow: this reduces screen-by-screen layout drift and gives Maestro tests
stable surfaces to target.

### 2. Text and Button Wrappers With Accessibility Defaults

Ignite's `Text` and `Button` wrappers consistently apply typography presets,
accessibility roles/states, disabled styling, and accessory slots. Khala has
beautiful product-specific controls, but many ordinary labels/buttons still use
raw RN primitives. A thin OpenAgents-native wrapper would make the boring UI
more consistent without dulling the Arwes/Skia identity.

Recommended Khala shape:

- `KhalaText` with `variant` values such as `body`, `muted`, `mono`, `heading`,
  `caption`, and optional `numberOfLines`.
- `KhalaButton` for plain actions, with `accessibilityRole="button"`,
  disabled state, loading state, and left/right accessory support.
- Keep `ArwesButton` for intentional cyberpunk/high-signal actions; do not make
  every button Skia-heavy.

Why borrow: better default accessibility and less duplicated text class
composition.

### 3. Maestro Flows For the Pending Device Smoke

Ignite's `.maestro` folder is small and useful: shared startup, shared login,
environment-driven `MAESTRO_APP_ID`, and scenario flows that assert real visible
state. Khala's own docs already name the missing proof:
`khala_mobile.platform.launched_app_interaction_smoke.v1`.

Recommended Khala shape:

- Add `clients/khala-mobile/.maestro/shared/_OnFlowStart.yaml`.
- Add a public-safe smoke flow for launch -> sign-in fallback or fixture auth
  -> open/create thread -> type/send message -> observe composer/thread state.
- Keep credentials out of the flow. Use fixture/dev-only public-safe auth
  plumbing or an explicitly documented manual precondition.
- Run locally only. This should not imply EAS or GitHub-hosted CI.

Why borrow: it converts the current "builds" evidence into actual launched app
interaction evidence.

### 4. Dependency-Cruiser Architecture Rules

Ignite's dependency-cruiser config is valuable because React Native apps
accumulate accidental coupling quickly. Khala has separate domains
(`auth`, `security`, `sync`, `native`, `status`, `theme`, `components`) and
would benefit from mechanical checks as those domains grow.

Recommended Khala rules:

- No production import from `tests`.
- No circular dependencies.
- No missing/unknown package deps.
- No devDependency imports from production app/source files.
- Optional: domain rules, for example `src/security` must not import UI,
  native modules import through `src/native/modules.ts`, and route files in
  `app/` may import domain modules but domain modules may not import route
  files.
- Add monorepo-aware exceptions for workspace packages and Expo/Metro
  resolution.

Why borrow: it is a cheap way to protect the route/source split and the
security boundary.

### 5. Typed Public Config With a Secrets Warning

Ignite's config module carries a blunt, correct warning: bundled config is
public. Khala already follows the important rule through SecureStore, but
`app.json` has public endpoints in `extra.khala` and the README explains the
security model in prose. A typed public config module would make that boundary
harder to blur.

Recommended Khala shape:

- Add `src/config/public-config.ts` that reads Expo constants and validates
  only public values: base URLs, update owner, build metadata.
- Put the "bundled config is public, never store secrets here" warning in that
  module and/or README.
- Keep `src/security/keychain.ts` as the only bearer/API-key persistence path.

Why borrow: it gives reviewers a named place to reject future secret-bearing
config edits.

### 6. Local Templates For Screens, Components, and Contract Oracles

Ignite's generator templates are successful because they encode local
conventions. Khala does not need the full CLI, but it would benefit from a
tiny repo-local scaffold for repeatable mobile additions.

Recommended Khala shape:

- `clients/khala-mobile/templates/screen` for Expo Router screen shells.
- `clients/khala-mobile/templates/component` for `KhalaText`/`KhalaButton`/
  NativeWind conventions.
- `clients/khala-mobile/templates/contract-oracle` for adding a UX contract
  entry plus matching Bun test skeleton.
- A simple Bun script can render templates; avoid adding a large CLI dependency.

Why borrow: every new mobile feature should start with the same layout,
accessibility, and test shape.

## Borrow Later

### I18n And Copy-Key Discipline

Ignite has a full i18n spine: locale detection, RTL setup, typed translation
key paths, `tx` props, and a missing-key test. Khala currently has little or no
i18n surface, and that is acceptable while the app is still proving device
parity. Once onboarding, settings, wallet/payout state, or customer-facing
errors grow, copy keys become worthwhile.

Recommended trigger: introduce this when the same user-facing phrase appears in
multiple screens or when owner/customer copy needs review gates.

### Error Boundary And Crash Reporting Seam

Ignite wraps the app navigator in an `ErrorBoundary` and leaves a clean
`crashReporting.ts` seam for Sentry/Bugsnag/Crashlytics. Khala currently
suppresses all LogBox UI warnings in `app/_layout.tsx` because the dev warning
pill is unreadable. That may be fine as a short-term dev-chrome fix, but it is
not a durable diagnostics strategy.

Recommended trigger: before wider TestFlight/user testing, add an app-level
error boundary that renders a public-safe recovery screen and logs only
redacted, nonsecret diagnostics. Keep crash reporting off or local until the
privacy boundary is explicit.

### Devtools Commands

Ignite's Reactotron setup is useful because it provides deliberate dev-only
commands: reset navigation, navigate, go back, show dev menu. Khala should not
copy Reactotron by default, but the idea maps well to Khala's needs.

Recommended trigger: when the app has stable fixture auth/sync states, add
dev-only helpers to reset local sync state, jump to a thread, seed fixture
threads, and inspect connectivity. Keep these behind `__DEV__` and never print
tokens or chat bodies.

### Asset Generators

Ignite has app-icon and splash-screen generators that handle platform-specific
asset sizes. Khala currently has only `assets/images/icon.png`. This is useful
later when the app needs a polished launch image/adaptive icon set.

Recommended trigger: before production store polish. Keep generation local and
do not adopt EAS cloud lanes.

## Do Not Borrow

- Do not borrow Ignite's EAS build/submit/update scripts. Khala Mobile's
  contract is local prebuild/Xcode/Gradle/native upload tools only, with
  OpenAgents Updates for OTA.
- Do not replace Expo Router with Ignite's classic React Navigation shell.
  Khala's file-route split is already aligned with Expo's modern path.
- Do not use Ignite's MMKV wrapper for secrets. At most, adapt the wrapper
  shape for nonsecret preferences. API keys and bearer material stay in
  `expo-secure-store` through `src/security/keychain.ts`.
- Do not add `apisauce` only to mirror Ignite. If Khala needs typed HTTP
  problem mapping, implement it around the existing `fetch`, Effect, and
  Khala Sync contracts.
- Do not import Reactotron wholesale before deciding what private data it can
  observe. Khala's chat/sync surfaces are privacy-sensitive.
- Do not copy Ignite's demo/showroom screens. Khala already has a strong
  product-specific visual language; borrow structure, not demo content.

## Suggested Implementation Order

1. Add a Khala `Screen` primitive and migrate the smallest static screen first
   (`settings` is probably the easiest).
2. Add `KhalaText` and `KhalaButton` wrappers, then use them only in new or
   touched screens until the shape proves itself.
3. Add a package-local `depcruise` script in warning mode, then tighten one
   rule at a time after false positives are known.
4. Add `.maestro` shared startup plus one public-safe smoke flow for the
   pending launched-app interaction contract.
5. Add typed public config with the explicit "bundled config is public" warning.
6. Add local templates once the primitives are stable, so generated code starts
   from the settled conventions.

## Existing Khala Strengths To Preserve

- The secure-store/keychain invariant is stronger and more product-specific
  than Ignite's generic local storage wrapper.
- The OpenAgents Updates path is already aligned with the repo's no-EAS-cloud
  mobile policy.
- The behavior-contract registry is more rigorous than a typical mobile
  boilerplate checklist.
- The custom Bun React Native mount harness for `ChatComposer` is a real asset.
  Extend it; do not replace it with Jest just because Ignite uses `jest-expo`.
- Native modules under `modules/` are explicit about unavailable/pending states,
  which is the correct shape for STT and Apple Foundation Models work.

## Bottom Line

Borrow Ignite's boring production discipline: a shared screen shell, accessible
primitive wrappers, architecture checks, local generators, typed public config,
Maestro device flows, and error/devtools seams. Keep Khala's product-specific
authority boundaries, Expo Router app shape, Bun test infrastructure, secure
storage rules, self-hosted OTA, and native module fail-closed behavior.
