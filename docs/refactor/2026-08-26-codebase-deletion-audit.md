# Codebase deletion audit

- Date: 2026-08-26
- Baseline: `801dd14246fb0f9d87ec0acb24396e08a9b8129a`
- Scope: `OpenAgentsInc/openagents`, compared with the live Phoenix application
  in `OpenAgentsInc/openagents.com`
- Purpose: identify code that can be deleted, code that needs a bounded
  migration, and code that requires external decommissioning before deletion

## Executive conclusion

This repository still behaves as if it owns a TypeScript web application, a
TypeScript API, two CLIs, mobile and desktop clients, several independent
services, a managed-computer control plane, voice infrastructure, and a large
contract laboratory. That is not the current product boundary.

The current product has two confirmed execution roots:

- The Phoenix application in `OpenAgentsInc/openagents.com` owns the public
  website, API, forum, forge, inference, account, credit, conversation, Box,
  computer, and voice-facing backend surfaces.
- `crates/openagents-cli` builds the released `openagents` binary and its
  `coder` front door.

The strongest deletion candidate is the entire legacy
`apps/openagents.com` tree. The Phoenix taxonomy explicitly identifies its
TypeScript Worker as historical and says that it no longer answers for the
domain. The monorepo nevertheless continues to build, test, and expose deploy
commands for that old application. This duplicate tree contains 3,000 or more
tracked files and about 95.7 MiB of tracked content when its site, Worker,
packages, documentation, scripts, and staged assets are counted together.

The repository can also remove several unused Rust crates, the retired
TypeScript CLI after its remaining consumers migrate, and many TypeScript
packages after their obsolete root applications are removed. Voice, Pylon,
Khala services, and managed-computer infrastructure need a live Google Cloud
inventory before deletion. Source references and Terraform state prove that
these systems were built; they do not prove that they still receive production
traffic.

The recommended end state is a smaller repository with these responsibilities:

- the Rust CLI, its plugin development kit, and its release tooling;
- any verified external voice or managed-computer services that Phoenix still
  calls;
- canonical schemas and generators that the retained Rust or external services
  actually consume;
- focused acceptance and benchmark tools for those retained products;
- durable historical records under `docs/transcripts` and other records that
  repository policy requires preserving.

Everything else should be deleted from the active workspace. Git history is
the archive for source code.

## Product direction added after the audit

The owner clarified the target architecture on 2026-08-27:

- Rust is the default language for the CLI, local runtimes, and standalone
  services.
- Phoenix and Elixir remain the web and backend authority.
- Rust can supplement Phoenix through an explicit service, port, or NIF
  boundary.
- Plan to retire most or all TypeScript. A retained TypeScript path must name
  a current consumer, owner, replacement boundary, and retirement plan.
- Coder remains the front door to Computers, Forge, Forum, Inference, Issues,
  Memory, Nostr, Plugins, Sarah, Traces, Training, and Wallet.
- Local persistence remains the default. Cloud synchronization remains
  optional.

This direction strengthens the deletion test without narrowing the product.
Delete obsolete TypeScript implementations, but preserve or replace the CLI
namespaces and contracts that express current product intent. Product intent
is evidence for a replacement boundary; it is not evidence that a stale
implementation remains deployable.

## First-principles test

A path belongs in the active repository only if it satisfies at least one of
these conditions:

1. A confirmed product execution root imports or executes it.
2. A current release process packages it.
3. A verified deployed service runs it.
4. A retained generator produces bytes that a current product compiles or
   validates.
5. A repository rule requires the record to remain, such as
   `docs/transcripts`.

The following facts are not sufficient evidence of current use:

- inclusion in `pnpm-workspace.yaml` or the root Cargo workspace;
- a root test script that names the path;
- another stale package importing it;
- a Dockerfile, deploy script, Terraform declaration, or historical receipt;
- recent commits made to keep an obsolete aggregate check green;
- documentation that calls the path current when the live application's
  taxonomy says otherwise.

This distinction matters because the root `package.json` currently creates a
closed loop: stale applications import stale packages, and the aggregate test
suite then treats both sides of that loop as required. The loop proves internal
consistency, not product reachability.

## Evidence and limits

This audit used these sources:

- the Cargo and pnpm workspace manifests;
- package dependency and reverse-dependency edges;
- Rust symbol use, build scripts, release scripts, Dockerfiles, and Terraform;
- the Phoenix source tree, routes, Mix configuration, deployment tree, and
  `docs/taxonomy.md`;
