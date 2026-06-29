# OpenAI Sites Parity Implementation Audit

Date: 2026-06-05

Status: implementation audit and launch-readiness assessment. This document
does not change runtime policy, create a Site, deploy a Site, dispatch
the internal Adjutant supervisor, or alter customer/order visibility by itself.

## Executive Summary

OpenAgents product surface now has a serious Autopilot Sites foundation. It is no longer only a
plan. The repo contains:

- D1 tables for `site_projects`, `site_versions`, `site_deployments`,
  `site_storage_bindings`, `site_environment_values`, `site_access_grants`,
  and `site_events`.
- A typed `AutopilotSitesService` for creating Sites from software orders,
  saving versions, deploying versions, updating access, recording environment
  references, granting access, disabling deployments, rolling back deployments,
  listing events, and requesting generation.
- Operator APIs under `/api/operator/sites`.
- A public runtime route for `sites.openagents.com/<slug>` that serves active
  public static R2 deployments and can dispatch active Workers for Platforms
  deployments.
- An internal Adjutant run lifecycle path that can ingest a runner-emitted Site
  artifact receipt and save it as a normal `site_versions` row.
- Customer and public-safe projections that expose order progress, deployed
  Site URLs, public Autopilot milestones, and usage receipts without raw runner
  internals.

Compared to OpenAI Sites, OpenAgents product surface is strongest on the control-plane ledger:
projects, versions, deployments, public launch checklist, events, R2 artifact
storage, rollback/disable, and customer order integration are concrete and
test-covered. OpenAgents product surface is weakest on the self-serve product loop: there is no
customer-visible Sites plugin, no `@Sites`-style invocation, no hosted Sites
sidebar, no automatic source compatibility/build pipeline, no `.openagents`
metadata file writer, no per-Site Cloudflare resource provisioner, no protected
runtime auth for non-public Sites, and no hosted environment/secrets panel that
injects values into generated runtimes.

The honest launch statement for tomorrow is:

```text
Autopilot Sites public beta can launch operator-supervised, static public
customer Sites on sites.openagents.com from approved software orders.
```

The dishonest launch statement would be:

```text
Autopilot Sites has 100 percent OpenAI Sites feature parity today.
```

OpenAgents product surface has enough for Ben's OTEC public Site if the operator path is followed:
create/confirm the Site, assign Autopilot through the internal Adjutant
supervisor workflow, run enrichment/review, launch a Site-generation run, save
the produced artifact receipt as a version, review it, deploy it, and verify
`https://sites.openagents.com/otec`. It does not yet have enough to let
arbitrary users prompt, save, deploy, manage access, and manage hosted secrets
with the same self-service surface OpenAI describes.

## Naming Contract

`Adjutant` remains the internal codename for the supervisor implementation,
route names, payload tags, and database identifiers that coordinate Sites work.
Product UI, customer copy, public activity, share pages, and operator-facing
status text should call that capability `Autopilot`.

## Source Scope

OpenAI feature source:

- Official OpenAI Sites page fetched on 2026-06-05:
  `https://developers.openai.com/codex/sites`.
- User-provided Sites text in the current thread. The fetched official page
  matched the provided feature set.

OpenAgents product surface evidence reviewed:

- `docs/sites-plan.md`
- `docs/sites.md`
- `docs/2026-06-05-adjutant-sites-supervisor-audit.md`
- `docs/2026-06-05-ben-otec-exa-enrichment-runbook.md`
- `docs/autopilot-tasks/AGENTS.md`
- `docs/autopilot-tasks/2026-06-05-adjutant-site-fulfillment-runbook.md`
- `docs/autopilot-tasks/adjutant-site-task-template.md`
- `workers/api/migrations/0030_software_orders.sql`
- `workers/api/migrations/0032_autopilot_sites.sql`
- `workers/api/src/sites.ts`
- `workers/api/src/operator-sites-routes.ts`
- `workers/api/src/site-runtime.ts`
- `workers/api/src/site-runtime-routes.ts`
- `workers/api/src/adjutant-run-lifecycle.ts`
- `workers/api/src/adjutant-site-artifact-receipts.ts`
- `workers/api/src/customer-orders.ts`
- `workers/api/src/adjutant-public-activity.ts`
- `workers/api/src/sites.test.ts`
- `workers/api/src/operator-sites-routes.test.ts`
- `workers/api/src/site-runtime-routes.test.ts`
- `workers/api/src/adjutant-run-lifecycle.test.ts`
- `workers/api/src/customer-order-routes.test.ts`

