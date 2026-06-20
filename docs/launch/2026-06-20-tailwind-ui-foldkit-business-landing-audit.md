# Tailwind UI Foldkit Components + `/business` Landing Audit

Date: 2026-06-20
Repo: `OpenAgentsInc/openagents`
Scope: `@openagentsinc/ui`, the Tailwind UI v4 Foldkit port boundary, the
`/components` gallery, and the public `/business` landing page in
`apps/openagents.com/apps/web`.

## Status

The Tailwind UI port does exist, but not where the older notes say it lives.
The shared Foldkit component library is now `packages/ui` and the old app-local
`apps/openagents.com/apps/web/src/ui/index.ts` is only a barrel shim:

```ts
export * from '@openagentsinc/ui'
export * from './tenant-theme'
export * from './credits-panel'
```

The useful answer is therefore:

- **Yes, we have Foldkit versions of Tailwind UI families.** They live in
  `packages/ui/src/*`.
- **No, `/business` is not yet composed from those new component versions.** It
  imports `../ui`, but almost every page section is still locally hand-built
  with `Ui.className(...)` class strings.
- **The current `/business` page is already newer than the original June 16 C3
  page.** It now has the June 20 business offering menu, quick-win ladder,
  hidden referral-code capture, workspace invite copy, pricing note, Slack
  opt-in, and the public signup form.

### Implementation Status

- 2026-06-20 / #5832: Completed the honesty patch before visual polish. The
  product-promise registry now points at the actual business page/test files,
  `/business` separates live backing pieces from current caveats, and the intake
  spec uses `Available now`, `Operator-assisted`, and `Roadmap` instead of the
  older coarse `now/soon/roadmap` shorthand.
- 2026-06-20 / #5833: Added the shared `@openagentsinc/ui/business` family with
  business landing heroes, availability badges, offering menus/cards,
  quick-win ladders, proof caveats, project invites, and the public intake form.
  `/components` now renders the Business family with both dark and light sample
  presentations, and package tests cover the new `data-ui-family` markers plus
  the default business intake field contract.
- 2026-06-20 / #5834: Studied Forum's light/dark/system selector and added a
  shared `@openagentsinc/ui/public-theme` family for landing pages. The landing
  script keeps the Forum preference model but scopes resolved attributes to
  `[data-public-landing-shell]` instead of writing to `<html>`, so light public
  pages can coexist with the dark-only app root and Forum's own theme tokens.

## Sources Reviewed

- Repo guidance: `AGENTS.md`, `INVARIANTS.md`,
  `apps/openagents.com/AGENTS.md`, `apps/openagents.com/INVARIANTS.md`,
  `apps/openagents.com/DESIGN.md`
- Launch context:
  - `docs/launch/JUNE16_ROADMAP.md`
  - `docs/launch/JUNE19_ROADMAP.md`
  - `docs/launch/2026-06-19-ai-slop-refactor-wave-sales-opportunity.md`
  - `docs/launch/2026-06-19-near-term-product-priorities.md`
  - `docs/launch/2026-06-19-previous-hud-systems-audit.md`
- Business context:
  - `docs/business/2026-06-20-openagents-business-intake-spec.md`
  - `docs/business/2026-06-20-business-offering-promise-coverage.md`
  - `docs/launch/vertex-fleet/business.intake_quick_win_offering.v1.md`
- Component code:
  - `packages/ui/src/README.md`
  - `packages/ui/src/public.ts`
  - `packages/ui/src/public-theme.ts`
  - `packages/ui/src/forms.ts`
  - `packages/ui/src/shared.ts`
  - `packages/ui/src/layout.ts`
  - `packages/ui/test/coverage.test.ts`
  - `apps/openagents.com/apps/web/src/page/components.ts`
  - `apps/openagents.com/apps/web/src/page/business.ts`
  - `apps/openagents.com/apps/web/src/page/forum.ts`
  - `apps/openagents.com/apps/web/src/page/publicHeader.ts`
  - `apps/openagents.com/apps/web/src/styles.css`

## Tailwind UI Port Inventory

The local Tailwind UI downloads are present:

| Kit | Local variants | Family dirs |
| --- | ---: | ---: |
| `application-ui-v4/html` | 364 | 49 |
| `ecommerce-v4/html` | 114 | 21 |
| `marketing-v4/html` | 179 | 23 |

