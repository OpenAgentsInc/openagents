# TanStack Start unified docs migration audit

- Date: 2026-07-15
- Snapshot: OpenAgents `1c0c6f7ee9260fbabecf33809b367f462d66ad84`
- Class: architecture, dependency, performance, and reader-UX audit
- Status: recommendation; no implementation or deployment authority
- Dispatch: no; use the bounded implementation slices in this audit if the
  owner authorizes the migration
- Owner: OpenAgents web and docs architecture
- Final disposition: retain until the unified reader is accepted and the separate Blume service is retired
- Decision: fold the complete public docs reader and its generated artifacts
  into the existing TanStack Start application, then retire Blume, Astro, and
  the independent `openagents-docs` service after a measured parity cutover
- Supersedes: the destination topology in
  [the 2026-07-14 Blume integration audit](./2026-07-14-blume-docs-site-integration-audit.md),
  while retaining that audit's content-safety and agent-readability boundaries

## Executive decision

OpenAgents can and should serve the current `/docs` experience from the
existing TanStack Start application. There is no product or technical reason
for the present public corpus to require a separate Astro application, Blume
runtime boundary, Cloud Run service, serverless NEG, backend service, or load
balancer path split.

The replacement should **not** turn the docs into hard-coded React pages. Keep
the curated Markdown content tree, add a narrow build-time content compiler,
and render its typed output through native TanStack Start routes. Preserve the
reader capabilities that matter: nested navigation, responsive navigation,
local search, table of contents, code highlighting and copy, previous/next
links, SEO metadata, raw Markdown, `llms.txt`, `llms-full.txt`, the agent
readability manifest, sitemap, redirects, and docs-scoped not-found behavior.

Do not reproduce Blume features that OpenAgents does not use. The current
content imports no MDX components, so the first TanStack implementation needs
Markdown, not MDX. Ask AI, MCP, remote content adapters, OpenAPI rendering,
feedback, export, RSS, server-side search, a CMS, and Blume's generic page
actions are not requirements. This distinction is the main simplification.

The target topology is:

```text
openagents.com/* ──► Google Cloud load balancer
                         │
                         ▼
                  openagents-monolith
                         │
             ┌───────────┴───────────┐
             │                       │
       API first refusal      TanStack Start
                              ├─ public website
                              ├─ authenticated app
                              └─ /docs reader + artifacts

auth.openagents.com/* ──► openagents-monolith

No openagents-docs service, docs NEG, docs backend, or /docs path split.
```

This is a consolidation, not a visual redesign. A reader visiting `/docs`
should retain the current information architecture and Khala presentation
while gaining same-application navigation, one deployment authority, and real
HTTP redirects.

## What is live now

The current docs source is
the now-removed `apps/openagents.com/apps/docs` source application. It used
exact-pinned `blume@1.0.4` to generate an Astro static site, then wraps the
artifact in a small Node static server and deploys it as the independent
`openagents-docs` Cloud Run service. Terraform sends `/docs` and `/docs/*` to
that service while the remainder of `openagents.com` goes to the monolith.

The curated source tree contains 12 visible pages:

- the docs landing page;
- Getting Started;
- The Workroom;
- Review and Recovery;
- Security and Privacy;
- Troubleshooting;
- Agent-Readable Surfaces;
- the Future / Advanced landing page; and
- Marketplace, Nostr, Bitcoin and Lightning, and Remote Workrooms future pages.

The corpus is small: 402 source lines and approximately 23 KB of content at the
audited snapshot. It uses frontmatter and standard Markdown. It does **not**
import a custom MDX component or contain JSX. The only custom Blume component
is the site header, not content syntax. This means the migration does not need
an MDX runtime or a compatibility layer for Astro components.

The live site currently exposes:

- HTML at `/docs` and every content route;
- raw source at the corresponding `.md` routes;
- `/docs/llms.txt` and `/docs/llms-full.txt`;
- `/docs/agent-readability.json`;
- `/docs/sitemap.xml`;
- grouped and collapsible desktop navigation;
- a mobile navigation drawer;
- an H2–H3 table of contents;
- previous/next pagination;
- command-menu search backed by Orama;
- syntax-highlighted code and code-copy actions;
- Edit on GitHub, scroll-to-top, and copy-as-Markdown actions;
- canonical, description, Open Graph, Twitter, and JSON-LD metadata; and
- a sticky Khala-themed header.

