# GCE Always-On Autonomous Codex Control Node (cloud#95)

Status: deployed (control node persistent); live GCE per-session provisioner
proven end-to-end (acquire -> in_use -> release, zero leaks). Autonomous coding
push leg is gated on creds (see NEEDS-OWNER).

This runbook documents the always-on `oa-codex-control` node deployed on our
Google Cloud project `openagentsgemini`, and the ephemeral worker-VM lifecycle
it drives. The control instance is intentionally PERSISTENT. Ephemeral worker
VMs (`oa-codex-sess-*`) are always torn down by the provisioner.

## What is deployed

| Item | Value |
| --- | --- |
| GCP project | `openagentsgemini` |
| Control instance | `oa-codex-control-1` |
| Zone | `us-central1-a` |
| Machine type | `e2-small` (Container-Optimized OS, runs the control image as a managed container) |
| Service account | `oa-codex-control@openagentsgemini.iam.gserviceaccount.com` |
| SA roles | `roles/compute.admin` (project), `roles/iam.serviceAccountUser` on the default compute SA (so it can create worker VMs), `roles/artifactregistry.reader` on `oa-cloud`, `roles/logging.logWriter` |
| VM scopes | `cloud-platform` (in-VM ADC via metadata server, NO key files) |
| Image | `us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/oa-codex-control:cloud95` |
| Control port | `8787`, firewall `oa-codex-control-port`, restricted to IAP range `35.235.240.0/20` + owner CIDR (NOT open to the world) |
| Labels | `openagents-managed=control`, `openagents-component=codex-control` |
| Container networking | `hostNetwork: true` so the daemon's `:8787` listener is reachable on the VM NIC; the GCE firewall is the access boundary |

The instance external IP is ephemeral; read it live with
`gcloud compute instances describe oa-codex-control-1 --project openagentsgemini
--zone us-central1-a --format='value(networkInterfaces[0].accessConfigs[0].natIP)'`.

The control image is built by `scripts/build-cloud-images.sh` (adds
`oa-codex-control` alongside `oa-node`/`oa-workroomd`) from
`docker/oa-codex-control.Dockerfile`. It bundles `git` + the Google Cloud CLI so
the live GCE per-session provisioner can shell out to `gcloud` using the VM's
attached service-account identity (metadata-server ADC).

## In-VM ADC (no key files)

