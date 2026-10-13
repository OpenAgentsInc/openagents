# The decision GPUs (`clef-decisions-h100`, `clef-decisions-a100`)

Two always-on GPUs serve our open-weight Clef models to the decision
gateway: `POST /v1/decisions` (`openagents/decide`, `clef-flash`, `clef`)
and `POST /v1/systemone`, as the first door on both (Clef first, Jev as a
backup, since 2026-10-12) ([docs/api/decisions.md](../api/decisions.md),
[#11446](https://github.com/OpenAgentsInc/openagents/issues/11446)).

- **`clef-decisions-h100`**: a spot NVIDIA H100 80 GB, live since
  2026-10-12.
- **`clef-decisions-a100`**: an on-demand NVIDIA A100 40 GB, live since
  2026-10-12 ([#11457](https://github.com/OpenAgentsInc/openagents/issues/11457)).
  On-demand, so it is never preempted; it keeps hosted Clef up while the
  spot H100 is taken back.

The gateway treats each model as a pool of the two and balances between
them ([below](#the-pool-and-how-a-decision-picks-a-machine)).

## Where they are

| What | `clef-decisions-h100` | `clef-decisions-a100` |
| --- | --- | --- |
| Zone | `us-central1-a` (the restarter may move it to `-b` or `-c`) | `us-central1-b` (the restarter may move it to `-a`, `-c` or `-f`) |
| Machine | `a3-highgpu-1g`: 1 × H100 80 GB, 26 vCPU, 234 GB RAM | `a2-highgpu-1g`: 1 × A100 40 GB, 12 vCPU, 85 GB RAM |
| Pricing model | Spot, termination action **STOP** (a preemption stops it and keeps its disk) | On-demand (standard), never preempted; host maintenance stops it and GCE restarts it (`--restart-on-failure`) |
| Internal IP | `10.128.0.61`, the reserved address `clef-decisions-h100` | `10.128.0.75`, the reserved address `clef-decisions-a100` |
| Psionic bundle | pinned by metadata `clef-bundle=80d8a5c292` (an sm_90 build) | pinned by metadata `clef-bundle=80d8a5c292-sm80` (an sm_80 build) |

Both: project `openagentsgemini`, the `default` subnet, Deep Learning VM
image `common-cu129-ubuntu-2404-nvidia-580`, a 200 GB pd-ssd boot disk,
network tag `clef-decisions`, label `app=clef-decisions`. The reserved
internal IPs are kept across restarts and zone moves.

| What | Value |
| --- | --- |
| Runs as | `oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com` |
| Source | [`deploy/clef-decisions/`](../../deploy/clef-decisions/): `startup.sh` (boot), `restarter.sh`, `provision.sh` |

## What runs on each

The same on both machines. Two systemd services, both `Restart=always`, each a Psionic
`psionic-openai-server` with one Clef model on CUDA
(`--decision-device cuda --decision-chunk 2048 --decision-max-tokens 65536`):

| Service | Port | Model | Answers as |
| --- | --- | --- | --- |
| `clef-flash` | 18096 | Clef-Flash Q4_K_M (sha256 `fd3e9060…0638c`) | `clef-flash` |
| `clef-27b` | 18098 | Clef 27B Q4_K_M (sha256 `bd9b8ae2…d3f61`) | `clef` |

Both use about 24 GB together: of the H100's 80 GB, and of the A100's 40
GB (23.9 GB measured at boot). `clef-health.timer` checks
`/health` on both every minute and restarts a service that failed three
checks in a row.

Every boot runs `startup.sh` (fetched from
`gs://openagentsgemini-decision-throughput/clef-decisions/startup.sh`): it
fetches the Psionic CUDA bundle named by the instance metadata key
`clef-bundle` (default `bin/latest`, an sm_90 build that does not run on
the A100, so the A100 pins its own) when the one on disk differs, fetches
the two GGUFs from the GCS model cache onto the disk when they are missing
(checking their SHA-256 once), writes the units, and starts them. A fresh
disk serves both models about 60 seconds after boot; a restart of a stopped
VM is faster, the files already on disk.

## How the gateway reaches them

The gateway runs as the `gateway` sidecar of the Cloud Run service `coder`
(openagents.com), with Direct VPC egress on the subnet
`openagents-web-production` (10.42.27.0/26); staging uses
`openagents-web-staging` (10.42.26.0/26). Its launcher
([`deploy/staging/gateway.sh`](../../deploy/staging/gateway.sh)) sets
`decisions.clef_models`:

| Model | Pool |
| --- | --- |
| `clef-flash` | `h100=http://10.128.0.61:18096,a100=http://10.128.0.75:18096` |
| `clef` | `h100=http://10.128.0.61:18098,a100=http://10.128.0.75:18098` |

`DECISION_CLEF_FLASH_URL` and `DECISION_CLEF_27B_URL` override them; empty
turns one off, and a single URL (no `name=`) is a pool of one named by its
host and port.

## The pool and how a decision picks a machine

The policy is deliberately simple and measurable. Since 2026-10-13 each
machine serves two requests per model together (#11441's CUDA batching,
`clef-batch-requests=2`) and still refuses with `503 busy` past 8 queued.

1. **Fewest in flight.** A decision goes to the backend with the fewest of
   the gateway's decisions in flight on it; a tie goes to the one listed
   first, the H100 (about twice as fast: Flash 0.074 s against 0.149 s at
1k tokens, 27B 0.21 s against 0.42 s, [gpu-options.md](gpu-options.md)).
   One decision at a time all go to the H100; two or more at once spread.
2. **Bench and move on.** A backend that answers `busy` (or
   `rate_limited`, `overloaded`) sits out 2 seconds; one that is
   unreachable, times out, answers another 5xx, or answers in the wrong
   shape sits out 15 seconds. Either way the decision goes straight to the
   next backend in the pool, and only when every backend has been tried to
   the next door (Pylons, Jev, Vertex; `clef-flash` and `clef` have no next
   door and answer `503 unavailable`). A request the backend refused as
   invalid is not retried elsewhere. When every backend is sitting out, the
   one whose bench ends first is still tried. When the last backend left
   said busy, the pool is asked again after 150 ms and after 300 ms (as the
   Pylon door does) before the decision moves on: a burst larger than one
   machine's queue of 8 then waits rather than failing.
3. **Visible.** The answer names the machine: `service.backend` in a
   `/v1/systemone` answer, `openagents.backend` in a `/v1/decisions`
   answer, the `X-Decision-Backend` header on both, and `backend` in the
   decision record (`<registry>/decisions/YYYY-MM-DD.jsonl`, each attempt
   too). Traffic split per machine:

   ```sh
   jq -r 'select(.door=="clef") | .backend' <registry>/decisions/2026-10-12.jsonl | sort | uniq -c
   ```

The counts are per gateway instance (Cloud Run may run several); each
balances its own decisions.

### Adding or removing a backend

- **Add:** a VM serving the same two ports, reachable from the gateway
  subnets (network tag `clef-decisions` gets the firewall below). Append
  `,<name>=http://<ip>:18096` to the `clef-flash` pool and
  `,<name>=http://<ip>:18098` to the `clef` pool in `gateway.sh` (or set
  `DECISION_CLEF_FLASH_URL`/`DECISION_CLEF_27B_URL`), and deploy the
  gateway. A new GPU VM needs a `shape` entry in `restarter.sh` and its
  address in `provision.sh`.
- **Remove:** drop it from both pools and deploy; until then the gateway
  benches it every 15 seconds at the cost of one failed connect. Then
  delete the VM and release its address.
- **Take one out briefly** (maintenance): label it `clef-decisions=paused`
  and stop it. The pool benches it and the other machine answers.

The ports are not on the internet. Firewall rules on the VM's tag
`clef-decisions` (both VMs):

| Rule | Priority | Allows |
| --- | --- | --- |
| `clef-decisions-from-gateway` | 900 | tcp 18096, 18098 from the two gateway subnets |
| `clef-decisions-iap-ssh` | 900 | tcp 22, 18096, 18098 from Identity-Aware Proxy (35.235.240.0/20), which needs a Google identity with IAP access |
| `clef-decisions-deny-other-ingress` | 910 | nothing else: denies every other source, overriding `default-allow-internal` and `default-allow-ssh` |

Each VM keeps an ephemeral external IP for outbound traffic only (GCS, the
OS mirror); nothing inbound is allowed on it. We use a plain HTTP door over
the VPC rather than `openagents pylon serve --decide` on the VM: no relay
hop or NIP-44 per call, the gateway names the model per request, and the
payload never leaves our network. Pylons stay their own door.

## The restarter

Spot capacity can be taken back at any time. The Cloud Run job
`clef-decisions-restarter` (image `cloud-sdk:slim`, running
`gs://openagentsgemini-decision-throughput/clef-decisions/restarter.sh`)
runs every 3 minutes from the Cloud Scheduler job of the same name, for
each of the two VMs (the A100 is on-demand and GCE restarts it after host
maintenance itself; the restarter is its backstop, and recreates it if it
is ever deleted):

- `RUNNING` (or starting): nothing.
- `TERMINATED` (preempted, or stopped): `gcloud compute instances start`.
- Missing: created in the first zone of its list with capacity.
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

While one VM is down, the pool sends every hosted decision to the other.
Only while both are down do `openagents/decide` and `/v1/systemone` fall
through to the Pylons, then Jev, then Vertex, and `clef-flash` and `clef`
answer `503 unavailable`.

## Checking it

```sh
export CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config
gcloud compute instances list --project openagentsgemini --filter=labels.app=clef-decisions \
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
# The same for the A100: clef-decisions-a100 --zone us-central1-b.
# End to end:
curl -s https://openagents.com/api/v1/decisions -H 'content-type: application/json' \
  -d '{"model":"clef-flash","input":"I was charged twice.","questions":[{"type":"predicate","name":"money","instructions":"Did the customer lose money?"}]}'
```

The gateway's decision record (`<registry>/decisions/YYYY-MM-DD.jsonl` on
the account-store NFS share) names the door (`clef`) and the machine
(`backend`: `h100` or `a100`) of every answer.

## Upgrading the Psionic bundle

When a new CUDA build lands (for example #11441's batching), build its
bundle and point the VMs at it. The H100 takes the sm_90 bundle the recipe
makes; the A100 needs the same commit built for sm_80 (the recipe's image
built with `CUDAARCHS=80`, packed the same way, as `bin/<sha10>-sm80/`),
then `clef-bundle=<sha10>-sm80` on `clef-decisions-a100`. Upgrade one
machine at a time: the pool keeps answering from the other.

```sh
scripts/deploy/att-provider.sh gpu-build origin/main          # the att-provider-gpu image for that commit
scripts/bench/decision-throughput/h100/h100-run.sh bundle origin/main   # -> gs://.../bin/<sha10>/, bin/latest = <sha10>
gcloud compute instances add-metadata clef-decisions-h100 --zone us-central1-a \
  --project openagentsgemini --metadata clef-bundle=<sha10>
gcloud compute ssh clef-decisions-h100 --zone us-central1-a --project openagentsgemini --tunnel-through-iap \
  --command 'sudo google_metadata_script_runner startup'
```

The boot script swaps the bundle in (the old one stays at `/opt/clef.old`)
and restarts both services: about a minute with that machine out, during
which the pool sends its decisions to the other machine. Pinning `clef-bundle` keeps a
later `bin/latest` from changing the VM on its next restart; remove the key
to follow `bin/latest`. Roll back by setting the previous tag.

**Batching (#11441).** `startup.sh` reads two optional metadata keys:
`clef-batch-requests` (2–4) puts `PSIONIC_CLEF_BATCH_REQUESTS` in both
units, and `clef-max-queue` sets `--decision-max-queue` (default 8); a unit
that changed restarts. Both machines run with `clef-batch-requests=2`.

**Prefix reuse (#11442).** On the H100 the sm_90 build reuses prefixes by
default (exact; a 32nd of the device, at most 2 GiB per service; clef-native.md
Round 9). Responses report `usage.cached_input_tokens`. The A100's sm_80
build keeps it off.

**Status 2026-10-13, 03:30 UTC (applied): the head in one round trip.**
Both machines run `80d8a5c292`, whose CUDA head runs in one submission
(clef-native.md Round 11; `PSIONIC_CLEF_CUDA_HEAD=staged` in a unit's
environment would bring back the old path). Rolled out A100 first.

| Machine | `clef-bundle` | Batching | Prefix reuse |
| --- | --- | --- | --- |
| `clef-decisions-a100` | `80d8a5c292-sm80` | 2 | off |
| `clef-decisions-h100` | `80d8a5c292` | 2 | on (2 GiB) |

- e2e (40 requests, 104 questions) on each machine against its previous
  bundle: identical answers on both models (max |Δp| 0). Flash vs the CPU
  goldens: A100 100 %, max 0.0049; H100 100 %, max 0.0010 (both unchanged).
- Production bursts (16 `clef-flash` + 16 `clef` at once, two questions,
  three rounds each), 0 errors. Median `openagents.latency_ms` before → after:
  Flash H100 312–435 → 221–261 ms; Flash A100 1,182–1,306 → 710–838 ms;
  27B H100 310–499 → 432–498 ms; 27B A100 1,763–2,066 → 1,670–1,880 ms.
  Bursts on the live pool are noisy; the controlled H100 measurement is in
  clef-native.md Round 11 (head 6.1 → 2.0 ms, Flash −6 to −8 % at ≤ 1k
  tokens, −10 % on prefix hits).
- One call at a time: `clef-flash` 19–23 ms gateway round trip on the H100.

Rollback to the previous bundles (one machine at a time, then rerun the
startup script): A100 `add-metadata clef-bundle=23f44c8ad5-sm80`, H100
`add-metadata clef-bundle=15aac3e386`.

**Status 2026-10-13, 01:30 UTC (superseded above).**

| Machine | `clef-bundle` | Batching | Prefix reuse |
| --- | --- | --- | --- |
| `clef-decisions-a100` | `23f44c8ad5-sm80` | 2 | off |
| `clef-decisions-h100` | `15aac3e386` | 2 | on (2 GiB) |

- e2e (40 requests, 104 questions) after the swap:
  - A100: Flash vs the CPU-lane goldens 100 %, max |Δp| 0.0049; both models
    identical to the A100's previous bundle.
  - H100: Flash vs the goldens 100 %, max |Δp| 0.0010 (0.0022 before).
    27B vs the old `42334c876b` build 98.1 %, max 0.022: two near ties
    (0.492/0.508 and a three-way 0.345/0.346). That is the old build's
    f16 drift; the Hopper path agrees with the f32 reference (#11443).
- A burst direct to each machine returns `psionic.batch.max_requests` 2.
- Prefix reuse on the H100 (`15aac3e386`): the same 1.9k-token state with a
  new question restores 1,872 tokens (Flash 102 → 35 ms, 27B 271 → 60 ms
  server time). The cache fills to its budget and evicts least recently
  used.
- The chat worker runs release `9f2a916688` without `OPENAGENTS_DECISIONS=pylon`
  (env backup `/etc/coder-worker/coder-worker-chat.env.before-9f2a916688`).
  Its journal shows `judge answered by door clef`.

Production, 16 `clef-flash` and 16 `clef` calls at once (one predicate on a
short unique input). The figures are the median `openagents.latency_ms` per
machine. 0 errors in every round.

| | Flash H100 | Flash A100 | 27B H100 | 27B A100 |
| --- | ---: | ---: | ---: | ---: |
| before (serial engines) | 866 ms | 1,260 ms | 867 ms | 1,266 ms |
| A100 upgraded | 774–854 ms | 825–869 ms | 774–837 ms | 1,272–1,389 ms |
| both upgraded | 230–330 ms | 842–916 ms | 338–343 ms | 1,304–1,438 ms |

Rollback, one machine at a time:
- A100: `add-metadata clef-bundle=661e02d3ed-sm80`, `remove-metadata --keys
  clef-batch-requests`, then rerun the startup script.
- H100: `add-metadata clef-bundle=42334c876b` (or `remove-metadata --keys
  clef-bundle` for `bin/latest`), `remove-metadata --keys clef-batch-requests`,
  then rerun the startup script.
- Chat worker: `sudo ln -sfn /opt/coder-worker/releases/6818ae7d7d
  /opt/coder-worker/chat && sudo cp -p
  /etc/coder-worker/coder-worker-chat.env.before-9f2a916688
  /etc/coder-worker/coder-worker-chat.env && sudo systemctl restart
  coder-worker-chat`.

Changing `startup.sh` or `restarter.sh`: edit them here, then
`sh deploy/clef-decisions/provision.sh upload`. A change to the VM's shape
(machine type, disk) is `restarter.sh create`'s; recreate by deleting the
VM and letting the restarter create it (about 5 minutes).

## Pausing one (stop spending)

The same for `clef-decisions-a100` (zone `us-central1-b`).

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

### The pool, H100 and A100 (2026-10-12)

In production through `https://openagents.com/api/v1/decisions`, two
questions (a predicate and a choice of three) about a short unique input,
16 calls at once per row. Times are `openagents.latency_ms` (the gateway's
round trip to the machine, queueing included), split by
`openagents.backend`:

| Model | H100: calls, median, max | A100: calls, median, max | Errors |
| --- | --- | --- | ---: |
| `clef-flash` | 8, 217 ms, 383 ms | 8, 385 ms, 638 ms | 0 |
| `clef` | 8, 579 ms, 1,011 ms | 8, 895 ms, 1,531 ms | 0 |
| `openagents/decide` | 8, 211 ms, 381 ms | 8, 375 ms, 631 ms | 0 |
| `clef-flash`, the A100's Flash service stopped | 16, 415 ms, 776 ms | — | 0 |
| `clef-flash`, the H100's Flash service stopped (8 at once) | — | 8, 328 ms, 535 ms | 0 |

One call at a time, every call goes to the H100: 47 ms for `clef-flash`; with
the H100's Flash stopped, the A100 answers the same call in 85 ms. Before the
busy retry, 16 calls at once on the H100 alone overflowed its queue of 8 and
3 answered `503`; with it, none did. The A100's answers match the H100's to
about 2e-5 (the same probability, 0.96558 against 0.96560, on one test).

### The H100 alone

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
| `a2-highgpu-1g` on-demand (A100 40 GB, vCPUs, RAM) | about $3.67/h | about $2,680 |
| 2 × 200 GB pd-ssd | $0.17/GB-month | about $68 |
| External IPs, restarter job, scheduler | | under $15 |
| **Total, both always on** | about $10.10/h | **about $7,430** |

The A100 is the cheaper machine to keep on: never preempted, and per
measured token about the same as a spot H100 for Flash
([gpu-options.md](gpu-options.md)). A 1-year commitment would bring it to
about $2.31/h; 3 years, $1.29/h.

At the measured batched throughput (39 decisions/s for `clef-flash` at 13
questions a request), a fully busy H100 costs about $0.045 per 1,000
decisions ([decision-throughput.md](decision-throughput.md)).

On-demand would end preemptions but costs more per hour and needs an
H100 quota the project does not have; the request for one on-demand H100 in
us-central1 is an owner step (`NEEDS_OWNER.md`). Moving to it is the same
`create` with `--provisioning-model STANDARD` and no termination action.
