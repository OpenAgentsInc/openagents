# Tailwind UI v4 Foldkit Port Coverage

This note records the current state of the OpenAgents Foldkit UI registry after
porting from the local Tailwind UI downloads:

- `~/Downloads/application-ui-v4/html`
- `~/Downloads/ecommerce-v4/html`
- `~/Downloads/marketing-v4/html`

The registry lives in `apps/web/src/ui/index.ts`.

## Current Evidence

`apps/web/src/ui/coverage.test.ts` pins every top-level family directory found
under the three downloaded `html` folders. When the local downloads are present,
it also walks every `.html` variant file and verifies that each variant maps to
a registered Foldkit UI family.

Current local variant inventory:

- Application UI v4: `364` HTML variants
- Ecommerce UI v4: `114` HTML variants
- Marketing UI v4: `179` HTML variants

The test fails if a family is removed from the registry arrays, if a local
variant points at an unregistered family, or if the expected download inventory
changes.

The same test file also scans production `apps/web/src` files outside
`apps/web/src/ui` and fails if app pages call `h.Class(...)` directly. Page
surfaces should compose through the Foldkit UI registry; Tailwind classes belong
inside registry primitives and composites.

Run:

```bash
bun run test:web
```

## Application UI

All Application UI family paths from the local download are represented in
`applicationUiV4Families`, including shells, stats, calendars, form controls,
lists, tables, navigation, overlays, and page examples.

The current Foldkit registry includes reusable primitives for the families the
OpenAgents product surface app actually uses today, plus workroom-specific compositions mapped back
to Application UI families:

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

The registry includes dark OpenAgents/Foldkit versions of banners, headers,
heroes, landing-page composites, bento grids, blog sections, contact sections,
content sections, CTA sections, FAQ sections, feature sections, footers, logo
clouds, newsletter sections, pricing grids, stats timelines, team grids, and
testimonials.

## Ecommerce UI

All Ecommerce UI family paths from the local download are represented in
`ecommerceUiV4Families`.

The registry includes product/category grids, filters, checkout forms, order
details, order summaries, product lists, product overviews, cart-style line
items, reviews, incentives, promo sections, and store navigation.

## OpenAgents product surface Codebase Boundary

The React/Foldkit app surfaces now compose through `apps/web/src/ui` for the
public landing page, local login form, logged-in shell, dashboard, settings,
and chat/workroom.

The Cloudflare Worker still renders production `openagents.com` HTML with
server-side string templates. Those templates carry `data-ui-family` markers
for the same Tailwind UI taxonomy, but they are not Foldkit components because
the Worker route currently owns auth/session rendering directly.

## Remaining Completion Standard

This note proves family-path coverage, local variant-to-family coverage, and
broad component-library coverage. It does not claim that every individual
Tailwind UI example variant file has been ported one-for-one. A stricter
completion pass would need either:

1. a generated manifest of every variant HTML file and a corresponding Foldkit
   variant/export for each, or
2. an explicit project decision that family-level primitives and composites are
   the intended port granularity.
