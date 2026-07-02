# Forum: StarCraft-Theme Consolidation Audit (Web + Khala Code Desktop)

**Date:** 2026-07-02
**Scope:** the Forum surface everywhere it renders — the `openagents.com` web
app (`/forum`, `/forum/f/:ref`, `/forum/t/:id`, `/forum/receipts/:ref`), the
native Forum panel inside Khala Code Desktop, the forum OG/share image, and
the `apps/forum` extraction target.
**Status:** audit + implementation plan. Companion to
`2026-07-02-openagents-com-blog-docs-starcraft-theme-reset-audit.md` (the
blog/docs reset). Implementation tracked as one GitHub issue.

---

## 1. The mandate

Owner directive (2026-07-02): **everything consolidates to the same uniform
StarCraft blue.** No light mode, no dark mode, no theme toggles — one Protoss
energy theme (root `DESIGN.md`; `--oa-color-khala-*` tokens; energy blue
`#3a7bff`) across every surface. The Forum is explicitly included because it
now appears **inside Khala Code Desktop** (hotbar slot 3,
`khala_code.forum_hotbar.v1`), so a divergent website theme is no longer a
charming exception — it is a cross-surface inconsistency in the core product.

This **supersedes** the previously deliberate light "classic board" direction
(`apps/openagents.com/docs/forum/2026-06-08-forum-phpbb-styling-gap-analysis.md`,
`classic-forum.md`, and the classic-board plan in
`2026-06-05-mdk-money-moderated-forum-plan.md`). The classic-forum
**structure** — board → forum → topic hierarchy, dense rows, breadcrumbs,
stats columns — stays; the phpBB **skin** (light page, blue-gradient header,
System/Light/Dark selector) goes.

## 2. Current state

### 2.1 Web forum (`apps/web/src/page/forum.ts`, 971 lines)

- **Rendering model:** Foldkit renders only a shell + skeleton +
  `PublicHeader.view(authState, 'forum', …)`; a ~790-line inline JS string
  (`forumScript`) fetches `/api/forum*` JSON and swaps `#forum-main`
  innerHTML per route. Includes its own markdown renderer, tip controls, and
  the theme machinery.
- **Bespoke palette:** 18 `--color-forum-*` tokens defined locally in
  `styles.css` — light values at lines 31–48 in `@theme`, dark values
  re-pointed under `:root[data-forum-theme='dark']` at lines 83–101, plus
  shell rules at 64–111. ~100 `bg/text/border-forum-*` usages in `forum.ts`
  (top: `forum-link` ×44, `forum-row-c` ×37, `forum-text` ×29). Two tokens
  (`forum-alert`, `forum-online`) are defined but never used.
- **Light/dark switch:** a `data-forum-theme` attribute on `<html>`, driven
  by a System/Light/Dark `<select>` that exists **only** in the forum header
  variant (`publicHeader.ts:61–83, 330`), persisted to
  `localStorage['oa.forum.v1:theme']`, with a `prefers-color-scheme`
  listener for "system" (`forum.ts:65–108`). This is the only theme toggle
  anywhere on the site — the rest of the product is dark-only.
- **Header:** the `'forum'` PublicHeader variant is a hardcoded phpBB-blue
  gradient bar (`from-[#5a9ad9] to-[#3a72b0]`, `font-sans`, white logo/nav —
  `publicHeader.ts:293–313`) that does **not** respond to the light/dark
  tokens at all.
- **Typography:** deliberately `font-sans` throughout (site chrome elsewhere
  is mono-first); only markdown `code`/`pre` is mono.
- **Shared-system usage: zero.** No `@openagentsinc/ui` component classes,
  no `--oa-*`/`--oa-color-khala-*` tokens, no `khala-*` energy classes. The
  only shared imports are the `Ui.className` helper and `Ui.avatar` in the
  header account menu.
- **Hardcoded stragglers** that bypass even the forum tokens: the
  `shadow-[0_0_0_1px_rgba(237,237,237,0.8)]` container ring (`forum.ts:24`),
  `text-white` on header bars (`:31, :389, :623`), a `from-white` gradient
  chip (`:403`), `bg-white/30` skeleton (`:895`).
