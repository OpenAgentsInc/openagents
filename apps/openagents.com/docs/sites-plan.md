# Autopilot Sites Product Plan

Date: 2026-06-04

Status: launch plan and implementation source authority. This document does
not change runtime policy by itself. It turns `docs/sites.md`, the current
customer software-order work, and the Autopilot operator runbook into a
concrete plan for launching Autopilot Sites on June 5, 2026.

## Executive Summary

Autopilot Sites should be OpenAgents' Cloudflare-native answer to OpenAI
ChatGPT Sites: a customer asks for a site or lightweight app, Autopilot creates
it, OpenAgents saves a reviewable version, an operator or approved customer
deploys it, and the result is served from OpenAgents infrastructure with
durable data, files, access controls, runtime configuration, versions,
deployment status, and inspection.

The launch wedge is Ben's active software order:

```text
bensilone/openagents
"Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter."
```

The first public target should be:

```text
https://sites.openagents.com/otec
```

The near-term product should be simple on purpose:

1. A customer order can become an Autopilot Site project.
2. Operators can generate and save a first deployable version from the order.
3. Operators can deploy the saved version to `sites.openagents.com/<slug>`.
4. Customers can see a clean status page and final URL without seeing runner
   internals.
5. Core operators can inspect source, build output, versions, deployments,
   storage bindings, access mode, and logs.

The longer-term product should reach documented OpenAI Sites parity while using
OpenAgents product surface's own primitives: OpenAuth, GitHub auth, D1, R2, Workers, Workers for
Platforms, Queues, Workflows, existing Autopilot runner dispatch, Stripe
credits, email ledger, and OpenAgents Sync.

## Source Material Reviewed

Local docs and task packets:

- `docs/sites.md`
- `docs/autopilot-tasks/AGENTS.md`
- `docs/autopilot-tasks/done/2026-06-04-customer-software-ordering-flywheel.md`
- `docs/autopilot-tasks/2026-06-04-programmatic-autopilot-operator-runbook.md`
- `docs/2026-06-04-programmatic-autopilot-work-runbook-audit.md`
- `docs/2026-06-04-cloudflare-containers-runner-backup-audit.md`
- `docs/autopilot-tasks/2026-06-04-cloudflare-containers-runner-backup-implementation.md`
- `docs/2026-06-02-shc-agent-deployment-runbook.md`
- `docs/2026-06-02-cloudflare-only-openagents-sync-audit.md`
- `docs/2026-06-04-stripe-effect-service-audit.md`
- `docs/autopilot-tasks/done/2026-06-04-stripe-effect-service-implementation.md`
- `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`
- `AGENTS.md`
- `INVARIANTS.md`

Local code surfaces:

- `workers/api/migrations/0030_software_orders.sql`
- `workers/api/src/customer-orders.ts`
- `workers/api/src/onboarding/routes.ts`
- `workers/api/src/admin-overview-routes.ts`
- `apps/web/src/page/loggedIn/page/order.ts`
- `apps/web/src/page/loggedIn/page/admin.ts`
- `workers/api/src/omni-runs.ts`
- `workers/api/src/omni/deployment-repository.ts`
- `workers/api/src/omni/assignments.ts`
- `workers/api/src/omni/public-service.ts`
- `workers/api/src/bindings.ts`
- `workers/api/wrangler.jsonc`

Official external docs checked on 2026-06-04:

- OpenAI Sites guide:
  `https://developers.openai.com/codex/sites`
- Cloudflare Workers Static Assets:
  `https://developers.cloudflare.com/workers/static-assets/`
- Cloudflare Workers for Platforms:
  `https://developers.cloudflare.com/cloudflare-for-platforms/workers-for-platforms/`
- Cloudflare Workers for Platforms dynamic dispatch:
  `https://developers.cloudflare.com/cloudflare-for-platforms/workers-for-platforms/configuration/dynamic-dispatch/`
- Cloudflare Workers for Platforms static assets:
  `https://developers.cloudflare.com/cloudflare-for-platforms/workers-for-platforms/configuration/static-assets/`

## Current Production Starting Point

The user-provided production snapshot for 2026-06-04 says there are still two
active `software_orders` and no new software orders:

| Repository | Request | Created | Status |
| --- | --- | --- | --- |
| `bensilone/openagents` | Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter. | 2026-06-04T20:39:14.534Z | submitted |
| `OpenAgentsInc/openagents` | Testing | 2026-06-04T20:28:59.755Z | submitted |

OpenAgents product surface already has a narrow customer software-order substrate:

- D1 table `software_orders`.
- Customer active-order API at `/api/customer-orders/active`.
- Customer order page at `/order`.
- Admin overview that lists software orders.
- Existing Autopilot run and deployment ledgers.
- Existing D1/R2 Worker bindings.
- Existing Stripe credit service, config-gated for production.
- Existing email ledger boundary.
- Existing operator preflight/checklist/continuation/callback retry model.

The Sites launch should not create a parallel intake system. Sites should be a
product lane attached to `software_orders`, then promoted to first-class tables
when the site lifecycle needs durable version and deployment authority.

## Product Definition

Autopilot Sites is a managed site/app lifecycle inside OpenAgents.

Customer-facing promise:

- Submit a website or lightweight app request.
- OpenAgents builds and hosts it.
- Review a live URL when it is ready.
- Keep billing, hosting, storage, identity, and deployment operations out of
  the customer's critical path.

Operator-facing promise:

- Convert an order into a site project.
- Generate source and build artifacts through Autopilot.
- Save reviewable versions before production deployment.
- Deploy approved versions to OpenAgents-hosted URLs.
- Inspect source, build logs, storage bindings, secrets, access mode, and
  deployment status.
- Roll back or disable unsafe deployments.

Platform-facing promise:

- Host generated sites on Cloudflare.
- Use D1 for structured per-site app state.
- Use R2 for files, uploads, assets, screenshots, source archives, and build
  artifacts.
- Use Workers or Workers for Platforms for generated app execution.
- Use OpenAgents auth and access policy instead of leaking auth state through
  URLs.
- Use typed Effect services, Schema models, and D1 migrations.

## OpenAI Sites Parity Contract

The target is 100 percent parity with the documented OpenAI Sites product, not
with undocumented internals or unpublished limits.

| OpenAI Sites capability | Documented behavior | Autopilot Sites parity target |
| --- | --- | --- |
| Create | Codex creates websites, web apps, dashboards, internal tools, and games from prompts. | Autopilot creates site projects from `software_orders`, direct operator prompts, or future customer Sites intake. |
| Existing project deploy | Codex can prepare and publish a compatible existing project. | Autopilot can import a GitHub repo/path and produce a Sites-compatible build. |
| Build validation | Codex validates the build before save/deploy. | Every saved version requires a recorded build command, exit status, artifact manifest, and failure summary. |
| Save version | Codex saves a deployable version associated with a source Git commit. | `site_versions` records commit SHA, generated source archive, build artifact refs, migrations, checksum, and review status. |
| Deploy version | Codex deploys a saved version and returns a production URL. | `site_deployments` promotes a saved version to `sites.openagents.com/<slug>` or a future custom hostname. |
| Inspect versions | Codex can list/inspect saved versions and deployment status. | Operator UI and APIs list versions, diffs, source refs, artifact refs, build status, deployment URL, and rollback candidates. |
| Hosting runtime | Sites hosts Cloudflare Worker-compatible ES module output. | Generated apps target Worker-compatible ES modules first; static-only sites use Worker Static Assets or R2-backed static serving. |
| Metadata file | `.openai/hosting.json` links source to a hosted project and binding names. | `.openagents/site.json` links repo source to `siteId`, `projectId`, D1 binding, R2 binding, access mode, target runtime, and last saved version. |
| D1 | Sites uses D1 for durable structured data. | Per-site D1 is represented by dedicated D1 databases when needed, or by D1 table namespaces for MVP when a dedicated database is not provisioned yet. |
| R2 | Sites uses R2 for files and uploads. | Per-site R2 prefixes or buckets store uploads, generated assets, source archives, build bundles, screenshots, and file metadata. |
| Workspace identity | Sites can use workspace-authenticated user identity. | Access-controlled sites use OpenAuth/OpenAgents product surface session identity and optional customer/team membership. |
| Public/external auth | Sites can support public sign-in or external identity provider projects. | Later phase supports public visitor auth through OpenAuth or site-owned auth adapters; MVP supports public anonymous and OpenAgents-authenticated modes. |
| Access modes | `admins_only`, `workspace_all`, `custom`. | `owner_admins`, `openagents_core`, `customer_owner`, `custom_users`, `public` where `public` is explicit and review-gated. Mapping preserves OpenAI modes while adding public launch semantics. |
| Secrets/env | Hosted env vars and secrets are managed outside source and require redeploy. | `site_env_vars` stores non-secret metadata; secrets live in Cloudflare secrets/Secrets Store or encrypted operator-owned config, never in source or docs. |
| Admin governance | Workspace admins can enable/disable Sites and published sites. | OpenAgents core admins can enable/disable the product, disable a site, revoke deployment, change access, and audit versions. |
| Review before share | Review source, migrations, build, audience, secrets, URL. | A release checklist is required before deploy or access widening. |

## `.openagents/site.json`

OpenAgents product surface uses `.openagents/site.json` as the local source metadata contract for
Autopilot Sites. It is intentionally linkage metadata only. Runtime
environment values and secrets remain hosted/operator-managed and must not be
stored in this file.

Current schema version with the planned optional payment extension:

```json
{
  "schemaVersion": "openagents.site.v1",
  "siteId": "site_project_...",
  "hostedProjectId": "hosted_site_...",
  "softwareOrderId": "software_order_...",
  "accessMode": "owner_admins",
  "visibility": "private",
  "source": {
    "provider": "github",
    "owner": "OpenAgentsInc",
    "name": "openagents",
    "ref": "main"
  },
  "target": {
    "runtimeKind": "openagents_static_r2",
    "slug": "example-site",
    "url": null
  },
  "bindings": {
    "d1": "DB",
    "r2": "ASSETS"
  },
  "lastSavedVersionId": "site_version_...",
  "activeDeploymentId": null,
  "agentSurface": {
    "preset": "proof_and_challenge",
    "capabilityManifestUrl": "/.well-known/openagents.json",
    "proofUrl": "/api/public/proof/otec"
  },
  "payments": {
    "enabled": true,
    "provider": "mdk",
    "merchantMode": "openagents_hosted",
    "checkoutBasePath": "/checkout",
    "products": [
      {
        "id": "consultation_deposit",
        "title": "Consultation deposit",
        "description": "Reserve an initial consultation.",
        "price": { "currency": "USD", "amountCents": 5000 },
        "kind": "one_time",
        "requiresCustomerData": ["email", "name"]
      }
    ],
    "paidActions": [
      {
        "id": "download_report",
        "method": "GET",
        "path": "/api/reports/download",
        "price": { "currency": "SAT", "amountSats": 100 },
        "settlementMode": "deferred",
        "agentReadable": true
      }
    ]
  }
}
```