## OpenAI Sites Feature Inventory

OpenAI Sites, as documented, includes these product capabilities:

1. Codex can create hosted websites, web apps, dashboards, tools, and games
   from a prompt.
2. Codex can prepare and publish a compatible existing project.
3. A Sites plugin can be installed and invoked with `@Sites`.
4. Enterprise admins can enable Sites through RBAC; Business workspaces have it
   enabled by default.
5. Codex validates the build before save or deploy.
6. Saving a version is distinct from deploying a version.
7. A saved version is associated with the source Git commit used for the build.
8. Deploying a version publishes a production URL.
9. Every deployment URL is production; review should happen before deploy.
10. Users can return to Sites projects, inspect saved versions, check
    deployment status, and change access.
11. A local `.openai/hosting.json` links the source tree to a hosted project
    and optional D1/R2 binding names.
12. Existing projects must build Cloudflare Worker-compatible ES module output.
13. New projects can start from a recommended Sites starter.
14. D1 is used for durable structured data.
15. R2 is used for files and uploads.
16. D1 plus R2 support uploaded files with searchable metadata.
17. Workspace-authenticated identity is available for internal Sites.
18. Public sign-in or an external identity provider is supported for
    authentication-enabled projects.
19. Access modes include `admins_only`, `workspace_all`, and `custom`.
20. Runtime environment values and secrets are managed through the Sites panel,
    not committed to source or `.openai/hosting.json`.
21. Updating hosted environment values requires redeploying the approved saved
    version.
22. Before deploy or access widening, operators should review source changes,
    database migrations, build success, selected version, intended audience,
    runtime secrets, deployment status, and production URL.

## OpenAgents product surface Implementation Map

### Project Model

Current state: partially implemented and durable.

Evidence:

- `workers/api/migrations/0032_autopilot_sites.sql` defines
  `site_projects`.
- `workers/api/src/sites.ts` defines `AutopilotSiteProject`.
- `AutopilotSitesService.createProjectFromSoftwareOrder` creates one active
  Site project per software order and one active Site per slug.
- `site_projects` stores `software_order_id`, `owner_user_id`, `team_id`,
  `project_id`, `slug`, `title`, `prompt`, `status`, `access_mode`,
  `visibility`, source repository fields, active version, and active
  deployment.

Comparison to OpenAI:

- OpenAgents product surface has a stronger order-linked project authority than OpenAI's public docs
  describe.
- OpenAgents product surface now has a typed `.openagents/site.json` metadata contract helper. The
  runtime can parse, validate, serialize, and derive linkage metadata from Site
  project/version/deployment records. The remaining parity gap is writing that
  file into customer source trees during provision/save/deploy workflows.

Required for parity:

- Wire `.openagents/site.json` generation/update into project provisioning and
  existing-project deploy workflows.
- Require the file for existing-project deploys, or generate it during project
  provisioning.
- Store `siteId`, optional source repo/path, D1 binding, R2 binding, access
  mode, target runtime, last saved version, active deployment, and optional
  `agentSurface` metadata in the file.
- Add tests proving Codex/Autopilot can create a new project without `siteId`
  and later backfill it after provisioning.

### Prompt-To-Site Creation

Current state: operator-supervised, not self-serve.

Evidence:

- Customer intake is `software_orders` backed, not a direct `@Sites` prompt.
- `AutopilotSitesService.requestGeneration` creates a generation packet,
  marks the Site `generating`, and records `site_generation.requested`.
- `docs/autopilot-tasks/adjutant-site-task-template.md` describes what a
  Site-generation packet must contain.
- `workers/api/src/operator-sites-routes.ts` exposes
  `/api/operator/sites/:siteId/generate`.

Comparison to OpenAI:

- OpenAI lets a user start a Sites task in a Codex thread with `@Sites`.
- OpenAgents product surface requires a submitted order or operator-created Site plus an operator
  generation request.
- OpenAgents product surface has the packet shape, but not a customer-facing plugin, composer
  affordance, or direct prompt-to-hosted-project workflow.

Required for parity:

