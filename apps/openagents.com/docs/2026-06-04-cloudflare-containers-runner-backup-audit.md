# Cloudflare Containers Runner Backup Audit

Date: 2026-06-04

Status: implementation audit only. No product route, runtime policy, billing
policy, or backend schema has been changed by this note.

## Executive Summary

Cloudflare Containers are a viable backup runner substrate for OpenAgents product surface, but not a
drop-in replacement for the current SHC dispatch path.

The correct target is:

```text
OpenAgents Worker API
  -> one typed runner assignment contract
  -> RunnerGatewayService
  -> SHC VM primary adapter
  -> Cloudflare Containers backup adapter
  -> GCP/GCloud reference and sensitive-work fallback adapter
  -> one callback/event/artifact/receipt path back into OpenAgents product surface
```

Cloudflare Containers should be added as a **low-to-medium trust, burst and
backup execution lane** after the SHC and GCloud assignment/event contracts are
kept equivalent. GCloud should remain the sensitive, reference, and canonical
rerun lane because Containers have ephemeral disk, Cloudflare-controlled
placement, no Firecracker/KVM profile, and no current proof that they meet the
sensitive-work isolation bar.

The immediate implementation prerequisite is an adapter refactor. Today
`OmniDispatchService` calls SHC-specific functions directly. The repo needs a
backend-neutral runner gateway before Containers are added, otherwise the first
Cloudflare Containers patch will multiply SHC-specific assumptions through the
Worker.

## Sources Reviewed

Cloudflare documentation:

- Containers overview:
  <https://developers.cloudflare.com/containers/>
- Get started:
  <https://developers.cloudflare.com/containers/get-started/>
- Container class:
  <https://developers.cloudflare.com/containers/container-class/>
- Scaling and routing:
  <https://developers.cloudflare.com/containers/platform-details/scaling-and-routing/>
- Limits and instance types:
  <https://developers.cloudflare.com/containers/platform-details/limits/>
- Pricing:
  <https://developers.cloudflare.com/containers/pricing/>
- Examples:
  <https://developers.cloudflare.com/containers/examples/>
- SSH:
  <https://developers.cloudflare.com/containers/ssh/>
- Wrangler container commands:
  <https://developers.cloudflare.com/workers/wrangler/commands/containers/>
- Wrangler container configuration:
  <https://developers.cloudflare.com/workers/wrangler/configuration/#containers>
- 2026-02-25 resource-limit changelog:
  <https://developers.cloudflare.com/changelog/post/2026-02-25-higher-container-resource-limits/>
- 2026-01-05 custom instance type changelog:
  <https://developers.cloudflare.com/changelog/post/2026-01-05-custom-instance-types/>
- 2026-05-12 SSH default changelog:
  <https://developers.cloudflare.com/changelog/post/2026-05-12-ssh-enabled-by-default/>

OpenAgents product surface files inspected:

- `README.md`
- `AGENTS.md`
- `INVARIANTS.md`
- `workers/api/wrangler.jsonc`
- `workers/api/package.json`
- `workers/api/src/bindings.ts`
- `workers/api/src/config.ts`
- `workers/api/src/config.test.ts`
- `workers/api/src/omni-runs.ts`
- `workers/api/src/omni-runs.test.ts`
- `workers/api/src/omni/dispatch-service.ts`
- `workers/api/src/omni/assignments.ts`
- `workers/api/src/omni/runner-events.ts`
- `workers/api/src/omni/errors.ts`
- `workers/api/src/omni-handlers.ts`
- `workers/api/src/billing.ts`
- `packages/sync-schema/src/index.ts`
- `workers/api/migrations/0010_omni_agent_runs_and_deployments.sql`
- `workers/api/migrations/0019_agent_runtime_modes.sql`
- `docs/2026-06-02-cloudflare-only-openagents-sync-audit.md`
- `docs/2026-06-02-shc-agent-deployment-runbook.md`
- `docs/2026-06-02-provider-account-implementation-notes.md`
- `docs/2026-06-04-openagents-zero-tech-debt-caller-inventory.md`

## Current OpenAgents product surface Runner State

OpenAgents product surface already has the right high-level authority split:

```text
Foldkit web app
  -> Worker API
  -> D1 source of truth
  -> provider-account grant issue
  -> runner dispatch
  -> runner callback ingest
  -> D1/R2/sync projection
```