The implementation lives in `workers/api/src/site-source-metadata.ts` and
provides typed parse, serialize, validation, and derivation helpers. It rejects
unsupported access/runtime values and fails closed when metadata contains
secret-shaped keys or values such as auth tokens, cookies, bearer material, or
provider account refs.

The optional `payments` block is the planned MDK checkout primitive for Sites.
It lets generated Sites declare human-facing checkout products and agent-facing
paid actions without embedding MDK merchant secrets in generated source. In v0,
`merchantMode: "openagents_hosted"` means static Sites and WFP Site Workers call
OpenAgents product surface's hosted payment service to create checkout intents or L402 challenges.
Later `customer_mdk_account` mode can bind customer-owned MDK credentials
through reviewed hosted secrets, but the source metadata file must still never
contain `MDK_ACCESS_TOKEN`, `MDK_MNEMONIC`, webhook secrets, raw invoices,
preimages, wallet mnemonics, or payout credentials.

The MDK Next.js checkout package is source reference, not an OpenAgents product surface dependency.
The synced `mdk-checkout` source shows the Next.js server route and hook are
thin re-exports into `@moneydevkit/core`. OpenAgents product surface should recreate the core
checkout actions, API-contract shapes, metadata validation, signed checkout
URL handling, and L402 token flow in Effect TypeScript Worker services, then
expose those through hosted Site payment APIs and WFP service bindings.

Payment products and paid actions are versioned with the Site version. A
rollback must restore the previous product/action definitions instead of
silently changing prices, entitlement scopes, or agent-readable paid API
semantics.

## Agent Capability Manifest

OpenAgents publishes a machine-readable public discovery document at:

```text
GET /.well-known/openagents.json
```

The manifest is intentionally a discovery document, not an authorization grant.
It points agents to public proof/activity resources, current browser-session
flows, the OpenAPI endpoint, supported auth modes, public and authenticated
rate-limit posture, and future scoped API key / Lightning-L402 recovery
caveats. It must stay public-safe and omit private runner payloads, provider
account refs, auth grants, callback tokens, and secrets.

OpenAgents also publishes initial stable OpenAPI docs for core agent-facing
APIs at:

```text
GET /api/openapi.json
```

The OpenAPI document currently covers discovery, public proof/activity,
customer order status, operator Sites lifecycle, Adjutant assignment lifecycle,
and operator email delivery inspection. It is intentionally public-safe: auth
modes are documented, but secret-bearing request/response fields and private
runner/provider payloads are not.

## Site-Specific Agent Instruction Cards

Public Site proof projections can include an `agentInstructionCard` object for
the visible "Send your agent to this Site" loop. The first shipped card is on
`GET /api/public/proof/otec`.

The card is a public discovery aid, not an authorization grant. It includes:

- a stable title, version, Site slug/title/URL when public;
- the Site proof URL when available;
- canonical links to `/.well-known/openagents.json`, `/api/openapi.json`, and
  `https://openagents.com/AGENTS.md`;
- copyable instructions for browser or CLI agents;
- public allowed actions such as proof inspection, OpenAPI inspection, status
  summarization, site-improvement proposals, and owner-review requests;
- caveats for missing proof or missing deployment URL.

The projection degrades safely when a Site has no public proof or active
deployment. It fails closed instead of emitting a card when the projected card
contains secret-shaped material. It never exposes private runner logs, provider
account refs, auth grants, source archives, unpublished artifacts, or
customer-private data.

## First-Site Agent Challenges

Public Site proof projections can also include `agentChallenges`. The first
challenge ships on `GET /api/public/proof/otec` with the challenge URL:

```text
https://openagents.com/api/public/proof/otec#agent-challenges
```

The first OTEC challenge asks agents to inspect the public proof and propose
stronger public evidence or clearer Site copy. It supports public-only proof
inspection, research-source proposals, and copy-improvement proposals. Required
evidence includes public source URLs/titles, a one-sentence explanation of how
the source supports OTEC/SWAC/floating-datacenter claims, before/after copy for
wording changes, and the proof field or Site section being improved.

The challenge is explicitly proposal-only until scoped contribution APIs and
owner-claim flows are live. Funding, Lightning, L402, bounty, and reward
settlement paths are marked `planned_not_live`; no accepted outcome, payment,
reward, or settlement is claimed without a receipt.

The canonical agent instructions are served at:

```text
GET /AGENTS.md
```

The document starts with dry-run discovery, points agents to the capability
manifest and OpenAPI docs, lists prohibited actions without scoped authority,
and states that prompt docs are discovery UX only. The capability manifest
mirrors the canonical document hash through `docs.instructionSha256`.