The live provisioner (`crates/oa-codex-control/src/gce_capacity.rs`) refuses to
provision unless Application Default Credentials are detected. `adc_available()`
now also recognizes the GCE metadata-server identity (the VM's attached SA),
signaled by `OA_CODEX_GCE_USE_METADATA_ADC=true` (set in the container env by the
deploy script) or the standard `GCE_METADATA_*` env hints. This is what lets the
in-VM control daemon provision worker VMs with no key files on disk.

## Control daemon env (set in the container declaration)

```text
OA_CODEX_CONTROL_BIND=0.0.0.0:8787
OA_CODEX_CONTROL_TOKEN=<generated bearer; required on every request>
OA_CODEX_CONTROL_STATE_ROOT=/var/lib/openagents/codex-control
OA_CODEX_AUTH_JSON_ROOT=/var/lib/openagents/codex-accounts   # required by Config
OA_CODEX_CONTROL_ALLOW_LOCAL_AUTH_ONLY=true                  # no grant resolver yet
OA_CODEX_GCE_PROVISIONER=live
OA_CODEX_GCE_PROJECT_ID=openagentsgemini
OA_CODEX_GCE_ZONE=us-central1-a
OA_CODEX_GCE_MACHINE_TYPE=e2-small
OA_CODEX_GCE_USE_METADATA_ADC=true
```

The container runs with `hostNetwork: true` (konlet container declaration) so
the `:8787` listener is reachable on the VM NIC. `OA_CODEX_AUTH_JSON_ROOT` is
mandatory at startup even for provisioner-only operation; the account subdir is
only populated when a real Codex run is enabled (see NEEDS-OWNER).

## Reaching the control API

Direct external access depends on your workstation egress reaching the VM's
public IP on 8787 (allowed only for the IAP range + owner CIDR). The robust,
recommended path is IAP TCP forwarding (no public exposure required beyond IAP):

```bash
gcloud compute start-iap-tunnel oa-codex-control-1 8787 \
  --local-host-port=localhost:18787 \
  --project openagentsgemini --zone us-central1-a &
curl -sS http://localhost:18787/healthz -H "Authorization: Bearer $OA_CODEX_CONTROL_TOKEN"
```

The bearer token is held by the owner; it is not committed. Rotate by redeploy
with a fresh `--control-token`.

## How worker VMs are provisioned and torn down

For a `cloud-gcp` lane run, the control daemon:

1. `acquire` -> creates one ephemeral `oa-codex-sess-<digest>` VM (e2-small,
   `--no-address`, default network) + a session firewall rule
   `oa-codex-sess-fw-<digest>` (IAP SSH only), labeled `openagents-managed=true`.
2. `in_use` -> attaches the lease to the run.
3. `release` (always, on any terminal status incl. failure/cancel) -> deletes
   the VM + firewall rule, then verifies via a label/name-filtered
   `instances list` that ZERO session VMs remain. Mints a cleanup receipt.

Worker VMs are never left running. The control instance is the only persistent
resource.

## Enqueue / start a coding run

The control API listens on `:8787` (bearer required). Primary async path:

```bash
curl -sS -X POST "http://<control-ip>:8787/v1/codex-runs/start" \
  -H "Authorization: Bearer $OA_CODEX_CONTROL_TOKEN" \
  -H "Content-Type: application/json" \
  -d '{
    "runId": "agent_run_demo_1",
    "lane": "cloud-gcp",
    "goal": "Make the requested change.",
    "repository": "OpenAgentsInc/three-effect",
    "providerAccountRef": "provider-account_...",
    "authGrantRef": "codex-auth-grant_...",
    "sandboxMode": "danger_full_access"
  }'
```

`lane: "cloud-gcp"` selects the GCE ephemeral-per-session worker-VM lane. See
`docs/control/CODEX_CONTROL_API.md` for the placement (`/v1/placement`) and
durable queue (`/v1/queue`, cloud#97) entry points and full envelope.

## Redeploy from main

```bash
# Pull latest main, rebuild the image, redeploy the persistent instance.
git pull --no-rebase origin main
gcloud builds submit --project openagentsgemini \
  --config /tmp/cloudbuild-oa-codex-control.yaml .   # or scripts/build-cloud-images.sh --push --registry ...
# Recreate the instance with the new image (stop/delete first if it exists):
gcloud compute instances delete oa-codex-control-1 \
  --project openagentsgemini --zone us-central1-a --quiet
scripts/gcp-codex-control-deploy.sh \
  --project openagentsgemini \
  --control-token "$OA_CODEX_CONTROL_TOKEN" \
  --control-source-cidr <owner-ip>/32 \
  --apply
```

NOTE: cloud#96 (git writeback) and cloud#97 (durable unattended queue) land
separately. When they merge to main, redeploy so the always-on node runs the
queue-drain + push loop. This image is pinned to repo revision recorded in the
image label `org.opencontainers.image.revision`.

## STOP / DESTROY (owner spend control)

```bash
# Stop (keeps disk, stops billing for CPU):
gcloud compute instances stop oa-codex-control-1 \
  --project openagentsgemini --zone us-central1-a

# Fully destroy the persistent control node:
gcloud compute instances delete oa-codex-control-1 \
  --project openagentsgemini --zone us-central1-a --quiet

# Remove the control-port firewall:
gcloud compute firewall-rules delete oa-codex-control-port \
  --project openagentsgemini --quiet

# Safety sweep: ensure no ephemeral worker VMs/firewalls were left behind:
gcloud compute instances list --project openagentsgemini \
  --filter="name~^oa-codex-sess" --format="value(name,status)"
gcloud compute firewall-rules list --project openagentsgemini \
  --filter="name~^oa-codex-sess" --format="value(name)"
```

## End-to-end proof (2026-06-14)

The live GCE per-session provisioner was exercised against real
`openagentsgemini` via the production lease path
(`GceLease::acquire` -> `mark_in_use` -> `release`,
`OA_CODEX_GCE_PROVISIONER=live`):

- Created real worker VM `oa-codex-sess-88d1342b93bccd93` (e2-small,
  us-central1-a) + session firewall rule.
- Release: `deleted_vm=true, removed_firewall_rule=true,
  revoked_ssh_metadata=true, result=Completed`.
- Cleanup receipt `sha256:0576575a344e79d9c0d2f88ebf657f52831960467c81979103b0b8d24cbd0eb1`.
- Post-run `instances list` / `firewall-rules list` filtered to
  `^oa-codex-sess`: ZERO remaining. No leak.

Deployed-node proof (2026-06-14): a `cloud-gcp` run posted to the live
`oa-codex-control-1` node (`POST /v1/codex-runs/start`, via IAP tunnel) drove
the provisioner using the VM's metadata-server ADC (NO key files):

- `cloud.gce.provisioned` event with `provisionerKind: "live"` (real lane, not
  fake fallback), instance `gce-instance-ref://sha256/bfea0b82e8c6848e`.
- Worker VM `oa-codex-sess-bfea0b82e8c6848e` observed STAGING -> RUNNING ->
  STOPPING -> gone.
- `openagents.resource_usage_receipt.v1` emitted (`vmSeconds: 27`).
- Run terminal status `failed` at the Codex step (this provisioner-only image
  bundles no codex/opencode binary and no auth grant — expected; the VM
  lifecycle still completed and tore down). Post-run: zero `oa-codex-sess-*`.

This is the full provision -> attach -> teardown loop proven from the deployed
always-on node. The remaining gap to a green coding run is creds + a run-capable
image (NEEDS-OWNER below).

Reproduce the pure provisioner cycle locally (billable, one VM create+delete):

```bash
OA_CODEX_GCE_PROVISIONER=live OA_CODEX_GCE_PROJECT_ID=openagentsgemini \
OA_CODEX_GCE_ZONE=us-central1-a \
cargo test -p oa-codex-control live_gce_acquire_release_leaves_no_leak \
  -- --ignored --nocapture
```

## NEEDS-OWNER (autonomous coding push leg)

A full autonomous coding run that pushes code requires creds this environment
cannot fully provide:

1. Codex/ChatGPT connected-account auth for the run. A local cache exists at
   `.secrets/codex-chatgpt-chris-openagents/auth.json` (real ChatGPT OAuth, not
   an API key) and can back a run via the control daemon's local-auth bypass
   (`OA_CODEX_CONTROL_ALLOW_LOCAL_AUTH_ONLY=true` +
   `OA_CODEX_AUTH_JSON_ROOT=<dir>/<provider-account-ref>/auth.json`). For the
   managed always-on node, the preferred path is the openagents.com grant
   resolver (`OA_CODEX_GRANT_RESOLVE_URL` + `OA_CODEX_RUNNER_GRANT_TOKEN`), which
   is NOT present in `.secrets`.
2. The control image does not bundle the `codex` / `opencode` CLI; a run-capable
   image (or worker bootstrap) needs them. Today's image is provisioner-only +
   control API.
3. GitHub push: the control daemon enables repo checkout/push only when a write
   grant is resolved from `OA_OPENAGENTS_GITHUB_WRITE_GRANT_RESOLVE_URL` (default
   `https://openagents.com/api/github-write/grants/resolve`) using
   `OA_OPENAGENTS_GITHUB_WRITE_GRANT_TOKEN` / `OA_CODEX_RUNNER_GRANT_TOKEN`. No
   such runner grant token is in `.secrets`. The local `gh` OAuth token is not
   wired into this resolver flow. cloud#96 adds the git writeback path.

OWNER ACTION to complete the push loop:
- Provide an `OA_CODEX_RUNNER_GRANT_TOKEN` (runner identity for the openagents.com
  Codex + git-write grant resolvers), OR a fine-grained GitHub PAT / deploy key
  wired through the cloud#96 writeback path, AND
- a run-capable control image (codex/opencode bundled) once cloud#96/#97 land.

Until then the node is deployed and the worker-VM provision/teardown loop is
proven; the code-change-and-push leg is the remaining creds-gated step.