- Add an explicit `@Sites` or "Create Site" action in OpenAgents product surface's product surface.
- Route it through a typed semantic/product selector, not keyword matching.
- Bind it to Site project creation, Autopilot assignment, preflight, and
  generation request.
- Show customers a clean Sites task state without exposing runner mechanics.

### Existing Project Deploy

Current state: deterministic compatibility receipt implemented; import/build
workflow still missing.

Evidence:

- `AutopilotSiteVersionSourceKind` includes `github_import`.
- `site_projects` can store a GitHub source repository from the linked
  software order.
- `saveVersion` can accept `sourceCommitSha`, source archive text, build
  command, build log text, and artifacts.
- `site_compatibility_checks` stores durable compatibility receipts for
  existing project/source inspection.
- `workers/api/src/sites-compatibility.ts` inspects submitted project files for
  package manager, build script, static output, Worker module entrypoint,
  unsupported SSR/Node runtime assumptions, D1/R2 needs, auth needs, env key
  names, findings, blockers, warnings, and evidence refs.
- `POST /api/operator/sites/:siteId/compatibility/check` records a receipt and
  `site_compatibility.checked` event.
- `GET /api/operator/sites/:siteId/compatibility` returns the latest receipt.

Comparison to OpenAI:

- OpenAI lets Codex inspect an existing compatible project, make required
  changes, and publish it.
- OpenAgents product surface can now record compatibility status for a source snapshot, but it does
  not currently clone, build, adapt, or import an existing project into the
  Sites lifecycle.

Required for parity:

- Add an import run that can generate a saved `github_import` version from a
  repo commit.
- Reject deploy attempts when compatibility has not been proven.
- Feed the compatibility receipt into task packets, build validation, and
  hosted import/save automation.

### Build Validation

Current state: deterministic build-validation receipts are implemented, while
live build execution is outside the Sites service.

Evidence:

- `site_build_validations` records compatibility check ID, source kind,
  source repository, source commit, stable source hash, package manager,
  requested/selected build command, output kind/path, manifest, bounded logs,
  truncation metadata, findings, blockers, warnings, evidence refs, and
  customer-safe status/next action.
- `workers/api/src/sites-build-validations.ts` validates operator-provided
  build candidate file snapshots deterministically, rejects credential-shaped
  inputs, bounds logs, preserves compatibility blockers, blocks SSR and
  unsupported Node runtime APIs, and emits `site_build_validation.checked`.
- `POST /api/operator/sites/:siteId/build-validations` records a validation
  receipt.
- `GET /api/operator/sites/:siteId/build-validations/latest` returns the
  latest validation receipt.
- `site_versions.build_status` supports `planned`, `building`,
  `build_failed`, `saved`, `rejected`, and `superseded`.
- `SaveAutopilotSiteVersionInput` requires `buildStatus` to be `saved` or
  `build_failed`.
- `saveVersion` stores build command, redacted build log, artifact manifest,
  source archive, and worker module in R2.
- `deployVersion` rejects versions unless `buildStatus === 'saved'`.
- `adjutant-run-lifecycle.ts` can save a version from an
  `AdjutantSiteArtifactReceipt`.

Comparison to OpenAI:

- OpenAI's flow has Codex validate a build before saving or deploying.
- OpenAgents product surface validates the persisted version state but does not itself run a build
  inside `AutopilotSitesService`.
- Build authority is delegated to Autopilot-runner receipts.

Required for parity:

- Define a typed Sites build receipt that includes command, exit code,
  artifact hash, output mode, static manifest, migrations, warnings, and
  compatibility result.
- Require a successful build receipt before `buildStatus: 'saved'`.
- Store failed build attempts as first-class versions or build attempts with
  bounded logs.
- Add tests proving a deploy cannot happen from a manually asserted saved
  version without a valid build receipt.

### Save Version

Current state: implemented and test-covered.

Evidence:

- `site_versions` table.
- `AutopilotSitesService.saveVersion`.
- R2 artifact storage for source archive, build log, worker module, and static
  asset manifest.
- `site_version.saved` and `site_version.build_failed` events.
- `workers/api/src/sites.test.ts` covers saving versions and saving generated
  work through the normal lifecycle.
- `workers/api/src/adjutant-run-lifecycle.test.ts` covers saving a completed
  Site artifact receipt as one generated Site version.