- current repository invariants and prior cleanup records;
- path-specific Git history as secondary evidence.

At the baseline commit, the repository contains:

| Area | Tracked files | Tracked size |
| --- | ---: | ---: |
| `apps` | 5,254 | 112.5 MiB |
| `packages` | 3,112 | 38.2 MiB |
| `docs` | 2,792 | 137.6 MiB |
| `crates` | 173 | 5.0 MiB |
| `plugins` | 80 | 2.3 MiB |
| `scripts` | 233 | 2.2 MiB |
| `infra` | 98 | 0.3 MiB |

The repository has 5,860 tracked `.ts` files, 291 tracked `.tsx` files, 168
tracked `.mjs` files, 27 Cargo packages across all workspaces, and 118
`package.json` files including 88 package manifests under `packages`. The
Phoenix repository has no runtime import of these TypeScript packages. It uses
Node for frontend asset compilation, which does not make this repository's
TypeScript backend part of Phoenix.

This audit did not query or mutate production infrastructure. Any disposition
that depends on a running service is therefore marked **verify and
decommission**, not **delete now**.

## Deletion cohort 1: legacy `openagents.com` application

### Disposition

Verify that its old deployment is drained, then delete the entire
`apps/openagents.com` directory as one coordinated change.

### Evidence

The live Phoenix taxonomy states that:

- Phoenix serves the current `openagents.com` product and its API.
- The TypeScript Worker in this monorepo no longer answers for the domain.
- The old `/trace/{uuid}` and `/api/traces` surfaces return `404` on the live
  domain.
- The directory named `apps/openagents.com` is not the live Phoenix
  application.

The obsolete tree still contains:

- `apps/openagents.com/apps/start`, a TanStack site;
- `apps/openagents.com/workers/api`, a Node API with historical Cloudflare and
  Cloud Run paths;
- six nested packages for email, Mullet, and sync behavior;
- four standalone Rust/Wasm demos plus the `infra-explainer` generator;
- hundreds of scripts, documents, fixtures, and staged public assets.

The root `package.json` still exposes `dev:openagents.com`,
`test:openagents.com`, and an old `check:deploy` path. The nested manifests
still expose build and deployment commands. These commands should be removed,
not preserved as a second backend.

### Delete with the application

- `apps/openagents.com/apps/start`
- `apps/openagents.com/workers/api`
- `apps/openagents.com/packages/email-templates`
- `apps/openagents.com/packages/mullet-schema`
- `apps/openagents.com/packages/mullet-sim`
- `apps/openagents.com/packages/sync-client`
- `apps/openagents.com/packages/sync-schema`
- `apps/openagents.com/packages/sync-worker`
- `apps/openagents.com/apps/diamond-hands`
- `apps/openagents.com/apps/market-demo`
- `apps/openagents.com/apps/work-demo`
- `apps/openagents.com/apps/infra-explainer`
- `apps/openagents.com/apps/infra-explainer/diagrams-gen`
- old site Dockerfiles, deploy scripts, route manifests, staged Wasm, and
  site-only documentation under the same root

Delete the directory as a unit instead of attempting to preserve isolated
tests or historical routes. Git history retains those implementations.

### Root cleanup required in the same change

- Remove the workspace entries from `pnpm-workspace.yaml`.
- Remove the old development, test, typecheck, and deploy scripts from the root
  `package.json`.
- Regenerate `pnpm-lock.yaml` after the package graph changes.
- Remove old app exceptions from architecture guards and inventories.
- Remove the Phoenix branch from `.githooks/pre-push`; this repository no
  longer contains the Phoenix application.
- Update `AGENTS.md`, `INVARIANTS.md`, `PRODUCT.md`, and documentation that
  assigns current product authority to this directory.

## Deletion cohort 2: stale Rust crates

The repository has eight root Cargo workspace members, 14 plugin workspace
packages, and five standalone web or build crates.

