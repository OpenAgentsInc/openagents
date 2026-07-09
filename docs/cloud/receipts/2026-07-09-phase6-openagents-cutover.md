# Phase 6 cutover receipt — openagents-built Cloud control plane

Date: 2026-07-09
Tracking: [#8591](https://github.com/OpenAgentsInc/openagents/issues/8591)
Owner-directed execution (explicit “do it all now”).

## Source

| Field | Value |
| --- | --- |
| Monorepo | `OpenAgentsInc/openagents` |
| Image git short | `a82a8dd358e0` (origin/main at build time) |
| Private cloud | historical only — not in deploy path |

## Images (Artifact Registry, linux/amd64)

Repository: `us-central1-docker.pkg.dev/openagentsgemini/oa-cloud`

| Image | Tag |
| --- | --- |
| `oa-codex-control` | `openagents-main-a82a8dd358e0` (+ `openagents-main-amd64`) |
| `oa-workroomd` | `openagents-main-a82a8dd358e0` |
| `oa-node` | `openagents-main-a82a8dd358e0` |

Build note: first push from Apple Silicon produced **linux/arm64** and failed to start on GCE. Rebuild forced `--platform linux/amd64`. `scripts/cloud/build-cloud-images.sh` now always targets amd64.

## Deployed control plane

| Field | Value |
| --- | --- |
| Instance | `oa-codex-control-1` |
| Zone | `us-central1-a` |
| Project | `openagentsgemini` |
| External IP (post-redeploy) | `35.223.189.76` |
| Health | `GET /healthz` → `200 {"service":"oa-codex-control","status":"ok"}` |
| Image | `…/oa-codex-control:openagents-main-a82a8dd358e0` (linux/amd64) |
| GCE provisioner env | `live` (metadata ADC on instance SA) |
| Cloud-VM provisioner on this host | `fake` (e2-small has no `/dev/kvm`; Agent Computer live Firecracker remains on `agent-computer-gce-1`) |

Operator env (gitignored local secrets only):

- `OA_CLOUD_CONTROL_URL=http://35.223.189.76:8787`
- `OA_CLOUD_CONTROL_TOKEN` / `OA_CODEX_CONTROL_TOKEN` in `.secrets/oa-codex-control-gce.env`

## Live smoke (against openagents-built node)

Public-safe sequence (no secrets in this receipt):

1. `POST /v1/placement/start` lane `cloud-gcp` → **202**, capacity class `gce.ephemeral.standard.v1`
2. Polled `GET /v1/codex-runs/<run_id>/events` until:
   - `cloud.gce.provisioned`
   - `cloud.run.started`
   - `cloud.gce.resource_usage_receipt`
   - (run may `cloud.run.failed` if Codex binary/auth not present on control host — expected for control-only smoke)
3. `POST /v1/codex-runs/<run_id>/cancel` → **202** `cancelRequested`
4. `POST /v1/cloud-vm/sessions` → **200**, `provisionerKind=fake`, `cleanupReceipt.tornDown=true`

Example run id: `phase6_live2_1783560050`

## Agent Computer host

- Host `agent-computer-gce-1` remains the nested-virt Firecracker lane (`/dev/kvm`).
- In-repo `oa-workroomd` image is published for guest bake; staging binary via
  `apps/pylon/deploy/agent-computer/build-workroomd-for-image.sh` / Docker pull of
  `oa-workroomd:openagents-main-a82a8dd358e0`.

## #8503 full DoD status

| Item | Status |
| --- | --- |
| Control plane built from openagents | **done** (this receipt) |
| Live placement + GCE capacity receipts | **done** |
| Fake Cloud-VM lifecycle on control host | **done** |
| Mobile-dispatched turn inside Firecracker microVM with full writeback + exact token receipt | **not re-run in this cutover** — prior substrate proofs exist; full mobile DoD still requires arming `work_context` + live Cloud-VM on nested-virt host with agent-computer rootfs |

## Security notes

- Control firewall temporarily includes `0.0.0.0/0` on port 8787 for operator cutover smoke. Prefer tightening to IAP + office egress when not actively testing.
- Tokens remain only in Secret Manager / local `.secrets/` — never in git.

## Definition of done for deploy path

**Production control plane no longer depends on checking out `OpenAgentsInc/cloud`.**
Images and deploy scripts run from the public monorepo.