Comparison to OpenAI:

- OpenAgents product surface matches the save-before-deploy concept.
- OpenAgents product surface does not strictly require the source Git commit for every saved
  version. `sourceCommitSha` is optional, while OpenAI says saved versions are
  associated with the source Git commit used for the build.

Required for parity:

- Add a hosted build/import runner that clones or receives source, executes the
  build in an isolated environment, and feeds real build logs/artifacts into the
  receipt.
- Require `sourceCommitSha` for `autopilot_generated` and `github_import`
  saved versions unless a documented no-Git exception applies.
- Record source tree hash and artifact hash so a saved version is reproducible
  even if generated outside a normal Git checkout.
- Add an operator inspect endpoint that shows source refs, artifacts, build
  receipt, and review state together.

### Deploy Version

Current state: implemented for control-plane deployment and static runtime;
Workers for Platforms dispatch is metadata-backed but not provisioned.

Evidence:

- `site_deployments` table.
- `AutopilotSitesService.deployVersion`.
- `deployVersion` requires `buildStatus: 'saved'`.
- Public deployments require the launch checklist.
- Existing active deployments are rolled back when a new deployment is
  activated.
- `activeDeploymentUrl` returns `https://sites.openagents.com/<slug>`.
- `site-runtime-routes.ts` serves active public static artifacts and dispatches
  `workers_for_platforms` deployments through `SITES_DISPATCH`.
- `site-runtime-routes.test.ts` covers active static serving, clean URL
  redirects, public access checks, disabled Site/deployment rejection, and
  Workers for Platforms dispatch.

Comparison to OpenAI:

- OpenAgents product surface matches the save/deploy split and production URL semantics for public
  static deployments.
- OpenAgents product surface does not yet provision or upload a Workers for Platforms user Worker.
  It can dispatch a deployment only when runtime script and namespace metadata
  already exist.
- OpenAgents product surface does not yet expose a customer/operator deployment status page
  equivalent to an app sidebar project view.

Required for parity:

- Add deploy states `queued`, `deploying`, `active`, `failed` as real
  asynchronous states, not just immediate active records.
- Add Cloudflare deployment/provisioning service for static bundles and WFP
  Workers.
- Add deployment health checks and status polling.
- Add custom domain or subdomain management if the product promises it.

### Inspect Projects, Versions, And Deployments

Current state: operator API is present; product UI is incomplete.

Evidence:

- Operator API supports list/read Sites, list events, deploy, disable, and
  rollback.
- Admin view includes Autopilot assignment review and Site/order details.
- Customer `/order` projection exposes only safe order status and active Site
  URL.

Comparison to OpenAI:

- OpenAI exposes a Sites app/sidebar where users can return to projects,
  inspect saved versions, check deployment status, and change access.
- OpenAgents product surface exposes operator APIs and partial admin/customer projections, but no
  first-class Sites project browser for customers or operators.

Required for parity:

- Add `/sites` or `/admin/sites` UI with project list, filters, saved versions,
  deployments, access, environment values, event timeline, and rollback/disable
  actions.
- Add customer-safe `/order` or `/sites/:slug` inspection that shows only
  approved artifacts and deployment URL.
- Add route tests and scene tests for the Sites project browser.

### Supported Site Shapes

Current state: static public Sites are launchable; dynamic Worker Sites are
partially represented.

Evidence:

- Runtime kinds are `openagents_static_r2` and `workers_for_platforms`.
- Static assets manifest maps asset paths to R2 keys and optional HTTP
  metadata.
- Worker deployments require `workerModuleR2Key`, `runtimeScriptName`, and
  `dispatchNamespace`.

Comparison to OpenAI:

- OpenAI requires Cloudflare Worker-compatible ES module output for hosted
  projects.
- OpenAgents product surface can model and dispatch Worker-compatible runtime metadata, but does
  not yet build, upload, or bind generated ES modules.
- OpenAgents product surface has no recommended starter equivalent.

Required for parity:

- Define the OpenAgents product surface Sites starter and its build output contract.
- Add a compatibility checker for generated and existing projects.
- Add a WFP upload/provision path.
- Add tests proving a generated ES module can be deployed through
  `SITES_DISPATCH` with D1/R2 bindings.

### D1 Durable Structured Data

