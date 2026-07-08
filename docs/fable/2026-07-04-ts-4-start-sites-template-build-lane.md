# TS-4 Start Sites Template Build Lane Receipt

**STATUS (2026-07-08): SUPERSEDED by `docs/fable/MASTER_ROADMAP.md`
§EN (rev 6) — the Effect Native full-conversion mandate.** Kept as
the historical record of the earlier decision; do not implement
from this document.


Status: implemented for #8346.

## What Landed

- Canonical Autopilot Sites TanStack Start template v1:
  `autopilot_sites.tanstack_start.v1`.
- Template files include typed TanStack Router routes, one server function,
  Tailwind 4, the shared OpenAgents React token CSS, `wrangler.jsonc`, an
  `AGENTS.md`, and self-contained agent surfaces:
  `/robots.txt`, `/sitemap.xml`, `/llms.txt`, and
  `/.well-known/openagents.json`.
- Container build lane planner for saved builder versions:
  `bun install --frozen-lockfile && bun run build` in the
  `container_metered` tier, with build logs destined for the existing
  `site_versions.build_log_r2_key` field.
- WfP handoff metadata for the existing Sites deploy gate:
  `runtimeKind=workers_for_platforms`,
  `dispatchNamespace=openagents-sites-production`, a per-site runtime script
  name, upload receipt ref, passed health check, and all launch checklist
  fields.
- `wrangler.jsonc` is now recognized by the existing Sites compatibility and
  build-validation scanners, so Start candidates are classified as Worker
  modules instead of unsupported generic SSR.

## Dogfood Site

The focused test instantiates a dogfood vertical landing page for the
OpenAgents funnel:

- site id: `site_project_openagents_funnel`
- slug: `openagents-funnel`
- title: `OpenAgents AI operations funnel`
- build tier: `container_metered`
- output module: `dist/server/index.js`
- deploy runtime: `workers_for_platforms`

The test dynamically imports the generated Worker module and verifies the
landing page, `/.well-known/openagents.json`, and `/llms.txt` respond. This is
the local receipt for "saved version -> deterministic build lane -> deployable
WfP module" without activating a live customer deployment.

## Authority Boundary

This change does not deploy a live Site, create Cloudflare resources, mutate
promise state, or bypass owner/operator gates. Per-site secrets remain Worker
bindings recorded at deploy; generated source and build receipts only carry
public-safe refs and redacted metadata. `omega_static_r2` remains available for
prerender/static-only output.

## Verification

```sh
bun run --cwd apps/openagents.com/workers/api test -- \
  src/sites-start-template.test.ts \
  src/sites-compatibility.test.ts \
  src/sites-build-validations.test.ts
```