The current implementation is still SHC-shaped:

- `packages/sync-schema/src/index.ts` defines `RunnerBackend` as
  `shc_vm | gcloud_vm`.
- `workers/api/src/omni-runs.ts` defaults agent runs to `backend: "shc_vm"`,
  `runtime: "opencode_codex"`, and `runnerId: "oa-shc-katy-01"`.
- `buildAppDeployAssignment` sets `primaryBackend: "shc_vm"` and
  `fallbackBackend: "gcloud_vm"`.
- `workers/api/src/omni/dispatch-service.ts` exposes
  `dispatchAgentRun` and `dispatchDeployment`, but both delegate to
  `dispatchAgentRunToShc` / `dispatchDeploymentToShc`.
- `workers/api/src/config.ts` has only `SHC_*` dispatch configuration and a
  `SHC_DISPATCH_MODE` of `live | unconfigured`.
- `workers/api/src/omni-handlers.ts` allows only `shc_vm | gcloud_vm` from
  request selectors and defaults launches to `shc_vm`.
- Fleet API projection currently reports
  `routingPolicy: "shc_primary_gcloud_fallback"`.
- `workers/api/wrangler.jsonc` has D1, R2, KV, Queue, Cron, and `SYNC_ROOM`
  Durable Object bindings, but no `containers` entry and no Container Durable
  Object binding.
- `workers/api/package.json` does not depend on `@cloudflare/containers`.
- Billing currently uses an internal `container_usage` ledger source for
  "computer usage" at `CONTAINER_RATE_CENTS_PER_MINUTE`; it is not a
  Cloudflare Containers platform cost model.

The docs already point toward this direction. The SHC runbook says Cloudflare
is the future home for a Container runner gateway after SHC/GCP parity, while
also warning that Workers were not the first home for OpenCode/Codex process
workloads.

## Cloudflare Containers Platform Facts That Matter

Cloudflare Containers run alongside Workers and are controlled from Worker code.
The Worker defines a `Container` subclass from `@cloudflare/containers`, adds a
`containers` config entry, binds the container class through Durable Objects,
and uses helpers such as `getContainer` or `getRandom` to address instances.

Relevant properties for OpenAgents product surface:

- Containers require the Workers Paid plan.
- Images must run on `linux/amd64`.
- The Worker can route HTTP and WebSocket requests to a container via
  `container.fetch(request)`.
- A Container class can define `defaultPort`, `sleepAfter`, `envVars`,
  `requiredPorts`, `entrypoint`, lifecycle hooks, and internet access behavior.
- Each container instance is backed by a Durable Object. That maps well to
  one-run or one-deploy addressing by explicit run ID.
- For stateful or short-lived jobs, Cloudflare recommends explicit instance IDs
  and lifecycle control. For stateless pools, `getRandom` picks from a fixed
  instance count.
- Built-in autoscaling for stateless applications is not available today in
  the docs reviewed; routing/load balancing must be owned by the Worker.
- Cloudflare reports cold starts often in the 1-3 second range, but image size
  and entrypoint behavior affect startup time.
- Disk is ephemeral. When a Container sleeps, the next start receives a fresh
  disk from the image.
- SSH is through `wrangler containers ssh`; it does not expose a public SSH
  port. As of the May 2026 changelog, SSH through Wrangler is enabled by
  default, but access still requires an `ssh-ed25519` public key in
  `authorized_keys`.
- Current public instance-type docs/changelogs include predefined types up to
  `standard-4` at 4 vCPU, 12 GiB memory, and 20 GB disk, custom instance types,
  and account-level concurrent live-container limit increases to 1,500 vCPU,
  6 TiB memory, and 30 TB disk.
- Pricing is based on active runtime in 10 ms increments, with memory/disk
  billed on provisioned resources and CPU billed on active usage. The $5
  Workers Paid plan includes monthly allocations before overage.

## Fit Assessment

Cloudflare Containers fit OpenAgents product surface as a backup runner when the task can tolerate:

- ephemeral workspace disk;
- internet-enabled container networking;
- no KVM/Firecracker proof;
- Cloudflare platform placement rather than a named SHC/GCP host;
- Worker-owned lifecycle and routing;
- cold-start and image-pull/boot latency;
- strict closeout before sleep.