Current state: control-plane D1 exists; per-Site app D1 is metadata-only.

Evidence:

- `site_storage_bindings` records `kind = 'd1'`.
- `site_versions.d1_binding_name` stores the binding requested by a saved
  version.
- `upsertStorageBinding` records shared-prefix storage binding metadata.

Comparison to OpenAI:

- OpenAI Sites can provide D1 as relational storage for durable app data.
- OpenAgents product surface records that a version needs a D1 binding, but it does not provision a
  per-Site D1 database, create per-Site app tables, inject a D1 binding into a
  runtime Worker, or expose a D1 migration review loop for generated apps.

Required for parity:

- Decide between dedicated per-Site D1 databases and shared D1 table
  namespaces for MVP.
- Add a storage provisioner that records Cloudflare resource refs.
- Add migration artifacts to saved versions and require operator review before
  deploy.
- Bind D1 into WFP runtime deployments.

### R2 Files And Uploads

Current state: R2 is used for build artifacts; app upload storage is not
implemented.

Evidence:

- `AutopilotSitesService.saveVersion` stores source archives, build logs,
  worker modules, and static manifests in `ARTIFACTS`.
- `site_runtime` reads static assets from R2.
- `site_storage_bindings` can record `kind = 'r2'`.

Comparison to OpenAI:

- OpenAI Sites uses R2 for files and uploads.
- OpenAgents product surface uses R2 for platform artifacts and static hosting, but generated Sites
  do not have an upload API, object metadata model, or per-Site R2 prefix
  capability exposed to runtime code.

Required for parity:

- Add per-Site R2 prefix or bucket provisioning.
- Add runtime upload/download APIs and access checks.
- Add D1 metadata for uploaded files when searchable metadata is requested.
- Add redaction and content policy boundaries for uploaded files.

### Workspace Identity And Internal Sites

Current state: public runtime only.

Evidence:

- `site_projects.access_mode` can be `owner_admins`, `openagents_core`,
  `customer_owner`, `custom_users`, or `public`.
- `site_access_grants` can record users, teams, admins, or public grants.
- `site-runtime.ts` only resolves rows that are public active:
  `access_mode === 'public'` and `visibility === 'public'`.
- Non-public rows return 404 from the public runtime tests.

Comparison to OpenAI:

- OpenAI Sites supports workspace-authenticated user identity for internal
  Sites.
- OpenAgents product surface records internal access intent but does not serve protected Sites
  through authenticated runtime routes.

Required for parity:

- Add authenticated site runtime routes for non-public Sites.
- Enforce `owner_admins`, `openagents_core`, `customer_owner`, and
  `custom_users` against OpenAgents sessions and `site_access_grants`.
- Inject current user identity into dynamic Worker Sites through a typed
  request context.
- Add tests for protected Site access and denial.

### Public Sign-In Or External Identity Provider

Current state: not implemented.

Evidence:

- OpenAgents product surface uses OpenAuth/GitHub for the main app and customer sessions.
- Sites schema has access grants but no site-owned auth provider config.
- Runtime has no public visitor sign-in flow for generated Sites.

Comparison to OpenAI:

- OpenAI documents public sign-in or external identity provider support for
  authentication-enabled Sites projects.
- OpenAgents product surface does not yet support this for generated Sites.

Required for parity:

- Add site-owned auth mode metadata.
- Decide whether public visitor auth is OpenAuth-hosted, OAuth-provider-hosted,
  or generated-app-owned.
- Add callback URLs, session scopes, consent copy, and safe user projection.
- Add a strict no-credentials-in-generated-source policy.

### Access Modes

Current state: broader internal enum, incomplete runtime enforcement.

Evidence:

- OpenAgents product surface access modes: `owner_admins`, `openagents_core`, `customer_owner`,
  `custom_users`, `public`.
- Operator route can update access.
- Public access changes require the launch checklist.
- Access grants can be recorded.

Comparison to OpenAI:

- OpenAI modes are `admins_only`, `workspace_all`, and `custom`.
- OpenAgents product surface can represent `admins_only` as `owner_admins`.
- OpenAgents product surface can represent `custom` as `custom_users` plus grants.
- OpenAgents product surface does not have a direct `workspace_all` mode. The nearest internal mode
  is `openagents_core`, which is narrower than all workspace users.