| Path | Disposition | Reason |
| --- | --- | --- |
| `crates/openagents-cli` | Keep | `ops/release-cli.sh` builds and uploads this package's `openagents` binary as the current CLI. |
| `crates/all-work-contract` | Delete now | The CLI declares the crate but uses no symbols from it. The crate only wraps generated Rust from the TypeScript package. Remove the unused dependency, workspace member, crate, and Rust-only conformance command together. |
| `crates/oa-desktop-audio` | Delete now | The deleted Electron desktop application was its consumer. No current release or deployment builds it, and Phoenix voice does not reference it. |
| `crates/oa-cloud-run-bridge` | Verify and decommission | Its own `HISTORICAL.md` calls it transitional and says new production paths do not use it. Terraform names the Cloud Run service; the old deploy configuration and runbook name an apparently out-of-band secret. Drain and remove each resource before source deletion. |
| `crates/oa-codex-control` | Hold for cloud inventory | Docker and Google Cloud scripts build it. The GCE provider is implemented, but Phoenix has no endpoint or client boundary to it, so repository evidence does not establish current product traffic. |
| `crates/oa-node` | Hold for cloud inventory | It forms the managed-computer subsystem with `oa-codex-control` and `oa-workroomd`. Repository wiring does not prove live use. |
| `crates/oa-workroomd` | Hold for cloud inventory | It has a large current-looking Codex and workroom implementation, but Phoenix does not import or call it directly. |
| `crates/openagents-cloud-contract` | Keep with cloud group; remove from CLI | The three cloud daemons use it. The CLI declares it but uses no symbols from it. Remove the CLI edge now and decide the crate with the cloud subsystem. |

The standalone Rust demo crates under `apps/openagents.com` are not members of
the root workspace. Their only consumers are the old Node application's Wasm
routes. Delete them with deletion cohort 1.

### Plugin workspace

Keep `plugins/pdk` and the 13 guest plugin crates. The Rust CLI discovers
`plugins/*/manifest.json`, `/resume` requires `plugins/foreign-sessions`, and
Rust CLI tests load several checked-in digest-pinned Wasm artifacts, including
`word-stats`, `file-stats`, and `foreign-sessions`.

The current release script ships only the CLI binary. It does not bundle the
plugin catalog. That is a distribution gap, not evidence that the plugin source
is stale. Decide whether plugins are checkout-only developer features or
installed product features before pruning individual plugins.

## Completed deletion cohort 3: TypeScript CLI

Issue #136 retired `packages/openagents-cli`. The native
`crates/openagents-cli` crate now owns the installed command, Git credential
helper, benchmark adapter, and generated Coder surfaces. The review pipeline
moved to the focused `packages/coder-review` package because it analyzes
benchmark artifacts and is not a user-facing CLI namespace.

The npm package remains published only as a historical artifact. Deprecating
it in the npm registry requires registry credentials and does not justify
retaining its source tree.

## Deletion cohort 4: duplicate and leaf applications

These applications have strong code-level deletion cases but should be removed
in bounded changes so their package closures and any external resources can be
handled together.

| Path | Disposition | Required action |
| --- | --- | --- |
| `apps/forum` | Delete | Phoenix owns the live forum. This directory describes itself as a narrow mount-contract stub. |
| `apps/forge-git-service` | Verify, then delete | Phoenix owns the forge and Git plane. Verify that the old Cloud Run service has no traffic, then remove its deploy and Terraform wiring. |
| `apps/acceptance-runner` | Delete with old Worker | It leases work from and sends callbacks to the old TypeScript Worker. Preserve it only if an external settlement loop is proved live. |
| `apps/oa-queue-worker` | Verify and decommission, then delete with old Worker | It posts to the retired API and mirrors retired queue names, but it also has a Cloud Run deploy path and named secrets. |
| `apps/ai-sdk-harness-poc` | Deleted | Commit `14b35d1a5f` removed the unconsumed proof of concept. |
| `apps/qa-runner` | Deleted | No deployed service or current product root consumed it. The cleanup also removed its root orchestration, private Khala QA harness, and test-only production APIs. The immutable npm artifact remains historical only. |
| `apps/openagents-mobile` | Deleted | Commit `090b7a4a9a` removed the unsupported app and its command-outbox closure after the push worker and scheduler were deleted. Database rows remain for the database export review. |
| `apps/aiur` | Deleted | Commit `e65fc2238c` removed the source after its Cloud Run service was deleted. |
| `apps/khala-capture` and `apps/khala-live-hub` | Deleted | A seven-day sample found no client traffic. The capture daemon produced 9,995 of the 10,000 bounded entries by posting to `/append`; the rest were health checks. The three Cloud Run services were deleted before source removal. Cloud SQL remains. |

## TypeScript package reduction

There are 87 package manifests under `packages` and 3,112 tracked package
files. The Rust CLI has no npm runtime dependency graph. After deletion cohort
1, most package references will originate only from other stale TypeScript
components or aggregate root tests.

Delete packages by obsolete root application and its transitive dependency
closure. Do not evaluate each package only by whether another package imports
it.