They are especially attractive for:

- bursty Autopilot coding tasks that need Linux, Git, Bun/Node, shell tools,
  and a full filesystem;
- fallback when `oa-shc-katy-01` is down, saturated, or being maintained;
- sandbox-like short-lived agent runs where the run ID can address one
  container instance;
- canarying a Cloudflare-native runner path without moving product authority
  out of the Worker.

They are not yet the right default for:

- customer-sensitive or wallet-bearing workloads;
- canonical grading where GCP remains the reference lane;
- workloads requiring persistent local disk after closeout;
- privileged nested virtualization;
- large images or workspaces that exceed verified Container image and disk
  constraints;
- tasks where loss of the container before artifact upload would be
  unacceptable.

## Target Backend Policy

Recommended routing policy:

```text
shc_primary_cloudflare_container_backup_gcloud_reference
```

Practical interpretation:

| Backend | Role |
| --- | --- |
| `shc_vm` | Primary low-to-medium trust OpenCode/Codex workroom runner. |
| `cloudflare_container` | Backup and burst lane for low-to-medium trust process workloads. |
| `gcloud_vm` | Sensitive, reference, canonical rerun, and last-resort fallback lane. |

This keeps GCloud active. Cloudflare Containers should not erase the GCloud
fallback lane; they should reduce dependency on a single SHC host for routine
bursty work.

## Required Schema And Storage Changes

1. Extend `RunnerBackend`.

Current:

```text
RunnerBackend = "shc_vm" | "gcloud_vm"
```

Target:

```text
RunnerBackend = "shc_vm" | "cloudflare_container" | "gcloud_vm"
```

Use the full `cloudflare_container` name instead of `cf_container` in the
external schema. It is clearer in D1 rows, public projections, receipts, and
operator logs.

2. Add a D1 migration for backend constraints.

Tables that need updated checks:

- `agent_runs.backend`
- `deployments.primary_backend`
- `deployments.fallback_backend`

The migration should preserve historical `shc_vm` and `gcloud_vm` rows. The
older `local_fake` value from migration `0010` appears to be historical and is
not in the current shared schema. Do not reintroduce it as a public backend.

3. Keep assignment envelope version stable at first.

`openagents.agent_run_assignment.v1` and
`openagents.app_deploy_assignment.v1` can carry the new backend value without a
new schema version if the only change is an added enum case. A new version is
only needed if the assignment shape gains Container-specific fields that are
not generic runner policy.

4. Add backend-specific dispatch metadata as event payloads, not public
assignment secrets.

Allowed public metadata:

- backend;
- runner ID;
- container instance name or opaque external ID;
- image tag/digest;
- instance type;
- lifecycle status;
- Cloudflare deployment ID;
- sanitized endpoint path.

Disallowed public metadata:

- provider auth payloads;
- OpenCode auth JSON;
- callback bearer token values;
- service actor tokens;
- raw SSH private keys;
- raw Cloudflare account tokens.

## Required Worker And Wrangler Changes

1. Add dependency:

```text
@cloudflare/containers
```

The package belongs in `workers/api/package.json` because the Container class
is Worker-runtime code.

2. Add a Container class.

The first class should live in Worker code, not the web app:

```text
workers/api/src/containers/openagents-runner-container.ts
```

Shape:

```text
OpenAgentsRunnerContainer extends Container
  defaultPort = runner control server port
  sleepAfter = short idle timeout
  requiredPorts = [control port]
  enableInternet = true for GitHub/OpenAI/Git operations
  onStart/onStop/onError emit sanitized lifecycle events
```

The class should not embed provider secrets. It can pass static non-secret
runtime configuration through `envVars`, but per-run secrets must still flow
through provider-account grants and runner-side resolution.

3. Add Wrangler config.

`workers/api/wrangler.jsonc` needs:

```jsonc
"containers": [
  {
    "class_name": "OpenAgentsRunnerContainer",
    "image": "./containers/openagents-runner/Dockerfile",
    "max_instances": 5
  }
],
"durable_objects": {
  "bindings": [
    {
      "name": "RUNNER_CONTAINER",
      "class_name": "OpenAgentsRunnerContainer"
    },
    {
      "name": "SYNC_ROOM",
      "class_name": "SyncRoomDurableObject"
    }
  ]
},
"migrations": [
  {
    "tag": "v1",
    "new_sqlite_classes": ["SyncRoomDurableObject"]
  },
  {
    "tag": "v2",
    "new_sqlite_classes": ["OpenAgentsRunnerContainer"]
  }
]
```