- OpenAgents product surface adds `customer_owner` and `public`, which are useful for OpenAgents'
  customer Sites lane but not equivalent to OpenAI's workspace model.

Required for parity:

- Add `workspace_all` or define `openagents_workspace` if OpenAgents creates a
  full workspace member model beyond core team.
- Enforce every mode at runtime.
- Add access-mode transition tests that prove public widening requires
  checklist review.

### Runtime Environment Values And Secrets

Current state: metadata and safety checks exist; hosted runtime injection is
not implemented.

Evidence:

- `site_environment_values` stores `plain` values or `secret` references.
- Secret env values require `secretRef` and reject inline plain values.
- Plain env values and secret refs are scanned for secret-shaped material.
- Updating environment values on an active Site marks the Site `needs_review`.
- Events redact secret refs as `[SECRET_REF]`.

Comparison to OpenAI:

- OpenAI Sites manages hosted env vars/secrets in the Sites panel, keeps them
  out of `.openai/hosting.json`, and requires redeploy after changes.
- OpenAgents product surface records env metadata and flags redeploy review, but there is no Sites
  panel, no delete route, no Cloudflare secret write, no runtime binding
  injection, and no redeploy automation.

Required for parity:

- Add environment value list/update/delete UI.
- Store secret values only in Cloudflare Secrets Store, Worker secrets, or a
  dedicated encrypted secret service. D1 should store only references.
- Inject env bindings into WFP deployments.
- Require redeploy after every hosted env change.
- Add tests proving secrets never appear in source archives, logs, events,
  issue comments, customer projections, or public runtime responses.

### Review Before Share

Current state: checklist is implemented for public deploy and public access
widening; review surface is incomplete.

Evidence:

- `AutopilotSiteLaunchChecklist` has `sourceReviewed`, `buildReviewed`,
  `audienceReviewed`, `secretsReviewed`, and `urlReviewed`.
- Public deployments and public access changes require the checklist.
- Build status must be `saved` before deployment.
- Secret-shaped material is rejected from source archive, worker module,
  static manifest, plain env values, secret refs, event payloads, and
  generation packets.

Comparison to OpenAI:

- OpenAgents product surface's checklist directly covers most OpenAI review-before-share items.
- OpenAgents product surface lacks explicit database migration review for generated Site app data.
- OpenAgents product surface lacks a Codex-style review pane for source changes and diffs.
- OpenAgents product surface lacks a full selected-version review UI.

Required for parity:

- Add migration artifacts to saved versions.
- Add source/diff/asset/build log review UI.
- Add selected-version confirmation before deploy.
- Add access/audience and environment value review panels.

### Production URL And Clean URL Policy

Current state: implemented for public static and dispatch Sites.

Evidence:

- `activeDeploymentUrl` uses `https://sites.openagents.com/<slug>`.
- `site-runtime-routes.ts` matches only the configured `sitesHost`.
- Query-bearing public Site URLs redirect to clean URLs.
- Slug parser rejects unsafe path segments.
- Runtime returns 404 for disabled Sites/deployments and protected Sites.

Comparison to OpenAI:

- OpenAI says every Sites deployment URL is production.
- OpenAgents product surface treats active deployments as production under `sites.openagents.com`.
- OpenAgents product surface's clean URL redirect aligns with repo invariants.

Required for parity:

- Add a deployment status API and UI that confirms the production URL after
  deploy.
- Add custom hostname support only after DNS, certificate, access, and rollback
  policies are modeled.

## Feature Parity Matrix

