# OA Sites And Cloudflare VibeSDK Gap Analysis

Date: 2026-06-05

Status: implementation gap analysis. This document does not change runtime
policy, deploy a Site, dispatch Autopilot, provision Cloudflare resources, or
create GitHub issues by itself.

## Source Set

Local OpenAgents/OpenAgents product surface sources:

- `docs/sites-plan.md`
- `docs/2026-06-05-openai-sites-parity-implementation-audit.md`
- `docs/2026-06-05-autopilot-sites-agent-ready-master-roadmap.md`
- `docs/sites/2026-06-05-ben-otec-site-trace.md`
- `docs/sites/2026-06-05-ben-otec-site-quality-postmortem.md`
- `docs/sites/2026-06-05-customer-site-revision-feedback-api.md`
- `workers/api/migrations/0032_autopilot_sites.sql`
- `workers/api/src/sites.ts`
- `workers/api/src/site-runtime.ts`
- `workers/api/src/site-runtime-routes.ts`
- `workers/api/src/sites-build-validations.ts`
- `workers/api/src/site-source-metadata.ts`
- `workers/api/src/openagents-capability-manifest.ts`
- `workers/api/src/openagents-openapi.ts`

VibeSDK reference sources:

- `/Users/christopherdavid/work/projects/repos/vibesdk/README.md`
- `/Users/christopherdavid/work/projects/repos/vibesdk/sdk/README.md`

Cloudflare docs checked at a high level on 2026-06-05:

- Workers for Platforms architecture and dispatch namespaces.
- Workers for Platforms features such as custom limits, observability, and
  tags.
- Durable Objects, D1, R2, Workers web-app patterns, AI Gateway, and
  Containers as current Cloudflare primitives.
- Cloudflare pricing docs for Workers, Pages Functions/static assets, R2,
  Workers for Platforms, Containers, and Sandbox SDK.

## Executive Summary

OpenAgents Sites and Cloudflare VibeSDK overlap on the final promise: a user
describes an app or site, the system builds it, previews it, and deploys it on
Cloudflare infrastructure.

They differ in product center of gravity:

- VibeSDK is a self-serve AI app builder. Its core loop is chat -> phasic code
  generation -> live container preview -> deploy to Workers for Platforms ->
  optional GitHub export. It also ships a TypeScript SDK for programmatic
  build sessions.
- OA Sites is currently an operator-supervised fulfillment and workroom
  product. Its core loop is order -> Site project -> Autopilot assignment ->
  research/preflight -> saved version -> review -> deploy -> receipt/proof.

The gap is therefore not one missing feature. It is the absence of a
first-class builder session layer between customer intent and reviewed Site
versions. OpenAgents product surface has the control-plane pieces for orders, Sites, versions,
deployments, public proof, manifests, OpenAPI, email, and review. VibeSDK has
the interactive generation, preview, SDK, and Cloudflare app-builder
experience.

Full VibeSDK parity for OA Sites should mean:

```text
user or agent prompt
-> durable builder session
-> phase plan and streamed generation events
-> generated file tree snapshots
-> live sandbox/container preview
-> follow-up chat and automatic build repair
-> deployable saved version
-> operator/customer review boundary
-> Workers for Platforms deployment with D1/R2/env bindings
-> SDK/API access to the same lifecycle
-> receipts, public proof, billing, and workroom state
```

This should still be built on OpenAgents/OpenAgents product surface infrastructure, not by
outsourcing product authority to VibeSDK. The right interpretation of
"VibeSDK parity while built on their infra" is Cloudflare-infrastructure
parity: Workers, Durable Objects or Workflows, D1, R2, KV where needed,
Containers for previews/builds, AI Gateway for model routing, and Workers for
Platforms for generated app deployment. OpenAgents product surface remains the authority for auth,
orders, workrooms, payments, receipts, projections, and release gates.