`packages/ui/test/coverage.test.ts` pins the family taxonomy and, when the
downloads are present, walks each local `.html` variant to verify that it maps
to a registered family. The narrow package test passed on this audit:

```text
15 pass, 0 fail
```

Important limitation: this is **family-level coverage**, not one Foldkit export
per Tailwind UI variant. That is probably the right granularity for OpenAgents,
but the docs should say so plainly. The older
`apps/openagents.com/docs/2026-06-03-tailwind-ui-foldkit-port-coverage.md` is
now partly stale: it says the registry lives in `apps/web/src/ui/index.ts`, and
it says the coverage test scans production app pages for direct `h.Class(...)`.
The current `packages/ui/test/coverage.test.ts` does not do that scan.

## Current Component Library Shape

The reusable exports are split sensibly:

- `primitives.ts`: tones, button classes, taxonomy arrays, `kitFamily`
  (`data-ui-family`)
- `shared.ts`: `button`, `linkButton`, `headingBlock`, `avatar`, dropdowns
- `forms.ts`: `inputGroup`, `textareaGroup`, `checkboxList`, `toggleRow`,
  `comboboxList`
- `layout.ts`: `pageShell`, `container`, `section`, `card`, dialogs/drawers
- `public.ts`: marketing/public families: `marketingHero`,
  `marketingLandingPage`, `featureSection`, `pricingGrid`, `faqSection`,
  `testimonialGrid`, `logoCloud`, `footer`, etc.
- `public-theme.ts`: scoped public landing `light` / `dark` / `system`
  primitives: `publicLandingThemeShell`, `publicLandingThemeSelector`, and the
  browser script that resolves system preference without touching the app root.
- `page-examples.ts`: larger app/page composites
- `workroom.ts`, `v4.ts`, `ai-elements/*`: operational/chat/workroom families

### Forum Theme Study

Forum stores a user preference under `oa.forum.v1:theme`. Absence means
`system`; the script resolves that with
`matchMedia('(prefers-color-scheme: dark)')`, then writes the resolved mode to
`<html data-forum-theme="...">`. The CSS defaults Forum tokens to light and
repoints `--color-forum-*` from the `:root[data-forum-theme='dark']` selector,
while `color-scheme` is applied only inside `[data-forum-shell]`.

The public landing theme keeps the same control model but changes the write
target. It sets `data-public-landing-theme` and
`data-public-landing-theme-preference` only on `[data-public-landing-shell]`
elements, and the dark token overrides are scoped to that same shell. That
lets `/business` and future landing pages offer light and dark compositions
without changing `:root`, `data-forum-theme`, or the dark-only application
surfaces.

`/components` is the live workbench for this package. It renders real component
instances, not only metadata, and the route tests passed:

```text
2 files passed, 14 tests passed
```

## `/business` Route Audit

Route ownership is clear:

- Route schema: `apps/openagents.com/apps/web/src/route.ts` (`BusinessRoute`)
- View entry: `apps/openagents.com/apps/web/src/view.ts`
- Page: `apps/openagents.com/apps/web/src/page/business.ts`
- Test: `apps/openagents.com/apps/web/src/business-route.test.ts`
- Intake endpoint: `apps/openagents.com/workers/api/src/business-signup-routes.ts`

The page is public and auth bootstrap is skipped by
`routeRequiresAuthBootstrap`. The form posts to
`/api/public/business-signup`, which records an intake receipt only. It grants
no Slack, workspace, spend, payout, or agent authority.

What is good:

- The page now reflects the June 20 Business offer better than the original
  minimal page: offering menu, quick-win ladder, workspace invite, Slack opt-in,
  referral code capture, and first-class phone field are present.
- The form remains a plain server-posted form, which is fine for a public
  funnel page.
- The intake endpoint validates/sanitizes fields, caps body size, treats Slack
  Connect as `manual_invite_pending`, and keeps referral binding public-safe.
- The route tests for `/business` still pass.

What is not yet on the new component system:

- `business.ts` has **47 `Ui.className(...)` sites** and no meaningful calls to
  `Ui.marketingHero`, `Ui.inputGroup`, `Ui.textareaGroup`, `Ui.button`,
  `Ui.section`, `Ui.card`, or the public marketing composites.
- The page emits no `data-ui-family` markers from `kitFamily`, so the live DOM
  does not tell us which Tailwind UI family each section is composed from.