| OpenAI Sites feature        | OpenAgents product surface current state                             | Parity status                 | Primary gap                            |
| --------------------------- | ----------------------------------------------- | ----------------------------- | -------------------------------------- |
| Create from prompt          | Software order plus operator generation packet  | Partial                       | No self-serve `@Sites` entrypoint      |
| Existing project deploy     | Compatibility receipts plus `github_import` metadata | Partial                       | No importer/build runner               |
| Sites plugin install/invoke | No plugin surface                               | Missing                       | Need product command/plugin equivalent |
| RBAC availability           | Core admin route checks                         | Partial                       | No role-based Sites feature gate       |
| Build validation            | Runner receipt and build status metadata        | Partial                       | No Sites-owned build executor/checker  |
| Save version                | `site_versions` plus R2 artifacts               | Mostly implemented            | Commit SHA optional                    |
| Deploy version              | `site_deployments` plus runtime route           | Implemented for static public | WFP provisioning missing               |
| Inspect versions/status     | Operator API and events                         | Partial                       | No first-class Sites UI/sidebar        |
| `.openai/hosting.json`      | Planned `.openagents` equivalent only           | Missing                       | No file contract implementation        |
| Worker ES module support    | `workers_for_platforms` metadata and dispatch   | Partial                       | No upload/provision/build path         |
| Recommended starter         | Not present                                     | Missing                       | Need starter and contract tests        |
| D1 app storage              | Binding metadata only                           | Partial                       | No per-Site D1 provisioning/binding    |
| R2 uploads                  | Artifact/static R2 only                         | Partial                       | No generated-app upload storage        |
| Upload metadata search      | Not present                                     | Missing                       | Need D1 metadata plus R2 object model  |
| Workspace identity          | Main app auth only                              | Partial                       | Protected Site runtime not implemented |
| Public/external auth        | Not present                                     | Missing                       | Need site-owned auth mode              |
| Access modes                | Internal enum plus grants                       | Partial                       | Runtime only enforces public/hidden    |
| Env/secrets panel           | D1 refs and safety checks                       | Partial                       | No UI, delete, secret store, injection |
| Redeploy after env change   | Marks active Site `needs_review`                | Partial                       | No automated approved-version redeploy |
| Review before share         | Launch checklist and secret scanning            | Partial                       | No source/migration/version review UI  |
| Disable/rollback            | Implemented                                     | Ahead of base doc             | Needs UI and operational runbooks      |
| Customer/public projection  | Implemented narrow projection                   | OpenAgents-specific           | Not an OpenAI feature, useful for us   |

## Ben OTEC Launch Assessment

Ben's order:

```text
Website for ocean based, OTEC powered, SWAC cooled, gigawatt scale, floating datacenter.
```

Target:

```text
https://sites.openagents.com/otec
```

What is real enough for the OTEC launch:

1. A `site_projects` row can be created from the software order with slug
   `otec`.
2. Autopilot can be assigned to the Site/order through the operator route.
3. Exa enrichment, research brief review, task packet creation, preflight, and
   launch are documented and test-covered in the internal Adjutant runbooks.
4. A runner can emit an `openagents.adjutant.site_artifact_receipt.v1` payload.
5. `adjutant-run-lifecycle.ts` can save that receipt as an
   `autopilot_generated` Site version.
6. An operator can deploy the saved version after the launch checklist.
7. The runtime can serve the active public R2 static artifact at the target
   URL.
8. Customer/public projections can show the deployed Site URL without exposing
   provider state, callback payloads, private prompts, or raw runner logs.

What remains manual or operator-only for the OTEC launch:

1. The customer cannot yet invoke a Sites plugin or self-serve generation.
2. The operator must create/confirm the Site, assignment, task packet, and
   launch.
3. The build must be performed by the runner or operator, then reported via a
   Site artifact receipt.
4. Version review is API/admin-surface driven rather than a polished Sites UI.
5. Deployment is static R2 first unless WFP metadata and script upload have
   already been provided.
6. Access is public-only at runtime for launched Sites.

Launch verdict:

```text
Go for a supervised public beta OTEC static Site.
No-go for claiming self-serve OpenAI Sites parity.
```

## Highest-Priority Parity Issues

These are the implementation issues required before claiming full parity.

### 1. Sites Project Browser And Review UI

Build a first-class Sites UI for core operators and customer-safe views:

- project list;
- Site detail;
- saved versions;
- deployment status;
- source/archive/build log references;
- static manifest;
- environment values;
- access grants;
- event timeline;
- deploy, disable, and rollback actions;
- source/build/audience/secrets/URL checklist confirmation.

### 2. Build And Compatibility Service

Create a Sites build service that can:

- inspect a repo or generated source bundle;
- classify static, Worker, D1, R2, upload, and auth needs;
- run a build in a controlled runner;
- validate Worker ES module compatibility;
- emit a typed build receipt;
- fail safely with bounded logs;
- prevent `saved` versions without proof.

### 3. `.openagents/site.json`

Implementation note, June 5, 2026:

- `workers/api/src/site-source-metadata.ts` defines the canonical
  `.openagents/site.json` schema version `openagents.site.v1`.