The same document now includes copyable public dry-run examples for:

- Codex or ChatGPT-style coding agents;
- generic browser/API agents;
- first-Site challenge participants.

The examples point agents to the manifest, OpenAPI docs, `https://openagents.com/AGENTS.md`, and the OTEC challenge URL while repeating the no-secrets,
no-mutation, owner-claim/scoped-auth, and future credits/L402 caveats.

Implementation note, June 5, 2026:

- OpenAgents product surface now has a deterministic existing-project compatibility checker for
  operator-provided project file snapshots.
- `POST /api/operator/sites/:siteId/compatibility/check` records a durable
  `site_compatibility_checks` receipt and `site_compatibility.checked` event.
- `GET /api/operator/sites/:siteId/compatibility` returns the latest receipt.
- The receipt records source metadata, package manager/build command hints,
  static versus Worker/SSR output, Worker blockers, D1/R2/auth needs,
  environment key names only, findings, blockers, warnings, evidence refs, and
  customer-safe status/next action.
- OpenAgents product surface now also has deterministic build-validation receipts for operator
  provided build candidates.
- `POST /api/operator/sites/:siteId/build-validations` records a durable
  `site_build_validations` receipt and `site_build_validation.checked` event.
- `GET /api/operator/sites/:siteId/build-validations/latest` returns the
  latest receipt.
- The receipt records compatibility check ID, source kind/repository/commit,
  stable source hash, package manager/build command, static/Worker/SSR output,
  manifest, bounded logs, truncation metadata, findings, blockers, warnings,
  evidence refs, and customer-safe status/next action.
- The checker and validator do not clone, execute a live build, adapt, save, or
  deploy a project yet; that remains the import-run, hosted build runner, and
  save/deploy automation work.
| Preview pricing | OpenAI is free during preview; future pricing unknown. | OpenAgents launch is free public beta for selected orders, then credit-priced through the existing billing ledger and Stripe checkout. |

## Cloudflare Primitive Mapping

Use Cloudflare as the data plane and OpenAgents product surface as the control plane.

| Need | Cloudflare primitive | OpenAgents product surface owner |
| --- | --- | --- |
| Main product API and dashboard | Worker | `workers/api` |
| Customer session and admin auth | OpenAuth on Workers, KV storage | Existing auth boundary |
| Site metadata authority | D1 | New `site_*` tables |
| Site build source archives | R2 | `ARTIFACTS` bucket with `sites/<siteId>/...` prefixes |
| Site generated static assets | Workers Static Assets or R2 | Sites deployment service |
| Generated dynamic app execution | Workers for Platforms user Workers | Sites runtime service |
| Site request routing | Dynamic dispatch Worker | `sites.openagents.com/*` route |
| Per-site isolated runtime | Dispatch namespace user Worker | Cloudflare for Platforms |
| Per-site app structured data | D1 binding | Site storage provisioner |
| Per-site uploads | R2 binding/prefix | Site storage provisioner |
| Egress policy | Workers for Platforms outbound Worker | Later safety phase |
| Async builds/deploys | Queues and Workflows | Sites build/deploy services |
| Realtime operator updates | OpenAgents Sync | Existing sync outbox and DO |
| Logs and receipts | Workers observability plus D1/R2 receipts | Sites deployment service |
| Billing | D1 billing ledger, Stripe | Existing billing services |

### MVP Data Plane

For the June 5, 2026 launch path, the fastest honest implementation is:

```text
sites.openagents.com/*
  -> OpenAgents product surface Worker route
  -> D1 lookup by slug
  -> active deployment record
  -> R2 static bundle or generated Worker dispatch target
  -> response
```

This is enough for static and mostly-static generated customer sites such as
the OTEC launch page. It also gives the operator version/deploy lifecycle
without waiting for full Workers for Platforms automation.

### Full Parity Data Plane

For full OpenAI Sites parity, evolve to:

```text
sites.openagents.com/*
  -> Dynamic dispatch Worker
  -> D1/KV route lookup
  -> Workers for Platforms dispatch namespace
  -> per-site user Worker
  -> per-site D1/R2 bindings
  -> optional outbound Worker for egress control
```

Workers for Platforms is the right long-term primitive because Cloudflare
documents it for platforms that run untrusted customer or AI-generated code at
scale, with dynamic dispatch, per-user Workers, custom limits, observability,
tags, bindings, and customer domains.

## URL And Domain Model

Launch domain:

```text
https://sites.openagents.com
```

Path format:

```text
https://sites.openagents.com/<slug>
https://sites.openagents.com/<slug>/<site-path>
```

Ben OTEC launch:

```text
https://sites.openagents.com/otec
```

Admin/operator routes stay on the main app:

```text
https://openagents.com/admin/sites
https://openagents.com/admin/sites/<siteId>
https://openagents.com/admin/sites/<siteId>/versions/<versionId>
https://openagents.com/admin/sites/<siteId>/deployments/<deploymentId>
```

Customer routes stay clean:

```text
https://openagents.com/order
https://openagents.com/orders/<softwareOrderId>
```

Do not put deployment status, checkout status, OAuth state, access result, or
site errors into first-party product route query strings. This preserves the
clean public URL invariant.

