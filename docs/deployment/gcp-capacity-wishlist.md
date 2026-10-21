# Google Cloud capacity wishlist

Written 2026-10-21 for quota increase requests and for the owner's Google
account team. Project: `openagentsgemini`. Every machine we start goes
through the spend guard (`scripts/dev/cloud-budget.py`).

## Why we're asking

The DeepSeek-V4.1 work (#12369) serves the full 510 GB model natively in
Psionic, split into stages across GPU machines that run as Pylons. On
2026-10-21 we ran it as four stages:

- on one `g4-standard-192` (4 × RTX PRO 6000);
- on four `g4-standard-48` machines;
- on two `g4-standard-96` machines.

It ran at about 29 tokens per second per user. The B200 reference runs
used `a4-highgpu-8g`.

What blocks us is **capacity, not quota.** Our failures were zone
stockouts:

- **B200 (A4):** spot was preempted after 40 s in us-west2-c and stocked
  out in every A4 zone we tried. Flex-start requests in us-central1-b,
  us-east1-b, europe-west4-b and us-west2-c expired without capacity.
- **RTX PRO 6000 (G4):** spot and on-demand sweeps of every US zone found
  no capacity during one session. Ten US zones were exhausted before
  us-west3-a granted a machine.

Quota still matters once capacity exists. Through the API, our service
account sees the per-family GPU quotas (`NVIDIA_RTX_PRO_6000`,
`NVIDIA_B200`, `NVIDIA_H100`, `NVIDIA_H200`) with no regional values.
Check the actual limits in the console (IAM & Admin > Quotas, filtered on
"GPUs per GPU family") before filing a request.

## What we've used, for scale

| Machine | GPUs | Price we paid | Use |
| --- | --- | --- | --- |
| `g4-standard-48` | 1 × RTX PRO 6000 | about $1.66/h spot | One stage per machine |
| `g4-standard-96` | 2 × RTX PRO 6000 | about $3.33/h spot | Two stages per machine |
| `g4-standard-192` | 4 × RTX PRO 6000 | $6.65/h spot | Whole model on one machine |
| `a4-highgpu-8g` | 8 × B200 | about $40/h spot, $64.44/h flex-start | Reference runs and device measurements |
| `n2d-highmem-96` | none | $5.47/h | CPU reference runs |

Spend so far against the $3,000 DeepSeek budget is about $125.

## The wishlist

Ranked by how much each unblocks. "On-demand" means the regular GPU
quota; "spot" means its preemptible counterpart. Ask for both, because
spot is how we stay within budget.

### 1. RTX PRO 6000 (G4): the serving GPU

This is the target Pylon hardware, and the one we need every day.

| Ask | Regions | Why |
| --- | --- | --- |
| 8 GPUs on-demand, 16 spot, per region | us-central1, us-west3, us-east4, europe-west4, asia-southeast1 | One 4-GPU group plus a spare machine for failure tests, in at least three regions so one stockout doesn't stop work |
| 32 GPUs spot in one region | us-central1 or europe-west4 | Several groups at once: qualification, the launch demo, and measurements in parallel |
| A small reservation: 1 × `g4-standard-192` for 30 days, or recurring weekday blocks | us-central1 | A group that's always available for the public Grid demo and for reruns |

### 2. B200 (A4): the reference GPU

DeepSeek's reference inference runs on B200 (SM100), and so does its
open-source engine, DeepInfer. We need it for reference captures and device
measurements, in short sessions.

| Ask | Regions | Why |
| --- | --- | --- |
| 8 GPUs spot, 8 on-demand | us-central1, us-east1, europe-west4 | One `a4-highgpu-8g` session at a time |
| Dynamic Workload Scheduler calendar-mode access, 1–2 days, 8 GPUs | Any A4 region | Booked reference sessions, instead of flex-start requests that expire |

### 3. Hopper (A3), lower priority

| Ask | Regions | Why |
| --- | --- | --- |
| 8 × H100 or H200 spot | us-central1 or europe-west4 | Comparing our kernels on Hopper, and a fallback when G4 is stocked out |

### 4. Large decode groups, exploratory

DeepSeek serves decode on 64–128 GPU groups (see
[DeepInfer notes](../research/deepseek/2026-10-21-deepinfer-engine-notes.md)).
Ask the account team whether a short `a4x` (GB200 NVL72) or multi-node A4
trial is possible for research. We don't need it this month.

### 5. Supporting quota

| Ask | Regions | Why |
| --- | --- | --- |
| `N2D_CPUS` 192 | us-central1, us-east1 | Two `n2d-highmem-96` machines for CPU reference runs |
| SSD persistent disk or Hyperdisk 20 TB, with matching provisioned throughput | Each G4 region above | G4 machines have no local SSD. Each machine copies 467 GB of weights onto a fast boot disk |
| `GPUS_ALL_REGIONS` (global) at least the sum of the above | Global | The global GPU cap overrides regional grants |

### 6. Commercial

- Startup credits, or a committed-use discount on G4.
- Spot price and capacity outlook for G4 and A4 by region.
- Whether G4 capacity is pooled per zone, so we know which zones to prefer.

## Running outside the US

Nothing stops us from running elsewhere. G4 is offered in Europe, Asia,
the Middle East and South America, and A4 in europe-west4, europe-north1,
europe-west1, asia-southeast1, asia-northeast1 and asia-south2. The cost
is the weights:

- The weights bucket is regional in us-central1.
- Each launch on another continent pulls 467 GB across regions, which
  costs about $25–40 in egress and takes longer.
- For repeated work in one region, copy the weights once to a bucket in
  that region. That costs about the same egress once, plus storage.

## Email to the Google account team (draft)

> Subject: GPU capacity for OpenAgents (RTX PRO 6000 and B200)
>
> Hi [name],
>
> We're serving DeepSeek-V4.1 natively across RTX PRO 6000 machines on
> GCP (project `openagentsgemini`), split across machines as a network of
> compute providers. This week we ran it on `g4-standard-48`, `-96` and
> `-192` machines, with B200 (`a4-highgpu-8g`) for reference runs.
>
> Our blocker is capacity. G4 was stocked out across every US zone during
> one session, and A4 flex-start requests expired in four zones. Could you
> help with:
>
> 1. RTX PRO 6000 quota of 8 on-demand and 16 spot GPUs in us-central1,
>    us-west3, us-east4, europe-west4 and asia-southeast1, and 32 spot in
>    one region.
> 2. A small G4 reservation: one `g4-standard-192` for 30 days in
>    us-central1.
> 3. B200: 8 spot and 8 on-demand in us-central1, us-east1 and
>    europe-west4, plus Dynamic Workload Scheduler calendar-mode access for
>    1–2 day bookings.
> 4. Guidance on which zones have the most G4 and A4 capacity, and any
>    startup credits or committed-use options for G4.
>
> Thanks,
> Chris

## Filing quota requests

In the console, go to IAM & Admin > Quotas & System Limits, filter on the
quota name (for example "GPUs per GPU family" with `gpu_family`
`NVIDIA_RTX_PRO_6000`), select the region, and choose Edit. Include the
justification above. Then file the preemptible counterpart and check the
global GPU cap.
