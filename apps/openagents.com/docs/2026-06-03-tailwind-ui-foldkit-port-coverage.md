# Tailwind UI v4 Foldkit Port Coverage

This note records the current state of the OpenAgents Foldkit UI component
library after porting from the local Tailwind UI downloads:

- `~/Downloads/application-ui-v4/html`
- `~/Downloads/ecommerce-v4/html`
- `~/Downloads/marketing-v4/html`

## Current Location

The shared registry now lives in `packages/ui`.

The app-local path `apps/openagents.com/apps/web/src/ui/index.ts` is only a
compatibility shim:

```ts
export * from '@openagentsinc/ui'
export * from './tenant-theme'
export * from './credits-panel'
```

New Foldkit UI components should be added to typed family modules under
`packages/ui/src/*` and exported from `packages/ui/src/index.ts`. App pages may
compose those components and keep small local layout glue, but repeated public
landing sections should not be rebuilt as page-local Tailwind class blocks.

## Completion Standard

The current completion standard is **family-level coverage**, not one Foldkit
export for every Tailwind UI HTML variant.

That means:

- every Tailwind UI v4 family directory from the local downloads is represented
  in the package taxonomy arrays;
- when the proprietary local downloads are present, every `.html` variant maps
  to a registered family path; and
- OpenAgents exports reusable primitives/composites for the families the product
  actually composes.

This note does **not** claim every individual Tailwind UI example variant file
has a one-to-one Foldkit export. A stricter future pass would need either a
generated per-variant manifest or an explicit project decision to keep the
family-level granularity.

## Current Evidence

`packages/ui/test/coverage.test.ts` pins every top-level family directory found
under the three downloaded `html` folders. When any local kit is present, the
test requires all three kits to be present and verifies that every local HTML
variant maps to a registered family.

Current local variant inventory:

- Application UI v4: `364` HTML variants
- Ecommerce UI v4: `114` HTML variants
- Marketing UI v4: `179` HTML variants

The test fails if a family is removed from the registry arrays, if a local
variant points at an unregistered family, or if the expected download inventory
changes.

Run:

```bash
cd packages/ui
bun run test
```

## Public Landing Guard

The older version of this note claimed the coverage test scanned production
pages for direct `h.Class(...)` usage. That scan no longer exists in
`packages/ui/test/coverage.test.ts`.

The current guardrail is split by responsibility:

- `packages/ui/test/coverage.test.ts` protects Tailwind UI family taxonomy and
  local download inventory.
- `apps/openagents.com/apps/web/src/business-route.test.ts` renders `/business`
  and asserts major `data-ui-family` markers for the recomposed landing page.
- `apps/openagents.com/apps/web/src/components-route.test.ts` renders the live
  component workbench, including the Business and Public theme families.
- `apps/openagents.com/scripts/check-zero-debt-architecture.mjs` runs in the
  pre-push `check:deploy` gate and now includes a `/business` public landing
  composition guard. It requires the route source to keep composing through the
  shared `@openagentsinc/ui` public theme and business families and caps local
  `Ui.className(...)` calls to layout glue.

## Application UI

All Application UI family paths from the local download are represented in
`applicationUiV4Families`, including shells, stats, calendars, form controls,
lists, tables, navigation, overlays, and page examples.

The current Foldkit package includes reusable primitives for the families the
OpenAgents product surface uses today, plus workroom-specific compositions
mapped back to Application UI families:

- application shells: sidebar, stacked, multi-column/workroom split
- data display: stats, description lists, calendars
- elements: avatars, badges, buttons, button groups, dropdowns
- feedback: alerts, empty states
- forms: action panels, checkboxes, comboboxes, form layouts, input groups,
  radio groups, select menus, sign-in forms, textareas, toggles
- headings: card, page, section
- layout: cards, containers, dividers, list containers, media objects
- lists: feeds, grid lists, stacked lists, tables
- navigation: breadcrumbs, command palettes, navbars, pagination, progress
  lists, sidebar navigation, tabs, vertical navigation
- overlays: drawers, modal dialogs, notifications
- page examples: detail, home, settings

## Marketing UI

All Marketing UI family paths from the local download are represented in
`marketingUiV4Families`.

The package includes OpenAgents/Foldkit versions of banners, headers, heroes,
landing-page composites, bento grids, blog sections, contact sections, content
sections, CTA sections, FAQ sections, feature sections, footers, logo clouds,
newsletter sections, pricing grids, stats timelines, team grids, testimonials,
and the OpenAgents Business landing components added for the `/business` funnel.

## Ecommerce UI

All Ecommerce UI family paths from the local download are represented in
`ecommerceUiV4Families`.

The package includes product/category grids, filters, checkout forms, order
details, order summaries, product lists, product overviews, cart-style line
items, reviews, incentives, promo sections, and store navigation.