## Product State Machine

### Site Project

```text
draft
  -> generating
  -> generated
  -> needs_review
  -> approved
  -> archived
  -> disabled
```

### Site Version

```text
planned
  -> building
  -> build_failed
  -> saved
  -> rejected
  -> superseded
```

### Site Deployment

```text
queued
  -> deploying
  -> active
  -> failed
  -> disabled
  -> rolled_back
```

### Site Access Mode

```text
owner_admins
openagents_core
customer_owner
custom_users
public
```

OpenAI access-mode mapping:

| OpenAI mode | OpenAgents equivalent |
| --- | --- |
| `admins_only` | `owner_admins` or `openagents_core` |
| `workspace_all` | `openagents_core` for internal OpenAgents workspace, later `team_all` |
| `custom` | `custom_users` |

OpenAgents adds `public` because customer landing pages and launch pages are a
first-class product need. Public mode must be explicit, review-gated, and
disable-able by core admins.

## D1 Model

Add a migration after `0031_stripe_billing.sql`.

### `site_projects`

Fields:

- `id`
- `software_order_id`
- `owner_user_id`
- `team_id`
- `project_id`
- `slug`
- `title`
- `prompt`
- `status`
- `access_mode`
- `visibility`
- `source_repository_provider`
- `source_repository_owner`
- `source_repository_name`
- `source_repository_ref`
- `active_version_id`
- `active_deployment_id`
- `created_at`
- `updated_at`
- `archived_at`

Constraints:

- `slug` unique while not archived.
- `software_order_id` nullable but unique when present.
- `status` is a checked enum.
- `access_mode` is a checked enum.
- No secret values.

### `site_versions`

Fields:

- `id`
- `site_id`
- `source_kind`
- `source_commit_sha`
- `source_archive_r2_key`
- `artifact_manifest_r2_key`
- `build_log_r2_key`
- `build_status`
- `build_command`
- `worker_module_r2_key`
- `static_assets_manifest_json`
- `d1_binding_name`
- `r2_binding_name`
- `metadata_json`
- `created_by_user_id`
- `created_by_run_id`
- `created_at`
- `saved_at`
- `rejected_at`

Rules:

- A version can be deployed only when `build_status = 'saved'`.
- Build logs in R2 must be redacted.
- `metadata_json` must be length-bounded and must not contain secret-shaped
  data.

### `site_deployments`

Fields:

- `id`
- `site_id`
- `version_id`
- `slug`
- `url`
- `runtime_kind`
- `runtime_script_name`
- `dispatch_namespace`
- `status`
- `deployed_by_user_id`
- `external_deployment_id`
- `started_at`
- `activated_at`
- `failed_at`
- `disabled_at`
- `rolled_back_at`
- `created_at`
- `updated_at`

Rules:

- Only one active deployment per site.
- Deployment must reference a saved version.
- URL must be derived from slug/domain policy, not arbitrary user input.

### `site_storage_bindings`

Fields:

- `id`
- `site_id`
- `kind`
- `binding_name`
- `cloudflare_resource_ref`
- `scope`
- `created_at`
- `updated_at`

Kinds:

- `d1`
- `r2`
- `kv`

MVP can use shared resources with per-site prefixes. Full parity should support
dedicated resources where billing and isolation require them.

### `site_environment_values`

Fields:

- `id`
- `site_id`
- `key`
- `kind`
- `secret_ref`
- `plain_value`
- `created_at`
- `updated_at`
- `deleted_at`

Rules:

- `kind = 'secret'` uses `secret_ref` only.
- `plain_value` is allowed only for non-secret configuration.
- Secret values must never be stored directly in D1, docs, logs, fixtures, or
  source archives.
- A change to environment values marks the next deployment as requiring
  redeploy.

### `site_access_grants`

Fields:

- `id`
- `site_id`
- `principal_kind`
- `principal_ref`
- `role`
- `created_at`
- `revoked_at`

Principal kinds:

- `user`
- `team`
- `admin`
- `public`

### `site_events`

Fields:

- `id`
- `site_id`
- `version_id`
- `deployment_id`
- `type`
- `summary`
- `actor_user_id`
- `actor_run_id`
- `payload_json`
- `created_at`

Rules:

- Events are the operator audit trail.
- `payload_json` is optional, redacted, bounded, and never public by default.

## API Surface

Customer-safe APIs:

```text
GET  /api/customer-orders/active
GET  /api/customer-orders/:orderId
GET  /api/customer-orders/:orderId/site
```

Operator-only APIs:

```text
GET  /api/operator/sites
POST /api/operator/sites
GET  /api/operator/sites/:siteId
PATCH /api/operator/sites/:siteId
POST /api/operator/sites/:siteId/generate
GET  /api/operator/sites/:siteId/versions
POST /api/operator/sites/:siteId/versions/:versionId/save
POST /api/operator/sites/:siteId/versions/:versionId/deploy
POST /api/operator/sites/:siteId/deployments/:deploymentId/disable
POST /api/operator/sites/:siteId/deployments/:deploymentId/rollback
GET  /api/operator/sites/:siteId/events
GET  /api/operator/sites/:siteId/storage
PATCH /api/operator/sites/:siteId/access
GET  /api/operator/sites/:siteId/env
PUT  /api/operator/sites/:siteId/env/:key
DELETE /api/operator/sites/:siteId/env/:key
```