- The schema covers `siteId`, hosted project ID, software order ID, GitHub
  source refs, access mode, visibility, target runtime/slug/URL, D1/R2 binding
  names, last saved version, active deployment, and optional `agentSurface`
  links.
- Parse and serialize helpers reject unsupported access/runtime values and
  secret-shaped keys or values before writing metadata.
- This is now a typed source metadata contract, but not yet an automatic writer
  in the source checkout lifecycle.

Implement the local hosted-project metadata file:

```json
{
  "site_id": "<site-id>",
  "project_id": "<project-id>",
  "d1": "DB",
  "r2": "ASSETS",
  "access_mode": "owner_admins",
  "last_saved_version_id": "<version-id>"
}
```

It should be generated or updated by the Sites provisioner and never contain
secret values.

### 4. Protected Runtime Access

Add authenticated runtime serving for:

- owner/admin access;
- OpenAgents core team access;
- customer owner access;
- custom user/team grants.

Runtime must not leak the existence of private Sites to unauthenticated users.

### 5. D1/R2 App Storage Provisioning

Turn storage binding metadata into real resources:

- per-Site D1 or namespaced shared D1;
- per-Site R2 prefix or bucket;
- D1 migration review;
- upload object metadata;
- runtime binding injection;
- storage cleanup/retention.

### 6. Environment And Secret Management

Add hosted env/secrets management:

- operator UI;
- add/update/delete;
- secret store integration;
- redeploy-required state;
- runtime injection;
- redaction tests.

### 7. Workers For Platforms Deployment Automation

Implement actual generated Worker deployment:

- upload generated worker module;
- configure dispatch namespace;
- attach D1/R2/env bindings;
- record external deployment IDs;
- health check;
- rollback/disable;
- observability tags.

### 8. Self-Serve Sites Entry Point

Add a typed Sites action equivalent to `@Sites`:

- customer/operator prompt;
- existing project deploy request;
- durable order/site/assignment creation;
- preflight status;
- saved version review;
- deploy confirmation.

Do not implement this as keyword matching. It must use a typed command,
operator route, semantic selector, or explicit product action.

## Immediate Recommendation

For the next 24 hours, define the product as:

```text
Autopilot Sites public beta: OpenAgents Core can fulfill approved customer
software orders as public, reviewed, hosted Sites on sites.openagents.com.
```

Do not define it as:

```text
Self-serve OpenAI Sites parity for every customer and existing project.
```

The correct OTEC launch path is:

1. Keep `docs/2026-06-05-ben-otec-exa-enrichment-runbook.md` as the evidence
   runbook for the canonical OTEC smoke.
2. Use `docs/autopilot-tasks/2026-06-05-adjutant-site-fulfillment-runbook.md`
   for the operator launch sequence.
3. Require source, build, audience, secrets, and URL review before deploy.
4. Deploy the first OTEC version as `openagents_static_r2` unless WFP provisioning
   is complete.
5. Record the deployed URL in the Site and customer order projection.
6. Publicly expose only approved Site status, URL, public-safe milestones, and
   sanitized usage receipts.

## Completion Standard For True Parity

OpenAgents product surface reaches OpenAI Sites parity only when this audit can be updated to say
all of the following are true:

- A user can explicitly start a Sites task from a prompt or existing project.
- A Sites project is durably linked to source through `.openagents/site.json`.
- The system can validate and build compatible static and Worker output.
- Saving and deploying are separate product actions.
- Saved versions are tied to source commits or documented generated-source
  hashes.
- D1 and R2 app storage are provisioned and bound to generated runtime code.
- Runtime env values and secrets are managed outside source and injected on
  redeploy.
- Protected Sites enforce owner/admin/workspace/custom access at runtime.
- Public/external auth exists for auth-enabled Sites.
- Operators can inspect versions, deployments, access, env values, source,
  migrations, build logs, and production URL in a Sites UI.
- Deployment status, rollback, disable, and customer notification are
  auditable.
- No secrets, raw provider payloads, private prompts, callback tokens, or
  runner internals leak into customer or public projections.

Until then, OpenAgents product surface should market Autopilot Sites as an operator-supervised beta
that is intentionally narrower than OpenAI Sites but already aligned with the
same Cloudflare-native architecture.