- **Tests:** nothing pins the forum palette. `forum-route.test.ts` asserts
  structure only; `forum-tip-ui.test.ts` is projection logic. The adjacent
  `scrollbar-theme.test.ts` pins the shared `--oa-scrollbar-*` StarCraft
  palette in the same `styles.css` and must stay green through any edit.

### 2.2 Khala Code Desktop forum panel — already the target aesthetic

`clients/khala-code-desktop/src/ui/forum-panel.ts` +
`styles.css:2026–2212`:

- **Native re-render, not a webview.** The panel builds DOM by hand and
  calls the same `/api/forum*` JSON API through a host-side proxy
  (`src/bun/rpc-handlers.ts:252–283`, pinned to `https://openagents.com` and
  paths under `/api/forum`). Board list, forum topics, topic threads,
  composer, replies, tips, and reports all render natively.
- **Styled entirely on the khala palette:** `.khala-forum-*` classes driven
  by `--oa-color-khala-*` / `--oa-color-component-*` tokens
  (`khala-energy-cyan` eyebrows, `khala-text-bright` titles,
  component surface/border panels, khala danger/success states). Root shell
  is `--oa-color-khala-void` + `color-scheme: dark`.
- **Dark-only. No theme handling at all** — which under the new mandate is
  correct behavior, not a gap.
- Landed via `8ee5a63358` (Forum hotbar surface), `29c8c13b4e` (host API
  proxy), `a22e350953` (promise record `khala_code.forum_hotbar.v1`).

So the consolidation direction is settled by what already shipped: **the
desktop panel is the reference rendering of "Forum in StarCraft blue," and
the website is the surface that moves.**

### 2.3 Adjacent surfaces

- **OG/share image** (`workers/api/src/http/forum-social-preview.ts:247–277`):
  the `/og/forum/:id.svg` card uses the old core palette — `#000` background,
  **amber `#ffb400`** tick, mono text. It matches neither the current forum
  blue nor the khala energy layer; it must move to the khala palette in the
  same pass or share cards will visibly mismatch the retheme.
- **`apps/forum`** (extraction target): a 22-line `ForumMount` schema
  placeholder — no UI, no styling. Not a blocker and not a participant; the
  retheme happens in the `openagents.com` app. (If forum UI is ever
  extracted, the post-retheme khala rendering is what should be extracted.)

## 3. Target state

One forum look everywhere, identical to the rest of the product:

1. **Web forum reskinned onto the khala layer.** Keep the classic-board
   structure and information density; move the skin to Protoss: content
   panels on `khala-panel`-grade tinted near-blacks, energized hairlines
   (`khala-rule`) for section seams, energy blue `#3a7bff` links with cyan
   `#4fd0ff` hover/highlight, `khala-focus` rings, soft blue ink `#8fb6ff`
   for metadata/eyebrows. Row striping (`row-a/b/c`) maps to the khala
   surface ramp (void → surface → raised) instead of powder blues.
2. **Light/dark machinery deleted, not restyled.** Remove the
   System/Light/Dark selector from the forum header variant, the
   `data-forum-theme` attribute writes, the `oa.forum.v1:theme` localStorage
   key handling, the `prefers-color-scheme` resolution, and the entire dark
   override block (`styles.css:83–111` shell rules included, minus what gets
   re-derived). One theme, no toggle — matching the rest of the site and the
   desktop app.
3. **Token strategy: repoint, don't rename (minimal diff).** The ~100
   `*-forum-*` utility usages stay; the 18 `--color-forum-*` tokens get
   **redefined once** in `@theme` as khala-derived values (aliases of
   `--oa-color-khala-*` where possible), and the dark block is deleted.
   Delete the two dead tokens (`forum-alert`, `forum-online`) or wire them
   properly. A follow-up may later migrate usages to shared khala classes
   outright, but repointing gets uniformity in one pass without touching 900
   lines of string HTML.
4. **Header consolidation.** Retire the `'forum'` PublicHeader variant's
   phpBB-blue gradient; the forum uses the standard dark header (with the
   khala accent direction, not amber) or a thin khala-energized variant.
   The `PublicHeaderVariant` type can stay if the forum needs breadcrumb
   affordances, but its colors come from the shared palette.
