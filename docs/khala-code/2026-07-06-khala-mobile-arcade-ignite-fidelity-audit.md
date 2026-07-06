# Khala Mobile — Arcade Visual Style / Ignite Structure Fidelity Audit

Date: 2026-07-06

Owner mandate: "I need ALL of it to use components from either arcade repo
or ignite repo, NOTHING else, NOTHING looking different, use the EXACT
visual style from arcade / structure of components from ignite."

Scope read: `clients/khala-mobile/src/{components,screens,navigators,theme}`
against `projects/repos/arcade/app/**` (owned `OpenAgentsInc/arcade`,
visual-style source of truth) and Ignite boilerplate conventions
(structure/architecture source of truth). Builds on two prior audits that
are still accurate and not duplicated here:
`docs/design/2026-07-05-arcade-ui-harvest-audit.md` (the component harvest
plan) and `docs/khala-mobile/2026-07-05-ignite-structure-audit.md` (the
navigator/provider-spine structure plan).

## Bottom line

The harvest genuinely landed at the component level — Frame,
BackgroundGradient, Toggle, DrawerIconButton, TouchableFeedback, ReText,
SwipeableItem+Donut, BlurredPopup, ArwesButton, and the Skia
ActivityIndicator are all real, faithful ports (verified against arcade
source: sizes, techniques, and animation curves match, not stubs). But
**wiring is badly incomplete and inconsistent**: several ports exist and are
never called anywhere in the app, and the app's single most common visual
affordance — loading spinners — still uses the plain React Native primitive
in the highest-traffic call sites instead of the ported Skia one.

There is also one **conflict that needs an explicit decision, not a guess**:
see §0.

## §0 — Needs an explicit answer: literal arcade colors, or arcade technique + Khala's own palette?

`clients/khala-mobile/src/theme/tokens.ts` routes through
`@openagentsinc/design-tokens` (OpenAgents' own shared token package used by
other apps too), not arcade's literal palette. Concretely:

- Arcade: `tint #5BC6E0`, `palette.cyan400 #22d3ee`, background
  `palette.almostBlack #030303` (`projects/repos/arcade/app/theme/colors.ts`).
- Khala: accent `#4fd0ff`, background `#02060d` — a **deliberate** prior
  recolor to OpenAgents' own blue, documented in the harvest audit.
- Khala's `theme/motion.ts` timings and NativeWind-class-only styling
  (no arcade-style named spacing scale) are also documented, deliberate
  prior choices, not oversights.

So today Khala already uses arcade's **techniques** (Skia frame/glow,
Reanimated worklet patterns, breathing sweep-gradients) applied to
**OpenAgents' own palette**, not arcade's literal hex values. The owner's
new mandate ("EXACT visual style," "nothing looking different") could mean
either:

- (a) Keep OpenAgents' palette, apply arcade's techniques/structure
  everywhere consistently (finish the harvest wiring — §3/§4 below), or
- (b) Literally recolor to arcade's palette (`#5BC6E0`/`#22d3ee`/`#030303`/
  `neutral*`), which means forking or replacing `@openagentsinc/design-tokens`
  consumption in this app — a much larger, cross-cutting, harder-to-reverse
  change since that token package is shared with other apps.

This audit does not pick for the owner. Proceeding under interpretation
(a) — finish wiring arcade's real techniques/components everywhere,
keep the existing OpenAgents palette — unless told otherwise, since it is
reversible and lower-risk; a literal-palette rebrand can be layered on top
later if that's actually what's wanted.

## §1 — Components already faithfully ported from arcade

