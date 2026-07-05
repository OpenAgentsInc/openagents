# Khala Mobile vs. Ignite Structure Audit - 2026-07-05

Status: structure audit only. This document compares the current
`clients/khala-mobile` Expo React Native app against the local Ignite
reference at `/Users/christopherdavid/work/projects/repos/ignite`. It does not
change a product promise or claim new device proof.

Owner direction, clarified after the first pass: **do not preserve Expo Router
as the target architecture.** Treat the current Expo Router layout as migration
source material and adopt Ignite's React Navigation stack/app-spine structure
more directly: `App` entry, typed stack/tab navigators, navigation utilities,
provider spine, screen primitives, error boundary, i18n/copy discipline,
Maestro flows, dependency-cruiser, and local generators. Keep only the Khala
authority boundaries that Ignite cannot know about.

Scope:

- Khala Mobile: `clients/khala-mobile`
- Ignite app reference: `projects/repos/ignite/boilerplate`
- Ignite CLI/reference mechanics: `projects/repos/ignite/src` and
  `projects/repos/ignite/docs`

Guardrail: `clients/khala-mobile/AGENTS.md` says Khala Mobile must build and
submit locally only, must use OpenAgents Updates instead of Expo hosted update
service, and must keep bearer material in the secure-store/keychain adapter.
Any Ignite pattern that conflicts with those rules is explicitly out of scope.

## Tracking Issues

