# TanStack Start unified documentation migration receipt

- Class: receipt
- Date: 2026-07-15
- Status: complete
- Dispatch: no; closure evidence for #8854
- Implements: `2026-07-15-tanstack-start-unified-docs-migration-audit.md`
- Owner: OpenAgents web and public documentation

## Result

`https://openagents.com/docs` and every retained child route are now owned by
the existing TanStack Start application and served by `openagents-monolith`.
There is no documentation-specific application runtime, Cloud Run service,
load-balancer backend, serverless NEG, workspace package, lockfile graph, or
deployment script.

The migration preserved the curated public content contract while replacing
Blume/Astro rendering with a Start-owned reader. Human navigation, local
search, page metadata, table of contents, pagination, code copy, raw Markdown,
edit links, mobile navigation, and the fixed header remain available. The
agent contract remains available as raw Markdown, `search.json`, `llms.txt`,
`llms-full.txt`, `agent-readability.json`, and `sitemap.xml`.

## Landed implementation

- Commit `56cfb0823f` added the typed Markdown compiler, 12 curated source
  pages, generated lazy page modules, Start routes, reader UI, local Orama
  search, SEO metadata, agent artifacts, redirects, and route/artifact tests.
- Commit `a9a82255fb` made the monolith the sole `openagents.com` and
  `auth.openagents.com` load-balancer owner and removed the docs-specific path
  rule from configuration.
- The commit containing this receipt removes the Blume workspace and package
  graph, retires the old service shell, updates authority documents, fixes the
  nested Cloud Build context, and makes the mounted docs navigation links
  render-preload all 12 small page modules.

The public content source is now exactly
`apps/openagents.com/apps/start/content/docs`. The compiler rejects raw HTML,
validates frontmatter and internal links, emits deterministic generated
artifacts, and exposes a strict `--check` mode. Repository-wide internal docs,
audits, runbooks, transcripts, and evidence remain outside the public tree.

## Production cutover

Cloud Build `72779ee2-8f35-4107-a1d4-4286597756ed` produced the first verified
unified revision, `openagents-monolith-00138-ndg`. The URL map was then changed
atomically so `openagents.com` and `auth.openagents.com` both use the monolith
matcher with no `/docs` exception.

Cloud Build `2e18c4fe-4c87-47b6-85f9-7884a0496632` and nested container build
`719ed763-40c4-47c5-80ef-d7fc99d14784` produced the final preload-enabled
revision, `openagents-monolith-00139-4f4`, at 100 percent traffic. Direct Cloud
Run and public-origin checks both selected its new `index-C5YIloUc.js` client
asset.

The canonical deploy initially exposed a nested source-context bug: the outer
Cloud Build ignored the worker's `.gcloudignore`, so the inner source deploy
fell back to `.gitignore` and omitted `dist-cloudrun`. The repository root now
preserves that narrow worker ignore file, and the worker ignore file explicitly
re-includes the generated bundle directory. Both successful production builds
used that corrected path.

## Verification

### Source and build

- deterministic docs generation check: 12 pages and agent artifacts passed;
- internal-link, raw-HTML, navigation, route-ownership, and artifact assertions:
  passed in the unified implementation commit;
- frozen Linux install, Start production build, API bundle, portable runtime
  dependency staging, container build, Cloud Run deploy, health smoke, and
  `/sarah` tombstone smoke: passed in both successful Cloud Builds;
- OpenTofu format and validation: passed; and
- retirement scan: no active web authority document, package script, infra
  root, or application source still references Blume or `apps/docs`.

The full Start TypeScript check still reports only the disclosed pre-existing
Effect Native failures: two TS2 unfailable-catch messages and the missing
`khalaUi` token in `-stage1-effect-native-theme.ts`. The docs compiler,
generated modules, new links, and docs UI add no TypeScript error.

### Infrastructure

The production root contains unrelated pre-existing Cloud SQL drift, and the
automation service account cannot read Certificate Manager resources. The
cutover therefore used reviewed saved targeted plans rather than applying an
unrelated full plan. The first combined apply was rejected without changing
traffic because Google Cloud would not delete a backend still referenced by
the URL map. The accepted sequence was:

1. update only `openagents-url-map`;
2. wait for consistent public TanStack responses;
3. destroy `openagents-docs-backend` and `openagents-docs-neg`;
4. disable deletion protection on the detached rollback service; and
5. remove its module and destroy `openagents-docs`.

Post-retirement Google Cloud describes return not-found for the old service,
backend, and NEG. Public docs, search, and the concrete Start asset continued
to pass after each removal.

### Live HTTP and browser acceptance

- all 12 human routes: `200 text/html`;
- `search.json` and `agent-readability.json`: `200 application/json`;
- `llms.txt` and `llms-full.txt`: `200 text/plain`;
- `sitemap.xml`: `200 application/xml`;
- raw page sources: `200 text/markdown`;
- concrete Start JavaScript and CSS: `200` with the correct content types;
- `/docs/product-promises` and `/docs/api`: `301` to
  `/docs/agent-readable`;
- `/docs/connect-codex-fleet`: `301` to `/docs/getting-started`;
- `/docs/openagents`: `301` to `/`; and
- repeated direct and public warm probes: roughly 0.12–0.26 seconds to first
  byte for documents and artifacts during the acceptance window.

The live browser pass covered the normal desktop layout and a 390 by 844
mobile viewport. It confirmed the Khala `#05070d` background and sans/mono
type contract, visible side navigation, search results, the functional mobile
drawer, focusable controls, the separate Docs link, the OpenAgents brand link
to `/`, absence of a top-level Promises link, and a fixed header remaining at
top after a 600-pixel scroll.

## Decommission boundary

`apps/openagents.com/apps/docs` is deleted. Its Blume, Astro, MDX, server,
Docker, deploy, theme, duplicated content, and tests are gone. The root lockfile
no longer contains the workspace importer or its Blume/Astro-only transitive
packages. `openagents-docs`, its backend, and its NEG are also deleted from
Google Cloud and OpenTofu state.

Reintroducing a separate docs runtime or a second docs content authority is a
new architecture and policy change. This receipt closes the migration approved
by #8854.