The exact migration tag must account for the existing deployed Durable Object
history. Do not guess the production migration sequence in code; verify the
current Worker deployment state before applying.

4. Add binding types.

`workers/api/src/bindings.ts` should expose the binding through the same typed
runtime capability boundary that already wraps D1, R2, Queues, and
`SYNC_ROOM`. Avoid direct `env.RUNNER_CONTAINER` reads scattered across route
handlers.

5. Add config fields.

Do not overload `SHC_DISPATCH_MODE`. Add a backend-neutral dispatch policy and
Cloudflare-specific availability flag:

```text
RUNNER_DISPATCH_POLICY=shc_primary_cloudflare_container_backup_gcloud_reference
CLOUDFLARE_CONTAINER_DISPATCH_MODE=live | unconfigured
CLOUDFLARE_CONTAINER_INSTANCE_COUNT=<number, for fixed stateless pool only>
CLOUDFLARE_CONTAINER_IMAGE_TAG=<optional public digest/tag metadata>
```

If the implementation only uses explicit per-run IDs, avoid adding
`INSTANCE_COUNT` until a stateless pool exists.

## Required Runner Image Work

The Container image needs to run an OpenAgents product surface-compatible runner control server.
The clean path is to reuse the SHC control contract rather than creating a
second control protocol.

Minimum image contents:

- Linux amd64 base image;
- Bun or Node runtime required by OpenCode and OpenAgents product surface runner code;
- Git;
- GitHub CLI if GitHub writeback still depends on it;
- OpenCode/Codex runtime entrypoint;
- a small HTTP control server that accepts the same sanitized assignment body
  that `dispatchAgentRunToShc` sends today;
- runner event callback client;
- artifact closeout uploader or callback manifest emitter;
- credential scrubber;
- process supervision with graceful SIGTERM handling.

The image should not include:

- baked provider auth;
- operator SSH private keys;
- Cloudflare API tokens;
- customer repository material;
- wallet or production cloud credentials.

The control server should support:

- `POST /v1/codex-runs` or an internal equivalent;
- `POST /v1/codex-runs/:id/cancel`;
- health/readiness endpoint;
- status/event endpoint only if the Worker needs to poll;
- callback retries or queue handoff for events.

The container must upload or callback all required artifacts before the
instance sleeps, because Cloudflare Container disk is ephemeral.

## Required Dispatch Refactor

Current:

```text
OmniDispatchService
  -> dispatchAgentRunToShc
  -> dispatchDeploymentToShc
```

Target:

```text
RunnerGatewayService
  -> select backend by assignment.backend, dispatch policy, health, capacity
  -> ShcRunnerAdapter
  -> CloudflareContainerRunnerAdapter
  -> GcloudRunnerAdapter
```

Adapter contract:

```text
dispatchAgentRun(assignment, context) -> DispatchResult
dispatchDeployment(assignment, context) -> DispatchResult
cancelAgentRun(run, context) -> ShcControlActionResult-compatible result
health(context) -> RunnerBackendHealth
```

The dispatch context should include typed capabilities, not raw global objects:

- fetcher;
- config;
- current time;
- container binding access;
- D1/R2 as needed through existing services;
- event append/notification service;
- logger/observability service.

Cloudflare Containers dispatch should use explicit run IDs:

```text
container = getContainer(env.RUNNER_CONTAINER, runId)
container.fetch(startRequest)
```

Do not use `getRandom` for stateful Autopilot runs. `getRandom` is only a fit
for stateless worker pools where any instance can handle any request.

## Provider Account And Secret Boundary

The existing provider-account contract should remain unchanged:

```text
browser
  -> issue provider account grant
  -> assignment carries grantRef only
  -> runner resolves grant through service route
  -> runner materializes OpenCode auth locally
  -> runner scrubs after closeout
```

For Cloudflare Containers:

- the Worker should not resolve provider grants and inject raw auth into the
  container start body;
- the container runner should call the existing grant resolve service route
  using a programmatic service actor or narrowly scoped runner credential;
