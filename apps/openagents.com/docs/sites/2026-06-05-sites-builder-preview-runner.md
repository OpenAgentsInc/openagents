# Sites Builder Preview Runner

Issue #196 adds the first cost-tiered preview runner contract for OpenAgents
Sites builder sessions.

## Implemented Contract

The preview runner does not claim full Cloudflare Container execution yet. It
implements the safe decision and receipt layer that later preview/build workers
will call:

- static candidates select `r2_static`
- Worker-compatible module candidates select `wfp_staging`
- candidates that need dependency install, build execution, dev-server
  behavior, SSR-like runtime, dependency-heavy validation, or repair from a
  real runtime/build error select `container_metered`

The runner records:

- a `site_builder_previews` row through the existing builder-session preview
  repository
- a customer-visible `preview_created` event for SSE replay
- bounded metadata: candidate kind, selected tier, selected reason,
  customer-safe summary, and whether metered Container work is gated

## Current Limits

This slice is the planner and receipt layer. It does not yet upload static
bundles to R2, upload Worker modules to Workers for Platforms, or start a
Container. Those execution paths are intentionally separated so later issues can
add real runtime adapters behind the same preview-tier receipt.

The Container tier is recorded as gated work. That means the system can tell a
customer or operator why a heavier preview is needed without silently spending
unbounded compute.

## Safety Rules

- preview metadata avoids raw logs, provider payloads, source archives, wallet
  material, checkout material, and secrets
- customer-visible events expose the tier and a plain-language reason only
- a preview URL must still be an OpenAgents HTTPS URL
- `container_metered` does not imply work was executed or billed; it means the
  candidate needs reviewed metered execution before preview

## Follow-Up Work

- implement the static R2 artifact upload path
- implement the staging Workers for Platforms upload path
- implement the metered Container runner adapter and quote/credits/402 gate
- wire preview request APIs and UI controls to the runner
- feed build/runtime failures into the bounded repair loop