The live implementation does not publish `robots.txt` or RSS and has Blume's
Ask AI, MCP, feedback, export, Open Graph image generation, and remote content
features disabled.

### Existing redirect defect

Blume currently emits static meta-refresh pages for compatibility aliases. The
generated targets for three aliases lose the `/docs` base:

| Public URL | Current generated target | Correct target |
| --- | --- | --- |
| `/docs/product-promises` | `/agent-readable` | `/docs/agent-readable` |
| `/docs/api` | `/agent-readable` | `/docs/agent-readable` |
| `/docs/connect-codex-fleet` | `/getting-started` | `/docs/getting-started` |
| `/docs/openagents` | `/` | `/` |

The Start migration must replace these pages with real HTTP redirects. It must
not preserve the first three broken targets. `Promises` remains absent from
human top-level navigation; its compatibility route points to the
agent-facing documentation.

## Current reader technical health

Score: **15/20 — good foundation, unnecessary platform boundary**.

| Area | Score | Evidence |
| --- | ---: | --- |
| Accessibility | 3/4 | Semantic header, main, article, nav, aside, and dialog landmarks; labelled controls; focus-visible treatment; reduced-motion support; and coarse-pointer target adjustments. Focus containment, Escape behavior, and scroll locking need regression coverage during the React port. |
| Performance | 3/4 | Static HTML, approximately 40 KB of uncompressed initial client JavaScript, lazy search, and navigation prefetching are sound. Each page repeats roughly 50 KB of HTML, search adds about 66 KB of Orama client code when opened, and the build emits ten generated WOFF2 assets in addition to the local mono font. The separate service and warm-instance cost are avoidable. |
| Responsive behavior | 3/4 | The current layout has an effective mobile drawer, breakpoints, touch-target rules, and responsive sidebars. Those behaviors are not inherent to TanStack Start and must be explicitly ported. |
| Theming | 3/4 | The reader uses the Khala darkest blue-black (`#05070d`), Inter-compatible sans typography, Commit Mono, restrained borders, and dark-only behavior. The implementation still bridges hard-coded Khala values through Blume-specific theme variables. |
| Anti-patterns | 3/4 | The adapted shell is substantially cleaner than stock Blume. Remaining generic docs chrome, pills, and outbound “open in chat” actions are optional rather than product-essential. The major anti-pattern is architectural: a second framework and service for a tiny same-origin content tree. |

There are no observed P0 reader defects. The broken compatibility redirects and
duplicate deployment authority are P1 concerns. Search loading, font pruning,
mobile focus behavior, and generated-artifact correctness are P2 concerns.

## Feature disposition

| Capability | Current Blume behavior | TanStack Start replacement | Disposition |
| --- | --- | --- | --- |
| Markdown authoring | Curated `.mdx` files | Curated `.md` files compiled at build time | Keep; remove unused MDX authority |
| Nested routes | Generated from content paths | `/docs` layout plus index and splat routes backed by a typed manifest | Keep |
| Navigation | Grouped Blume sidebar | Explicit typed navigation metadata and React sidebar | Keep |
| Mobile navigation | Astro dialog/drawer | Accessible React dialog/sheet with focus containment, Escape, and scroll lock | Keep |
| Table of contents | Generated H2–H3 outline | Compiler-emitted heading list rendered by the page shell | Keep |
| Previous/next | Content-order pagination | Derived from the same navigation manifest | Keep |
| Local search | Lazy Orama search | Direct, exact-pinned Orama dependency and lazy search chunk | Keep |
| Syntax highlighting | Shiki at build time | Shiki at build time; no browser highlighter | Keep |
| Code copy | Blume client action | Small delegated React/client action | Keep |
| Edit on GitHub | Blume page action | Manifest-derived repository URL | Keep |
| Scroll to top | Blume page action | Native anchor or small client action | Keep |
| Copy as Markdown | Blume page action | Fetch the route's generated `.md` mirror and copy it | Keep |
| Open in v0/ChatGPT/Claude | Blume submenu | None initially | Drop; not core documentation behavior and creates outbound context ambiguity |
| SEO metadata | Blume-generated head | Route `head` metadata from the content manifest | Keep |
| JSON-LD | Blume `WebSite` and `TechArticle` | Compiler-derived JSON-LD emitted through route head scripts | Keep |
| Sitemap | Static XML | Build-generated `/docs/sitemap.xml` | Keep |
| Raw Markdown | One `.md` mirror per page | Build-generated exact-path static artifacts | Keep |
| Agent files | `llms.txt`, full text, readability JSON | Build-generated static artifacts from the same manifest | Keep |
| Redirects | Static meta-refresh pages | Start/server HTTP redirects with correct base paths | Keep and fix |
| Docs 404 | Blume docs shell | Docs layout `notFoundComponent` | Keep |
| Ask AI / MCP | Disabled | None | Do not add |
| Remote content | Disabled | None; local curated content only | Do not add |
| OpenAPI renderer | Unused | None until a product requirement exists | Do not add |
| Feedback / export / RSS | Disabled | None | Do not add |
| CMS or database | None | None | Do not add |