The implementation should be cost-tiered. VibeSDK leans on Containers for
live previews because it is an app-builder platform. OA Sites should default
to cheaper preview paths where they fit: R2/static candidate previews for
static output and staging Workers for Platforms deployments for valid Worker
modules. Containers should be a conditional builder/preview primitive for
cases that need package installation, build execution, dev-server behavior,
SSR-like runtime checks, dependency-heavy validation, or automatic repair from
real build/runtime errors. When a customer or agent chooses a feature that
needs Containers, the extra usage should be metered and recoverable through
credits, quote approval, or explicit pass-through pricing.

## Current OA Sites Baseline

Built or materially present:

- Customer software-order intake and D1-backed order status.
- Site lifecycle tables for projects, versions, deployments, storage bindings,
  environment values, access grants, and Site events.
- `AutopilotSitesService` for project creation, generation packets, saved
  versions, deployments, storage/env/access metadata, and events.
- Static public runtime at `https://sites.openagents.com/<slug>` backed by R2
  artifact manifests.
- Runtime dispatch support for active Workers for Platforms deployments when a
  saved Worker module, runtime script name, and dispatch namespace metadata
  exist.
- `.openagents/site.json` parse/serialize/validation helpers with
  secret-shaped-material rejection.
- Existing-project compatibility receipts and deterministic build-validation
  receipts.
- Adjutant assignment lifecycle, Exa enrichment, research briefs, task packet
  freshness, preflight blockers, and usage receipts.
- Public capability manifest, OpenAPI route, `https://openagents.com/AGENTS.md`, OTEC
  proof projection, first Site instruction card, and first Site challenge.
- Customer-visible Site revision listing and feedback APIs.
- Typed email service and lifecycle notification hooks.

Designed but not complete enough for VibeSDK parity:

- Self-serve Sites entry point.
- Real build execution in an isolated runner.
- Live preview environments before deploy.
- Automatic Workers for Platforms upload/configuration.
- Hosted D1/R2/env/secrets provisioning and injection into generated apps.
- A real-time builder session with phase/file/preview/deploy events.
- In-browser generated file tree, phase timeline, app management, and
  follow-up iteration loop.
- TypeScript SDK equivalent to `@cf-vibesdk/sdk`.
- GitHub export or clone-token flow for generated source.

## VibeSDK Capability Map

| VibeSDK capability | OA Sites state | Gap | Parity target |
| --- | --- | --- | --- |
| Natural-language app creation | Orders and operator generation packets exist. | No self-serve prompt-to-builder session. | Add typed Sites build action that creates a builder session, Site project, and assignment without manual stitching. |
| Interactive chat iteration | Autopilot/Adjutant runs and customer feedback exist. | No live generation chat tied to file snapshots and preview. | Add builder-session chat with follow-ups, image attachments, and durable event history. |
| Phase-wise generation | Task packets and build validations exist. | No product-visible phase timeline like planning, foundation, core, styling, integration, optimization. | Add phase model and events, then map Autopilot/agent progress into it. |
| Intelligent error correction | Build-validation receipts identify blockers. | No automatic build/run/fix loop. | Add container build execution, error classifier, repair prompt, and bounded retry policy. |
| Live sandbox previews | Public static deployment exists after approval. | No disposable preview URL per generation session. | Use R2/static previews and staging WFP previews by default; add Container preview sessions only for build/dev-server/runtime cases that justify the cost. |
| React/TypeScript/Tailwind generated apps | Some generated static artifact support exists. | No recommended starter enforced by builder. | Add OA Sites starter with React, TypeScript, Tailwind, tests, `.openagents/site.json`, and Worker compatibility rules. |
| Workers for Platforms deploy | Runtime can dispatch WFP metadata. | No automated upload/config/binding workflow. | Add WFP deployment automation with dispatch namespace upload, bindings, tags, health checks, rollback, and observability. |
| D1/R2/KV backing services | Metadata tables and binding records exist. | Resource provisioning and runtime injection are incomplete. | Add per-Site or namespaced D1/R2/KV provisioning, migration review, upload metadata, and retention. |
| AI Gateway model routing | OpenAgents product surface has provider account/Codex runner paths and Exa. | No builder-level AI Gateway abstraction matching VibeSDK. | Add model gateway service for generation/repair tasks while preserving Codex account fleet for coding missions. |
| Durable agent state | OpenAgents product surface has D1 run/order state. | No Durable Object or equivalent for low-latency WebSocket build sessions. | Add Durable Object or Workflow-backed builder session coordinator with D1 as ledger authority. |
| Programmatic TypeScript SDK | OpenAPI exists for core public/customer/operator APIs. | No JS SDK with WebSocket session, waits, files, phases, app management. | Publish `@openagentsinc/sites-sdk` or `@openagentsinc/autopilot-sdk` with build/connect/session APIs. |
| App listing and management | Operator/admin views and customer order views exist. | No builder app library with mine/recent/public/favorites/visibility. | Add Sites project library, visibility controls, delete/archive, clone/export, and public-safe gallery if desired. |
| GitHub export | Source repo fields and GitHub auth are present elsewhere. | No generated-source export flow or clone token. | Add GitHub export app, branch/PR/export receipts, and expiring clone/download tokens. |
| One-click platform deploy | VibeSDK is self-hostable by Deploy to Cloudflare. | Not an OA product requirement. | Defer unless OpenAgents wants a white-label/self-hosted Sites platform. |