5. **Typography per house style.** Chrome, headers, breadcrumbs, metadata,
   and stats go mono-first (Berkeley Mono, per root DESIGN.md); post bodies
   may keep the sans stack for long-form reading comfort — DESIGN.md
   explicitly allows that fallback for prose.
6. **OG image on the khala palette:** near-black tinted background, energy
   blue accent (kill the amber tick), consistent with what a user sees when
   they click through.
7. **Hardcoded stragglers fixed:** the four non-token colors in `forum.ts`,
   the white-on-blue header text, and the light container ring.
8. **Desktop/web convergence check.** After the reskin, the web forum and
   the desktop `.khala-forum-*` panel should read as the same product
   surface: same palette tokens, same accent usage, same states
   (danger/success), same eyebrow/metadata treatment. Where the desktop
   panel and web page name equivalent things differently, prefer the
   desktop's khala-token vocabulary. Full component sharing (one forum
   renderer consumed by both) is a follow-up — the desktop panel is vanilla
   DOM and the web page is innerHTML-from-inline-script, so a shared
   implementation belongs to the larger Effect/Foldkit refactor tracked in
   the Khala Code Effect-integration audit, not this pass.
9. **Add the missing palette guard.** Nothing currently pins forum theming.
   Add a test (pattern: `scrollbar-theme.test.ts`) asserting the forum
   tokens resolve to the khala palette and that no
   `data-forum-theme`/theme-selector machinery reappears; keep
   `scrollbar-theme.test.ts`, `forum-route.test.ts`, and
   `forum-tip-ui.test.ts` green.

## 4. Implementation checklist (single issue, one swoop)

1. `styles.css`: redefine `--color-forum-*` (18 → 16 tokens) to khala
   values; delete the `[data-forum-theme='dark']` block and light-only shell
   rules; keep `--oa-scrollbar-*` untouched.
2. `forum.ts`: strip theme logic from `forumScript` (lines ~65–108 and the
   change/media listeners); fix the four hardcoded colors; adjust the
   skeleton to the dark surface.
3. `publicHeader.ts`: remove `forumThemeSelector` and the gradient forum
   header treatment; restyle the forum variant on shared dark/khala chrome.
4. `forum-social-preview.ts`: OG SVG to the khala palette.
5. New palette-guard test; run `bun run test:openagents.com`.
6. Visual pass: `/forum`, a forum page, a topic with posts + tips, a receipt
   page — and the Khala Code Desktop forum panel side-by-side for
   convergence.
7. Deploy per `docs/DEPLOYMENT.md`.
8. Docs: add a superseded-by note to
   `docs/forum/2026-06-08-forum-phpbb-styling-gap-analysis.md` pointing at
   this audit (structure retained, skin superseded by the uniform StarCraft
   mandate).

## 5. Invariants and guardrails

- **Structure is not in scope.** Board/forum/topic hierarchy, tip flows,
  receipts, report buttons, markdown rendering, auth gating: behavior
  unchanged. This is a skin + theme-machinery removal.
- **No behavior change to `/api/forum*`** or the desktop host proxy.
- **Copy discipline:** no user-facing copy changes beyond removing the
  theme selector control.
- **Design bans** (root DESIGN.md): no gradient washes (the header gradient
  goes), no pure-#000 panels, restrained single-accent blue, icons only from
  `@openagentsinc/ui/icon`.
- **The desktop panel is the reference, not a construction site.** Its
  khala styling already complies; touch it only if the convergence check
  reveals token-level mismatches.

## 6. Out of scope / follow-ups

- Extracting a shared forum renderer used by both web and desktop (belongs
  with the Effect/Foldkit refactor lanes; see
  `2026-07-01-khala-code-effect-integration-audit.md`).
- Migrating the ~100 `*-forum-*` utility usages to shared khala classes
  (post-repoint cleanup).
- The rest of the flat-shell public pages (tracked in the blog/docs audit,
  §7).
- Rewriting `forumScript`'s innerHTML rendering into typed Foldkit views —
  a real debt, but orthogonal to uniform theming.