Public site runtime:

```text
GET  https://sites.openagents.com/:slug
GET  https://sites.openagents.com/:slug/*
```

All operator APIs require core admin/team authorization. Customer APIs enforce
owner access. Public site requests enforce the site's access mode before
serving protected content.

## Build And Deploy Pipeline

### Generate

Inputs:

- `softwareOrderId`
- target slug
- source repository and branch
- product prompt
- access mode
- storage needs
- optional customer-supplied assets

Execution:

1. Create or find `site_projects` row.
2. Create a durable Autopilot goal linked to the site and order.
3. Dispatch Autopilot with a task packet or generated assignment.
4. Runner creates source, tests locally, and emits artifacts.
5. Worker records generation events and artifact refs.
6. Operator inspects the generated source and build result.

No ad hoc keyword routing should decide whether an order is a Site. For legacy
orders like Ben's OTEC request, an operator can explicitly convert the order to
a Site. Future automated classification must use a typed semantic selector or
structured planner with tests.

### Save Version

Inputs:

- `siteId`
- source commit or generated artifact ref
- build command
- generated bundle
- build log
- storage binding declaration

Execution:

1. Run deterministic build validation and inspect the latest receipt.
2. Redact and store build log in R2 when a live build runner exists.
3. Store source archive and artifact manifest in R2.
4. Insert `site_versions` with `build_status = 'saved'`.
5. Add `site_events` row.
6. Notify operator sync scopes.

### Deploy Version

Inputs:

- `siteId`
- `versionId`
- deployment audience/access mode

Execution for MVP static route:

1. Verify version is saved.
2. Create `site_deployments` row with `queued`.
3. Promote R2 artifact manifest to active deployment.
4. Update `site_projects.active_version_id` and
   `site_projects.active_deployment_id`.
5. Serve through the OpenAgents product surface Worker route under `sites.openagents.com/<slug>`.
6. Mark deployment active and record URL.
7. Notify customer/order and operator sync scopes.

Execution for full Workers for Platforms route:

1. Verify version is saved.
2. Create or update the user Worker script in the dispatch namespace.
3. Attach static assets and bindings.
4. Update dispatch route mapping.
5. Mark deployment active and record external deployment ID.
6. Record a rollback pointer to the previous active deployment.

## OTEC Launch Plan

Goal:

```text
Launch a public Autopilot Site for Ben's OTEC/SWAC floating datacenter website
at https://sites.openagents.com/otec.
```

Operator steps:

1. Confirm the `bensilone/openagents` `software_orders` row is still active.
2. Create `site_projects` with slug `otec`, public access, and
   `software_order_id` pointing to Ben's order.
3. Generate a first static site version from the request.
4. Include at minimum:
   - first-viewport OTEC/SWAC floating datacenter signal;
   - concrete sections for ocean thermal energy, SWAC cooling, gigawatt-scale
     floating datacenter operations, environmental concerns, commercial
     rationale, and contact/action;
   - no fake technical claims beyond what the prompt supports;
   - no private customer data;
   - OpenAgents attribution if product policy wants it.
5. Save the version.
6. Operator reviews content, links, assets, mobile layout, and public URL.
7. Deploy to `sites.openagents.com/otec`.
8. Update Ben's order status to `delivered` or a more specific future
   `site_deployed` status.
9. Record the deployment URL in the order/site projection.
10. Email or otherwise notify through the approved email ledger when enabled.

Day-one fallback if automated generation is not ready:

- The foreground/operator implementation may create the first OTEC site as a
  static generated artifact and push it through the Sites deployment service.
- This is acceptable only as a launch bootstrap. It must still create the same
  `site_projects`, `site_versions`, `site_deployments`, and `site_events`
  records so the product lifecycle is real.

## Customer Experience

### Order Status Page

The current customer order page should gain a Sites block when an order is
linked to a Site:

- Site title.
- Site status.
- Active URL when deployed.
- Last saved version status.
- Access mode.
- Expected next action.

It must not show:

- raw runner logs;
- provider account refs;
- callback tokens;
- hidden prompts;
- private build logs;
- shell output;
- Cloudflare API details.

### Admin Overview

The current admin overview should gain:

- Site project count.
- Site status.
- Active site URL.
- Link to operator site detail page.
- Order-to-site association.

### Operator Site Detail

The first operator detail page should show:

- order/customer summary;
- slug and URL;
- status and access mode;
- generate/save/deploy buttons;
- versions table;
- deployments table;
- storage bindings;
- environment key list without secret values;
- audit events.

## Security And Governance

Hard rules:

- No secrets in source, docs, logs, fixtures, D1 plain text, public projection,
  or issue comments.
- Backend authorization enforces customer/operator/public splits. UI hiding is
  not enough.
- Public mode must be explicit and review-gated.
- Disable and rollback are core operator actions.
- Secrets and environment changes require redeploy.
- Public pages must not carry OAuth, checkout, access, deployment, or runner
  result state in query strings or fragments.
- Customer-generated site code is untrusted until reviewed.
- User-generated dynamic Worker code should move through Workers for
  Platforms isolation before public dynamic execution.