## Architectural Delta

### Control Plane

OA Sites already has the more rigorous control plane. It has orders, operator
review, research gates, receipts, public projection, clean URL policy, email
ledger, and workroom direction. VibeSDK is more direct and self-serve.

Parity should not remove the OA review boundary. A Vibe-style builder session
can produce previews and deployable candidates, but production deployment
should still require the existing saved-version and review gates for customer
or public Sites.

### Builder Runtime

VibeSDK's strongest missing piece in OpenAgents product surface is the builder runtime:

- a durable session ID;
- WebSocket or event-stream connection;
- streamed assistant output;
- phase timeline;
- generated file events;
- file-tree snapshot access;
- container preview deploy;
- preview URL status;
- deploy-to-Cloudflare status;
- reconnect and state restoration.

OpenAgents product surface should add this as a Sites Builder subsystem, not as ad hoc fields on
`site_projects`. `site_projects` should remain the durable hosted artifact and
review namespace. A builder session is an attempt to create or modify a
version.

### Preview And Build Execution

OpenAgents product surface has compatibility and build-validation receipts, but the current
documented implementation notes say the checker and validator do not yet
clone, execute a live build, adapt, save, or deploy a project.

To match VibeSDK without inheriting unnecessary cost, OpenAgents product surface needs a
cost-aware preview ladder:

1. **Static candidate preview.** For already-generated HTML/CSS/JS or
   prebuilt static bundles, write candidate assets to R2 under a preview
   prefix and serve them through the existing Sites runtime or a preview
   variant of it. This should be the default cheapest path.
2. **Staging Workers for Platforms preview.** For a generated Worker module
   that already passes compatibility checks, upload to a staging dispatch
   namespace with temporary bindings and tags. This is the default dynamic-app
   preview path when no package install or dev server is needed.
3. **Container build/preview.** Use Cloudflare Containers only when the system
   must install dependencies, run a build command, boot a dev server, test
   SSR-like behavior, execute dependency-heavy smoke tests, or reproduce
   runtime errors for automatic repair.

The third path is still required for VibeSDK-class builder parity, but it
should not be the default Site preview. Containers are billed while active;
Cloudflare's pricing docs say CPU is based on active usage, while memory and
disk are based on provisioned resources. Static asset serving and ordinary
Worker/WFP execution are much cheaper for simple previews and production
traffic.

When Containers are used, OpenAgents product surface should record a preview/build cost receipt and
apply a payment policy:

- free only for bounded public-beta slices and operator-approved smoke runs;
- included in a quote for customer-paid Site builds;
- charged against prepaid credits for self-serve users and agents;
- exposed as a `402` or quote-required recovery path when an agent requests a
  heavier preview/build than the free tier allows; and
- visible to operators with instance type, duration, resource class, and
  customer-safe reason.

Container-sensible use cases:

- building from an arbitrary GitHub repo that needs `npm`, `bun`, `pnpm`, or
  framework-specific build steps;
- validating a React/Vite/Tailwind app before producing static output;
- previewing a dev-server-only candidate before exporting a static or Worker
  bundle;
