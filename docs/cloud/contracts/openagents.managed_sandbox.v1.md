# `openagents.managed_sandbox.v1`

Status: **the contract, lifecycle store, turn store, default-off GCE runtime,
Box facade, and private turn driver are admitted. Guest I/O, consumers, the
independent live gate, and production availability remain gated by SBX-05
through SBX-10**.

Owning package: `packages/managed-sandbox-contract`

## Purpose

This is the canonical OpenAgents domain contract for one owner-scoped,
lease-bounded agent sandbox on Google Cloud. Product consumers, Sarah, the IDE,
the control plane, target adapters, and compatibility facades all exchange this
identity. Provider objects and compatibility API objects are projections. They
are never the resource authority.

## Stable identity

`ManagedSandboxResourceSchema` binds:

- `sandboxRef`, owner, tenant, program, and exact work-unit identity.
- portable attachment identity and attachment generation.
- independently fenced resource generation, optimistic version, and event
  sequence.
- one `openagents_managed` Google Cloud target, region, adapter, and admitted
  `gce_vm` or `firecracker_microvm` isolation class.
- immutable image digest and profile ref.
- bounded lease, USD-micro budget, and explicit capabilities. And
- distinct lifecycle, lease, guest, filesystem, ingress, runtime, `acceptingWork`,
  and cleanup facts.

The distinct facts are deliberate.
A quiet process does not prove a stopped sandbox.
An accepted API request does not prove deletion.
An absent provider object in one projection does not prove cleanup.

## Command boundary

`ManagedSandboxCommandSchema` is a closed tagged union:

1. `Create`
2. `Inspect`
3. `Update`
4. `Stop`
5. `Resume`
6. `Delete`
7. `Dispatch`
8. `Interrupt`

Every command carries a command ref, authenticated actor, owner, tenant,
idempotency ref, and request time. An existing-resource mutation additionally
requires its exact sandbox ref and expected version. Create binds the target,
digest-pinned image, profile, lease, budget, and requested capabilities before
the first provider effect.
Dispatch additionally binds an exact runtime provider, model, harness, and
optional reasoning effort beside its prompt digest and turn ref.

## Event and receipt boundary

`ManagedSandboxEventSchema` is the append-only native lifecycle plane. Events
are generation-fenced and strictly sequenced. `ManagedSandboxReceiptSchema`
binds each command outcome to the final generation, version, lifecycle,
events, artifacts, optional typed error, and public-safe observation time.

A caller retries with the same command and idempotency bytes. Different bytes
under the same idempotency identity refuse. They do not reinterpret a previous
effect. Reconciliation records uncertainty as `recovery_required`. It never
rounds an unknown provider result up to success.

## Durable lifecycle authority

Migration `0080_managed_sandbox_authority.sql` creates the canonical Cloud SQL
aggregate, command, generation, event, receipt, turn-order, and projection
cursor tables.
Migration `0081_managed_sandbox_runtime_turns.sql` adds canonical turn JSON,
runtime identity, prompt digest, per-turn sequence, terminal receipt, and
interrupt-command coordinates while preserving the one global native event
sequence.

`PostgresManagedSandboxStore` takes a sandbox-scoped advisory lock and row lock.
It records the decoded command and its SHA-256 fingerprint before provider
effects.
One partial unique index permits only one `pending` command per sandbox.
Another permits only one generation with `acceptingWork: true`.

Create records `ProvisionRequested` before the provisioner effect.
Stop, resume, and delete also record their intent before target effects.
Resume fences the prior generation before guest readiness can admit work in the
next generation.
The native event sequence stays global and dense across that change.

An exact retry returns the durable `pending` state or settled receipt.
Different bytes under the same command or idempotency identity refuse.
A settled receipt retry must match the durable settlement fingerprint.
A process restart can list commands with `pending` status for reconciliation.
Unknown cleanup becomes `recovery_required`. It never becomes deletion.

Native reconnect pages read the append-only event sequence.
Each compatibility translator has a separate optimistic projection version.
A projection cursor cannot advance beyond native authority or move backward.

## Runtime turn authority

`ManagedSandboxTurnSchema` is the canonical prompt-run identity. It binds the
sandbox scope, resource versions, dispatch command, capability, and prompt
digest. It also binds exact provider, model, and harness truth. The turn
status vocabulary distinguishes
`pending`, `running`, `interrupting`, `settled`, `failed`, and `interrupted`.

