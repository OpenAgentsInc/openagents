# OpenAgents Foldkit UI

`@openagentsinc/ui` is the shared component library. The original Foldkit
surface remains consumed as source (no build step), mirroring
`@openagentsinc/autopilot-ui` conventions. TS-9 adds the React edition at
`@openagentsinc/ui/react` for TanStack Start, Khala Code desktop React work,
and future Expo/NativeWind consumers.

React edition:

- import `@openagentsinc/ui/react.css` once in a React/Tailwind app to load the
  Tailwind 4 `@theme inline` projection over the canonical `--oa-*` tokens.
- import components from `@openagentsinc/ui/react`; the first surface includes
  buttons, panels, nav, cards, form fields, and code blocks for funnel pages.
- `openAgentsNativeWindTokens` exports literal StarCraft-blue token values for
  NativeWind so TS-8 can consume the same palette without CSS variables.
- `ReactEditionSmokeFixture` is covered by `bun run --cwd packages/ui
  visual-smoke` as the Storybook-less fixture lane.
- The package intentionally stays dark-only. Do not add theme toggles or
  `dark:`/`light:` variants here; use the existing `scheme-only-dark` root.

Notes for future consolidation work:

- `icon.ts` is co-located here because `workroom.ts` depends on it and it is
  used app-wide. It is a generated file
  (`apps/openagents.com/scripts/sync-fireball-icons.mjs`). TODO: that generator
  still hardcodes its output path; if regenerating, point it at
  `packages/ui/src/icon.ts` instead of the old app path
  (`apps/openagents.com/apps/web/src/icon.ts`, now a thin re-export shim).
- Most moved kit families still compose Tailwind utilities directly while the
  component layer finishes consolidating onto central tokens. `class-foldkit.ts`
  is the neutral Foldkit bridge for stable CSS class names. Shared Autopilot
  dark values live in `@openagentsinc/design-tokens`; `tokens.ts` re-exports
  that neutral package for UI consumers. The first shared render helpers, form
  controls, Prompt Input AI Element, workroom surfaces, and Autopilot domain
  components now use token-backed component classes.
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