## Recommended owned architecture

### Repository shape

Move the public source into the Start application and make its ownership
obvious:

```text
apps/openagents.com/apps/start/
├── content/docs/
│   ├── index.md
│   ├── getting-started.md
│   ├── workroom.md
│   ├── review-and-recovery.md
│   ├── security-and-privacy.md
│   ├── troubleshooting.md
│   ├── agent-readable.md
│   └── future/
│       ├── index.md
│       ├── marketplaces.md
│       ├── nostr.md
│       ├── bitcoin-and-lightning.md
│       └── remote-workrooms.md
├── src/docs/
│   ├── content-schema.ts
│   ├── docs-navigation.ts
│   ├── docs-content.generated.ts
│   ├── docs-search.generated.ts
│   ├── DocsLayout.tsx
│   ├── DocsPage.tsx
│   ├── DocsNavigation.tsx
│   ├── DocsSearch.tsx
│   └── docs.css
├── src/routes/docs/
│   ├── route.tsx
│   ├── index.tsx
│   └── $.tsx
└── scripts/generate-docs.mjs
```

Generated filenames are illustrative; implementation can split one module per
page. The ownership rules are not:

- `content/docs` is the only public docs source tree;
- repository-wide `docs/` remains internal and must never be globbed into the
  public site;
- generated files are never hand-edited;
- every reader surface and agent artifact comes from one validated manifest;
  and
- source order and visibility are explicit metadata, not filesystem accident.

### Content compiler contract

Run one owned generator before Start development, typecheck, test, and build.
It should:

1. enumerate only `apps/start/content/docs/**/*.md`;
2. parse frontmatter and validate it with a typed schema;
3. reject duplicate slugs, invalid paths, missing titles or descriptions,
   broken internal links, unrecognized navigation groups, and draft/internal
   content in production;
4. reject raw HTML for the initial implementation;
5. parse GitHub-flavored Markdown;
6. create stable heading IDs and an H2–H3 table-of-contents array;
7. highlight code with Shiki during the build;
8. emit sanitized/trusted rendered HTML or a typed React-consumable syntax
   tree, plus source Markdown and document metadata;
9. derive navigation, previous/next links, search records, canonical URLs,
   last-modified metadata, and edit URLs; and
10. generate the sitemap and agent-readable artifacts from that same document
    set.

Using compiled HTML through `dangerouslySetInnerHTML` is acceptable only
because the compiler consumes a repository-owned, reviewed content tree and
rejects raw HTML. If remote or user-authored content is ever introduced, it
must cross a sanitizer and a new trust-boundary review; that is explicitly
outside this migration.

The generator should fail the build rather than silently omit malformed pages.
It should also be deterministic: the same content and toolchain produce the
same route manifest and static artifact bytes, except for explicitly modeled
last-modified data.

### Why Markdown rather than MDX

The current `.mdx` extension is incidental. No page contains JSX or imports a
component. Adding an MDX compiler would preserve theoretical capability rather
than live functionality, expand executable-content authority, and weaken the
initial trust boundary.

Start with Markdown. If a later public page has an accepted need for a custom
interactive component, evaluate MDX as a separate change with a component
allowlist. Do not add it preemptively.

### Routing and rendering

TanStack Router's file-based routing supports nested layout routes, index
routes, splat parameters, route-level code splitting, loaders, and composable
document head metadata. The native shape is:

- `routes/docs/route.tsx` owns the persistent docs shell and
  `notFoundComponent`;
- `routes/docs/index.tsx` resolves the docs landing page; and
- `routes/docs/$.tsx` reads the `_splat` path, resolves a document from the
  generated manifest, and throws `notFound()` for an unknown slug.

These are documented framework primitives, not a custom router:

- [TanStack file-based routing](https://tanstack.com/router/latest/docs/routing/file-based-routing)
- [route and splat concepts](https://tanstack.com/router/latest/docs/routing/routing-concepts)
- [automatic code splitting](https://tanstack.com/router/latest/docs/guide/code-splitting)
- [document head management](https://tanstack.com/router/latest/docs/framework/react/guide/document-head-management/)
- [not-found handling](https://tanstack.com/router/latest/docs/guide/not-found-errors)

Render the complete article in SSR output. Page navigation must not wait for a
client fetch of Markdown or a server search request. Use TanStack `<Link>`
navigation with intent preloading, which is already the Start router default,
so the persistent docs layout remains mounted while article route chunks are
loaded.

The generated document modules may be lazy imports so each page does not ship
the entire corpus. Vite's
[`import.meta.glob`](https://vite.dev/guide/features.html#glob-import) can
generate those imports without a handwritten switch. The shared manifest must
stay small enough to support navigation and route lookup; page HTML belongs in
per-page chunks or server-only loader data.

The Start route allowlist in
[`src/route-table.ts`](../../apps/openagents.com/apps/start/src/route-table.ts)
must gain the single owned family `^/docs(?:/.*)?$`. The monolith's API-first
dispatch remains authoritative before Start rendering.

### Search

Retain Orama because it already provides the current local-search behavior and
because repository policy prohibits ad hoc keyword matching for retrieval.
Depend on Orama directly instead of receiving it through Blume.

The generator should emit a minimal search record per public page: route,
title, description, headings, and plain-text body. Do not include hidden,
internal, or disabled agent-only source by accident. Load the search dialog and
Orama engine only on search intent, build or hydrate the in-browser index once,
then reuse it for the session. Orama supports typed schemas and browser-local
creation, insertion, and search:

- [Orama JavaScript documentation](https://docs.orama.com/docs/orama-js)
- [creating an index](https://docs.orama.com/docs/orama-js/usage/create)
- [inserting documents](https://docs.orama.com/docs/orama-js/usage/insert)

Do not add Pagefind while article pages are SSR routes rather than a separate
prerendered site. Do not put search behind a database, API, or authenticated
service for this corpus.

### Static and agent-readable artifacts

The generator must continue to produce:

```text
/docs/llms.txt
/docs/llms-full.txt
/docs/agent-readability.json
/docs/sitemap.xml
/docs/index.md
/docs/getting-started.md
/docs/...every other public page....md
```

Generate these into the Start client artifact during the build, not into a
second application. The existing monolith already serves exact files from the
Start client output before falling through to SSR. Extend its content-type map
to return `application/xml` for `.xml`; preserve `text/markdown` for `.md`,
`text/plain` for `.txt`, and JSON for the manifest. Add `HEAD` parity and
appropriate cache headers.

The Start package task graph should make generation an explicit prerequisite
of development, typecheck, tests, and build. The deployment bundle assertion
must verify that the docs manifest, every raw Markdown route, the two LLM files,
the readability manifest, and sitemap are present. A build that renders HTML
but drops agent artifacts is a failure.

If exact static asset serving proves awkward for `/docs` versus `/docs.md`, use
TanStack Start server routes with explicit `GET` and `HEAD` responses. Server
routes support wildcard paths and explicit response headers and status codes:
[TanStack Start server routes](https://tanstack.com/start/latest/docs/framework/react/guide/server-routes).
Do not introduce a second server merely for content types.

### SEO contract

Every article route should derive its head from validated metadata:

- unique title and description;
- canonical `https://openagents.com/docs/...` URL;
- Open Graph and Twitter metadata;
- `WebSite` data on the docs root;
- `TechArticle` JSON-LD on articles;
- stable `dateModified` only when source history can produce it
  deterministically; and
- `noindex` only for an explicitly modeled non-public page.

The sitemap and head canonicals must be tested against the same route manifest.
There must be no route that is public in navigation but absent from the
sitemap or raw Markdown set.

### Reader UI and accessibility contract

Port the current information architecture, not Blume's implementation:

- the top-left OpenAgents brand links to `/`;
- a separate Docs link points to `/docs`;
- the header remains pinned without scroll bounce;
- desktop navigation groups retain current order and collapse state;
- Future / Advanced stays visually and textually marked as future,
  experimental, removed, or deprecated capability rather than current MVP;
- desktop table of contents follows H2–H3 headings;
- mobile navigation is a real modal dialog with a visible close action, focus
  containment, Escape close, focus restoration, and background scroll lock;
- search is keyboard reachable and returns focus when closed;
- the active route is exposed semantically, not by color alone;
- all interactive targets remain at least 44 CSS pixels on coarse pointers;
- focus styles remain visible against the Khala background;
- reduced-motion preferences disable nonessential transitions; and
- code blocks remain horizontally operable without forcing whole-page
  overflow.

Use the existing website's sans font and Khala tokens rather than creating a
docs-only font stack. Keep Commit Mono for code and retain its license if the
font file moves. The initial implementation should load only the font weights
the reader uses; it should not reproduce Blume's ten generated WOFF2 assets.

The new React components remain a narrow docs renderer inside the existing
Start exception. This migration does not authorize a second visual system or a
new application-wide component authority.

## Dependency plan

### Add directly to the Start package

Use exact pins selected and proven during implementation:

- one Markdown pipeline (for example Unified with remark/rehype GFM support);
- a frontmatter parser;
- Shiki as a build-only highlighter;
- Orama as the lazy client-side search engine; and
- the repository's existing typed schema authority for validating frontmatter
  and the generated manifest.

Prefer the smallest coherent pipeline. Do not pull a full docs framework into
Start to recreate Blume indirectly. Do not add multiple Markdown parsers,
multiple search engines, or both Markdown and MDX compilers.

### Remove after cutover

Delete the docs package's direct and transitive authority for:

- `blume`;
- `astro` and Astro adapters;
- `@astrojs/mdx`;
- Blume's generated Astro project;
- docs-specific Tailwind/Vite integration;
- Blume's broad optional AI, MCP, OpenAPI, export, diagram, remote-content,
  and deployment dependency surface; and
- `@shikijs/twoslash` unless the migrated corpus begins using verified
  Twoslash examples.

At `1.0.4`, Blume declares 49 direct dependencies, including Astro
adapters, React, Orama, Shiki, AI/MCP, OpenAPI, export, diagram, and deployment
packages. OpenAgents currently uses only a small fraction of them. Directly
owning the necessary Markdown, highlighting, and search packages makes that
boundary reviewable.

Do not claim dependency savings until the final lockfile diff and production
bundle are measured. The meaningful win is reduced authority and fewer build
and deployment paths, not a speculative package-count headline.

## Infrastructure and task-graph retirement

After production parity is accepted, remove:

- the now-removed `apps/openagents.com/apps/docs` source application, after
  moving its curated content and any retained font/license assets;
- the docs `Dockerfile`, static server, deployment script, and service-specific
  tests;
- `dev:docs`, `build:docs`, `typecheck:docs`, `test:docs`, and `verify:docs`
  from the web package scripts, replacing them with Start-owned content checks;
- the `openagents_docs` Cloud Run module in
  [`infra/prod/main.tf`](../../infra/prod/main.tf);
- `docs_cloud_run_service` and `docs_host` inputs from the global load balancer
  module;
- the docs serverless NEG and backend service;
- the `/docs` and `/docs/*` URL-map rule; and
- the invariant and agent guidance that declares Blume the independent docs
  owner.

With the docs path rule gone, `openagents.com/docs` naturally reaches the
monolith's existing default backend. Keep host separation required by other
services, including Components, but do not retain a docs-only matcher.

The migration changes an invariant. Its implementation commit must update the
root [`INVARIANTS.md`](../../INVARIANTS.md), web
[`INVARIANTS.md`](../../apps/openagents.com/INVARIANTS.md), and web
[`AGENTS.md`](../../apps/openagents.com/AGENTS.md) language in the same change
that establishes the new route owner. The new invariant should say that
TanStack Start owns `/docs` and its generated public artifacts, the monolith is
the sole runtime authority, and `apps/start/content/docs` is the only curated
public docs source.

## Cutover sequence

### TS-DOCS-1 — content compiler and contract tests

- Copy, do not yet delete, the curated content into Start.
- Convert `.mdx` to `.md` only after proving there is no JSX.
- Add typed frontmatter, route, navigation, and artifact generation.
- Add snapshot tests for every current source document and URL.
- Fail on broken internal links, duplicate slugs, raw HTML, or unlisted public
  content.

Acceptance: one deterministic manifest produces all 12 pages, all navigation,
all raw Markdown, search records, agent artifacts, and sitemap entries.

### TS-DOCS-2 — native Start reader

- Add the `/docs` layout, index, and splat route.
- Port the Khala shell, navigation, mobile dialog, table of contents,
  pagination, and retained page actions.
- Add docs-scoped not-found handling and corrected compatibility redirects.
- Keep article SSR complete and route code split.

Acceptance: local browser coverage passes at narrow mobile, tablet, desktop,
keyboard-only, reduced-motion, and high zoom widths without horizontal page
overflow or inaccessible navigation.

### TS-DOCS-3 — search, SEO, and agent parity

- Add lazy Orama search.
- Add route head metadata and JSON-LD.
- Serve `.md`, `.txt`, `.json`, and `.xml` with correct status, content type,
  charset, and cache behavior.
- Add build assertions and link checks.

Acceptance: a route-by-route manifest comparison shows no missing public page,
raw mirror, canonical, search record, sitemap entry, or agent artifact.

### TS-DOCS-4 — monolith deployment and load-balancer switch

- Deploy the monolith with the new reader while the public load balancer still
  sends `/docs` to the old service.
- Smoke the monolith revision directly or through a temporary protected test
  path.
- Apply the Terraform change that removes the docs path split.
- Verify public status, headers, navigation, search, mobile drawer, redirects,
  raw Markdown, LLM files, readability JSON, sitemap, and unknown-route 404.

Acceptance: public `/docs` is served by the monolith and matches or improves
the accepted reader contract. Do not infer success from the homepage alone.

### TS-DOCS-5 — delete the old authority

- Observe the new route through the agreed rollback window.
- Remove the Blume/Astro app, package scripts, lockfile subtree, Cloud Run
  service module, NEG, backend, path rules, and obsolete tests.
- Update invariants and deployment documentation.
- Delete the old Cloud Run service only after Terraform and rollback evidence
  are accepted.

Acceptance: repository search finds no active Blume/Astro docs authority,
production has no `openagents-docs` service or route, and the monolith build is
the sole source of `/docs`.

## Verification and performance gates

The migration is complete only when all of these hold:

1. Every current public page returns 200 at the same canonical URL.
2. Unknown docs routes return a real 404 within the docs shell.
3. All compatibility aliases return a real redirect to the correct target.
4. The top-left brand returns to `/`; Docs returns to `/docs`.
5. Product Promises is not a human top-level link and remains represented in
   the agent-facing material.
6. Future / Advanced topics retain their status warnings.
7. Raw Markdown and all agent artifacts are byte- or semantically equivalent
   after intentional metadata normalization.
8. Search indexes every and only public document and works after client-side
   navigation.
9. Navigation after the initial load does not perform a full document reload.
10. The mobile drawer opens, traps focus, closes by close button and Escape,
    restores focus, and does not leave the page darkened or scroll locked.
11. The header remains pinned without elastic scroll displacement.
12. A keyboard-only pass reaches search, navigation, article actions, table of
    contents, and pagination with visible focus.
13. Lighthouse accessibility and best-practice scores do not regress from the
    accepted live baseline; automated results are supplemented by manual focus
    and screen-reader landmark checks.
14. Initial docs JavaScript is no larger than the current approximately 40 KB
    uncompressed baseline without a documented reason.
15. Search code remains absent from the initial route chunk and its opened
    transfer is no larger than the current approximately 66 KB Orama module
    without a documented reason.
16. No app, workroom, query cache, or authentication implementation chunk is
    loaded by an anonymous docs route.
17. The build emits only the required font files and does not reproduce the
    current ten generated WOFF2 assets.
18. Cold and warm TTFB, first contentful paint, largest contentful paint,
    interaction latency, route-navigation latency, transferred bytes, and
    request count are recorded before and after on the same routes and network
    profile.
19. Monolith health and API first-refusal behavior are unchanged.
20. Terraform plan removes only docs-specific infrastructure and does not
    disturb unrelated host routing.

The performance goal is not “React is faster than Astro.” The goal is that
SSR documents stay complete, article navigation becomes same-router
navigation, search remains lazy, unrelated application code stays split out,
and an entire build/runtime/deployment boundary disappears. Measure those
claims rather than relying on framework reputation.

## Findings by severity

### P0 — blockers

None observed in the audited source. The corpus does not depend on Blume-only
content components, so there is no known rendering blocker.

### P1 — must resolve before cutover

1. **Route authority is currently split.** Terraform intercepts `/docs` before
   the monolith. Start code alone cannot change production ownership.
2. **Three compatibility aliases currently lose the `/docs` base.** The new
   routes must correct them and return HTTP redirects.
3. **Public-source curation is a security boundary.** A repository-wide docs
   glob could publish internal audits, runbooks, or evidence. The generator
   must hard-code the one curated source root.
4. **Compiled HTML needs an explicit trust model.** Reject raw HTML initially;
   never pass remote or user content through the trusted compiler path.
5. **Deletion must follow live parity.** Removing Blume before the monolith is
   publicly routed and verified would turn rollback into a rebuild.

### P2 — required quality work

1. Preserve mobile focus containment, Escape close, focus restoration, and
   scroll locking; appearance alone is insufficient.
2. Keep search and article modules lazy so anonymous docs do not inherit the
   authenticated app bundle.
3. Add exact content types, `HEAD`, caching, canonical, sitemap, JSON-LD, and
   raw-artifact tests.
4. Prune fonts and verify code highlighting does not ship Shiki to the browser.
5. Test navigation latency after client routing and after a hard refresh on
   every nested Future route.
6. Add a docs-scoped error/not-found state that does not expose an application
   login surface.

### P3 — optional refinement

1. Remove the generic outbound “open in chat” submenu unless a later product
   requirement defines consent, target behavior, and content disclosure.
2. Reduce generic pill and card styling while keeping current hierarchy.
3. Consider prebuilding the Orama index if first-search construction becomes
   measurable; do not optimize before profiling.

## Positive findings to preserve

- The current content tree is curated and small.
- Human-facing MVP documentation is already separated from Future / Advanced
  material.
- Product Promises has already been removed from top-level human navigation.
- Agent-readable outputs are first-class rather than an afterthought.
- Search is local and lazy.
- Syntax highlighting is build-time.
- The Khala adaptation is substantially more appropriate than stock Blume.
- The reader has semantic landmarks and a functioning responsive information
  architecture.
- The monolith already serves Start assets before SSR and already uses
  route-level splitting and intent preloading.

These strengths belong to the OpenAgents documentation contract, not to
Blume. Preserve them while deleting the framework boundary.

## Rollback

Keep the last known-good `openagents-docs` revision, NEG, backend, and URL-map
configuration until the unified reader passes the observation window. If the
Start reader fails after the path switch, restore the Terraform `/docs` rule to
the old backend; do not perform an ad hoc console edit. The content sources
remain duplicated only during this bounded transition.

Once TS-DOCS-5 deletes the old service and package, rollback becomes a normal
repository revert and monolith deploy. Record the accepted monolith revision,
Terraform plan/apply, route probe results, and deletion revision in the final
receipt.

## Final recommendation

Proceed with the unified TanStack Start reader in five bounded slices. The
current Blume site proved the desired content and reader contract, but its
separate Astro application and Cloud Run topology are no longer justified.
Build a small owned Markdown compiler, native Start route shell, lazy Orama
search, and generated static artifact set; switch the load balancer only after
direct monolith parity; then delete Blume, Astro, and the docs service.

The result should be one web framework, one application build, one Cloud Run
runtime, one public content tree, and one route authority for all of
`openagents.com`, with `/docs` remaining a fast, accessible, agent-readable
documentation experience rather than becoming application chrome.

For implementation, the recommended UI passes are: shape the owned docs shell,
harden responsive and accessibility edge cases, optimize route/search/font
loading against the recorded baseline, polish only after those gates pass, and
then re-run the technical reader audit to verify the fixes.