Runtime events have two coordinates: one dense sandbox-global native sequence
and one dense turn-local sequence. `RuntimeTextDelta`, tool start/completion,
usage, interrupt request, failure, interruption, and structural settlement
remain native lossless events. An identical provider-byte replay is a no-op.
Changed bytes at a present turn sequence or a sequence gap conflicts.
Reconnect reads turn-local pages without making the Box cursor authoritative.

Dispatch reservation settles when the target accepts `RuntimeStarted`, not
when the long-running turn finishes. This releases the one-command lock so an
exact `Interrupt` command can be admitted while the turn remains active.
Interrupt first records `RuntimeInterruptRequested`. It becomes terminal only
after `RuntimeInterrupted` arrives from the same turn and generation.

No timer, output silence, polling gap, or Box status settles a turn. Only
structural provider completion, explicit interruption, a lease/budget
guardrail, or typed runtime failure may do so. Terminal events create
`ManagedSandboxTurnReceiptSchema` with exact runtime and usage when the
provider exposes it.

The private control endpoint is
`POST /v1/managed-sandbox/runtime/turns`. `oa-codex-control` forwards the
short `dispatch`, `sync`, or `interrupt` operation to the absolute executable
configured by `OA_MANAGED_SANDBOX_TURN_DRIVER`. It never invokes a shell or
returns driver stderr. The helper uses the ordinary Codex or Claude SDK inside
the isolated guest and keeps provider-private process/session handles outside
the public event plane. Missing configuration, an invalid response, scope or
generation drift, non-dense order, or invisible interrupt fails closed.

The helper may restart after sandbox resume, but memory, process handles, and
hidden provider session state are not checkpoint claims. Durable filesystem
and OpenAgents turn/event state are the only resumable facts until a later
provider-specific continuation is independently proven.

## Lifecycle invariants

The bounded executable model lives in
`packages/managed-sandbox-contract/src/lifecycle.ts` and exhaustively explores
the admitted graph in its test suite.

- `ready`, `idle`, and `running` require an observed guest.
- Work is accepted only by `ready`, `idle`, or `running` states.
- Stop cannot reach `stopped` until a durable filesystem checkpoint exists.
- Delete is admitted only from a stopped, failed, or recovery-required resource
  with `acceptingWork: false`.
- `deleted` requires observed cleanup, absent guest, deleted filesystem,
  revoked ingress, no runtime, and no work admission.
- Generation mismatch, replay, and event gaps fail closed.
- Any uncertain stop/delete/cleanup path becomes `recovery_required` or
  `failed`, never an invented terminal success.

## Box-v1 compatibility profile

Box-v1 is a development/conformance facade over this native contract. It is
not the control-plane domain model and does not authorize an OCI, Docker,
Kubernetes, or generic GCP claim.

The exact Phase-1 method/status/error corpus is exported as
`BOX_V1_PHASE1_OPERATIONS`:

| SDK method / route                                          | Success     | Canonical native service                         | Declared projection loss / refusal                                     |
| ----------------------------------------------------------- | ----------- | ------------------------------------------------ | ---------------------------------------------------------------------- |
| `me` · `GET /v1/me`                                         | 200         | `ManagedSandboxAccountProjection.currentOwner`   | owner-safe identity only. No credentials or provider account           |
| `limits` · `GET /v1/limits`                                 | 200         | `ManagedSandboxCapacityProjection.currentLimits` | bounded availability only. No quota authority or topology              |
| `boxes` · `GET /v1/boxes`                                   | 200         | `ManagedSandboxService.list`                     | Box state is derived. Native generation/facts remain authoritative     |
| `create` · `POST /v1/boxes`                                 | 200/201/202 | `ManagedSandboxService.create`                   | request is expanded into exact native scope or refused before effects  |
| `get` · `GET /v1/boxes/{id}`                                | 200         | `ManagedSandboxService.inspect`                  | lifecycle label cannot collapse native fact dimensions                 |
| `update` · `PATCH /v1/boxes/{id}`                           | 200         | `ManagedSandboxService.update`                   | only admitted lease/budget fields. Generic provider mutation refuses   |
| `remove` · `DELETE /v1/boxes/{id}`                          | 200/202/204 | `ManagedSandboxService.delete`                   | acceptance or `pending` is not cleanup. Native receipt owns completion |
| `stop` · `POST /v1/boxes/{id}/stop`                         | 200/202     | `ManagedSandboxService.stop`                     | stopped only after durable checkpoint and observed guest stop          |
| `resume` · `POST /v1/boxes/{id}/resume`                     | 200/202     | `ManagedSandboxService.resume`                   | filesystem restart only. No memory/process continuity claim            |
| `prompt` · `POST /v1/boxes/{id}/prompt`                     | 200/202     | `ManagedSandboxRuntimeService.dispatch`          | provider/model/harness/generation are native truth                     |
| `promptRunStatus` · `GET /v1/boxes/{id}/prompts/{promptId}` | 200         | `ManagedSandboxRuntimeService.inspectTurn`       | SDK status cannot substitute for terminal receipt                      |
| `events` · `GET /v1/boxes/{id}/events`                      | 200         | `ManagedSandboxRuntimeService.readEvents`        | translator/native cursor and omitted kinds are mandatory               |
| `interrupt` · `POST /v1/boxes/{id}/interrupt`               | 200/202     | `ManagedSandboxRuntimeService.interrupt`         | targets one native turn. Replay does not create another interrupt      |
| `readFile` · `GET /v1/boxes/{id}/files`                     | 200         | `ManagedSandboxWorkspaceService.readFile`        | native root/symlink/secret/quota policy applies first                  |
| `writeFile` · `PUT /v1/boxes/{id}/files`                    | 200/201     | `ManagedSandboxWorkspaceService.writeFile`       | native root/symlink/secret/quota policy applies first                  |
| `command` · `POST /v1/boxes/{id}/commands`                  | 200         | `ManagedSandboxWorkspaceService.executeCommand`  | bounded argv/cwd/time/output only. No ambient generic shell grant      |
| `artifact` · `GET /v1/boxes/{id}/artifacts?path=…`          | 200         | `ManagedSandboxWorkspaceService.readArtifact`    | digest/size/generation/retention remain native receipt truth           |

The admitted error vocabulary is
`authentication_required`, `permission_denied`, `resource_not_found`,
`conflict`, `validation_failed`, `rate_limited`, `capacity_unavailable`, and
`upstream_unavailable`, each returned in the typed Box error envelope with the
actual HTTP status. The later facade must not translate typed native
`recovery_required`, budget, cleanup, or reconciliation states into a false
success merely because Box lacks an equivalent field.

Every other method present in the pinned SDK is listed in
`BOX_V1_UNSUPPORTED_SDK_METHODS` and returns HTTP `501` with typed code
`capability_not_implemented`. A translator page includes its translator ref,
native event sequence, compatibility cursor, and omitted native event kinds so
projection loss is visible.

### Frozen upstream provenance

| Artifact                        | Frozen value                                                                                      |
| ------------------------------- | ------------------------------------------------------------------------------------------------- |
| npm package                     | `@asciidev/box-sdk@0.0.24`                                                                        |
| license                         | MIT                                                                                               |
| npm integrity                   | `sha512-w77vTWA+yrJ5O+FmchCkurjux1UZkQ5yeurnzX/FJTlQulEtj1xp0g/2cSh/GZWLXrgCV0exU99E+NyiilBeHA==` |
| npm shasum                      | `eb55554ffb5b231888a70e51857f8de336735ac1`                                                        |
| npm tarball SHA-256 / bytes     | `51ac532981c4791ab8662d800cd70b6f18d9a8a01abbd097c627bae3ae45aeb0` / `104618`                     |
| packaged LICENSE SHA-256        | `b7d51a8c93c3b34b607bdb4e15b547e4c7618cf21321c608689b44634f3e3183`                                |
| SBX-00 `pnpm-lock.yaml` SHA-256 | `4a814fe782c61098657f6f4cf96f501fcf1a73c28607e3e0ff1405c66995678b`                                |
| OpenAPI source                  | `https://api.ascii.com/openapi.json`                                                              |
| OpenAPI SHA-256                 | `9ae1e0b7ded4a2d537bfa076f8e047baa2bdf7e3736de2cc397d349457c3cbac`                                |
| captured                        | 2026-07-19                                                                                        |
| translator                      | `openagents.box_v1_translator.v1`                                                                 |