- `labelledField`, `pricingNoteView`, `workspaceInviteView`, `slackOptInView`,
  `offeringCardView`, `offeringsView`, and `ladderView` are all local page
  components. Several are useful enough to promote into the shared UI package.

## Copy / Promise Drift To Fix Before A Visual Rebuild

The biggest risk is not visual polish; it is overclaiming the current business
offer while making the page look more finished.

1. **Availability labels are too coarse.** The page comment says `now` maps to
   shipped/green, `soon` to yellow, and `roadmap` to planned. But some cards
   marked `Available now` include yellow, red, or planned sub-capabilities:
   inference paid usage, the device-capability dataset, fine-tuning/sandbox
   work, and general compute/training packaging. Split each card into
   "live now" and "caveat" lines, or source the card state from
   `docs/business/2026-06-20-business-offering-promise-coverage.md`.
2. **The pricing note is stale/risky.** It says users can "buy credits and
   spend them as you go." Current promise state says card-to-credit and the
   paid inference-credit path are not collectable end-to-end in production.
   Keep the required credit framing if the product wants it, but add the
   current caveat or change the test that freezes the older copy.
3. **Product-promise evidence has a bad file ref.**
   `business.intake_quick_win_offering.v1` cites
   `apps/openagents.com/apps/web/src/business-route.ts`, which does not exist.
   The real page file is `apps/openagents.com/apps/web/src/page/business.ts`;
   the real route test is `apps/openagents.com/apps/web/src/business-route.test.ts`.

## Component Gaps Blocking Landing-Page Composition

The public component family is broad enough for a generic landing page, but not
yet ergonomic for this business funnel.

Needed additions:

- `businessOfferingMenu` / `offeringCard`: state-aware cards that render
  availability, backing promise ids, live-now copy, caveats, and quick win.
- `availabilityBadge`: a shared primitive so the color/state semantics do not
  live in page-local records.
- `quickWinLadder`: a compact ordered timeline for Day 1 / Week 1 / Ongoing.
- `businessIntakeForm`: a form composite over `inputGroup`, `textareaGroup`,
  checkbox/toggle, hidden referral input, pricing/caveat slot, and submit.
- `publicProofCaveat` or similar: a small reusable note for promise-backed
  pages that need to say "operator-assisted today" without burying the CTA.

Those can live in `packages/ui/src/public.ts` if kept generic, or a new
`packages/ui/src/business.ts` if the semantics are OpenAgents Business-specific.
Either way, export through `packages/ui/src/index.ts` and render live examples
from `/components/business` or `/components/public`.

## Recommended Migration Plan

### Phase 0 - honesty patch before polish

- Correct the bad product-promise evidence ref.
- Reconcile `/business` offering states and pricing copy against
  `business-offering-promise-coverage.md`.
- Update `business-route.test.ts` so it locks the current honest copy, not the
  older June 16 exact pricing line if that line is no longer true enough.

### Phase 1 - add shared business/public primitives

- Add the missing public/business components to `packages/ui`.
- Ensure each emits `kitFamily(...)` / `data-ui-family` markers.
- Add package tests for the new exports and a `/components` live showcase.

### Phase 2 - recompose `/business`

- Replace local field helpers with `Ui.inputGroup` / `Ui.textareaGroup`.
- Replace local buttons with `Ui.button` / `Ui.linkButton`.
- Replace offering and ladder local blocks with the new shared components.
- Keep `PublicHeader.view` unless `Ui.marketingHeader` grows a mobile menu and
  auth-aware login behavior.
- Preserve the server-posted form contract and the referral hidden field.

### Phase 3 - prevent regression

- Add a route-level assertion that `/business` renders expected
  `data-ui-family` markers.
- Add a package or app guard that flags new public landing pages composed only
  from `Ui.className(...)` local blocks.
- Update `apps/openagents.com/docs/2026-06-03-tailwind-ui-foldkit-port-coverage.md`
  or replace it with a new note pointing at `packages/ui`.

## Verification Run

Commands run from this audit:

```sh
bun run test
# cwd: packages/ui

bun run test -- src/business-route.test.ts src/components-route.test.ts
# cwd: apps/openagents.com/apps/web
```

Results:

- `packages/ui`: 15 passed, 0 failed
- Web route tests: 2 files passed, 14 tests passed

No implementation files were changed in this audit.