| Khala file | Arcade source | Fidelity |
|---|---|---|
| `src/components/frame/*` | `app/components/Frame/*` | Faithful (corner-unfold + glow-fill `highlighted` state present) |
| `src/components/arwes-button.tsx` | `app/components/ArwesButton/index.tsx` | Faithful; one documented deviation (Reanimated `SharedValue` vs Arcade's Skia `useValue`+`runOnJS`) |
| `src/components/background-gradient/*` | `app/components/BackgroundGradient.tsx` | Faithful breathing SweepGradient+BlurMask port |
| `src/components/toggle/*` | `app/components/Toggle.tsx` | Faithful. **Zero call sites in the app** |
| `src/components/drawer-icon-button.tsx` | `app/components/DrawerIconButton.tsx` | Faithful; correctly wired into `app-header.tsx` |
| `src/components/touchable-feedback.tsx` | `app/components/TouchableFeedback.tsx` | Faithful; used in 3 places only (§4) |
| `src/components/re-text.tsx` | `app/components/ReText/index.tsx` | Faithful. **Zero call sites in the app** |
| `src/components/swipeable-item/*` | `app/components/SwipeableItem` + `AnimatedDonut` | Faithful; wired into thread-messages swipe-to-quote |
| `src/components/blurred-popup/*` | `app/components/BlurredPopup/*` | Faithful; wired into transcript long-press |
| `src/components/activity-indicator/*` | `app/components/ActivityIndicator/*` | Faithful two-arc port. **Inconsistently applied — the single biggest finding, §4** |

## §2 — Components already Ignite-structured (architecture matches, not a literal file copy)

`khala-screen.tsx`, `khala-text.tsx`, `khala-button.tsx`,
`khala-list-item.tsx`, `khala-empty-state.tsx`, `khala-text-field.tsx`,
`khala-error-boundary.tsx`, `theme/khala-theme-provider.tsx`, and
`navigators/*` all match Ignite's/arcade's structural shape (Screen presets,
variant-preset Text/Button, typed navigator param lists, error-screen
fallback pattern) with reasonable, documented simplifications (dark-only
theme, i18n via `src/i18n/copy.ts` instead of full `tx`/locale system). No
action needed here beyond the visual-consistency fixes in §3/§4.

## §3 — Bespoke components (no arcade/Ignite lineage) — real findings

| Khala file | Issue | Fix direction |
|---|---|---|
| `src/components/app-header.tsx` + `src/components/khala-thread-header.tsx` | Two independent, hand-rolled header components instead of Ignite's one parametrized `Header`; back chevron is a static glyph while the hamburger a few lines away is a full Reanimated morph | Unify into one Ignite-style `Header` (icon/title/tx props), reused by both the drawer screens and the thread screen |
| `src/components/khala-thread-header.tsx` | Back/new-note/more buttons are plain `Pressable`s with static glyphs, no `TouchableFeedback` | Swap to `TouchableFeedback` (harvest audit's named base pressable-row primitive) |
| `src/components/nexus-beam/nexus-sign-in-button.tsx` | Self-documented bespoke button sourced from a wireframe file, not arcade; diamond glyph (`◈`) has no arcade precedent | Fold into `KhalaButton`/`ArwesButton` instead of a fourth bespoke button component |
| `src/components/chat-composer.tsx` | Every button (send/stop, +, mic toggle, steer/queue, lane picker) is a plain `Pressable` with instant class-swap, zero `Frame`/`ArwesButton`/`TouchableFeedback` | Named by the prior harvest audit as `ArwesButton`'s intended home; still unaddressed. Highest-traffic control in the app |
| `src/components/shell.tsx` (`ScreenShell`, `NavigationTile`, `StatLine`, `Pill`) | Zero screens import it anymore (only two pure-logic files reference it) | Likely dead code from before the `KhalaScreen` migration — confirm with a full grep, then delete |
| `src/components/khala-scroll-to-latest-button.tsx` | No arcade precedent exists for this control | Low-stakes; adopt `TouchableFeedback` for consistency, no strong mandate either way |

## §4 — Wiring gaps: ported components that exist but aren't called where they should be

This is the most concrete, mechanical class of fix.

1. **Plain RN `ActivityIndicator` instead of the ported Skia one — the single biggest inconsistency in the app.** Three call sites still import from `"react-native"` instead of `"../components/activity-indicator"`:
   - `src/app.tsx` — the auth-gate full-screen loading spinner (first thing every signed-in user sees on cold launch); also hardcodes `"#4fd0ff"` as a literal string instead of `khalaMobileTheme.accent`.
   - `src/components/khala-button.tsx` — the `loading` state of every `KhalaButton` (sign out, delete account, model select, onboarding, error-boundary retry).
   - `src/components/khala-empty-state.tsx` — the `loading` state behind "Loading threads," "Loading messages," "Loading history," "Loading repositories" across 5 screens.

   Meanwhile `chat-composer.tsx`, `thread-list-screen.tsx`'s busy-dot, and `ota-update-gate.tsx` already use the ported one correctly — so two different spinner visual languages currently coexist on the same navigation flow.

2. **`ArwesButton` has zero real call sites** outside its own definition and internal comments — never wired into `chat-composer.tsx` as the prior audit recommended.
3. **`Toggle` has zero call sites** — fully ported, unused (no settings toggle exists yet; Settings is read-only cards + buttons).
4. **`ReText` has zero call sites** — fully ported, unused.
5. **`Frame` is used in exactly one place** (`settings-screen.tsx`'s on-device readiness card) despite being named the harvest audit's single highest-value component — absent from the composer, primary CTAs, thread header, credits/repo pickers.
6. **`TouchableFeedback` is used in only 3 places** (`khala-list-item.tsx`, the transcript tool row, and likely-dead `shell.tsx`) — not in either header component, `khala-button.tsx`, `khala-scroll-to-latest-button.tsx`, or `nexus-sign-in-button.tsx`.
7. **Staggered `FadeIn.delay(n*index)` entrance** is wired into `thread-messages-screen.tsx` and `settings-screen.tsx`'s on-device card, but **not** `thread-list-screen.tsx`, `credits-history-screen.tsx`, or `repo-picker-screen.tsx` — those three lists still render instantly.

## §5 — Screens summary

| Screen | Arcade-style chrome present | Gaps |
|---|---|---|
| `sign-in-screen.tsx` | Yes — faithful `CityBackground`/`HomeScreen` port | Bespoke sign-in button (§3) |
| `thread-list-screen.tsx` | Partial (breathing gradient header) | No stagger on the list; plain loading spinner |
| `thread-messages-screen.tsx` | Partial (stagger, swipe-to-quote, long-press popup) | Bespoke header, flat composer (biggest single gap) |
| `settings-screen.tsx` | Partial (Frame only on one card) | Account/Credits/Models/Notifications sections fully flat |
| `credits-history-screen.tsx`, `repo-picker-screen.tsx` | None | No stagger; plain loading spinner |
| `onboarding-flow.tsx` | None | Fully flat multi-step flow — first thing a brand-new user sees |

## §6 — Out of scope for this pass

Pure-logic `*-core.ts` files, `connectivity-dot.tsx` (currently unmounted),
pixel-level diffing of `khala-list-item.tsx`/`khala-text-field.tsx` beyond
structural comparison, and navigator-utility line-by-line diffing (already
covered by the existing structure audit). A full-repo grep confirming
`shell.tsx` is truly dead code (§3) was not run before writing this audit.

## Execution plan

Proceeding under §0 interpretation (a): OpenAgents' own palette kept; owner
clarified (2026-07-06) the mandate covers more than technique — font,
size, and structure should match arcade too, everywhere except the literal
StarCraft color values.

## Status (2026-07-06, same-day follow-through)

**Done:**

1. **Typography overhaul — the actual root cause of most drift.** `@openagentsinc/ui`'s shared NativeWind `font-sans`/`font-mono` tokens are web CSS font-stack strings (e.g. `"Inter, ui-sans-serif, ..."`) that silently fall back to the OS default on native — this app was never actually rendering an intentional font at all. New `theme/typography.ts` loads arcade's real fonts (Space Grotesk primary, Protomolecule for the `heading` preset only — matching arcade's own preset composition) plus JetBrains Mono for code/mono content (owner call: kept instead of arcade's bare system `Courier`/`monospace`). `khala-text.tsx` rewritten to arcade's exact `$sizeStyles` pixel/line-height values (`khalaMobileTextSizes`) via Tailwind arbitrary-value classes — NOT inline `style`, since NativeWind's explicit `style` prop always wins over `className`, and ~20 call sites already override size via `className` for legitimate one-off reasons; font FAMILY only goes through `style` (no call site overrides that). Font loading gates the whole app root (`app.tsx`) behind a themed spinner, matching Ignite's own font-gate pattern.
2. Fixed all three plain-`ActivityIndicator` call sites (`app.tsx`, `khala-button.tsx`, `khala-empty-state.tsx`) to use the ported Skia one; fixed `app.tsx`'s hardcoded `"#4fd0ff"` to `khalaMobileTheme.accent`.
3. `khala-button.tsx` now renders through `TouchableFeedback` instead of a plain `Pressable`.
4. Folded `nexus-sign-in-button.tsx` into `KhalaButton` (`variant="primary"`); deleted the file and the now-empty `nexus-beam/` directory. Sign-in screen's hero title now also uses `khalaMobileTypography.display` (Protomolecule), matching arcade's `HomeScreen` preset.
5. Wired `TouchableFeedback` into `chat-composer.tsx`'s Send/Stop buttons, `khala-thread-header.tsx`'s Back/New-note/More buttons, and `khala-scroll-to-latest-button.tsx`. Added a `hitSlop` prop to `TouchableFeedback` (not in arcade's original — a necessary addition to preserve existing touch-target sizing when replacing `Pressable`s that relied on it).
6. Added `FadeIn.delay` stagger to `thread-list-screen.tsx`, `credits-history-screen.tsx`, `repo-picker-screen.tsx` (previously only `thread-messages-screen.tsx` had it).
7. Deleted `shell.tsx` (confirmed zero real renderers via full-repo grep).
8. Full test suite (342 tests) + typecheck green. Two real test-infra issues surfaced and fixed along the way: a `provider-primitives-architecture.test.ts` assertion hardcoded the old Tailwind class strings; `bun:test`'s `mock.module` is process-global, so adding real `react-native-reanimated` usage to 3 screens collided with `chat-composer.test.tsx`'s existing reanimated mock (a `require("react-native")` call inside a mock factory throws "Requested module is already fetched" if Bun re-invokes that factory from a different file's context) — fixed by using statically-imported `View` bindings in both mocks instead of a lazy `require`.

**Deferred (not done this pass — flagged honestly, not silently skipped):**

- **Header unification** (execution-plan item 1): `app-header.tsx` and `khala-thread-header.tsx` remain two separate, hand-rolled components rather than one Ignite-style parametrized `Header`. This is a bigger structural refactor than the mechanical fixes above and deserves its own focused pass with device-level visual verification rather than a rushed blind merge.
- **`ArwesButton`/`Frame` for the composer's Send/Stop buttons**: given `Frame` is a rectangular Skia glow-frame and Send/Stop are currently circular icon buttons, swapping to `ArwesButton` means a real shape change I can't visually verify without a device/simulator. Applied the safe, unambiguous improvement instead (`TouchableFeedback`'s press cross-fade); the shape question is left for a follow-up with visual verification.
- `Toggle` and `ReText` remain unwired (no live use case exists yet for either — a settings toggle and a live-streaming counter respectively).
- Settings screen's Account/Credits/Models/Notifications sections and the onboarding flow remain visually flat (no `Frame`/stagger) — out of scope for this pass's time budget.