- testing SSR-like, API, upload, database, or auth flows that cannot be
  represented by static files;
- reproducing browser/runtime errors so Autopilot can repair them;
- running dependency-heavy smoke tests, migrations, or adapters before
  saving a version; and
- high-value paid customer work where the quote explicitly includes hosted
  preview/build compute.

Container-avoidance use cases:

- static landing pages already emitted as HTML/CSS/JS;
- simple proof pages such as the current OTEC static artifact;
- saved static bundles that only need review;
- valid Worker modules that can be staged directly in WFP;
- customer-safe screenshot or link review that does not require a live
  dev server; and
- agent dry-run discovery, manifest reads, OpenAPI reads, and proof
  inspection.

Any real isolated execution path still needs:

- source snapshot materialization;
- dependency install and build in an isolated environment;
- bounded log capture;
- preview server boot;
- health check;
- public or signed preview URL;
- teardown and resource accounting;
- automatic repair loop based on build/runtime errors.

Cloudflare Containers are the direct VibeSDK-aligned primitive for this. SHC
and external runner routes can stay as backup or heavy coding lanes, but the
Sites preview loop should have a Cloudflare-native path and a cheaper
R2/WFP-first fast path.

### Deployment Data Plane

OpenAgents product surface already models static R2 deployments and WFP deployments. VibeSDK
actually deploys generated apps to Workers for Platforms.

The parity gap is operational automation:

- upload user Worker modules to a dispatch namespace;
- attach D1/R2/KV/env bindings;
- tag deployments by Site/project/customer/environment;
- record external deployment IDs;
- run health checks;
- route requests through the dynamic dispatch Worker;
- disable or roll back prior deployments;
- expose status and logs to operators/customers.

The existing `site-runtime-routes.ts` and `site-runtime.ts` are a good base:
they can already resolve a public active WFP target and fetch it through
`env.SITES_DISPATCH.get(runtimeScriptName)`.

### SDK Surface

VibeSDK's SDK is a product surface, not just documentation. It exposes:

- client auth with API key or JWT;
- `build(prompt, options)`;
- `connect(agentId)`;
- `startGeneration`, `stop`, `resume`, `followUp`;
- wait helpers for generation, phases, deployability, preview deployment, and
  Cloudflare deployment;
- event listeners for generation, phase, file, preview, deployment, and
  errors;
- file tree and file content access;
- app list/get/delete/visibility/star/favorite;
- Git clone token.

OpenAgents product surface's OpenAPI route is the start of this, but full parity requires a
stateful SDK around builder sessions and event streams.

## Recommended Implementation Order

### Phase 0: Preserve Current OA Sites Authority

Goal: do not regress the operator-supervised Sites beta while adding VibeSDK
parity.

Work:

- Keep `site_projects`, `site_versions`, `site_deployments`, and `site_events`
  as the hosted Site authority.
- Keep save and deploy as separate product actions.
- Keep public/customer projections redacted and receipt-backed.
- Keep `.openagents/site.json`, `https://openagents.com/AGENTS.md`, OpenAPI, and
  capability manifest public-safe.
- Add a short architecture note that builder sessions create version
  candidates, not production deployment authority.

Exit criteria:

- Existing OTEC/static runtime and customer feedback APIs still pass.
- A failed builder session cannot publish or widen access by itself.

### Phase 1: Add Durable Builder Session Schema

Goal: create the missing VibeSDK session object before adding runtime
complexity.

Add D1 tables or equivalent repositories for:

- `site_builder_sessions`;
- `site_builder_messages`;
- `site_builder_phases`;
- `site_builder_files`;
- `site_builder_events`;
- `site_builder_previews`;
- `site_builder_artifacts`.

Minimum fields:

- session ID, Site ID, order ID, owner, team/project refs;
- prompt, project type, behavior type, selected template, model route;
- status: `planning`, `generating`, `building`, `previewing`, `deployable`,
  `failed`, `stopped`, `superseded`;
- current phase and phase counts;
- generated file snapshot hash;
- latest preview URL and health state;
- latest build validation receipt;
- link to resulting `site_version_id` when saved.

APIs:

