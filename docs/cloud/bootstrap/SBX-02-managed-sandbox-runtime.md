# SBX-02 managed-sandbox GCE runtime

Status: implemented component. Public rollout remains blocked by SBX-09

Issue: [#9028](https://github.com/OpenAgentsInc/openagents/issues/9028)

## Purpose

This runbook defines the first admitted Google Cloud runtime for
`openagents.managed_sandbox.v1`.
The isolation unit is one GCE VM.
The product calls the unit a managed sandbox.
It does not call the unit an OCI container.

The runtime uses the existing `oa-codex-control` service.
The private native route is:

```text
POST /v1/managed-sandbox/runtime/operations
```

The route supports `create`, `probe`, `stop`, `resume`, `delete`, and
`reconcile` provider operations.
The canonical TypeScript lifecycle and event store remain authoritative.
This provider journal records provider ownership and effect replay data.

## Admitted profile

The first profile has these fixed values:

| Field | Value |
| --- | --- |
| target | `target://openagents/google-cloud/managed-sandbox` |
| profile | `profile-ref://openagents/managed-sandbox/gce-e2-small-v1` |
| provisioner | `provisioner-ref://openagents/oa-codex-control/gce-v1` |
| region | `us-central1` |
| machine class | `e2-small` |
| isolation class | `gce_vm` |
| image name | `ubuntu-2404-noble-amd64-v20260717` |
| immutable image ID | `8995931917882208093` |
| image identity digest | `sha256:6db516ddd2287bf98ae0471f5d2f920748f55777af6167647da86eff7267bec2` |
| profile digest | `sha256:078db1b226e34dcc2df0738c81f0134b9bcb3d92b2b1242565fa6a5b27e9dedc` |
| network policy | `network-policy-ref://openagents/managed-sandbox/deny-all-v1` (historical SBX-02 probe profile) |
| control identity | `identity-ref://openagents/managed-sandbox/control` |
| guest identity | `identity-ref://openagents/managed-sandbox/guest-none` |
| minimum and prewarm | `0` |
| maximum and concurrent cap | `2` |
| maximum TTL | 24 hours |
| live acceptance TTL | 15 minutes |
| live acceptance maximum rate | 20,000 micro-USD per hour |
| live acceptance sandbox budget | 10,000 micro-USD |
| live acceptance program budget | 40,000 micro-USD |

The image digest is the SHA-256 digest of the immutable GCE image project,
name, and image ID tuple.
The provider checks the tuple and the `READY` image status before it creates a
firewall rule or VM.
An image family is not an admitted runtime input.

The profile digest covers the profile ref, region, machine class, isolation
class, network policy, and no-identity guest policy.
The deployment must supply both digests.
The service refuses a profile or image mismatch.

## SBX-09 operational profile overlay

The historical Ubuntu image and deny-all probe profile above remain the
evidence-bound SBX-02 component baseline. They are not accepted for owner
agent turns. SBX-09 builds a dedicated Debian guest image with
`scripts/cloud/build-managed-sandbox-guest-image.sh` and records its immutable
image ID, image digest, profile digest, source revision, and provisioner
revision in the acceptance evidence. Image sealing preserves an empty
`/etc/machine-id` path so systemd can generate a fresh clone identity and DHCP
client ID. Before admission, the builder boots the sealed image as a private,
no-identity smoke VM and requires observed DHCP, metadata startup, per-guest
SSH host keys, the workload metadata guard, and SSH service readiness. Only a
passing image receives `openagents-boot-smoke=passed`. A failed newly-created
image and its smoke VM are deleted by the builder cleanup path.

The operational native and Box-compatible authorities use the single profile
ref `profile.sbx.gce.e2-small.v1`. That ref is part of the operational profile
digest. The older `profile-ref://…` spelling in the historical component table
above is not admitted for SBX-09 placement.

The operational profile uses
`network-policy-ref://openagents/managed-sandbox/broker-only-v1`. Each
sandbox generation owns five firewall rules:

- allow egress only to the private control broker on TCP 8790 at priority 900.
- allow TCP 80 only to `169.254.169.254/32` for GCE metadata bootstrap at
  priority 900.
- deny all other egress at priority 1000.
- allow ingress only from the profile-bound reserved control IP `/32` on TCP
  22 at priority 900. And
- deny all other ingress at priority 1000.

The VM still has no external address, service account, OAuth scope, provider
credential, or ambient provider home. The Worker retains the provider
credentials. It issues a short-lived HMAC capability bound to the exact
tenant, sandbox, generation, turn, provider, model, and capability ref. The
guest can present that capability only through the private control broker.
The control bearer token and broker signing key live in Secret Manager and do
not appear in VM metadata. The dedicated control VM also has no external IP.
The per-generation control-ingress rule pairs that exact source `/32` with the
generation-owned guest target tag. GCP does not permit a source service
account and a target tag in the same firewall rule, so the runtime refuses
that invalid shape instead of treating it as identity evidence. Persistent
priority-900 rules admit only the managed-sandbox guest tag to TCP 8790, the
dedicated Direct VPC Cloud Run bridge tag to TCP 8787, and IAP to TCP 8787.
Priority-1000 rules deny every other source on those ports, including traffic
otherwise admitted by a default-VPC internal rule.

The metadata exception is a generation-owned bootstrap control, not ambient
provider egress. The guest has no service account or OAuth scope, so metadata
cannot mint a Google Cloud token. Legacy metadata endpoints, project SSH keys,
and OS Login are disabled. Metadata v1 supplies only the reviewed startup
marker and short-lived instance SSH-key path. The image generates unique SSH
host keys on first boot. An image-baked OUTPUT guard denies metadata to the
unprivileged `openagents` UID that runs SDK and I/O work. The startup marker
refuses to emit until that guard is present. Readiness observes the exact
metadata address, port, priority, target tag, marker, and deny-all rule.

Staging and production use distinct control nodes, private addresses, network
tags, firewall rules, Cloud Run bridge services, control-token secrets, broker
signing-key secrets, and native database authority. The deployment scripts
never reuse a staging secret for production or a production secret for
staging. The Cloud Run service has its platform invoker check disabled because
the public Worker-to-bridge hop is authenticated by the dedicated application
secret. The Worker presents that secret only as
`x-openagents-managed-sandbox-token` on the managed-sandbox runtime paths.
The bridge accepts that header on no generic control-plane path, compares it in
constant time, removes it, and synthesizes the private control
`Authorization` header. This prevents Cloud Run IAM from consuming the
application credential and keeps the control bearer out of request bodies,
URLs, metadata, and evidence.

## Isolation controls

The live provider applies these controls before it reports `ready`:

- The VM has no external IP address.
- The VM has no service account and no OAuth scopes.
- Project SSH keys are blocked.
- The historical readiness profile has no ingress rule and denies all IPv4
  egress.
- The operational turn profile admits only the generation-owned control SSH,
  provider-broker, and link-local metadata bootstrap paths described above.
  It denies every other ingress and egress path.
- Secure Boot, vTPM, and integrity monitoring are on.
- The boot disk is an auto-delete disk.
- IP forwarding is off.
- Only run-scoped capability refs pass profile admission.
- The guest receives no provider, SCM, or tool credential for the bounded
  readiness probe.

The guest startup script writes a generation-bound marker to the serial
console.
The control plane observes the provider state, the serial marker, the image,
the network rule, the address set, the service-account set, and the metadata
policy.
All observations must pass before the runtime reports `ready`.

The control service uses its attached GCE metadata identity.
The live runtime refuses `GOOGLE_APPLICATION_CREDENTIALS` and requires
`OA_CODEX_GCE_USE_METADATA_ADC=true`.
The workload VM does not receive that identity.

## Durable ownership and replay

The provider writes a private journal below:

```text
$OA_CODEX_CONTROL_STATE_ROOT/managed-sandbox-runtime/
```

The journal stores the exact scope, generation, admitted profile, provider
ownership, cleanup owner, state, measured run time, and the last 128 operation
settlements.
It writes cleanup ownership before the first provider effect.
It writes `stopping`, `resuming`, or `deleting` before the matching effect.

An exact operation retry returns the stored receipt.
A byte-different retry with the same operation or idempotency ref returns a
conflict.
A generation mismatch returns a conflict.
`reconcile` observes an uncertain create, resume, stop, or delete and converges
the same provider ownership.
It does not select a different region, image, machine, or provider.

## Cleanup

Delete is valid only after `stopped`, `failed`, `recovery_required`, or an
earlier `deleting` settlement.
Cleanup deletes the VM and all five generation-owned ingress and egress rules.
It then queries GCE until all these counts are zero:

- instance.
- firewall rules. And
- boot or scratch disk.

The guest receives no service-account grant. The operational profile creates
only the scoped control ingress rules described above.
The cleanup receipt records those sets as zero.
If any observed set is not zero, the runtime reports `recovery_required`.
It does not report `deleted`.

## Deployment inputs

The service is default-off.
Use `scripts/cloud/gcp-codex-control-deploy.sh --enable-managed-sandbox` to add
the exact live profile to the control container.
The deploy command requires the image project, exact image name, immutable
image ID, image digest, and profile digest.

Do not put a control token or a credential in a tracked file.
Do not set `GOOGLE_APPLICATION_CREDENTIALS` in the control container.
The control VM service account needs only the GCE operations for the admitted
resource and network policy.

Use `scripts/cloud/provision-managed-sandbox-runtime.sh --environment staging`
for the independent gate and `--environment production` only after staging
acceptance. Cloud Build submissions are asynchronous and the scripts poll the
authoritative build status. They do not depend on permission to stream the
default logs bucket. Re-running the immutable guest image build verifies and
reuses an exact `READY` image only when its recorded source revision matches.

## Owner-gated live component acceptance

The component acceptance script is default-off:

```text
scripts/cloud/managed-sandbox-live-acceptance.ts
```

It requires all deployed profile environment variables, a private control base
URL, a process-local control token, and this explicit gate:

```text
OA_MANAGED_SANDBOX_OWNER_GATE=I_ACCEPT_LIVE_GCP_COST
```

Run it with `--apply` and an evidence output path.
The script performs this sequence:

```text
create -> probe -> stop -> resume -> probe -> stop -> delete
```

It checks generation 1 before resume and generation 2 after resume.
It checks the measured cost against the sandbox budget.
It checks that all returned objects contain refs only.
It then runs an independent GCE residue query for the instance, firewall rule,
and disk.

The script has a cleanup path for every failure after create. If the bridge
times out before returning a receipt, the harness attempts reconcile. If the
journal is unavailable, it deletes only the exact deterministic VM, disk, and
five firewall names. The run stays failed even when emergency cleanup works.
An acceptance failure is not a reason to disable cleanup checks or use the
legacy fake provider.

The historical 2026-07-19 SBX-02 component run passed the exact seven-operation
sequence with final phase `deleted`, observed cleanup, measured cost below the
sandbox budget, and zero compute, firewall, disk, ingress, or grant residue.
It used the earlier probe profile and is not SBX-09 operational evidence. Its
refs-only evidence is
[`docs/sol/evidence/2026-07-19-sbx02-managed-sandbox-live.json`](../../sol/evidence/2026-07-19-sbx02-managed-sandbox-live.json).
Cloud Build `857943a8-6b19-4804-ade9-04ea9a261f00` produced the exercised
staging image. The image and staging control node were acceptance-only. The
node and its restricted ingress rule were deleted after the run.

SBX-02 component acceptance does not publish the route and does not prove the
Box facade, a Codex or Claude turn, Desktop, Sarah, mobile, or web.
SBX-09 owns the later independent end-to-end live acceptance and public
rollout decision.

## Deterministic verification

Run:

```bash
cargo test -p oa-codex-control managed_sandbox_runtime --no-fail-fast
cargo test -p oa-codex-control --no-fail-fast
bash -n scripts/cloud/gcp-codex-control-deploy.sh
```

The tests cover exact replay, generation conflicts, scope isolation, capacity
and budget refusal, run-scoped capabilities, fake-provider refusal, partial
create cleanup, recovery-required cleanup, stop, resume, and delete.

## Rollback

Remove `OA_MANAGED_SANDBOX_PROVISIONER=live_gce` from the control deployment to
disable new managed-sandbox provider effects.
Keep the state directory so a controlled reconciliation can clean existing
ownership.
Do not delete the journal before the independent residue oracle is zero.