- the runner should materialize `OPENCODE_AUTH_CONTENT` or isolated auth JSON
  inside the container only;
- callback/event payloads must pass the existing credential-shaped material
  scanner before they hit D1 or public sync projection;
- all service credentials used by the container need Cloudflare secret binding
  or runner-side secure config, not image-baked values.

This matches `docs/2026-06-02-provider-account-implementation-notes.md` and
keeps D1 from storing raw ChatGPT/Codex credential payloads.

## Artifact And Closeout Requirements

Because Container disk is ephemeral, the closeout contract is stricter than
SHC:

1. clone repository into container workspace;
2. run OpenCode/Codex;
3. write `result.md`;
4. write `github-writeback.json` when a GitHub work order exists;
5. upload artifacts to R2 or emit a manifest that the Worker can fetch before
   sleep;
6. send final callback events;
7. scrub provider auth and repository working directory;
8. stop or allow `sleepAfter` to stop the instance.

The required artifact list should stay backend-neutral:

```text
result.md
github-writeback.json when writeback is requested
redacted log/event manifest
optional receipt manifest
```

Do not depend on later SSH into the container to recover artifacts.

## Billing Requirements

OpenAgents product surface currently bills user-visible "computer usage" in
`workers/api/src/billing.ts` through:

```text
CONTAINER_RATE_CENTS_PER_MINUTE = 5
source = container_usage
meter = container_seconds
```

That name predates Cloudflare Containers. Once a real Cloudflare Container
backend exists, separate the concepts:

- product meter: user-facing Autopilot computer seconds;
- platform meter: Cloudflare Container vCPU/memory/disk/network cost;
- backend metadata: `shc_vm`, `cloudflare_container`, or `gcloud_vm`.

Recommended ledger cleanup:

```text
source = runner_usage
meter = runner_seconds
metadata.backend = cloudflare_container
metadata.instanceType = standard-2
metadata.platform = cloudflare_containers
```

If the source name cannot be changed immediately, at least add backend-specific
metadata and a doc note that `container_usage` is product terminology, not a
direct Cloudflare Containers bill.

The platform cost model must account for:

- active runtime billing in 10 ms increments;
- memory and disk charged on provisioned resources;
- CPU charged on active usage;
- included Workers Paid plan allocations;
- overage rates;
- container instances that stay awake due to long `sleepAfter`;
- `max_instances` saturation failures.

## Observability And Operations

Required runtime events:

- `runner.container_dispatch_started`;
- `runner.container_starting`;
- `runner.container_ready`;
- `runner.container_event_received`;
- `runner.container_artifact_uploaded`;
- `runner.container_completed`;
- `runner.container_failed`;
- `runner.container_stopped`;
- `runner.container_capacity_exhausted`.

Required operator views:

- backend health per `shc_vm`, `cloudflare_container`, and `gcloud_vm`;
- configured max instances;
- current active container instances;
- queue depth / pending launches;
- failures by adapter and endpoint;
- cold-start time;
- artifact closeout success rate;
- cost estimate by backend.

Cloudflare-native tools:

- Containers dashboard for status, health, metrics, and logs;
- `wrangler containers list`;
- `wrangler containers instances`;
- `wrangler containers ssh <INSTANCE_ID>` for break-glass only;
- Worker Observability for dispatch and callback events.

SSH should stay operator-only. Product dispatch must never depend on SSH.

## Security Posture

Cloudflare Containers improve operational proximity to the Worker, but they do
not automatically solve the runner trust problem.

Security requirements before enabling live user traffic:

- one explicit Container instance per run/deploy ID;
- no shared mutable workspace across users;
- no raw provider credentials in Worker request logs, D1, public events, or
  image layers;
- callback token and service actor credential scoped to runner use;
- all event payloads scanned through existing provider secret scanner;
- internet access justified and documented for GitHub/OpenAI/package manager
  access;
- artifact redaction before public projection;
- graceful SIGTERM closeout;
- `sleepAfter` short enough to limit idle exposure;
- SSH either disabled or restricted to named operator public keys;
- no customer-sensitive workloads until a formal trust-tier note says this
  backend is permitted.

Potential future hardening:

- separate images by trust tier;
- separate Workers/accounts/environments for untrusted public runs;
- Cloudflare Access in front of any operator-only routes;
- image digest pinning;
- container status hooks that record lifecycle evidence in D1;
- background Workflow to mark stale container runs failed if no callback
  arrives by timeout.

## Tests And Verification

Minimum tests before a code implementation can merge:

Schema and migration:

- `RunnerBackend` decodes `cloudflare_container`;
- invalid backend values still fail;
- D1 migration preserves existing rows and accepts the new backend;
- public run/deploy projections include the new backend without leaking
  container internals.

Config:

- minimal env remains valid with Containers unconfigured;
- malformed Container dispatch mode fails;
- live Container mode requires required binding/config;
- SHC live mode is not affected.

Dispatch:

- `RunnerGatewayService` selects SHC for `shc_vm`;
- selects Cloudflare Containers for `cloudflare_container`;
- does not call Cloudflare adapter when unconfigured;
- records capacity failures as typed dispatch failures;
- dispatch result external IDs are stable and namespaced, for example
  `cloudflare-container:<runId>`.

Callbacks:

- container runner events normalize through `OmniRunnerEventService`;
- duplicate event IDs/sequences are idempotent;
- credential-shaped payloads are rejected;
- provider reconnect-required events still update provider account health.

Artifacts:

- final closeout requires `result.md`;
- GitHub writeback runs require `github-writeback.json`;
- R2 refs are recorded before run completion;
- missing artifacts fail the run or mark it waiting for operator review.

Billing:

- runner seconds accrue for `cloudflare_container`;
- backend metadata is stored;
- no duplicate billing occurs if event callbacks retry;
- exhausted credits cancel or stop the container backend.

Operational smoke:

- deploy Worker with one small test image;
- launch a fake Codex run;
- receive lifecycle events;
- upload a test artifact;
- verify D1 run status and sync projection;
- stop/sleep the container;
- verify no raw auth material appears in logs or D1.

## Implementation Sequence

1. Add this audit to the current planning docs.
2. Create an issue for the runner gateway refactor.
3. Extend `RunnerBackend` with `cloudflare_container` and add D1 migration.
4. Replace SHC-only `OmniDispatchService` dependencies with backend adapter
   interfaces.
5. Keep SHC adapter behavior exactly equivalent and tests passing.
6. Add GCloud adapter config or at least a typed placeholder if it is not yet
   implemented in this repo.
7. Add Cloudflare Container binding, dependency, and a minimal Container class
   behind `CLOUDFLARE_CONTAINER_DISPATCH_MODE=unconfigured`.
8. Build a minimal runner image that only echoes fake events and artifacts.
9. Wire explicit-run-ID dispatch to the Container class.
10. Add fake-run smoke tests and deploy to staging.
11. Add OpenCode/Codex runtime to the image.
12. Add provider grant resolution inside the container runner.
13. Add artifact closeout to R2.
14. Enable the backend only for operator-selected runs.
15. Add health/capacity routing so SHC can fail over to Containers for
    low-to-medium trust tasks.
16. Keep GCloud fallback active for sensitive/reference/canonical work.

## Open Questions

- Which Cloudflare account/environment should own the first Container image and
  max instance quota?
- Should the first image reuse the SHC `oa-codex-control` implementation or
  introduce a new small runner control server?
- What exact instance type is enough for OpenCode/Codex plus Git operations?
- What is the acceptable cold-start budget for a backup run?
- Should SSH be disabled in production Containers, or should named operator
  keys be configured for break-glass access?
- Should the product-visible backend label be "Cloudflare" or stay hidden
  behind "computer" copy?
- Does the existing `container_usage` billing source need a migration before
  real Cloudflare Containers ship?
- Should `cloudflare_container` be eligible for deploy assignments immediately,
  or only agent runs until artifact/writeback closeout is proven?

## Recommendation

Proceed, but in this order:

```text
schema + adapter boundary
  -> fake Container runner
  -> staging smoke
  -> OpenCode image
  -> provider grant resolution
  -> artifact closeout
  -> operator-only live runs
  -> automatic SHC failover for low-to-medium trust tasks
```

Do not start by dropping OpenCode into a Dockerfile and wiring it directly from
`omni-handlers.ts`. That would couple Containers to the current SHC-specific
dispatch shape and make the future GCloud and Container backends harder to keep
equivalent.