- `POST /api/sites/build-sessions`
- `GET /api/sites/build-sessions/:sessionId`
- `GET /api/sites/build-sessions/:sessionId/events`
- `POST /api/sites/build-sessions/:sessionId/messages`
- `POST /api/sites/build-sessions/:sessionId/stop`
- `POST /api/sites/build-sessions/:sessionId/resume`

Exit criteria:

- A prompt can create a session and a Site project without running code.
- Events can be replayed after reconnect.
- Customer-safe status exists without private runner payloads.

### Phase 2: Add Event Stream And File Snapshot API

Goal: reach the SDK/UI foundation VibeSDK expects.

Work:

- Add WebSocket or Server-Sent Events for builder session updates.
- Store append-only event records for generation, phase, file, preview, build,
  save, deploy, and error events.
- Add file snapshot APIs:
  - list paths;
  - read file;
  - return tree;
  - export source archive ref.
- Bound file sizes and redact secret-shaped material before projection.
- Treat generated files as candidate artifacts until saved into
  `site_versions`.

Exit criteria:

- A client can connect, disconnect, reconnect, and restore the phase timeline
  and file tree.
- Tests prove file/event projections omit secret-shaped material and private
  provider state.

### Phase 3: Add Cost-Tiered Preview And Build Runner

Goal: close the biggest VibeSDK runtime gap.

Work:

- Add the cheap default preview path first: R2-backed static candidate
  previews for generated or built static output.
- Add staging Workers for Platforms previews for already-valid Worker modules.
- Provision a Cloudflare Containers based preview/build service only for
  source bundles that need install/build/dev-server/runtime execution.
- Define a build-run input contract: source archive, package manager, build
  command, runtime target, env key names, D1/R2 needs.
- Capture bounded install/build/runtime logs.
- Boot a preview server and expose a signed or public preview URL depending on
  Site visibility.
- Record container instance type, lease, started/stopped timestamps, health
  checks, and resource usage.
- Tear down previews automatically when superseded, expired, or disabled.
- Attach payment policy to Container previews: free-beta allowance, quote
  inclusion, credit spend, or `402`/quote-required recovery when the requested
  preview is economically heavier than the user's allowance.

Exit criteria:

- Static candidates preview without Containers.
- Valid Worker modules preview through a staging WFP path without Containers.
- A generated React/TypeScript/Tailwind starter can build in a Container when
  live build execution is required.
- The customer/operator can open the cheapest fitting preview URL before
  deploy.
- Build failures create typed validation receipts and customer-safe next
  actions.
- Container preview/build receipts are metered and can be passed through to
  the customer or agent through credits, quote approval, or `402` recovery.

### Phase 4: Add Phasic Generation And Repair Loop

Goal: match VibeSDK's phase-wise generation and intelligent error correction.

Work:

- Add phase templates: planning, foundation, core, styling, integration,
  optimization.
- Generate or adapt code phase by phase into the file snapshot ledger.
- Run build validation after relevant phases.
- Feed bounded errors back into a repair prompt.
- Add retry budgets and stop conditions.
- Route model calls through an OpenAgents product surface model gateway. AI Gateway can be the
  Cloudflare-aligned provider router, while Codex/ChatGPT account fleet remains
  available for coding missions that need browser-account dispatch.

Exit criteria:

- A session shows phase progress and generated file events.
- A simple failing app can be repaired automatically within a bounded retry
  count.
- Repair prompts cannot include private logs, secrets, provider grants, or raw
  credentials.

### Phase 5: Convert Deployable Preview To Saved Version

Goal: connect the Vibe builder loop to OA's stronger review lifecycle.

Work:

- Add `POST /api/sites/build-sessions/:sessionId/save-version`.
- Require successful build receipt, source hash, artifact manifest, bounded
  logs, and generated source archive.
- Write static assets, Worker module, source archive, manifest, and build log
  to R2.
- Create a `site_versions` row with `build_status = saved`.
- Generate or update `.openagents/site.json` inside the source archive.
- Mark previous candidate sessions superseded where appropriate.

Exit criteria:

- A live preview can become a saved version without deploy.
- Saved versions remain inspectable and reviewable.
- Deploy still requires explicit review/checklist action.

