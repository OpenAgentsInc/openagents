# openagents.com /blog + /docs: StarCraft-Theme Reset Audit

**Date:** 2026-07-02
**Scope:** `apps/openagents.com/apps/web` — the `/blog` and `/docs` public
surfaces, their divergence from the house StarCraft/Protoss design language,
and the zero-based content reset that reorients both surfaces around Khala
Code.
**Status:** audit + implementation plan. No runtime changes in this doc. The
implementation is tracked as a single GitHub issue (one swoop).

---

## 1. Why this audit exists

The owner directive (2026-07-02):

1. `/docs` and `/blog` were never updated to the actual StarCraft theme,
   despite a pass that was supposed to centralize all site design on it.
2. Zero-base the blog: remove every existing post from the `/blog` index, but
   **keep each post's content live at its URL** for direct links. Replace the
   index with a single placeholder post, **"Introducing Khala Code"**, dated
   **July 2, 2026**.
3. Zero-base the docs the same way: hide most pages from the index/sidebar
   (URLs stay live), leaving only an **Overview** page listed.
4. Everything reorients around **Khala Code** as the front-door product, per
   the framing in the repo root `README.md` ("Khala Code wraps the coding
   harness you already have… and connects you to the wider OpenAgents
   network").
5. The rebuilt surfaces must use the **centralized components from
   `packages/ui`** and the shared token system — no more per-page hardcoded
   hex styling.

## 2. Design-language ground truth

Three documents define the theme, in order of authority:

- **Root `DESIGN.md`** — the current house style: "StarCraft-Protoss energy: a
  dark void with luminous blue energy." Canonical tokens via
  `@openagentsinc/design-tokens`; the Protoss brand layer adds the glow
  palette (`#3a7bff` pylon-core blue, `#4fd0ff` energy cyan, `#8fb6ff` soft
  blue ink), tinted near-black surfaces (`#0c0f13` panel, `#11161d` raised,
  `#0a0d12` sunken), Berkeley Mono typography, energized hairlines, glow
  halos, and explicit bans (no gradient text, no side-stripe accents, no
  pure-#000 UI surfaces, no eyebrow-per-section).
- **`docs/design/starcraft.md`** — the long-form StarCraft/SC2 UI design
  guide (command-console layout grammar, faction skinning, Protoss "sacred
  technology" treatment). Reference material behind the house style.
- **`apps/openagents.com/DESIGN.md`** — the older Vortex-era baseline
  ("pure black foundations, warm off-white text, `--oa-highlight #ffb400`
  amber accent"). This is the layer `/blog` and `/docs` were built against,
  and it predates the Protoss energy layer. It is the source of the amber
  accent that now reads as off-brand on public pages.

**In code**, the StarCraft theme is real and centralized in two places:

- `packages/design-tokens/src/theme.css` — the full `--oa-color-khala-*`
  family (void, surface, borders, `khala-energy-blue #3a7bff`,
  `khala-energy-cyan #4fd0ff`, text/code/graph ramps), fonts, radii, motion.
  Re-exported through `@openagentsinc/ui/tokens`.
- `apps/web/src/styles.css:426–522` — the "Khala / Protoss energy layer":
  `@theme` colors (`--color-khala-energy`, `--color-khala-cyan`,
  `--color-khala-ink-blue`) and the signature utility/component classes
  **`khala-panel`**, **`khala-rule`**, **`khala-index`**, **`khala-glow`**,
  **`khala-glow-strong`**, **`khala-pulse`**, **`khala-focus`**. The
  energized scrollbar (`styles.css:210–265`) shares the palette;
  `src/scrollbar-theme.test.ts` calls it "the shared Starcraft energy
  palette."

`packages/ui` (`@openagentsinc/ui`) is the shared Foldkit component library
(layout shells, panels, nav, typography helpers, data display, AI elements,
icon catalog) consumed as source. `apps/web/src/ui/index.ts` is already just a
re-export shim for it, so the centralization plumbing exists — blog/docs just
don't use it beyond the `Ui.className` helper.

## 3. Current state of `/blog` and `/docs`

Both are single-file, client-rendered Foldkit pages with inline TypeScript
content records (no markdown files):

- **`apps/web/src/page/blog.ts` (~795 lines).** `blogPosts` array at lines
  151–451 holds five posts rendered oldest-era product framing:

  | Slug | Title | Date |
  |---|---|---|
  | `tassadar-run-is-live` | The Tassadar run is live | June 16, 2026 |
  | `pylon-autopilot-v1-rc1` | Pylon & Autopilot v1.0 — RC 1 | June 15, 2026 |
  | `introducing-autopilot-sites` | Introducing Autopilot Sites | June 5, 2026 |
  | `free-autopilot` | Episode 228: Free Autopilot | June 4, 2026 |
  | `get-paid-to-code` | Get Paid to Code | June 4, 2026 |

  Index view maps the whole array to cards (`blog.ts:505`); post lookup is
  `findBlogPost(slug)` against the same array (`blog.ts:453`).

- **`apps/web/src/page/docs.ts` (~687 lines).** `docsPages` array at lines
  46–439 holds ten pages: `openagents`, `get-paid-to-code`,
  `autopilot-basics`, `autopilot-sites`, `software-handoff`, `autonomous-qa`,
  `connect-codex-fleet`, `product-promises`, `forum`, `api`. Sidebar and
  index card grid both map over the full array (`docs.ts:540`, `docs.ts:564`).

### 3.1 Theme divergence (the actual gap)

Blog and docs never adopted the Protoss energy layer. Specifically:

- **No energy layer:** neither page uses `khala-panel`, `khala-rule`,
  `khala-glow`, `khala-pulse`, or `khala-focus`, and neither mounts the 3D
  pylon scene. Panels are flat `border border-[#222] bg-[#010102]`.
- **Wrong accent:** blog's accent is amber `#ffb400` (list bullets
  `blog.ts:654`, quote rule `:667`, links `:682`) — the deprecated Vortex-era
  highlight — instead of energy blue `#3a7bff` / cyan `#4fd0ff`.
- **No tokens:** every color is a hardcoded Tailwind arbitrary-hex string
  (`bg-[#000]`, `text-[#f1efe8]`, `border-[#222]`); neither page consumes
  `--oa-*` / `--oa-color-khala-*` tokens or `packages/ui` component families.
  The only shared pieces they use are the global stylesheet side effects
  (fonts, scrollbar) and `PublicHeader.view`.
- **Pure-#000 shells:** both wrap in `bg-[#000]`, violating the root
  DESIGN.md ban on pure-black UI surfaces (tinted near-blacks only; `#000`
  is reserved for the scene canvas).

Pages that DO carry the theme correctly (for reference during the rebuild):
`home.ts`, `stats.ts`, `tassadar.ts` (in `page/loggedOut/page/`), `code.ts`
(the Khala Code page), `khala-chat/page.ts`, `autopilot-onboarding/page.ts`,
and the shared `persistentScene.ts` backdrop + `backButton.ts` ("Protoss house
style: dark glass").

So the honest finding is: **the "centralize design on the StarCraft theme"
pass covered the hero/scene surfaces but never reached the plain public shell
family** — `/blog`, `/docs`, and also `terms.ts`, `privacy.ts`,
`business.ts`, `activity.ts`, `trace.ts`, and friends all still render the
old flat Vortex shell. Blog/docs are the two in scope now; the rest are noted
in §7 as follow-ups.

### 3.2 Routing facts that make the "keep URLs live" ask cheap

- Route parsers accept **any** slug: `blogPostRouter` (`route.ts:691`) and
  `docsPageRouter` (`route.ts:578`) parse `/blog/:slug` and `/docs/:slug`
  regardless of content arrays; the Worker only serves an SPA shell for the
  `BLOG`/`DOCS` path regexes (`route-table.ts:137,140`) — no server-side
  content changes needed.
- Content resolution is a render-time array lookup (`findBlogPost` /
  `findDocPage`), separate from the index render.
- Therefore: **keep records in the arrays, filter the index/sidebar maps.**
  Add a `listed: boolean` field; index/sidebar map over
  `posts.filter(p => p.listed)`. Direct URLs keep resolving. Deleting a
  record instead would 404-body the URL ("Blog post not found") — that is
  exactly what we must NOT do.
- Tests that pin these routes and must be kept green (and extended):
  `src/docs-blog-route.test.ts`, `src/route-coverage.test.ts`,
  `src/client-server-route-agreement.test.ts`.

## 4. Target state (zero-based)

### 4.1 `/blog`

- Index lists **exactly one** post: **"Introducing Khala Code"**, dated
  **July 2, 2026**, slug `introducing-khala-code`. Placeholder body for now:
  a short paragraph drawn from the README framing (Khala Code wraps your own
  local Codex install, adds fleet/swarm coordination, connects to the
  OpenAgents network; Free pay-with-data vs Paid private plans; Episode 245
  launch) with an explicit "full post coming" placeholder tone. No new
  product claims beyond what the product-promise registry already records
  (`khala_code.*` family; wrapper yellow, economics planned).
- All five existing posts: `listed: false`. Content untouched, URLs live,
  removed from the index. No redirects needed.
- Restyled to the Protoss layer (see 4.3).

### 4.2 `/docs`

- Index and sidebar list **only** the Overview page (`openagents` slug —
  retitle/rewrite toward "Khala Code + OpenAgents overview" per README
  framing; keep the slug so existing links hold).
- The other nine pages: `listed: false`. Content untouched, URLs live,
  hidden from sidebar/index. (External surfaces still deep-link some of
  these — e.g. `docs/autonomous-qa` is referenced from the repo README and
  `docs/product-promises` from CLAUDE.md/live surfaces — which is precisely
  why URLs must stay live.)
- Overview copy reoriented to Khala Code as the front door, per
  `README.md` ("What's Here Now → Khala Code"). Keep hedged claim language
  consistent with the promise registry.
- Restyled to the Protoss layer (see 4.3).

### 4.3 Theming both pages (the actual StarCraft pass)

- Replace the flat shells with the shared energy-layer components:
  `khala-panel` for the article/content panel, `khala-rule` for section
  dividers, `khala-index` markers where sectioning warrants, `khala-focus`
  on interactive elements.
- Kill the amber `#ffb400` accent on these pages; links/bullets/quotes move
  to energy blue `#3a7bff` / soft blue ink `#8fb6ff` per root DESIGN.md.
- Replace hardcoded hex utilities with token-backed classes
  (`--oa-color-khala-*` via the Tailwind theme / `@openagentsinc/ui/tokens`);
  tinted near-black surfaces instead of `bg-[#000]`.
- Compose `packages/ui` primitives where they fit (heading blocks, text
  links, badges, card/list primitives from `layout.ts` / `shared.ts` /
  `data-display.ts`) instead of bespoke inline markup — matching the
  package README's contract: "New web UI should compose these primitives
  first."
- Typography per root DESIGN.md: mono-first headings, eyebrow only on the
  hero, body at reading line-height, 65–75ch measure.
- Optional (nice-to-have, not required for the swoop): mount the shared
  persistent scene behind the panel like `/tassadar` does, at low scrim, so
  blog/docs stop being a dead-flat island. If skipped, the tinted-panel +
  glow treatment alone already closes the visible gap.

## 5. Implementation checklist (single issue, one swoop)

1. `blog.ts`: add `listed` to `BlogPost`; set five existing posts
   `listed: false`; add `introducing-khala-code` (July 2, 2026, placeholder
   body, `listed: true`); index maps listed-only.
2. `docs.ts`: add `listed` to `DocPage`; set all but `openagents`
   `listed: false`; sidebar + index map listed-only; rewrite Overview toward
   Khala Code per README framing.
3. Restyle both pages to the Protoss layer per §4.3 (khala-* components,
   token-backed colors, no amber, no pure-#000, `packages/ui` primitives).
4. Update/extend tests: `docs-blog-route.test.ts` (delisted slugs still
   resolve; new slug resolves; index shows only listed), route coverage and
   client/server agreement stay green; scrollbar/theme tests untouched.
5. Verify: `bun run test:openagents.com` green; visual pass on `/blog`,
   `/blog/introducing-khala-code`, one delisted post URL, `/docs`,
   `/docs/api` (delisted but live); deploy per `docs/DEPLOYMENT.md`.
6. Copy discipline: the placeholder post and Overview rewrite are
   user-facing copy changes explicitly ordered by the owner (2026-07-02);
   keep hedges aligned with the `khala_code.*` promise records; no new green
   claims.

## 6. Invariants and guardrails

- **URLs are promises.** No existing `/blog/:slug` or `/docs/:slug` may stop
  resolving to its content. Delisting ≠ deleting.
- **No routing regressions.** Slug parsing stays string-typed; the `listed`
  filter lives at render time only.
- **Claim discipline.** The placeholder post describes Khala Code with the
  same hedges as README/registry (no public installer yet; economics loop is
  `planned`). Blog copy is public claim surface.
- **Design bans** (root DESIGN.md): no gradient text, no side-stripe accents,
  no pure-#000 panels, no eyebrow-per-section, no bounce easing, icons only
  from `@openagentsinc/ui/icon`.

## 7. Out of scope / follow-ups

- The rest of the flat-shell family (`terms.ts`, `privacy.ts`,
  `business.ts`, `activity.ts`, `trace.ts`, `trace-compare.ts`,
  `pylonCodexAssignmentStatus.ts`, `artanis*.ts`, `demoLegal.ts`,
  `siteCheckoutDemo.ts`) has the same divergence and should get the same
  treatment in a follow-up pass once blog/docs establish the pattern.
- `/forum`'s light "classic board" theme was a deliberate exception at the
  time of this audit, but the owner directive later the same day supersedes
  it: **uniform StarCraft blue everywhere, no light/dark mode.** See
  `2026-07-02-forum-starcraft-theme-consolidation-audit.md` for the forum
  consolidation audit and its own implementation issue.
- `apps/openagents.com/DESIGN.md` still documents the amber-accent Vortex
  baseline; it should be reconciled with the root `DESIGN.md` Protoss layer
  (follow-up doc change) so future page work stops inheriting the amber
  accent.
- Writing the real "Introducing Khala Code" post body (replacing the
  placeholder) is a separate content task.