- [#8426](https://github.com/OpenAgentsInc/openagents/issues/8426) - migrate
  Expo Router shell to Ignite-style React Navigation stacks. Implemented:
  `index.tsx` now registers `src/app.tsx`, and typed React Navigation
  stack/drawer ownership lives under `clients/khala-mobile/src/navigators`.
- [#8427](https://github.com/OpenAgentsInc/openagents/issues/8427) - adopt
  Ignite provider spine and core UI primitives.
- [#8428](https://github.com/OpenAgentsInc/openagents/issues/8428) - add
  Maestro launched-app smoke flows.
- [#8429](https://github.com/OpenAgentsInc/openagents/issues/8429) - add
  dependency-cruiser guardrails and local generators.

## One-Line Verdict

Khala Mobile already has the hard product-specific pieces Ignite cannot give
us: Khala Sync, secure-store policy, OpenAgents Updates policy, native module
seams, behavior-contract tests, and a serious Bun test harness. The routing/app
shell is the piece to change: the audit target is now **Ignite-style React
Navigation stacks and more of Ignite's app spine**, not continued Expo Router.

The highest-leverage borrow is a structural migration: move Khala Mobile toward
Ignite's `App` + `AppNavigator` + typed stack/tab navigator pattern, then layer
Ignite's reusable screen/components/config/testing/devtools conventions around
Khala's existing sync/security/native domains.

## Side-by-Side Shape

| Area | Khala Mobile today | Ignite pattern | Audit read |
| --- | --- | --- | --- |
| App entry | Before #8426, Expo Router `app/_layout.tsx` owned `GestureHandlerRootView`, `StatusBar`, `KhalaAuthProvider`, auth gate, and signed-in `Stack`. Current app entry is explicit `index.tsx` -> `src/app.tsx`. | `app/app.tsx` owns `SafeAreaProvider`, `KeyboardProvider`, font/i18n readiness, `ThemeProvider`, navigation persistence, and `ErrorBoundary`. | Keep moving toward Ignite's explicit `App` entry/provider spine. The main app entry has migrated away from `expo-router/entry`. |
| Routes | Before #8426, file routes lived in `app/(drawer)` and `app/thread/[threadId].tsx`; current route ownership is typed React Navigation under `src/navigators` plus screens under `src/screens`. | Classic React Navigation stack/tab files in `app/navigators`, with typed `AppStackParamList`, stack screens, back-button handling, navigation refs, and optional tab/drawer navigators. | Adopted the React Navigation stack shape as target. Preserve the `src/*` domain split, and keep new route ownership in typed navigators/screens. |
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

### 1. Ignite-Style React Navigation Stacks

Tracking: [#8426](https://github.com/OpenAgentsInc/openagents/issues/8426)

The first architectural borrow should be Ignite's navigator spine, not a
surface-level component cleanup. Khala should migrate from Expo Router's file
routes to explicit React Navigation stacks/drawers/tabs, because that is the
shape the owner wants and it matches Ignite's mature app structure.

Recommended Khala shape:

- Replace `main: "expo-router/entry"` with an explicit app entry (`index.tsx`
  -> `src/app.tsx` or equivalent) that mounts the provider spine.
- Move route ownership out of `app/(drawer)` and `app/thread/[threadId].tsx`
  into typed screens plus `src/navigators/AppNavigator.tsx`,
  `src/navigators/navigationTypes.ts`, and
  `src/navigators/navigationUtilities.ts`.
- Model the signed-out/signed-in split as typed stack groups, with drawer or
  tab navigation inside the signed-in app where it is actually useful.
- Carry over Ignite's Android back-button handling, navigation refs, active
  route helpers, and optional navigation persistence after deciding what state
  is safe to persist.
- Keep Khala's auth gate, sync runtime, secure-store adapter, and native module
  seams. Those are product authority, not router concerns.

Why borrow: it makes navigation explicit, typed, testable, and closer to the
mobile app architecture we want long-term. It also lets future generated
screens patch navigator/type files the way Ignite does, instead of relying on
file-path conventions.

Migration note: this is a real app-shell migration, not a docs-only rename.
Expect package/app-entry changes, screen file moves, replacement typed route
params, and focused regression coverage for sign-in, drawer/settings, thread
open, and deep-link behavior.

### 2. Ignite's Provider Spine

Tracking: [#8427](https://github.com/OpenAgentsInc/openagents/issues/8427)

Ignite's `app/app.tsx` is useful because it centralizes readiness and global
providers: safe area, keyboard provider, theme provider, auth provider,
font/i18n readiness, navigation persistence, and the app navigator. Khala's
current `_layout.tsx` has some of that, but it is constrained by Expo Router's
route wrapper model.

Recommended Khala shape:

- Add an `App` component that mounts `GestureHandlerRootView`,
  `SafeAreaProvider`, a keyboard provider, `StatusBar`/system bar setup,
  `KhalaAuthProvider`, future `ThemeProvider`, and `AppNavigator`.
- Move the current auth gate into the navigator or an app-level stack decision
  instead of making Expo Router's layout component own it.
- Add an app-level error boundary around the signed-in navigator early in the
  migration, with public-safe fallback copy.

Why borrow: this creates one obvious place to reason about launch readiness,
auth state, system UI, crash containment, and navigation state.

### 3. A Khala `Screen` Primitive

Tracking: [#8427](https://github.com/OpenAgentsInc/openagents/issues/8427)

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

### 4. Text and Button Wrappers With Accessibility Defaults

Tracking: [#8427](https://github.com/OpenAgentsInc/openagents/issues/8427)

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

### 5. Maestro Flows For the Pending Device Smoke

Tracking: [#8428](https://github.com/OpenAgentsInc/openagents/issues/8428)

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

### 6. Dependency-Cruiser Architecture Rules

Tracking: [#8429](https://github.com/OpenAgentsInc/openagents/issues/8429)

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
  native modules import through `src/native/modules.ts`, navigator/screen
  files may import domain modules, and domain modules may not import
  navigator/screen files.
- Add monorepo-aware exceptions for workspace packages and Expo/Metro
  resolution.

Why borrow: it is a cheap way to protect the route/source split and the
security boundary.

### 7. Typed Public Config With a Secrets Warning

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

### 8. Local Templates For Screens, Components, Navigators, And Contract Oracles

Tracking: [#8429](https://github.com/OpenAgentsInc/openagents/issues/8429)

Ignite's generator templates are successful because they encode local
conventions. Khala does not need the full CLI, but it would benefit from a
tiny repo-local scaffold for repeatable mobile additions.

Recommended Khala shape:

- `clients/khala-mobile/templates/screen` for React Navigation screen files.
- `clients/khala-mobile/templates/navigator` for typed stack/drawer navigator
  additions.
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

Updated recommendation: borrow this sooner than the first audit suggested.
During the React Navigation migration, introduce the i18n module and typed copy
helpers for new navigator/screen chrome so the migrated app does not bake raw
strings into every new screen.

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
- Do not keep Expo Router as the target architecture merely because the current
  app already uses it. The clarified direction is to migrate toward Ignite's
  React Navigation stack shell.
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

1. Spike the React Navigation migration: explicit `App` entry, provider spine,
   typed `AppNavigator`, signed-out/sign-in stack, signed-in stack/drawer, and
   thread screen params.
2. Port the existing Expo Router screens into typed React Navigation screens,
   keeping Khala auth/sync/security/native modules in place.
3. Add a Khala `Screen` primitive and migrate the smallest static screen first
   (`settings` is probably the easiest).
4. Add `KhalaText` and `KhalaButton` wrappers, then use them only in new or
   touched screens until the shape proves itself.
5. Add a package-local `depcruise` script in warning mode, then tighten one
   rule at a time after false positives are known.
6. Add `.maestro` shared startup plus one public-safe smoke flow for the
   pending launched-app interaction contract.
7. Add typed public config with the explicit "bundled config is public" warning.
8. Add local templates once the navigators/primitives are stable, so generated
   code starts from the settled conventions.

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

Borrow Ignite's app structure more aggressively than the first audit said:
React Navigation stacks, explicit `App` entry, provider spine, typed
navigation utilities, shared screen shell, accessible primitive wrappers,
architecture checks, local generators, typed public config, i18n/copy tests,
Maestro device flows, and error/devtools seams. Keep Khala's product-specific
authority boundaries, Bun test infrastructure, secure storage rules,
self-hosted OTA, and native module fail-closed behavior.