### Phase 6: Automate Workers For Platforms Deployment

Goal: reach VibeSDK deployment parity for dynamic apps.

Work:

- Implement the actual WFP upload path for Worker modules.
- Choose a dispatch namespace strategy: one production namespace plus one
  staging namespace, not one namespace per customer.
- Attach generated app bindings for D1/R2/KV/env values.
- Apply Cloudflare tags for Site ID, order ID, environment, owner/team, and
  claim state.
- Record external deployment IDs and health-check results.
- Support disable and rollback from `site_deployments`.
- Keep `openagents_static_r2` for static-only outputs and WFP for Worker outputs.

Exit criteria:

- A Worker-compatible generated app is deployed through WFP and served by
  `sites.openagents.com/<slug>`.
- Static and Worker runtime paths share the same review, event, and projection
  model.
- Rollback can restore a prior active deployment.

### Phase 7: Provision App Storage, Env, Secrets, And Auth

Goal: match VibeSDK's full-stack app capability.

Work:

- Provision D1 per Site or namespaced D1 tables based on app needs.
- Provision R2 prefixes or buckets for uploads and generated assets.
- Add optional KV binding support for session/cache-style apps.
- Add migration review and apply flow.
- Add hosted environment value management.
- Store secrets in Cloudflare Secrets Store or the approved OpenAgents product surface secret
  boundary, not in source, docs, D1 plain text, logs, or public projections.
- Add public visitor auth and OpenAgents session auth adapters for generated
  apps.

Exit criteria:

- A generated app can safely use D1 and R2 at runtime.
- Env changes mark redeploy-required and can redeploy an approved saved
  version.
- Private Sites do not leak existence or metadata to unauthenticated users.

### Phase 8: Build The Self-Serve Sites UI

Goal: make the product feel like VibeSDK without dropping OA's review model.

Work:

- Add a Sites builder screen with prompt input, chat, phase timeline, file
  tree, preview pane, build status, and save/deploy actions.
- Add customer-safe Site detail with versions, feedback, preview, deployment
  URL, milestones, and usage receipts.
- Add operator Site browser with projects, versions, deployments, env refs,
  storage, access, events, build logs, and launch checklist.
- Add follow-up prompt support for adjustment requests.
- Add app visibility controls: private, team, public.

Exit criteria:

- A customer can create a Site candidate, preview it, submit feedback, and see
  saved/deployed state without shell/operator-only steps.
- Operators can inspect and approve before public deployment.

### Phase 9: Publish An OpenAgents Sites SDK

Goal: match VibeSDK's programmatic builder surface.

Work:

- Publish a TypeScript SDK with:
  - `client.build(prompt, options)`;
  - `client.connect(sessionId)`;
  - `session.startGeneration()`;
  - `session.stop()`;
  - `session.resume()`;
  - `session.followUp()`;
  - `session.deployPreview()`;
  - `session.saveVersion()`;
  - `session.deployCloudflare()` or `session.requestDeploy()`;
  - wait helpers for generation, phase, deployable, preview, saved version,
    and deployment;
  - event listeners for generation, phase, file, preview, deployment, and
    error events;
  - file tree and file read helpers.
- Use API keys or scoped agent tokens that exchange for short-lived session
  credentials.
- Require idempotency keys for mutating calls.
- Keep payment, rate-limit, and receipt semantics aligned with OpenAPI.

Exit criteria:

- A script can create a Site, wait for preview, inspect files, save a version,
  and request deployment through the same policy boundaries as the UI.

### Phase 10: Add GitHub Export And App Library Features

Goal: finish the VibeSDK convenience layer.

Work:

- Add generated source export to GitHub repo/branch/PR.
- Add expiring clone/download tokens for generated source archives.
- Add app/project management:
  - list mine;
  - list recent;
  - list public;
  - archive/delete;
  - set visibility;
  - favorite/star if public gallery becomes useful.
- Add public gallery only after projection, moderation, and public-proof rules
  are strong enough.

Exit criteria:

- Generated source can leave OpenAgents product surface through an auditable GitHub export or
  expiring clone/download path.
