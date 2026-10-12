# The decisions H100 (`clef-decisions-h100`)

Live since 2026-10-12. One always-on NVIDIA H100 80 GB that serves our
open-weight Clef models to the decision gateway: `POST /v1/decisions`
(`openagents/decide`, `clef-flash`, `clef`) and `POST /v1/systemone`, as
the first door on both (Clef first, Jev as a backup, since 2026-10-12) ([docs/api/decisions.md](../api/decisions.md),
[#11446](https://github.com/OpenAgentsInc/openagents/issues/11446)).

## Where it is

| What | Value |
| --- | --- |
| VM | `clef-decisions-h100`, project `openagentsgemini`, zone `us-central1-a` (the restarter may move it to `-b` or `-c`) |
| Machine | `a3-highgpu-1g`: 1 × H100 80 GB, 26 vCPU, 234 GB RAM; Deep Learning VM image `common-cu129-ubuntu-2404-nvidia-580`; 200 GB pd-ssd boot disk |
| Pricing model | Spot, termination action **STOP** (a preemption stops it and keeps its disk) |
| Internal IP | `10.128.0.61`, the reserved address `clef-decisions-h100` (us-central1, `default` subnet), kept across restarts and zone moves |
| Runs as | `oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com` |
| Source | [`deploy/clef-decisions/`](../../deploy/clef-decisions/): `startup.sh` (boot), `restarter.sh`, `provision.sh` |

## What runs on it

Two systemd services, both `Restart=always`, each a Psionic
`psionic-openai-server` with one Clef model on CUDA
(`--decision-device cuda --decision-chunk 2048 --decision-max-tokens 65536`):

| Service | Port | Model | Answers as |
| --- | --- | --- | --- |
| `clef-flash` | 18096 | Clef-Flash Q4_K_M (sha256 `fd3e9060…0638c`) | `clef-flash` |
| `clef-27b` | 18098 | Clef 27B Q4_K_M (sha256 `bd9b8ae2…d3f61`) | `clef` |

Both use about 24 GB of the 80 GB together. `clef-health.timer` checks
`/health` on both every minute and restarts a service that failed three
checks in a row.

Every boot runs `startup.sh` (fetched from
`gs://openagentsgemini-decision-throughput/clef-decisions/startup.sh`): it
fetches the Psionic CUDA bundle named by the instance metadata key
`clef-bundle` (default `bin/latest`) when the one on disk differs, fetches
the two GGUFs from the GCS model cache onto the disk when they are missing
(checking their SHA-256 once), writes the units, and starts them. A fresh
disk serves both models about 60 seconds after boot; a restart of a stopped
VM is faster, the files already on disk.

## How the gateway reaches it

The gateway runs as the `gateway` sidecar of the Cloud Run service `coder`
(openagents.com), with Direct VPC egress on the subnet
`openagents-web-production` (10.42.27.0/26); staging uses
`openagents-web-staging` (10.42.26.0/26). Its launcher
([`deploy/staging/gateway.sh`](../../deploy/staging/gateway.sh)) sets
`decisions.clef_models` to `http://10.128.0.61:18096` (`clef-flash`) and
`http://10.128.0.61:18098` (`clef`); `DECISION_CLEF_FLASH_URL` and
`DECISION_CLEF_27B_URL` override them, empty turns one off.

The ports are not on the internet. Firewall rules on the VM's tag
`clef-decisions`:

| Rule | Priority | Allows |
| --- | --- | --- |
| `clef-decisions-from-gateway` | 900 | tcp 18096, 18098 from the two gateway subnets |
| `clef-decisions-iap-ssh` | 900 | tcp 22, 18096, 18098 from Identity-Aware Proxy (35.235.240.0/20), which needs a Google identity with IAP access |
| `clef-decisions-deny-other-ingress` | 910 | nothing else: denies every other source, overriding `default-allow-internal` and `default-allow-ssh` |

The VM keeps an ephemeral external IP for outbound traffic only (GCS, the
OS mirror); nothing inbound is allowed on it. We use a plain HTTP door over
the VPC rather than `openagents pylon serve --decide` on the VM: no relay
hop or NIP-44 per call, the gateway names the model per request, and the
payload never leaves our network. Pylons stay their own door.

## The restarter

Spot capacity can be taken back at any time. The Cloud Run job
`clef-decisions-restarter` (image `cloud-sdk:slim`, running
`gs://openagentsgemini-decision-throughput/clef-decisions/restarter.sh`)
runs every 3 minutes from the Cloud Scheduler job of the same name:

- `RUNNING` (or starting): nothing.
- `TERMINATED` (preempted, or stopped): `gcloud compute instances start`.
- A start refused for capacity adds one to the VM's `stockouts` label.
  After 5 in a row (about 15 minutes) the VM is deleted with its boot disk
  kept (labelled `clef-decisions=spare-disk`; the previous spare is
  deleted) and created again in the next us-central1 zone with the same
  internal IP. The new disk fills itself from GCS at boot.
- A VM labelled `clef-decisions=paused` is left alone.

Tested 2026-10-12: the VM was stopped at 16:57:27 UTC; the restarter found
it `TERMINATED` at 16:58:37 and started it by 16:59:00; the boot script
finished, both models serving, at 17:00:48: about three and a half minutes
from stop to serving.

Its log: Cloud Logging, `resource.type="cloud_run_job"
resource.labels.job_name="clef-decisions-restarter"`.

While the VM is down, `openagents/decide` and `/v1/systemone` fall
through to the Pylons, then Jev, then Vertex; `clef-flash` and `clef` answer `503 unavailable` until it is back.

## Checking it

```sh
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
gcloud compute instances list --project openagentsgemini --filter=name=clef-decisions-h100 \
  --format='table(name,zone.basename(),status,labels)'
# The boot log:
gcloud compute instances get-serial-port-output clef-decisions-h100 --zone us-central1-a \
  --project openagentsgemini | grep -a clef-decisions | tail
# On the VM:
gcloud compute ssh clef-decisions-h100 --zone us-central1-a --project openagentsgemini --tunnel-through-iap \
  --command 'systemctl status clef-flash clef-27b --no-pager | head -20; curl -s 127.0.0.1:18096/health; nvidia-smi'
# From this computer through IAP, then the same request as production:
gcloud compute start-iap-tunnel clef-decisions-h100 18096 --local-host-port=localhost:28096 \
  --zone us-central1-a --project openagentsgemini
# End to end:
curl -s https://openagents.com/api/v1/decisions -H 'content-type: application/json' \
  -d '{"model":"clef-flash","input":"I was charged twice.","questions":[{"type":"predicate","name":"money","instructions":"Did the customer lose money?"}]}'
```

The gateway's decision record (`<registry>/decisions/YYYY-MM-DD.jsonl` on
the account-store NFS share) names the door (`clef`) of every answer.

## Upgrading the Psionic bundle

When a new CUDA build lands (for example #11441's batching), build its
bundle and point the VM at it:

```sh
scripts/deploy/att-provider.sh gpu-build origin/main          # the att-provider-gpu image for that commit
scripts/bench/decision-throughput/h100/h100-run.sh bundle origin/main   # -> gs://.../bin/<sha10>/, bin/latest = <sha10>
gcloud compute instances add-metadata clef-decisions-h100 --zone us-central1-a \
  --project openagentsgemini --metadata clef-bundle=<sha10>
gcloud compute ssh clef-decisions-h100 --zone us-central1-a --project openagentsgemini --tunnel-through-iap \
  --command 'sudo google_metadata_script_runner startup'
```

The boot script swaps the bundle in (the old one stays at `/opt/clef.old`)
and restarts both services: about a minute with no hosted Clef, during
which decisions are answered by the Pylons or Jev. Pinning `clef-bundle` keeps a
later `bin/latest` from changing the VM on its next restart; remove the key
to follow `bin/latest`. Roll back by setting the previous tag.

Changing `startup.sh` or `restarter.sh`: edit them here, then
`sh deploy/clef-decisions/provision.sh upload`. A change to the VM's shape
(machine type, disk) is `restarter.sh create`'s; recreate by deleting the
VM and letting the restarter create it (about 5 minutes).

## Pausing it (stop spending)

```sh
gcloud compute instances add-labels clef-decisions-h100 --zone us-central1-a \
  --project openagentsgemini --labels clef-decisions=paused
gcloud compute instances stop clef-decisions-h100 --zone us-central1-a \
  --project openagentsgemini --discard-local-ssd=true
```

Resume: set the label back to `clef-decisions=serving`; the restarter starts
it within 3 minutes. To stop everything, also `gcloud scheduler jobs pause
clef-decisions-restarter --location us-central1 --project openagentsgemini`.
A stopped VM still bills its 200 GB disk (about $34 a month).

## Measured

In production on 2026-10-12, three questions about one short input
(about 390 tokens) through `https://openagents.com/api/v1/decisions`: the
gateway's round trip to the H100 took 63 ms (median) for `clef-flash` and
215 ms for `clef`; from a Mac in the US, 276–304 ms and 433 ms with the
internet included. The OpenAI Python SDK conformance run
(`scripts/dev/decisions-conformance.py`) passed 18 of 18 checks for each of
`openagents/decide`, `clef-flash`, and `clef`.

## Cost

| Item | Rate | A month (730 h) |
| --- | ---: | ---: |
| `a3-highgpu-1g` spot (H100, vCPUs, RAM) | about $6.40/h | about $4,670 |
| 200 GB pd-ssd | $0.17/GB-month | about $34 |
| External IP, restarter job, scheduler | | under $10 |
| **Total, always on** | | **about $4,700** |

At the measured batched throughput (39 decisions/s for `clef-flash` at 13
questions a request), a fully busy H100 costs about $0.045 per 1,000
decisions ([decision-throughput.md](decision-throughput.md)).

On-demand would end preemptions but costs more per hour and needs an
H100 quota the project does not have; the request for one on-demand H100 in
us-central1 is an owner step (`NEEDS_OWNER.md`). Moving to it is the same
`create` with `--provisioning-model STANDARD` and no termination action.