The SDK is an exact development dependency only.
Conformance tests import its unmodified generated `BoxApi`.
They compare every admitted route with upstream request options.
They prove the configurable facade base path, bearer behavior, and structured
unsupported-operation response.

## Authority boundary

Sarah may request only the eight closed managed-sandbox actions under
`program.managed_agent_sandboxes` and `grant.sarah_managed_sandbox`. The runtime
must resolve owner, tenant, work unit, target, immutable image, profile, lease,
budget, capability, idempotency, and generation through the typed broker. The
grant conveys no raw `gcloud`, generic shell, database, guest address, service
account, provider credential, topology, or generic container administration.

The authority profile admits the action vocabulary in SBX-00.
Actual mutation refuses until the named broker and GCP target profile are
deployed and healthy.
They must remain within the current cloud budget and produce native lifecycle
and cleanup receipts.

## GCE runtime profile

SBX-02 adds the private native provider route
`POST /v1/managed-sandbox/runtime/operations` to `oa-codex-control`.
It supports create, probe, stop, resume, delete, and reconcile operations.
It does not replace the canonical command, event, receipt, or lifecycle store.

The first admitted isolation class is `gce_vm` on an exact `e2-small` profile
in `us-central1`.
The deploy configuration pins an exact GCE image name, immutable image ID,
image identity digest, and profile digest.
It also pins the provisioner, network policy, and component identity refs.
An image family cannot reach the provider effect.

The workload has no external IP address, guest service account, OAuth scope,
ingress rule, or ambient capability material.
A sandbox-specific egress rule denies all IPv4 egress.
The guest readiness marker is bound to the resource and generation and is
observed through the serial console.
Provider state alone cannot report ready.

The provider writes cleanup ownership before its first effect.
It records exact operation and idempotency fingerprints.
It fences resume with a new provider generation.
An uncertain create, stop, resume, or delete is reconciled against the same
ownership.
It never selects a replacement provider, region, image, or machine class.

Delete verifies zero instance, firewall rule, and disk residue.
The admitted profile creates no ingress or guest identity grant.
Any nonzero or unknown cleanup observation reports `recovery_required`.

The live provider is default-off and requires the control VM metadata identity.
It refuses a downloadable service-account key.
The legacy fake GCE and fake Cloud-VM lanes remain test tools and cannot return
managed-sandbox readiness.

The exact profile, deployment inputs, live component harness, and rollback are
in
`docs/cloud/bootstrap/SBX-02-managed-sandbox-runtime.md`.

SBX-04 adds the private turn adapter described in
`docs/cloud/bootstrap/SBX-04-managed-sandbox-turn-runtime.md`. Both it and the
Box facade remain default-off. The component tests do not prove a live GCP
SDK helper, provider account, or target image. SBX-09 owns that independent
proof and rollout decision.

## Verification

```bash
pnpm --filter @openagentsinc/managed-sandbox-contract typecheck
pnpm --filter @openagentsinc/managed-sandbox-contract test
pnpm --filter @openagentsinc/authority test
pnpm --filter @openagentsinc/khala-sync-server typecheck
pnpm exec vp test --run packages/khala-sync-server/src/managed-sandbox-store.test.ts
cargo test -p oa-codex-control managed_sandbox_runtime --no-fail-fast
cargo test -p oa-codex-control managed_sandbox_turn_runtime --no-fail-fast
cargo test -p oa-codex-control --no-fail-fast
bash -n scripts/cloud/gcp-codex-control-deploy.sh
```

The corresponding proof program is
`specs/openagents/managed-agent-sandboxes.assurance-spec.md`.
SBX-01 proves deterministic database reconciliation and cleanup truth.
SBX-02 proves the default-off GCE provider component and its bounded live
component harness. The accepted refs-only component receipt is
`docs/sol/evidence/2026-07-19-sbx02-managed-sandbox-live.json`.
SBX-04 proves the default-off turn-driver protocol and durable event authority
at the deterministic component rung. Its bounded receipt is
`docs/sol/evidence/2026-07-19-sbx04-managed-sandbox-turns.json`.
The independent live release gate, IDE, mobile, and Sarah dogfood receipts
belong to later SBX work.
This contract does not claim them early.