- Public listings do not expose private orders, runner logs, or unpublished
  artifacts.

## What Not To Copy Blindly

- Do not make generated app deployment automatic production release. VibeSDK
  optimizes for fast builder flow; OA Sites needs review, proof, and customer
  acceptance.
- Do not treat prompt files, remote skill docs, or app manifests as
  authorization. They are discovery UX only.
- Do not expose raw build logs, runner payloads, provider credentials, OAuth
  state, invoices, or wallet material through builder APIs.
- Do not create a namespace per customer by default. Use a production dispatch
  namespace and a staging namespace unless a stronger isolation requirement
  appears.
- Do not let self-serve generation bypass research policy, legal-sensitive
  review, payment policy, or public claim-state rules.
- Do not block the current supervised Sites beta on SDK, gallery, favorites,
  or self-hostable-platform features.

## Minimal Issue Batch

Suggested issue sequence:

1. `OPENAGENTS-SITES-VIBE-001`: Add builder session D1 schema and repository.
2. `OPENAGENTS-SITES-VIBE-002`: Add builder session create/read/event APIs.
3. `OPENAGENTS-SITES-VIBE-003`: Add session event stream and reconnect replay.
4. `OPENAGENTS-SITES-VIBE-004`: Add generated file snapshot ledger and file APIs.
5. `OPENAGENTS-SITES-VIBE-005`: Add cost-tiered R2/WFP/Container build and preview
   runner.
6. `OPENAGENTS-SITES-VIBE-006`: Add phasic generation timeline and phase events.
7. `OPENAGENTS-SITES-VIBE-007`: Add bounded auto-repair loop from build/runtime
   errors.
8. `OPENAGENTS-SITES-VIBE-008`: Save deployable builder output into
   `site_versions`.
9. `OPENAGENTS-SITES-VIBE-009`: Automate WFP upload, binding injection, health
   check, and deployment recording.
10. `OPENAGENTS-SITES-VIBE-010`: Add D1/R2/KV/env/secrets provisioner for generated
    apps.
11. `OPENAGENTS-SITES-VIBE-011`: Add self-serve Sites builder UI.
12. `OPENAGENTS-SITES-VIBE-012`: Publish OpenAgents Sites SDK with build/connect,
    event, wait, file, preview, save, and deploy helpers.
13. `OPENAGENTS-SITES-VIBE-013`: Add GitHub export and expiring source clone
    tokens.
14. `OPENAGENTS-SITES-VIBE-014`: Add app library visibility, archive/delete,
    public listing, and optional favorite/star features.

## Parity Definition

OA Sites reaches practical VibeSDK parity when all of the following are true:

- A user or authorized agent can start a Site/app build from a prompt through
  UI or SDK.
- The build has a durable session, event stream, phase timeline, and generated
  file tree.
- The system can preview through the cheapest fitting Cloudflare path:
  R2/static, staging WFP, or Container build/preview when runtime execution is
  required.
- Follow-up chat can modify the app and trigger bounded rebuild/repair.
- A deployable preview can be saved as a reviewable `site_version`.
- Saved versions remain separate from production deployment.
- Static output can deploy through the existing R2 runtime.
- Worker-compatible output can deploy through Workers for Platforms with
  bindings, tags, health checks, rollback, and deployment records.
- Generated apps can use D1, R2, KV where needed, and hosted env/secrets are
  injected safely outside source.
- Container-backed previews and builds are metered, policy-gated, and can be
  passed through to the customer or agent when they exceed the free or quoted
  allowance.
- OpenAPI and a TypeScript SDK expose the same lifecycle with scoped auth,
  idempotency keys, durable status, and receipt visibility.
- GitHub export or clone/download token support exists for generated source.
- Customer and public projections remain redacted, claim-state honest, and
  free of private runner/provider/payment mechanics.

The core product difference can remain intentional: VibeSDK is a fast
self-serve AI builder; OA Sites should become a fast builder plus a governed
fulfillment/workroom system. Full parity should add VibeSDK's speed and
developer ergonomics without deleting OpenAgents' review, receipt, economics,
and public-proof advantages.
