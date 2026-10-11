# oa-boat: our own Boat-compatible sandbox service on GCE

2026-10-10, [#11256](https://github.com/OpenAgentsInc/openagents/issues/11256).

Hosted Boat's API (boat.dev) returned 502 from about 23:05 UTC on
2026-10-10, and openagents.com Environments were switched off at revision
start. The owner's directive: stop using hosted Boat and run the Boat SDK
against a backend in our own Google Cloud project.

`oa-boat` is that backend. It answers the part of Boat's v1 API our code
calls, in Boat's own JSON shapes (the `boat` crate's models), so the SDK,
`boat-template`, the web Environments and `openagents chat work --on boat`
run unchanged. Hosted Boat is now opt-in only (`BOAT_HOSTED=1`).

## What runs where

| Part | Where |
| --- | --- |
| Service | Cloud Run `oa-boat`, us-central1, `https://oa-boat-157437760789.us-central1.run.app`, API under `/api/v1`, `GET /health` unauthenticated |
| Code | [`crates/oa-boat`](../../crates/oa-boat) (Rust, axum); image from [`deploy/oa-boat`](../../deploy/oa-boat) |
| Runtime account | `oa-boat@openagentsgemini` with `compute.instanceAdmin.v1` and `compute.storageAdmin`; reads only its two secrets |
| Auth | `Authorization: Bearer` our token, Secret Manager `oa-boat-api-key`. Cloud Run's own invoker check is off (`--no-invoker-iam-check`); every path but `/health` needs the token |
| Reaching VMs | Direct VPC egress through subnet `oa-boat-run` (10.42.28.0/26, network `default`); firewall `oa-boat-ssh-from-service` admits tcp:22 from it to tag `oa-boat-sandbox`. SSH as `user` with Secret Manager `oa-boat-ssh-key`, put on each VM in its metadata |
| Sandboxes | GCE instances `oa-boat-<suffix>` for sandbox `bx_<suffix>`, label `openagents-managed=oa-boat`, no service account, no external address (Cloud NAT for egress), Shielded VM, boot from the newest `oa-coder-host` image or a template |
| Templates | GCE images labelled `openagents-managed=oa-boat-template` (Boat's "named snapshots") |
| Scale | min = max = 1 instance (state that is not in GCE labels lives in memory), CPU always on, 2 vCPU / 2 GiB |

## The API it answers

| Boat operation | Here |
| --- | --- |
| `GET /limits` | active count against `OA_BOAT_MAX_ACTIVE` (50); no start budget |
| `GET/POST /sandboxes`, `GET/PATCH/DELETE /sandboxes/{id}` | GCE instances. Create takes `type`, `ttlSeconds`, `env`, `from`, `setupScript`, `Idempotency-Key`, and our extras `provisioning` (`standard`/`spot`, or header `X-OA-Provisioning`) and `idleStopSeconds`. Delete needs `X-Ascii-Confirm-Delete` |
| `POST /sandboxes/{id}/stop`, `resume`, `fork` | stop keeps the boot disk; resume starts the same VM with fresh `env`; fork copies the disk through a snapshot into a new VM |
| `POST /sandboxes/{id}/commands` | synchronous, `stream: true` (NDJSON `started`/`stdout`/`stderr`/`exit`/`error`), or `detached: true` under `~/.oa-boat/proc/<pid>` |
| `GET /sandboxes/{id}/commands/{pid}` | status, exit code, output tails, log paths (so the SDK's follower works) |
| `GET/PUT /sandboxes/{id}/files` | `cat` over SSH; utf8 or base64; 48 MiB per call |
| `GET /sandboxes/{id}/usage` | run seconds and dollars, plus `dollarsPerHour`, `diskDollarsPerHour`, `totalDollarsPerHour`, `machineType`, `provisioning` |
| `GET /sandboxes/{id}/snapshots/latest` | the stopped disk, named by its stop time |
| `GET /deletion-operations/{id}` | completed once the instance is gone |
| `GET/POST /named-snapshots`, `GET/DELETE /named-snapshots/{name}` | GCE images of a sandbox's boot disk |
| prompt, steer, interrupt, events, desktop, hosted ports, SSH keys, share | 501 `not_supported` (Boat's integrated agents; our runs use the headless Coder runtime instead) |

A connection failure during a command is `502 boat_direct_failed`, as on
Boat: the command may still be running. Commands are never retried.

## Sandboxes

- **Sizes** (`crates/oa-boat/src/sizes.rs`), list prices in us-central1:

  | `type` | Machine | vCPU / RAM | Least disk | On demand $/h | Spot $/h |
  | --- | --- | --- | --- | --- | --- |
  | `small` | e2-standard-2 | 2 / 8 GB | 50 GB | 0.067 | 0.040 |
  | `default` | e2-standard-4 | 4 / 16 GB | 100 GB | 0.134 | 0.080 |
  | `large` | n2d-standard-8 | 8 / 32 GB | 200 GB | 0.338 | 0.172 |
  | `xlarge` | n2d-standard-16 | 16 / 64 GB | 300 GB | 0.676 | 0.344 |

  The boot disk is pd-balanced, at least the template image's size
  (`oa-coder-host` is 200 GB: $0.027 an hour, billed while stopped too).
- **On demand by default** (interactive Environments must not be
  preempted). `provisioning: spot` asks for a spot VM that stops, keeping
  its disk, when Google takes it back.
- **Zones** `us-central1-a,b,c,f`, tried in order when one has no capacity.
- **Ready** is per boot: once the VM answers SSH, the service writes `env`
  to `/run/oa-boat/env` (tmpfs, so neither a stopped disk nor a template
  image holds it), runs `setupScript` once, puts `coder-cloud-runtime` on
  the PATH from the image's `coder` account, and marks the boot ready.
  Commands source that env file and run as `user` in `/home/user`, as on
  Boat.
- **Stops.** The TTL (`ttlSeconds`, default 1 h, from create or resume) and
  an idle stop: no API call about the sandbox for `idleStopSeconds`
  (default 30 min) and no detached command running. The reaper runs every
  minute. GCE's own `maxRunDuration` (24 h) is a backstop.
- **Cost** is accounted in labels (`oa-boat-acc`, `oa-boat-accu`), so usage
  survives restarts of the service.

## Using it

The SDK's default base is this service. A client needs only the token:

```sh
export BOAT_API_KEY=$(gcloud secrets versions access latest --secret oa-boat-api-key --project openagentsgemini)
# optional: export BOAT_API_BASE=https://oa-boat-157437760789.us-central1.run.app/api/v1
```

`~/work/.secrets/boat.env` on the owner's Mac now holds this token and base;
the hosted key moved to `~/work/.secrets/boat-hosted.env`.

Hosted Boat stays reachable only with `BOAT_HOSTED=1` (the SDK refuses a
boat.dev base without it; with it the default base is boat.dev and the key
comes from Secret Manager `boat-api-key`).

## Operating it

```sh
export CLOUDSDK_CONFIG=/Users/christopherdavid/work/.secrets/gcloud-sa-config
# Build from a pushed commit, then deploy
C=$(git rev-parse origin/main)
gcloud builds submit --no-source --config deploy/oa-boat/cloudbuild.yaml --substitutions _COMMIT=$C \
  --project openagentsgemini --region us-central1 \
  --service-account projects/openagentsgemini/serviceAccounts/oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com \
  --gcs-log-dir gs://openagentsgemini-oa-mvp-cloud-build-logs/oa-boat
gcloud run deploy oa-boat --project openagentsgemini --region us-central1 \
  --image us-central1-docker.pkg.dev/openagentsgemini/cloud-run-source-deploy/oa-boat:$C
# Roll back
gcloud run revisions list --service oa-boat --region us-central1 --project openagentsgemini
gcloud run services update-traffic oa-boat --to-revisions REVISION=100 --region us-central1 --project openagentsgemini
# What runs, and the log (ready times, stops and their reasons)
gcloud compute instances list --filter labels.openagents-managed=oa-boat --project openagentsgemini
gcloud logging read 'resource.labels.service_name="oa-boat"' --project openagentsgemini --freshness 1h --format='value(textPayload)'
```

First deploy settings (kept by later `gcloud run deploy --image`):
`--service-account oa-boat@openagentsgemini.iam.gserviceaccount.com
--no-invoker-iam-check --min-instances 1 --max-instances 1
--no-cpu-throttling --cpu 2 --memory 2Gi --timeout 3600 --concurrency 250
--network default --subnet oa-boat-run --vpc-egress private-ranges-only
--set-secrets OA_BOAT_TOKEN=oa-boat-api-key:latest,OA_BOAT_SSH_KEY=oa-boat-ssh-key:latest`.

Settings (environment): `OA_BOAT_ZONES`, `OA_BOAT_BASE_FAMILY`
(`oa-coder-host`), `OA_BOAT_TTL_SECONDS` (3600), `OA_BOAT_IDLE_SECONDS`
(1800), `OA_BOAT_PROVISIONING` (`standard`), `OA_BOAT_MAX_ACTIVE` (50),
`OA_BOAT_MAX_RUN_SECONDS` (86400), `OA_BOAT_REAP_SECONDS` (60).

## Limits and follow-ups

- SSH does not pin host keys (`StrictHostKeyChecking=no` inside our own
  VPC). Pinning them from the guest attributes GCE publishes is a
  follow-up.
- One service instance holds the per-boot readiness and pending `env` in
  memory. A restart re-checks readiness over SSH; a VM that was mid-start
  during a restart comes up without the `env` it was created with.
- Boat's integrated agents (`prompt`/`events`) are not offered.

## Measurements

See [the deployment record](../deployment/oa-boat.md).
