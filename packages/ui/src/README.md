# OpenAgents Foldkit UI

`@openagentsinc/ui` is the shared Foldkit component library. It is consumed as
source (no build step), mirroring `@openagentsinc/autopilot-ui` conventions.

Notes for future consolidation work:

- `icon.ts` is co-located here because `workroom.ts` depends on it and it is
  used app-wide. It is a generated file
  (`apps/openagents.com/scripts/sync-fireball-icons.mjs`). TODO: that generator
  still hardcodes its output path; if regenerating, point it at
  `packages/ui/src/icon.ts` instead of the old app path
  (`apps/openagents.com/apps/web/src/icon.ts`, now a thin re-export shim).
- The moved kit families do not reference a design-tokens module; they compose
  Tailwind utilities directly. No `./tokens` export is needed for this package
  yet. `@openagentsinc/autopilot-ui` keeps its own Autopilot-specific
  `./tokens` and is unaffected. TODO: revisit token consolidation in a later
  phase if/when a shared token source emerges.
- `tenant-theme.ts` and `credits-panel.ts` remain app-local in
  `apps/openagents.com/apps/web/src/ui/` and import shared bits from
  `@openagentsinc/ui`.

This folder is the Foldkit component-library port boundary for the downloaded
Tailwind UI reference kits:

- `application-ui-v4/html`
- `ecommerce-v4/html`
- `marketing-v4/html`

The source kit is used as a taxonomy and interaction reference, not vendored
HTML. Components here adapt those families to the OpenAgents design contract:
dark-only command surfaces, pure black foundations, compact mono typography,
thin borders, no marketing gradients, no nested cards, and no raw credential
refs in user-facing account surfaces.

Current covered families:

- application shells and navigation
- breadcrumbs, pagination, command palettes, sidebar navigation, and vertical
  navigation
- headings
- panels and section surfaces
- buttons and link buttons
- button groups, dropdown menus, badges, avatars, and avatar groups
- form controls
- input groups, select menus, checkbox lists, radio groups, combobox lists,
  textareas, and toggles
- stats and description lists
- calendars
- stacked lists and media rows
- table lists, feed lists, grid lists, list containers, cards, dividers, and
  media objects
- alerts and empty states
- badges/status dots
- modal dialogs, drawers, and notification stacks
- action panels, progress lists, and settings/home-screen composites
- workroom shell, consolidated sidebars, rails, split panes, compact controls,
  panel headers, tabs, key/value rows, and code blocks
- ecommerce category filters and store navigation
- ecommerce product/category grids and product overview cards
- ecommerce cart/order line items
- ecommerce order summaries and checkout field groups
- ecommerce reviews, incentives, and promo bands
- ecommerce category, checkout, and order-detail page composites
- marketing banners, headers, heroes, CTA/content/feature sections
- marketing bento grids, pricing grids, FAQs, testimonials, team, blog,
  contact, newsletter, stats timeline, logo cloud, and footer
- marketing landing-page composites
- public landing theme shells, selectors, and the scoped light/dark script
- OpenAgents Business landing components: availability badges, offering menus
  and cards, quick-win ladders, proof caveats, project invites, landing heroes,
  and the public intake form

New web UI should compose these primitives first. Add a new primitive here only
when it maps to a real Application UI family or removes duplicated app code.