### Delete with the legacy web and API tree

The following global packages have no confirmed consumer outside the old site
or its related tests and records:

- `agent-experience-memory`
- `agent-readiness`
- `analytics`
- `public-activity-timeline`
- `public-nostr-chat`
- `reactor-contracts`
- `tassadar-executor`
- `ui`
- the `mkt-swp`, `mkt-swp-compare`, `mkt-swp-destination`, `mkt-swp-pair`, and
  `mkt-swp-status` family

Some behavior contracts and documents refer to these packages. Update or
delete those records with the code; a documentation reference is not a runtime
consumer.

### Delete or consolidate leaf packages

These packages have no package consumer or product build root and should be
checked only for published-package obligations before deletion:

- `connector-sidecar`
- `forensic-loupe-adapter`
- `input-bindings`
- `mkt-swp-session-store`
- `review-round`
- `world-client`
- `world-contract`

This list is a starting set, not the full package deletion set. Once obsolete
application roots leave the workspace, generate a fresh reverse-dependency
report. Every remaining package must name a retained root, generator output,
or external consumer.

### Retain or consolidate carefully

- Keep `packages/all-work-contract` while `packages/omega-effectd` or another
  TypeScript consumer remains. If those
  consumers are deleted, decide whether its schema is still canonical. The
  unused Rust wrapper does not justify retaining the entire TypeScript package
  by itself.
- Keep `packages/cloud-contract` until the Rust cloud contract becomes the sole
  authority or a generator replaces the duplicated schemas.
- Keep `packages/atif` temporarily while retained trace consumers and migration
  tests use it. The Rust CLI now owns local conversation persistence and ATIF
  export. Replace or remove the TypeScript schema after the old Worker and its
  trace routes leave the repository.
- Keep voice and cloud package closures until their services are inventoried.

## External systems that require verification

### Voice and LiveKit

Treat this as one coherent possible exception:

- `apps/openagents-audio`
- `apps/openagents-audio-edge`
- `apps/sarah-livekit-agent`
- `apps/sarah-nostr-signer`
- `packages/audio-contract`
- `packages/authority`
- `packages/nip90`
- `packages/postgres-runtime`
- `packages/runtime-platform`
- `packages/sarah`
- `infra/livekit*` and `infra/modules/livekit-*`
- the corresponding Cloud Build files and deployment scripts

Phoenix exposes current voice-facing routes, but it does not import these
services. Inventory GKE workloads, Cloud Run services, DNS, Redis, Nostr signer
traffic, and Phoenix environment configuration. If Phoenix does not call this
stack, decommission it and delete the full closure rather than preserving it as
generic “voice/cloud stuff.”

### Managed computers

Treat these paths as one subsystem:

- `crates/oa-codex-control`
- `crates/oa-node`
- `crates/oa-workroomd`
- `crates/openagents-cloud-contract`
- related `docker/cloud` files, `scripts/cloud` files, fixtures, and cloud docs
- `apps/pylon/deploy/agent-computer`

Phoenix owns Box and computer-facing product routes, but current documentation
does not establish a live call from Phoenix to this control plane. Inventory
GCE instances, Cloud Run services, Artifact Registry images, secrets, logs,
and recent requests. If no live caller exists, delete the subsystem. If it is
live, document the Phoenix-to-control-plane boundary and remove unrelated
experiments around it.

### Khala, updates, and Pylon

The following services form independent closures and need external-state or
owner verification:

- `apps/pylon`
- its remaining Khala Sync, portable-session, SQLite, ACP, and update package
  dependencies

The update feed, standalone Khala capture/hub services, QA Runner, and private
Khala QA harness are removed. Cloud SQL remains under the duplicate-backend
drain because the old monolith, queue, and Forge paths still share it. Pylon
remains until its old backend and repository-check callers are removed or
replaced.

## Infrastructure and operations

Do not delete Terraform before decommissioning or transferring the resources
in its state. `infra/prod` still declares Cloud SQL instances, update and state
buckets, secrets, `oa-updates`, `oa-cloud-run-bridge`, the old monolith, and a
public load balancer. Removing HCL alone abandons resources; it does not stop
or delete them.

For each Terraform root:

1. List the state and map every resource to a retained product.
2. Check live traffic, logs, DNS, secrets, images, schedules, and billing.
3. Back up the state and the durable data before any state move or destroy.
4. Transfer retained resource ownership to the Phoenix repository or another
   explicit service repository. Import it into the destination state, verify
   the destination configuration and a no-op plan, and only then remove it
   from the source state. Use a reviewed state-move mechanism where supported.