- Use an outbound Worker or equivalent policy layer before allowing broad
  external egress from generated dynamic sites.
- Do not use ad hoc keyword matching for intent routing or site selection.

## Billing

Preview rule:

- Selected public beta Sites can be free while OpenAgents pays compute.

Paid rule:

- Future paid Sites use customer-facing OpenAgents credits, not raw provider or
  Cloudflare cost pass-through.
- Stripe checkout, when enabled, must use the existing typed Stripe Effect
  service and D1 billing ledger.
- Site build/deploy/runtime costs should create separate ledger sources so
  operators can distinguish generation cost, hosting cost, storage cost, and
  customer credits.

Suggested ledger source names:

- `site_generation_usage`
- `site_build_usage`
- `site_hosting_usage`
- `site_storage_usage`
- `site_credit_purchase`

## Implementation Slices

### Slice 1: Source Authority And Data Model

Deliverables:

- `docs/sites-plan.md`.
- D1 migration for `site_projects`, `site_versions`, `site_deployments`,
  `site_storage_bindings`, `site_environment_values`, `site_access_grants`,
  and `site_events`.
- Effect Schema domain module for Sites.
- Store/service layer with in-memory-testable D1 access.
- Tests for creation, order linking, slug uniqueness, active deployment, and
  secret redaction.

Acceptance:

- `bun run --cwd workers/api test <sites tests>`
- `bun run --cwd workers/api typecheck`

### Slice 2: Operator Sites API

Deliverables:

- Core-only operator routes to list, create, read, update access, and inspect
  Sites.
- Link a `software_orders` row to a Site project.
- Admin overview includes Sites summary.
- Tests for admin-only access and order linkage.

Acceptance:

- Non-admins receive forbidden/unauthorized.
- Admins can create a Site from Ben's order with slug `otec`.

Implementation note, June 4, 2026:

- Issue #59 added `/api/operator/sites`, `/api/operator/sites/:siteId`, and
  `/api/operator/sites/:siteId/access` for core-only list, create-from-order,
  read, and access updates.
- The admin overview software order projection now includes linked Site ID,
  slug, status, and visibility fields so orders can show their Site.
- Operator route errors return typed redacted JSON, including generic
  bad-request reasons for malformed payloads.

### Slice 3: MVP Static Site Runtime

Deliverables:

- `sites.openagents.com` Worker route.
- Runtime route lookup by slug.
- Active deployment serving from R2 static artifacts.
- Safe 404/disabled responses.
- No query-string state.

Acceptance:

- `GET https://sites.openagents.com/otec` can serve an active deployment.
- Disabled deployments stop serving.

Implementation note, June 4, 2026:

- Issue #60 added the `sites.openagents.com` Worker route and a typed static
  runtime backed by `site_projects`, active `site_deployments`, saved
  `site_versions`, and R2 artifact keys from `static_assets_manifest_json`.
- The MVP serves only public `openagents_static_r2` deployments. Missing,
  non-public, disabled, stale, or invalid deployments return a safe 404.
- Query-bearing Site URLs redirect to the same clean path without search
  parameters before runtime lookup.

### Slice 4: Version Save And Deploy

Deliverables:

- Save-version API.
- Deploy-version API.
- R2 artifact manifest format.
- Build log R2 storage with redaction.
- Deployment event receipts.

Acceptance:

- Saved version cannot deploy unless build succeeded.
- Deploy switches active version atomically enough for the MVP.

Implementation note, June 4, 2026:

- Issue #61 added operator save/deploy routes for reviewable Site versions:
  `POST /api/operator/sites/:siteId/versions` and
  `POST /api/operator/sites/:siteId/versions/:versionId/deploy`.
- Saved versions persist source archive refs, redacted build log refs, and
  static artifact manifest refs in R2 while storing the durable D1 version row.
- Deploying requires `build_status = 'saved'`, creates an active
  `openagents_static_r2` deployment, updates the Site active pointers, rolls back
  previous active deployments, and records rollback metadata in the event
  receipt.

### Slice 5: Autopilot Generation

Deliverables:

- Operator `generate` action that creates or continues an Autopilot goal.
- Task packet template for generated site work.
- Callback/event association between agent run, site, version, and order.
- OTEC generation packet or assignment.

Acceptance:

- A generated artifact can become a saved version.
- Product implementation remains in Autopilot unless the operator explicitly
  performs a launch bootstrap.

Implementation note, June 4, 2026:

- Issue #62 added `POST /api/operator/sites/:siteId/generate` for core
  operators to prepare a Site generation packet and mark the Site as
  `generating`.
- Generation packets include the target public URL, software order link, source
  repository, output contract, preflight checklist, and OTEC-ready generation
  goal while rejecting secret-shaped packet content.
- Generation requests record `site_generation.requested` events and can link
  an Autopilot run through `actor_run_id`; generated artifacts still become
  deployable only by saving a normal `autopilot_generated` Site version.

### Slice 6: Customer And Operator UI

Deliverables:

- Order page Sites block.
- Admin Sites list/detail.
- Version/deployment tables.
- Access mode controls.
- Generate/save/deploy/disable/rollback actions.

Acceptance:

- Customers see URL/status only.
- Core operators see lifecycle controls.

