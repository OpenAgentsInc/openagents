# Agent Computer production build and deploy

This runbook publishes the production Agent Computer control image and the
OpenAgents monolith that advertises and dispatches the managed mobile runtime.
It intentionally does not record host addresses or bearer values. Read live
topology from Google Cloud and bearer material from Secret Manager.

## Preconditions

- Work from a clean `main` at the exact revision being released.
- Use the isolated automation gcloud configuration documented in the workspace
  `AGENTS.md`.
- The nested-virtualization host is running, has `/dev/kvm`, and can pull from
  the `oa-cloud` Artifact Registry repository.
- Secret Manager has a current `oa-cloud-run-bridge-control-token` version.
- Secret Manager has a current `provider-token-custody-aes-key-b64` version.
  the monolith deploy mounts it as `PROVIDER_TOKEN_CUSTODY_AES_KEY_B64`.
- The control URL is reachable from the production Cloud Run VPC connector.

## 1. Build the control image

Choose an immutable tag derived from the release commit:

```sh
REVISION="$(git rev-parse --short=12 HEAD)"
IMAGE="us-central1-docker.pkg.dev/openagentsgemini/oa-cloud/oa-codex-control:live-${REVISION}"
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  gcloud builds submit \
  --project openagentsgemini \
  --region us-central1 \
  --config docker/cloud/cloudbuild-oa-codex-control.yaml \
  --substitutions="_IMAGE=${IMAGE},_REVISION=${REVISION}" \
  .
```

Record the resulting image digest. Deploy that digest, not a mutable tag, to
the nested-virtualization host. Run the container with KVM access, NET_ADMIN,
the host-side Firecracker state directories, the control bearer, and
`OA_CODEX_GCE_PROVISIONER=live`. Bind the service only on the private control
network.

## 2. Verify the live control plane

From a caller on the private network, make an authenticated request to:

```text
GET /v1/cloud-vm/readiness
```

Continue only when the response has schema
`openagents.agent_computer_readiness.v1`, `ready: true`, and
`provisionerKind: live`. A fake provisioner, malformed response, timeout, or
authentication failure is a hard stop.

Then run one bounded provision/exec/cleanup smoke. Confirm the command exits
successfully, cleanup returns `tornDown: true`, and the host has no remaining
Firecracker processes, taps, jails, or zombie children.

## 3. Configure the production control route

The production monolith needs `OA_CLOUD_CONTROL_URL` and the Secret Manager
mount `OA_CLOUD_CONTROL_TOKEN`. If a narrow proxy is used, its allow-list must
include both `/v1/placement` and `/v1/cloud-vm/readiness`, and it must forward
the same bearer to the private control service.

Do not commit an internal address. Pass the live URL to the deploy command:

```sh
export OA_CLOUD_CONTROL_URL="<private-or-bearer-gated-control-url>"
```

## 4. Deploy the OpenAgents monolith

```sh
cd apps/openagents.com
CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config \
  bash workers/api/scripts/deploy-cloudrun.sh production
```

Production renders `CLOUD_CODING_SESSIONS_ENABLED=true` and
`OA_CODEX_GCE_PROVISIONER=live`. Advertisement still fails closed unless the
authenticated readiness probe succeeds and the owner has healthy Codex and
GitHub connections.

## 5. Acceptance

Use a repository-bound coding thread. Confirm the mobile execution catalog
advertises `Agent Computer`, enqueue one bounded turn, and verify:

1. the repository branch/ref resolves server-side to an immutable 40-character
   commit SHA before placement, and exactly one durable `turn.started` wins.
2. the provider grant is issued only after that claim.
3. the live microVM executes the request.
4. a terminal `turn.finished` is synced to the thread.
5. the microVM and host network resources are torn down.

The 2026-07-12 production acceptance turn
`turn.agent-computer-smoke.20260712T235107Z` exercised the literal `main` binding.
It completed with one consumed Postgres provider grant. It produced one exact
usage row with 60,167 input, 294 output, and 60,461 total tokens. The terminal control
event reported exit code 0. No tap or runtime directory remained after cleanup.

If any gate fails, remove `Agent Computer` from the catalog by disabling the
cloud-coding flag or live provisioner setting before investigating. Never
substitute the fake provisioner as production acceptance evidence.