5. Drain obsolete resources and obtain backups or exports for durable data,
   including the forge repository disk and Cloud SQL databases.
6. Remove deletion protection and `prevent_destroy` guards in a separate,
   reviewed change only after the drain and backups are verified. Cloud SQL,
   Secret Manager, the forge network and disk, and LiveKit all contain such
   guards.
7. Destroy obsolete resources through an applied plan. Let the successful
   apply update state; do not use `state rm` as a substitute for destruction.
8. Remove the source, deployment scripts, and documentation after the resource
   move or destruction succeeds.

The `infra/prod` root manages the same
`openagentsgemini-terraform-state` bucket that its backend uses. Migrate every
state prefix and verify the new backend before removing that bucket from source
state ownership. Handle its final retention or deletion out of band; an
ordinary destroy must not attempt to remove its own active backend.

Other operational findings:

- Keep `ops/release-cli.sh`; it is the current seven-target Rust CLI release
  path.
- Fix `.githooks/pre-push`. It still runs `cargo check -p coder-lite` and
  `cargo test -p coder-lite`, which now fail because that crate was deleted.
  Target `openagents-cli` instead.
- Remove the hook's `apps/openagents.com` Phoenix branch. The real Phoenix app
  is in another repository.
- Retire the installed `ops/owned-runner/khala-code-qa-nightly.*` systemd
  units before deleting the checked-in files. Disable and stop the timer, stop
  an active service, remove the installed units, run `systemctl daemon-reload`,
  and verify that `systemctl list-timers` no longer lists it. The service
  fetches `origin/main` and runs the legacy TypeScript `qa:nightly` root.
- Treat `runners/py-bench-runner` and its Cloud Batch scripts as one lane.
  Before deletion, drain queued and running Batch jobs and inventory Artifact
  Registry tags, service accounts and IAM grants, Cloud Build activity, GCS
  task and artifact prefixes, and scheduler or automation callers.
- Do not recreate GitHub Actions as part of cleanup. Their absence is
  deliberate; establish a small forge or local gate for the retained roots.

Ignored local directories are local-state cleanup, not Git deletion. `dist`
and Terraform provider caches can be rebuilt. Inspect `.release` before
removal: close any interrupted transaction and copy any required receipts to
their durable documentation location first.

## Documentation and contract cleanup

The documentation tree is larger than the active Rust source tree by more than
an order of magnitude. Much of it records programs whose code and product
surfaces are no longer active.

Apply these rules:

- Preserve `docs/transcripts` as required by repository policy.
- Preserve accepted product, assurance, security, and migration records when
  another policy requires immutable historical evidence.
- Move current operational documentation next to its retained owner where that
  improves discoverability.
- Delete ordinary implementation guides, runbooks, generated inventories, and
  architecture descriptions with the code they describe.
- Replace broad historical narrative with a short tombstone that names the
  last commit only when readers still need a migration pointer.
- Correct `AGENTS.md` and `INVARIANTS.md` in the same change that removes an
  authority boundary. An invariant cannot make retired code current.

Do not create a second archive inside the active repository. Git retains every
deleted byte.

## Execution status

Phase 1 was completed on 2026-08-26 against recovery baseline
`84682e9c4862abb23d8da731370cfe99ce7e7447`:

- The pre-push hook now tests `openagents-cli` and no longer treats the legacy
  TypeScript tree as an in-repository Phoenix application.
- The CLI no longer declares unused dependencies on
  `openagents-all-work-contract` or `openagents-cloud-contract`.
- `crates/all-work-contract` and `crates/oa-desktop-audio` were deleted. The
  TypeScript `packages/all-work-contract` generator remains because it has
  current TypeScript consumers.
- Root authority documents now identify `OpenAgentsInc/openagents.com` as the
  sole current web application and backend and `crates/openagents-cli` as the
  terminal authority.
- Cargo metadata and the repository surface inventory were regenerated after
  the deletion.
- `cargo test --workspace`, the all-work generation check, the Coder surface
  check, the assure-repo inventory check, and the release-contract tests pass.

Phase 4 was completed in issue #136. Phases 2, 3, 5, and 6 remain open under
issues #144 through #150. The duplicate-backend inventory is recorded in
[`2026-08-27-duplicate-backend-inventory.md`](./2026-08-27-duplicate-backend-inventory.md).