Implementation note, June 4, 2026:

- Issue #63 added a customer-safe Site projection to the active order API and
  order page. Customers receive only Site status and active URL.
- The admin overview now includes redacted Site lifecycle aggregates: access
  mode, active pointers, version/deployment counts, latest version/deployment
  summaries, storage binding summary, and latest event summary.
- The logged-in admin UI includes a Sites panel with lifecycle state and
  controls. Generate posts to the operator generation endpoint and refreshes
  overview state; save/deploy/rollback/disable are visible but disabled until
  the overview screen collects the required artifact or version input.

### Slice 7: Full Workers For Platforms Parity

Deliverables:

- Dispatch namespace binding.
- Site user Worker deployment service.
- Static asset upload session support.
- D1/R2 binding provisioner.
- Optional outbound Worker policy.
- Per-site custom limits and observability tags.

Acceptance:

- A dynamic Worker-compatible ES module Site can be deployed and routed.
- Static-only Sites remain supported.

Implementation note, June 4, 2026:

- Issue #64 added the `SITES_DISPATCH` Workers for Platforms dispatch
  namespace binding and Wrangler configuration for
  `openagents-sites-production`.
- The Site runtime now resolves active public deployments as either
  `openagents_static_r2` static assets or `workers_for_platforms` user Worker
  targets. Dynamic targets dispatch through the namespace binding after the
  same public, active, and saved-version checks.
- Deploy-version now records dynamic runtime metadata: runtime kind, script
  name, dispatch namespace, external deployment ID, active version, and
  storage binding names. Saved versions upsert D1/R2 binding metadata for the
  operator overview and future provisioner.
- Tests cover static serving, dynamic dispatch routing without live Cloudflare
  API calls, binding metadata, and rejection of dynamic deployments without a
  saved Worker module artifact.

### Slice 8: Secrets, Custom Access, And Public Launch Hardening

Deliverables:

- Environment values UI/API.
- Secret refs only, no secret values in D1.
- Custom access grants.
- Disable/rollback receipts.
- Audit export.

Acceptance:

- Secrets require redeploy.
- Access mode changes are auditable.
- Public launch checklist is enforced.

Implementation note, June 4, 2026:

- Issue #65 added typed operator governance APIs for Site environment values,
  access grants, redacted event projection, deployment disable, and rollback.
- Secret environment values accept and store secret refs only. Plain values are
  rejected when they contain secret-shaped material; public/admin overview
  projections expose environment keys and kinds, never values.
- Public deploys and public access widening require a completed launch
  checklist covering source, build, audience, secrets, and URL review.
- Environment changes on active Sites mark the Site `needs_review` and record a
  redeploy-needed event. Disable and rollback actions update deployment/project
  state and record receipts.
- Admin Sites UI now shows access grant count and environment key summaries
  without exposing values.

## Proposed GitHub Issues

Create issues in this order:

1. `Autopilot Sites: add source authority and D1 lifecycle model`
2. `Autopilot Sites: add core operator API and order linkage`
3. `Autopilot Sites: serve MVP static deployments on sites.openagents.com`
4. `Autopilot Sites: save and deploy reviewable versions`
5. `Autopilot Sites: wire Autopilot generation and OTEC launch packet`
6. `Autopilot Sites: add customer and operator UI`
7. `Autopilot Sites: add Workers for Platforms dynamic runtime`
8. `Autopilot Sites: add secrets, custom access, and launch hardening`

Implement them sequentially. After each issue:

1. Comment on the issue with summary, files changed, tests run, and remaining
   follow-ups.
2. Update this plan or related docs if the implementation changes the contract.
3. Commit and push.
4. Close the issue only when its acceptance criteria are proven.

## Launch Checklist For June 5, 2026

- `sites.openagents.com` route is configured.
- Sites D1 migration is applied.
- Ben's `bensilone/openagents` software order is linked to a `site_projects`
  row with slug `otec`.
- A saved version exists for the OTEC site.
- The saved version has a reviewed source/artifact manifest.
- Deployment is active.
- Customer order page shows the Site URL.
- Admin overview shows the Site.
- Disable path is tested.
- No secrets appear in D1 plain text, docs, issue comments, or build logs.
- Public URL is clean.
- `git status` is clean.
- Branch is merged to `main`.
- Temporary worktree/branch made for this work is removed after merge.

## Open Questions

1. Should public Sites display "Made with OpenAgents" attribution by default?
2. Should `sites.openagents.com/<slug>` be the only launch URL, or should
   `openagents.com/sites/<slug>` mirror it?
3. Should each dynamic Site receive a dedicated D1 database immediately, or is
   table-prefix D1 acceptable for public beta?
4. What is the first paid Sites package once free public beta ends?
5. Should Ben's OTEC page use generated imagery on day one, or only text and
   structured layout until image rights and provenance are explicit?

## Non-Goals For The First Launch

- Custom customer domains.
- Live card checkout for a Site order unless Stripe production config is
  already installed and smoked.
- PHI, PCI, child-directed apps, crypto transfers, financial transactions, or
  regulated workflow hosting.
- Broad external egress from unreviewed generated dynamic code.
- Public display of runner streams or raw build logs.
- Automatic keyword-based conversion of every "website" order into a Site.
