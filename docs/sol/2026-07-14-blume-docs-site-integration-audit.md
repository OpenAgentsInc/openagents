# Blume docs-site integration audit

- Date: 2026-07-14
- Snapshot: OpenAgents `437d3f199c3984d93a1f0cf6eaaa61f7385a1a74`;
  Blume `3c07baf052180e3787427e4b32dbaefd7c641cc4`
- Class: architecture and dependency audit
- Status: recommendation; no implementation or deployment authority
- Dispatch: no; file a bounded post-VP-4 implementation issue and claim
- Owner: OpenAgents docs architecture
- Final disposition: retain until Blume is adopted, rejected, or superseded
- Decision: conditionally adopt Blume for a static `/docs` site after the
  atomic VP-4 authority cutover
- Toolchain authority: [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777)

## Executive decision

Blume is a good **conditional** fit for OpenAgents documentation, but not as a
second application runtime and not before the current toolchain program reaches
its atomic cutover.

Use Blume as an exact-pinned, build-only dependency that turns a curated public
Markdown/MDX tree into static files. Serve that complete static artifact from a
minimal, independently deployable docs service and route only
`https://openagents.com/docs` plus `/docs/*` to it through the existing Google
Cloud load balancer. Keep the OpenAgents application, API, agent discovery
surfaces, authentication, and runtime authority in their current owners. Do not
deploy Blume's server adapter, Ask AI endpoint, MCP server, or remote content
adapters in the first release.

This is a conditional recommendation because four facts need explicit handling:

1. OpenAgents has selected Node, pnpm 11, and one exact build-core identity, but
   the atomic conversion is not implemented yet.
2. Blume's published CLI runs on Node and its site is Astro-based, while Blume's
   own source build and all 81 unit-test files still use the outgoing runtime.
   Consuming the published package can be Node-only; vendoring or forking its
   source into this workspace cannot.
3. Blume generates a hidden Astro project. That generated config must prove it
   resolves the exact build core selected by VP-4 and remains subordinate to
   the root task graph. If it creates independent build authority, adoption
   stops or the project is ejected into an owned, reviewed Astro config.
4. OpenAgents currently mandates Effect Native for new owned UI. Blume is an
   Astro/Tailwind documentation renderer, so implementation needs a narrow,
   documented exception for the static docs reader. It must not become a general
   escape hatch for application UI.

## Direct answer: can a different codebase serve `openagents.com/docs/`?

Yes. A URL path is owned by the domain's front door, not by the repository or
framework that renders the rest of the hostname. The front door can send the
default route to the OpenAgents app and `/docs` plus `/docs/*` to a separately
built and deployed docs service. Both still appear to users as the same origin,
`https://openagents.com`; browser URLs do not expose the backend split.

There are three practical shapes:

| Shape | Same public origin | Operational cost | Recommendation |
| --- | --- | --- | --- |
| Separate static docs service behind the existing apex load balancer | Yes | Medium; independent deploy and rollback | **Preferred** |
| Separate docs build, copied under the monolith's static artifact at `docs/` | Yes | Lower infrastructure count; coupled deploy and rollback | Valid fallback |
| Blume server runtime embedded into the product server | Yes | Highest; combines failure and secret boundaries | Do not use |

Google documents path-based routing to Cloud Run services through an external
Application Load Balancer and serverless network endpoint groups. Cloud Run's
simple domain mapping cannot map a domain to a path such as `/docs`; an
Application Load Balancer or another path-aware front door is required for the
separate-service shape:

- [Cloud Run custom domains](https://docs.cloud.google.com/run/docs/mapping-custom-domains)
- [Serverless network endpoint groups](https://docs.cloud.google.com/load-balancing/docs/negs/serverless-neg-concepts)
- [URL maps](https://docs.cloud.google.com/load-balancing/docs/url-map-concepts)

OpenAgents already has that front door. The production Terraform provisions a
Global External Application Load Balancer, a serverless NEG for
`openagents-monolith`, and an explicit URL map whose path matcher currently
defaults every request to the monolith:

- [`infra/modules/global-external-lb/main.tf`](../../infra/modules/global-external-lb/main.tf)
- [`infra/prod/main.tf`](../../infra/prod/main.tf)
- [`apps/openagents.com/AGENTS.md`](../../apps/openagents.com/AGENTS.md)
- [current Cloud Run deployment script](../../apps/openagents.com/workers/api/scripts/deploy-cloudrun.sh)
- [domain cutover runbook](../cloud/2026-07-06-openagents-domain-cutover-runbook.md)

Extend that Terraform rather than making a manual `gcloud` route or adding a
new proxy tier. Provision an `openagents-docs` Cloud Run shell, serverless NEG,
and backend service; route exact `/docs` and `/docs/*` to it; and leave the
monolith as the default. The current host rule groups `openagents.com` and
`auth.openagents.com` into one matcher, so the change must split host matchers:
docs routing belongs only to `openagents.com`, while `auth.openagents.com`
continues to default wholly to the monolith.

The recommended first release therefore keeps the docs codebase, deployment,
and rollback separate while exposing one same-origin product. The docs runtime
is only a minimal Node static-file server around the generated artifact: no
Blume/Astro server output, application database, application secrets, or
dynamic docs endpoints.

## What was audited

The review covered Blume's repository history, package manifests, generated
Astro topology, content graph, base-path implementation and tests, deployment
adapters, static artifact generation, local search, Markdown/MDX components,
OpenAPI generation, agent-readable output, CI/release workflows, license, and
current upstream issues. It also covered OpenAgents' current route ownership,
existing `/docs` implementations, the TanStack Start/Cloud Run scaffold, the
live Sol toolchain issue set, and the application invariant ledger.

Important Blume sources are pinned to the audited commit:

- [README and architecture](https://github.com/haydenbleasel/blume/blob/3c07baf052180e3787427e4b32dbaefd7c641cc4/README.md)
- [`blume` package manifest](https://github.com/haydenbleasel/blume/blob/3c07baf052180e3787427e4b32dbaefd7c641cc4/packages/blume/package.json)
- [deployment guide](https://github.com/haydenbleasel/blume/blob/3c07baf052180e3787427e4b32dbaefd7c641cc4/apps/docs/content/docs/02-deployment.mdx)
- [`basePath` implementation](https://github.com/haydenbleasel/blume/blob/3c07baf052180e3787427e4b32dbaefd7c641cc4/packages/blume/src/core/base-path.ts)
- [`basePath` feature tests](https://github.com/haydenbleasel/blume/blob/3c07baf052180e3787427e4b32dbaefd7c641cc4/packages/blume/test/base-path-feature.test.ts)
- [generated Astro config template](https://github.com/haydenbleasel/blume/blob/3c07baf052180e3787427e4b32dbaefd7c641cc4/packages/blume/src/astro/templates.ts)
- [MIT license](https://github.com/haydenbleasel/blume/blob/3c07baf052180e3787427e4b32dbaefd7c641cc4/LICENSE)

## Why Blume fits

### The product shape is right

Blume takes Markdown or MDX, generates an Astro project under `.blume/`, and
emits static HTML. Its core theme is Astro-first and does not require a client
framework bundle for ordinary pages. That is a strong match for documentation:
fast documents, stable URLs, minimal runtime authority, and content stored next
to the code it describes.

Useful first-party capabilities include:

- generated navigation and typed configuration;
- Markdown/MDX components for steps, tabs, cards, code groups, diffs, file
  trees, type tables, and component examples;
- local search with Orama by default or Pagefind as an alternative;
- static raw-Markdown mirrors, `llms.txt`, `llms-full.txt`, and an
  `agent-readability.json` manifest;
- sitemap, canonical, Open Graph, JSON-LD, RSS, redirect, link-validation, and
  internationalization support;
- optional OpenAPI/AsyncAPI rendering; and
- static output that does not need Blume, Astro, or a build tool in the
  production runtime.

These are materially better than the current hand-maintained in-code docs
arrays. Both the legacy Foldkit app and the new Start scaffold duplicate a small
fixed slug catalog in TypeScript. The Start implementation supports only
`/docs` and one-segment `/docs/$slug`; it is not a scalable authoring system for
nested references, cross-links, generated navigation, search, or code samples.

### Subpath support is first-class

Blume distinguishes two concepts:

- `deployment.base` moves the **whole Blume application** under a host
  subdirectory; and
- `basePath` moves only generated content routes while a Blume project still
  owns other routes at its root.

The recommended OpenAgents topology is a separate docs build mounted wholesale
at `/docs`, so it should use:

```ts
deployment: {
  base: "/docs",
  output: "static",
  site: "https://openagents.com",
}
```

The build output is then served unchanged by the path-routed docs service, or
copied as a unit into the OpenAgents host's `docs/` static directory in the
fallback design. Source pages must live at the content root, not in a second
`content/docs/` directory. Do **not** also set `basePath: "/docs"`; either choice
would produce `/docs/docs/...`.

If OpenAgents instead makes Blume itself own `/` plus custom non-docs pages in a
single Blume project, `basePath: "/docs"` is the right setting. That topology is
not recommended because Blume must not replace the OpenAgents application.

Blume has focused regression coverage for both path layers, including Markdown
links, navigation, redirects, search, sitemap, canonical URLs, OG URLs,
`llms.txt`, raw Markdown negotiation, and composed bases. This reduces, but does
not eliminate, the need for an OpenAgents end-to-end subpath smoke.

### The license is compatible

Blume is MIT licensed. Using the npm package is straightforward. If OpenAgents
later runs `blume eject` or copies source components, preserve the license and
attribution in the relevant third-party notice because the copied implementation
will live in the OpenAgents tree.

## Why adoption must be bounded

### Upstream is very young and changing quickly

The audited repository began in June 2026, reached `1.0.0` on July 13, and was
already at `1.0.3` the same day. At the snapshot it had 513 commits, 327 stars,
12 forks, 81 unit-test files, one dominant human contributor, and two open
issues. The audited `main` CI, release, and CodeQL runs were green. Playwright,
accessibility, and visual tests exist, but the main CI workflow does not run
those suites. This is still an early, high-churn dependency rather than a
settled documentation platform.

The package should therefore be exact-pinned—not `^1.0.3`—with lockfile
integrity, a generated-output diff, accessibility smoke, link validation, and a
human-reviewed upgrade cadence. Do not auto-merge dependency updates.

### Blume's development topology conflicts with the Node destination

The published `blume` CLI is bundled with a Node shebang and declares Node
`>=22.12.0`; that consumer path aligns with OpenAgents' Node 24 destination.
However, upstream uses the outgoing runtime as its package-manager authority,
build bundler, test runner, and CI bootstrap. Its build script calls that
runtime's native bundler, and every package unit test imports its native test
runner.

Consequences:

- consume the published package; do not vendor the Blume monorepo;
- do not copy its root lockfile, scripts, test runner, task-orchestrator config,
  lint config, or GitHub Actions into OpenAgents;
- invoke `blume build --strict`, `blume check`, and `blume validate` only
  through the canonical root task graph; and
- require a clean machine with only the pinned destination bootstrap to install
  and build the docs package.

### Generated config can hide split-brain build authority

Blume does not consume a checked-in project build config. It generates an Astro
config that imports Tailwind's build integration, configures plugins and SSR
behavior, and drives Astro programmatically. The package's dependencies use
wide ranges such as `astro ^7.0.2`, TypeScript `^6.0.3`, and Tailwind build
plugins.

VP-4 requires one exact build-core identity, one pnpm lockfile, one root task
graph, and reviewed package-local host exceptions. A Blume proof must show:

1. the generated project resolves the exact core alias selected by VP-4;
2. Astro, Tailwind, React (if enabled), and Blume run without a second build or
   test-core identity;
3. the docs build is a static `vp run` task with declared inputs and outputs;
4. no Blume hook, formatter, test runner, package manager, or cache becomes
   independent repository authority; and
5. `.blume/` is generated/ignored and never treated as hand-owned source.

If the hidden project cannot satisfy those checks, use `blume eject` once and
own the resulting Astro config as the explicit docs host config, or reject
Blume. Do not weaken the root build contract to make it fit.

### Cloudflare server output is not ready evidence

Blume advertises a Cloudflare server adapter, but upstream issue
[#34](https://github.com/haydenbleasel/blume/issues/34) reports that `1.0.3`
fails during Cloudflare prerendering on a `node:path` import. This does not block
the recommended static build. It is a direct reason not to make Blume server
output or a new Blume Worker part of the first integration.

### The package is broad

The `1.0.3` tarball is about 3.5 MB unpacked and declares a wide dependency set:
Astro, React, AI SDK/MCP, Scalar, Orama, Pagefind, Shiki, Mermaid, KaTeX,
Tailwind, Takumi native packages, EPUB/PDF tooling, and multiple optional hosted
search/content providers. Not all of that reaches client JavaScript, but it
expands install and supply-chain surface.

The proof must record install size, production HTML/CSS/JS bytes, native package
behavior on owned runners, license inventory, and the actual reachable client
bundle. Feature defaults are not a substitute for a bundle and SBOM check.

## OpenAgents constraints from the live issue set

No current open issue mentions Blume, proposes a docs site, or authorizes a
`/docs` cutover. The issue set supplies constraints:

- [#8777](https://github.com/OpenAgentsInc/openagents/issues/8777) decides the
  full Node, pnpm, and unified-build replacement. It is not an additive pilot.
- [#8794](https://github.com/OpenAgentsInc/openagents/issues/8794) is closed.
  Its committed VP-0 receipt freezes outgoing-runtime/direct-tool growth and
  selects exact Node `24.13.1`, pnpm `11.10.0`, and build core `0.2.4` pins, but
  intentionally changes no runtime or package-manager authority.
- [#8796](https://github.com/OpenAgentsInc/openagents/issues/8796) owns the
  Node-native conversion of retained services and public CLIs. The proposed
  static docs server must use that owned Node platform seam, not preserve the
  outgoing runtime.
- [#8797](https://github.com/OpenAgentsInc/openagents/issues/8797) owns test and
  typecheck parity. A docs package cannot introduce a second test authority.
- [#8798](https://github.com/OpenAgentsInc/openagents/issues/8798) owns the
  atomic workspace-authority cutover. Blume must enter that exact topology, not
  land a transitional lockfile or config now.
- [#8799](https://github.com/OpenAgentsInc/openagents/issues/8799) owns web,
  Cloudflare, host, release, clean-image, and rollback stabilization. A docs host
  smoke belongs here after VP-4.
- [#8800](https://github.com/OpenAgentsInc/openagents/issues/8800) requires zero
  supported outgoing-runtime references in current instructions, scripts,
  examples, and release paths.
- [#8772](https://github.com/OpenAgentsInc/openagents/issues/8772),
  [#8773](https://github.com/OpenAgentsInc/openagents/issues/8773), and
  [#8774](https://github.com/OpenAgentsInc/openagents/issues/8774) are folded
  build/lint/format leaves whose final authority belongs to VP-4. They do not
  authorize a Blume-local runner or configuration island.
- [#8795](https://github.com/OpenAgentsInc/openagents/issues/8795) removes the
  non-MVP payment, wallet, credit, payout, and settlement product graph. New docs
  must not republish those capabilities.

The migration order is therefore:

```text
VP-0 landed -> VP-1/VP-2 -> VP-3 -> VP-4 atomic cutover
            -> Blume compatibility proof and static docs workspace
            -> VP-5 host integration and smoke -> content/route launch
```

VP-0 being complete does not authorize adding Blume, Astro, a docs package, a
lockfile, or direct build configuration. Wait for VP-4's one-lockfile atomic
cutover, then prove Blume inside that destination topology.

## Current OpenAgents `/docs` state

There are two legacy/transition implementations to retire rather than preserve
as architecture:

- `apps/openagents.com/apps/web/src/page/docs.ts` contains a fixed Foldkit docs
  catalog and a one-segment route policy.
- `apps/openagents.com/apps/start/src/routes/docs/` recreates the catalog as
  TanStack Start routes backed by `-funnel-data.ts`.

The new Start scaffold has a Cloud Run container path, but its current container
and server still use the outgoing runtime. That is migration evidence, not the
accepted destination. Its static server checks only exact files before falling
through to Start; it does not resolve `/docs` or `/docs/guide` to directory
`index.html` files. A coalesced Blume artifact therefore needs an owned
clean-URL resolver after the Node cutover.

The current docs catalogs also contain content made stale by the accepted MVP
pivot: “get paid to code,” paid-plan, credits, Bitcoin earning, markets,
settlement, hosted Sites, Autopilot, and other retired or non-retained claims.
Do not bulk-convert those arrays to MDX. Begin with a fresh, curated information
architecture grounded in the post-VP-1 ProductSpec and current public
capability/receipt authorities.

There is a second policy conflict to resolve. The repository contract currently
lists `/`, `/forum`, Forum descendants, and `/promises` as the retained public
product routes, while preserving stable report/docs evidence links. It also
requires new owned UI to use Effect Native wherever a renderer exists. Launching
a general `/docs` site through Astro/Blume changes both policies. The
implementation commit must update `AGENTS.md` and the relevant invariant or
model-boundary note to state:

- `/docs` and `/docs/*` are retained public documentation routes;
- the Blume/Astro static renderer is a docs-only third-party presentation
  exception;
- it grants no application, authentication, tool, API, payment, deployment, or
  public-claim authority; and
- owned application UI remains Effect Native.

## Recommended repository shape

After VP-4, create a build-only workspace such as:

```text
apps/openagents.com/apps/docs/
  package.json
  blume.config.ts
  content/
    index.mdx
    guides/
  public/
  theme.css
```

It remains part of the single `openagents.com` product surface; it is not a
fourth product application and does not own API/runtime authority. Keep content
in this dedicated public tree. Do not point Blume at the repository-wide
`docs/` directory, which mixes public product material with internal plans,
historical audits, operational runbooks, transcripts, and evidence that is not
an appropriate public navigation corpus.

An initial configuration should be intentionally narrow:

```ts
import { defineConfig } from "blume"

export default defineConfig({
  title: "OpenAgents Docs",
  description: "Build, operate, and verify with OpenAgents.",
  deployment: {
    base: "/docs",
    output: "static",
    site: "https://openagents.com",
  },
  content: {
    root: "content",
  },
  search: { provider: "orama" },
  theme: {
    mode: "dark",
    radius: "sm",
  },
  ai: {
    ask: { enabled: false },
    mcp: { enabled: false },
    llmsTxt: true,
  },
  feedback: false,
  seo: {
    og: { enabled: false },
    robots: false,
    contentSignals: {
      search: true,
      aiInput: true,
      aiTrain: false,
    },
  },
  github: {
    owner: "OpenAgentsInc",
    repo: "openagents",
    dir: "apps/openagents.com/apps/docs",
  },
})
```

Notes:

- Keep local Orama search. Hosted semantic search introduces credentials,
  indexing mutations, network dependence, and another privacy boundary.
- Treat `mode: "dark"` as an initial preference, not enforcement. Blume always
  renders a theme toggle and honors a saved light preference. The candidate
  must override that header control and map both token branches to the existing
  OpenAgents dark palette, or eject the layout if the override seam cannot
  enforce the repository's dark-only contract.
- Self-host Commit Mono from an owned artifact and map Blume's body, display,
  and mono tokens deliberately. Do not silently accept its default
  Inter/Inter Tight/IBM Plex Mono identity or a build-time Google Fonts fetch.
- Keep Ask AI and MCP off. OpenAgents already owns canonical agent discovery
  and model/tool authority; a docs generator must not create a parallel one.
- `llms.txt`, raw Markdown mirrors, and agent readability are useful static
  documents under `/docs/`. Link them from the canonical root agent surface
  rather than replacing it.
- Root `robots.txt` remains authoritative. Either merge Blume's sitemap into the
  root sitemap/index or have the root sitemap enumerate the docs routes.
- Set content signals explicitly rather than inheriting a changing generator
  default. The proposed policy allows search and user-requested AI input while
  declining model-training use; legal/product owners should approve that policy
  before launch.
- Defer generated OG images until Takumi native-package behavior is proven on
  the Node/pnpm owned-runner matrix.
- Use a checked-in OpenAPI snapshot generated from the retained post-VP-1 public
  API if API reference pages are added. Do not fetch a mutable production spec
  during a supposedly reproducible build.

## Preferred same-origin integration

The docs task should produce a static `dist/`, and an independently deployable
`openagents-docs` Cloud Run service should expose only that artifact. Implement
the service and its routing in Terraform with the existing Cloud Run and load
balancer modules:

```text
openagents.com/docs       -> openagents-docs backend
openagents.com/docs/*     -> openagents-docs backend
openagents.com/*          -> openagents-monolith backend (default)
auth.openagents.com/*     -> openagents-monolith backend (default)
```

Preserve the incoming `/docs` prefix all the way to the static server; do not
strip and reconstruct it in application code. Restrict the service to static
GET/HEAD responses, give it no application secrets or database attachment, and
apply the same public security headers. The service must resolve:

- `/docs` and `/docs/` to `docs/index.html`;
- `/docs/<path>` to `docs/<path>/index.html` (and, if emitted, exact files);
- `/docs/_astro/<hash>.*` with immutable cache headers;
- HTML and Markdown with short/no-cache revalidation appropriate to releases;
- `.md`, `.xml`, `.json`, fonts, images, JavaScript, CSS, and any search assets
  with correct content types; and
- a missing docs route to the docs 404 behavior, not the application SPA.

Delete the Start `/docs` and `/docs/$slug` placeholders when the static route is
cut over. Preserve intentionally stable legacy paths with explicit redirects or
replacement documents. Update every typed route inventory, navigation policy,
funnel budget, sitemap, and client/server route-agreement test in the same
change. Preserve `/docs/product-promises` in particular: it is already a stable
document path in repository policy and must have an explicit replacement or a
higher-priority route to its existing owner before the `/docs/*` catch-all
lands.

Manage every service, NEG, backend, host matcher, path rule, and rollback in
Terraform. Do not create an untracked manual `gcloud` URL-map mutation. This
shape still needs coordinated acceptance because same-origin cookies, Content
Security Policy, HSTS, cache policy, robots/sitemap, observability, and incident
ownership span both services.

## Fallback: coalesce artifacts in the monolith

If a second Cloud Run service is rejected on operational grounds, the docs
build can instead copy its complete output beneath the monolith's client
artifact at `docs/`. This saves a service and URL-map backend but couples docs
deploys and rollbacks to the application and requires the monolith's Node host
to implement the same clean-URL, 404, content-type, cache, traversal, and
containment behavior.

Use an explicit collision and containment check before copying. The docs build
may write only beneath `docs/`; it may not replace root assets, agent surfaces,
API documents, service workers, manifests, or application HTML. Treat this as a
fallback, not a reason to embed Blume server output in the product server.

## Content launch scope

Start small and factual:

1. overview and terminology;
2. install/use the retained OpenAgents product;
3. connect Codex and operate the retained workroom/fleet path;
4. public API and agent-readable surfaces that remain after VP-1;
5. security, privacy, and authority boundaries;
6. troubleshooting; and
7. links to product promises and evidence without copying their mutable state
   into prose.

Product promise status must remain sourced from the promise registry and its
receipts. Blume pages may explain the system and link to live state; they must
not turn build-time Markdown into a second truth source for readiness, payment,
settlement, deployment, or acceptance claims.

## Frontend fit audit

This is a source-level readiness audit of Blume's default renderer at the pinned
commit, not acceptance of an OpenAgents candidate artifact. No branded Blume
build exists yet, so browser scores must be rerun against the post-VP-4 package
before launch.

| Dimension | Score | Evidence and principal gap |
| --- | ---: | --- |
| Accessibility | 3/4 | Semantic landmarks, skip link, focus-visible rules, reduced-motion handling, and axe coverage exist; upstream does not run its browser suite in main CI and several controls are below the desired 44px coarse-pointer target. |
| Performance | 3/4 | Ordinary pages are static Astro with little client JavaScript and bounded 42rem prose; the broad package/search/font/code surface still needs measured production budgets. |
| Responsive design | 3/4 | Sidebar collapse, inert/`aria-hidden` drawer behavior, overflow containment, RTL handling, and a 480px Playwright case exist; 320px, landscape, text zoom, coarse pointer, and long-label cases are unproven. |
| Theming | 2/4 | Blume has coherent OKLCH tokens and light/dark branches, but its default typography, radius, system theme, and always-present theme toggle do not satisfy the OpenAgents dark-only mono system. |
| Anti-patterns | 2/4 | The information architecture is conventional and usable, but default pill navigation, rounded surfaces, card components, backdrop blur, and the glass/shadow search dialog read as generic SaaS docs rather than an OpenAgents command surface. |
| **Total** | **13/20** | **Acceptable foundation; substantial branded integration proof required.** |

Anti-pattern verdict: the default does not look broken or obviously generated,
but it is not distinctive or on-brand enough to ship unchanged. Avoid solving
that mismatch with decorative novelty. The target should remain a quiet,
high-density reader: pure-black foundations, warm off-white text, Commit Mono,
subtle borders, restrained state accents, and documentation-native hierarchy.

Priority findings:

- **P1 — dark-only contract is not enforced by config.** `theme.mode: "dark"`
  sets an initial mode, but the header always exposes a toggle and the startup
  script honors a saved light selection. Replace the Header through Blume's
  owned override seam and prove both token branches cannot leave the approved
  OpenAgents palette. If that cannot be done without fragile selectors, eject
  or reject the renderer.
- **P1 — upstream browser assurance is not a release gate.** Blume contains
  Playwright tests for axe structure, content contrast, skip-link focus,
  reduced motion, mobile drawer, search, components, and screenshots, but its
  main CI runs unit checks and a build only. OpenAgents must run its own
  candidate-site axe, keyboard, contrast, visual, and responsive suite on every
  docs change.
- **P1 — coarse-pointer targets need an owned policy.** Header icon/search
  controls are commonly 36px and several navigation rows are shorter. Preserve
  the compact desktop density, but raise interactive targets to at least 44px
  under coarse-pointer/mobile media and test the full drawer/search path by
  touch and keyboard.
- **P2 — the default visual grammar conflicts with the product register.** The
  stock search modal uses heavy blur and `shadow-2xl`; pill links and rounded
  cards are common. Override these into the existing pane/strip/register
  vocabulary and use content cards only when they are the correct semantic
  affordance.
- **P2 — responsive coverage is too narrow.** Add 320px and 480px portrait,
  small-landscape, tablet, desktop, 200% text zoom, long navigation labels,
  large code/table overflow, LTR/RTL, and no-JavaScript cases.
- **P2 — performance is a measured gate, not a framework claim.** Record HTML,
  CSS, client JavaScript, font, syntax-highlighting, image, and Orama-index
  bytes; verify self-hosted font caching and page/search interaction metrics on
  low-end mobile hardware.

Positive findings to preserve are the bounded prose measure, semantic page
landmarks, visible keyboard focus, skip navigation, reduced-motion support,
responsive sidebar/TOC structure, local search option, static-first output, and
the token override seam. These make adaptation plausible; they do not make the
default theme acceptable.

## Required proof before adoption

File a bounded implementation leaf after VP-4 and require all of the following:

### Toolchain proof

- exact `blume` version and npm integrity locked in the one pnpm lockfile;
- install, `blume build --strict`, check, and validate succeed with the pinned
  destination bootstrap and no outgoing-runtime executable present;
- generated Astro config resolves the exact selected build-core identity;
- one root `vp run` task owns build/check/validate and declares inputs/outputs;
- no second formatter, linter, test runner, lockfile, hook, or cache authority;
  and
- clean artifact/SBOM/license receipts.

### Static-host proof

- `/docs`, nested pages, trailing-slash policy, assets, raw Markdown, search,
  canonical tags, sitemap integration, and a real 404 pass on the candidate
  host;
- apex application, Forum, promises, auth, API, and root agent surfaces are
  byte/behavior unaffected outside the declared navigation and sitemap change;
- direct deep links work without SPA fallback;
- cache headers and content types are correct;
- path traversal and encoded-path tests fail closed;
- Terraform plan captures the docs service/NEG/backend and host-specific route
  split with no manual infrastructure drift; and
- rollback can restore `/docs{,/*}` to the monolith without disturbing the
  application default or `auth.openagents.com`.

### Product and accessibility proof

- no payment, wallet, credit, payout, settlement, deprecated product, or false
  availability claim survives the content review;
- no internal runbook, secret-shaped text, private trace, or operational
  topology enters the public content tree;
- keyboard navigation, focus, landmarks, heading order, contrast, reduced
  motion, mobile layout, and screen-reader smoke pass;
- dark-only token enforcement, Commit Mono self-hosting, 44px coarse-pointer
  targets, 200% text zoom, long-label overflow, and the full viewport matrix
  pass without the stock theme toggle or generic glass/pill treatment;
- link validation has zero unexplained internal failures; and
- performance budgets record HTML/CSS/JS and search-index size rather than
  relying on Blume's “fast by default” claim.

## Go/no-go rule

Proceed with a post-VP-4 static proof. Adopt Blume only if it remains a normal
exact-pinned workspace dependency, resolves the one selected build core,
produces a contained `/docs` artifact, and passes the route/content gates above
without the outgoing runtime or a second runtime.

Reject or eject Blume if it requires independent build/package-manager
authority, server output, Cloudflare-only behavior, unbounded source exposure,
or weakening the Effect Native and public-claim contracts. The value is its
documentation pipeline; none of its implementation choices are important
enough to override OpenAgents repository law.