Issue #149 has removed these abandoned roots and their deployment resources:

- the AI SDK harness proof of concept and Python benchmark runner;
- Aiur and the standalone update service;
- the mobile client and private mobile command outbox;
- Khala capture and live hub services;
- QA Runner, its root orchestration, and the private Khala QA harness.

Removing QA Runner exposed and removed test-only browser, terminal, behavior
receipt, plan-catalog, Pylon budget, and QA graph APIs. The workspace now has
108 packages, down from 118 at the audit baseline.

## Execution order

### Phase 1: repair the active boundary

1. Fix the pre-push hook to test `openagents-cli`.
2. Remove the CLI's unused `openagents-all-work-contract` and
   `openagents-cloud-contract` dependencies.
3. Delete `crates/all-work-contract` and `crates/oa-desktop-audio`.
4. Record the Phoenix repository as the sole current web/backend owner in root
   documentation.

Status: complete on 2026-08-26.

### Phase 2: drain the duplicate backend

1. Inventory DNS, load-balancer backends, Cloud Run traffic, secrets,
   databases, buckets, scheduled callers, and installed runners associated with
   `apps/openagents.com`, `apps/acceptance-runner`, and
   `apps/oa-queue-worker`.
2. Transfer any retained data or resource ownership to the Phoenix repository.
3. Drain and decommission obsolete services through the infrastructure process
   in this audit.

Status: in progress in issue #145. The first inventory found active monolith
schedulers, an authentication hostname on the old load balancer, deployed
queue and bridge services, and durable Forge and SQL state. Source deletion
must wait for the ordered drain and migration proofs.

### Phase 3: remove the duplicate backend

1. Delete `apps/openagents.com` in full.
2. Delete `apps/forum`, `apps/acceptance-runner`, and `apps/oa-queue-worker`
   with their obsolete Worker paths.
3. Remove root scripts, workspace entries, guards, and site-only packages.
4. Regenerate the pnpm lockfile and package inventory.

Status: issue #146, blocked by issue #145.

### Phase 4: finish the CLI consolidation

Completed in issue #136:

- Migrated the credential helper, benchmark adapter, shell entry points, and
  current documentation.
- Deleted `packages/openagents-cli` and its TypeScript-only generation and
  publishing paths.
- Kept one native CLI release gate and one Coder surface generator.

### Phase 5: collapse the TypeScript package graph

1. Choose the retained voice, cloud, Pylon, mobile, QA, and benchmark roots.
2. Compute package reachability from only those roots.
3. Delete every disconnected package component.
4. Consolidate duplicate TypeScript and Rust schemas before deleting the old
   authority.
5. Replace the root aggregate suite with checks for retained products only.

Status: issue #150, blocked by the root-disposition issues.

### Phase 6: decommission remaining external systems

1. Inventory Google Cloud, installed systemd units, package registries, and
   published npm consumers.
2. Drain and destroy services with no retained product caller.
3. Transfer retained infrastructure ownership out of legacy Terraform roots.
4. Delete the corresponding crates, applications, packages, Dockerfiles,
   scripts, fixtures, and documentation.

Status: issues #147 through #150. Voice, managed computers, and the remaining
application and runner cohort have separate disposition issues so durable data
and current callers are resolved before source deletion.

## Completion criteria

Cleanup is complete when all of these statements are true:

- The root workspace contains no TypeScript web or API server for
  `openagents.com`.
- The released CLI builds from one Rust package and has no compatibility
  dependency on the npm CLI.
- Every remaining application has a named product owner, current release path,
  and verified deployment or user.
- Every remaining package is reachable from a retained application, generator,
  or published contract with a known consumer.
- Every Cargo dependency corresponds to actual symbol use.
- Terraform contains only resources owned by retained products.
- The root test suite validates current products instead of keeping retired
  components mutually reachable.
- Current documentation names Phoenix as the web/backend authority and the
  Rust CLI as the terminal authority.
- Required historical evidence remains available through policy-protected
  records and Git history.

## Expected result

Deletion cohort 1 alone removes the largest duplicate product implementation.
The later package and service waves could remove many of the 6,000 or more
tracked TypeScript and TSX files. The exact final count depends on whether
voice, managed computers, Pylon, and Khala remain deployed.

The cleanup should optimize for a legible ownership graph, not a target line
count. A retained service should be obvious from its caller, release path, and
deployment. Anything that needs a long historical argument to prove it is
current is not current until production evidence says otherwise.
