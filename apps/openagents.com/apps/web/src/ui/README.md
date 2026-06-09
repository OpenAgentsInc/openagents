# OpenAgents Foldkit UI

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

New web UI should compose these primitives first. Add a new primitive here only
when it maps to a real Application UI family or removes duplicated app code.
