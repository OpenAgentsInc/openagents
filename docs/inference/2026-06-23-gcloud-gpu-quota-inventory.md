# Google Cloud GPU quota inventory for OpenAgents inference

Date: 2026-06-23

Update 2026-06-24: DeepSeek V4 Flash is now wired as an operator-selected
Fireworks backing lane for the single public Khala model (`openagents/khala`) in
OpenAgents issue #6198 / commit `84dbe64c93`. This does not prove Google
self-hosting for DeepSeek V4 Flash: the provider notes still point at stock
vLLM 0.20+ with DeepGEMM on 8x H100/H200/B200-class high-memory hardware. Treat
the Google G4/L4 inventory here as useful for smaller/owned probes, while the
immediate DeepSeek V4 Flash product path is the provider-backed Khala lane
documented in
[`2026-06-24-khala-deepseek-v4-flash-provider-backing.md`](./2026-06-24-khala-deepseek-v4-flash-provider-backing.md).

Update 2026-06-24: Hydralisk GPT-OSS 20B is now running on a fresh
`g2-standard-8` L4 VM, `hydralisk-gptoss20b-l4-20260624000550`, in
`us-central1-a`. Current `us-central1` on-demand L4 usage is 3 of 16:
the two Psion `gswarm508-clean2-20260325044551-*` L4 hosts plus the new
Hydralisk inference host. Public-safe Hydralisk evidence refs are recorded in
[`2026-06-23-hydralisk-python-nvidia-inference-stack.md`](./2026-06-23-hydralisk-python-nvidia-inference-stack.md).
The OpenAgents production Worker version
`b5c74b67-32a9-4865-a28e-83a878d0b81b`, from commit `009924bac5`, served
GPT-OSS 20B through the Hydralisk L4 host as `openai/gpt-oss-20b`; the
dereferenceable public receipt is
`receipt.inference.charge.chatcmpl_8434ec68f53249658d9f0d1f6bba1cba`. A live
catalog check showed `openai/gpt-oss-20b` present and
`openagents/khala-oss-20b` absent.
Visible-content follow-up receipts are
`receipt.inference.charge.chatcmpl_550afe2c0e894dec8c3624b664331353`
for non-streaming and
`receipt.inference.charge.chatcmpl_dcd97345b3f14699b672544138597c3d`
for streaming.

Update later on 2026-06-23: a follow-up `gcloud compute instances list
--filter='guestAccelerators:*'` refresh for the GLM-5.2 feasibility pass no
longer showed `oa-confidential-g4-probe-20260623-1` or any
`oa-confidential-g4-probe-*` instance. Treat the G4 entry below as proof that
the Spot G4 RTX PRO 6000 shape was allocatable during the original audit, not as
current live capacity. Current live GPU instances are the running L4 contributor
and coordinator plus the live Hydralisk L4 inference host.

Scope: local `gcloud` audit of project `openagentsgemini` for GPU capacity
usable by the OpenAgents / Khala inference work. This is an access and quota
inventory, not a capacity reservation. A quota number means Google will allow us
to ask for that many GPUs in the quota dimension; actual VM creation can still
fail because of zonal scarcity, preview entitlement, image/driver constraints,
reservation rules, or org policy.

## Bottom line

We have enough Google Cloud GPU access to run real inference probes now.

The most useful current lane is `us-central1`:

| Lane                           |                                                                                     Current quota signal |                       Current usage | Practical inference read                                                                                                                                                                    |
| ------------------------------ | -------------------------------------------------------------------------------------------------------: | ----------------------------------: | ------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------------- |
| L4 / `g2-standard-*`           |                                                                                    16 on-demand, 16 Spot |                       3 running L4s | Cheap always-on or small/medium open-weight inference lane; about 13 on-demand L4 quota free in `us-central1` after the Hydralisk GPT-OSS 20B host.                                         |
| RTX PRO 6000 / `g4-standard-*` |                                                      16 Spot RTX PRO 6000, 16 on-demand VWS RTX PRO 6000 | 0 current live in the later refresh | Best immediate Blackwell-ish single-GPU lane if re-created. We have proven VM creation for a Spot `g4-standard-48` with 1 RTX PRO 6000 in `us-central1-b`, but the probe is no longer live. |
| H100 / `a3-highgpu-*`          |                                                                                 64 Spot/preemptible H100 |                  0 observed running | Strong single/multi-GPU lane for vLLM/SGLang probes and the Prompt Encryption SDK path. Quota exists; allocation still needs a create test.                                                 |
| H200                           |                                                                                 64 Spot/preemptible H200 |                  0 observed running | Promising high-memory lane; visible in `us-central1-b`, but no create test yet.                                                                                                             |
| B200                           |                                                                                 64 Spot/preemptible B200 |                  0 observed running | Next-gen lane visible in `us-central1-b`; no create test yet.                                                                                                                               |
| A100 40GB                      |                                                                        16 on-demand, 64 Spot/preemptible |                  0 observed running | Useful fallback for engines that do not need H100/RTX PRO 6000.                                                                                                                             |
| A100 80GB                      | older Compute quota shows 0 in `us-central1`; Cloud Quotas did not show a usable nonblank regional value |                  0 observed running | Do not plan on A100 80GB here unless quota is raised or a create test proves otherwise.                                                                                                     |

GPU instances observed across the audit and 2026-06-24 update:

| Instance                                  | Zone            | Shape            | GPU                       | Scheduling                            | Confidential                    | Status                             |
| ----------------------------------------- | --------------- | ---------------- | ------------------------- | ------------------------------------- | ------------------------------- | ---------------------------------- |
| `oa-confidential-g4-probe-20260623-1`     | `us-central1-b` | `g4-standard-48` | 1 x `nvidia-rtx-pro-6000` | Spot/preemptible, stop on termination | `confidentialInstanceType: SEV` | no longer visible in later refresh |
| `gswarm508-clean2-20260325044551-contrib` | `us-central1-b` | `g2-standard-8`  | 1 x `nvidia-l4`           | standard on-demand                    | none                            | `RUNNING`                          |
| `gswarm508-clean2-20260325044551-coord`   | `us-central1-a` | `g2-standard-8`  | 1 x `nvidia-l4`           | not inspected in detail               | none                            | `RUNNING`                          |
| `hydralisk-gptoss20b-l4-20260624000550`   | `us-central1-a` | `g2-standard-8`  | 1 x `nvidia-l4`           | standard on-demand                    | none                            | `RUNNING`                          |

The G4 probe is still the biggest update versus the earlier confidential AI
access note: access to G4 was not just visible; one Spot G4 VM with an RTX PRO
6000 had already been allocated. A later refresh no longer shows it as live.

## Active gcloud context

`gcloud config list --format=json`:

- project: `openagentsgemini`
- account: `chris@openagents.com`
- default region: `us-central1`
- default zone: `us-central1-a`

Relevant enabled APIs:

- `aiplatform.googleapis.com`
- `cloudquotas.googleapis.com`
- `compute.googleapis.com`
- `container.googleapis.com`
- `serviceusage.googleapis.com`

`confidentialcomputing.googleapis.com` was not listed as enabled by the simple
enabled-service check. Compute Engine Confidential VM creation can still work
through Compute Engine, as the original G4 probe showed, but Confidential Space
and attestation workflows should verify API needs before productizing.

## `us-central1` quota details

Older Compute regional quota output for `us-central1`:

| Metric                              | Limit | Usage | Free by quota math |
| ----------------------------------- | ----: | ----: | -----------------: |
| `NVIDIA_L4_GPUS`                    |    16 |     3 |                 13 |
| `PREEMPTIBLE_NVIDIA_L4_GPUS`        |    16 |     0 |                 16 |
| `NVIDIA_A100_GPUS`                  |    16 |     0 |                 16 |
| `PREEMPTIBLE_NVIDIA_A100_GPUS`      |    64 |     0 |                 64 |
| `NVIDIA_T4_GPUS`                    |     8 |     0 |                  8 |
| `PREEMPTIBLE_NVIDIA_T4_GPUS`        |     8 |     0 |                  8 |
| `NVIDIA_V100_GPUS`                  |     8 |     0 |                  8 |
| `PREEMPTIBLE_NVIDIA_V100_GPUS`      |    16 |     0 |                 16 |
| `NVIDIA_P4_GPUS`                    |     1 |     0 |                  1 |
| `PREEMPTIBLE_NVIDIA_P4_GPUS`        |    16 |     0 |                 16 |
| `NVIDIA_P100_GPUS`                  |     1 |     0 |                  1 |
| `PREEMPTIBLE_NVIDIA_P100_GPUS`      |    16 |     0 |                 16 |
| `NVIDIA_K80_GPUS`                   |    16 |     0 |                 16 |
| `PREEMPTIBLE_NVIDIA_K80_GPUS`       |     1 |     0 |                  1 |
| `NVIDIA_A100_80GB_GPUS`             |     0 |     0 |                  0 |
| `PREEMPTIBLE_NVIDIA_A100_80GB_GPUS` |     0 |     0 |                  0 |

Newer Cloud Quotas API values whose `applicableLocations` include
`us-central1`:

| Quota ID                                                      | Value | Read                                                                                          |
| ------------------------------------------------------------- | ----: | --------------------------------------------------------------------------------------------- |
| `PREEMPTIBLE-NVIDIA-RTX-PRO-6000-GPUS-per-project-region`     |    16 | Spot quota for non-VWS G4 RTX PRO 6000. This is the quota lane used by the original G4 probe. |
| `PREEMPTIBLE-NVIDIA-RTX-PRO-6000-VWS-GPUS-per-project-region` |    16 | Spot quota for RTX PRO 6000 VWS.                                                              |
| `NVIDIA-RTX-PRO-6000-VWS-GPUS-per-project-region`             |    16 | On-demand RTX PRO 6000 VWS quota. I did not see normal non-VWS on-demand RTX PRO 6000 quota.  |
| `PREEMPTIBLE-NVIDIA-H100-GPUS-per-project-region`             |    64 | Spot H100 quota.                                                                              |
| `PREEMPTIBLE-NVIDIA-H100-MEGA-GPUS-per-project-region`        |    64 | Spot H100 Mega quota.                                                                         |
| `PREEMPTIBLE-NVIDIA-H200-GPUS-per-project-region`             |    64 | Spot H200 quota.                                                                              |
| `PREEMPTIBLE-NVIDIA-B200-GPUS-per-project-region`             |    64 | Spot B200 quota.                                                                              |
| `NVIDIA-L4-GPUS-per-project-region`                           |    16 | On-demand L4 quota.                                                                           |
| `PREEMPTIBLE-NVIDIA-L4-GPUS-per-project-region`               |    16 | Spot L4 quota.                                                                                |
| `NVIDIA-A100-GPUS-per-project-region`                         |    16 | On-demand A100 40GB quota.                                                                    |
| `PREEMPTIBLE-NVIDIA-A100-GPUS-per-project-region`             |    64 | Spot A100 40GB quota.                                                                         |

Cloud Quotas also returned committed GPU quota IDs with value `-1`. I am not
treating those as usable ad hoc inference capacity. They are commitment quota
dimensions, not evidence that we have a current reservation or immediate
capacity.

## GPU accelerator types visible to the project

Visible accelerator types are not the same as quota, but they tell us which GPU
families Google exposes to this project by zone.

| GPU family                             | Visible regions | `us-central1` zones visible                                        | Notes                                                                                                           |
| -------------------------------------- | --------------: | ------------------------------------------------------------------ | --------------------------------------------------------------------------------------------------------------- |
| NVIDIA RTX PRO 6000                    |      20 regions | `us-central1-b`, `us-central1-f`                                   | G4 lane. We proved Spot G4 allocation in `us-central1-b`, but the probe is no longer live in the later refresh. |
| NVIDIA RTX PRO 6000 VWS                |      20 regions | `us-central1-b`, `us-central1-f`                                   | Workstation quota exists; not the primary inference lane.                                                       |
| NVIDIA H100 80GB                       |      19 regions | `us-central1-a`, `us-central1-b`, `us-central1-c`                  | Strong H100 lane; quota signal is Spot/preemptible 64 per region.                                               |
| NVIDIA H100 80GB Mega                  |      19 regions | `us-central1-a`, `us-central1-b`, `us-central1-c`                  | Same visible zones as H100.                                                                                     |
| NVIDIA H200 141GB                      |       9 regions | `us-central1-b`                                                    | High-memory lane; quota signal is Spot/preemptible 64 per region.                                               |
| NVIDIA B200 180GB                      |      10 regions | `us-central1-b`                                                    | Next-gen lane; quota signal is Spot/preemptible 64 per region.                                                  |
| NVIDIA GB200 192GB                     |       4 regions | `us-central1-a`, `us-central1-b`                                   | Visible accelerator type, but I did not find an explicit usable GB200 quota row in this audit.                  |
| NVIDIA L4                              |      18 regions | `us-central1-a`, `us-central1-b`, `us-central1-c`                  | Cheap current inference lane. Three on-demand L4s are running, including the Hydralisk GPT-OSS 20B host.        |
| NVIDIA A100 80GB                       |       5 regions | `us-central1-a`, `us-central1-c`                                   | Visible, but quota is not currently favorable in `us-central1`.                                                 |
| NVIDIA A100 40GB (`nvidia-tesla-a100`) |      10 regions | `us-central1-a`, `us-central1-b`, `us-central1-c`, `us-central1-f` | Usable fallback; 16 on-demand / 64 Spot in `us-central1`.                                                       |
| NVIDIA V100                            |       5 regions | `us-central1-a`, `us-central1-b`, `us-central1-c`, `us-central1-f` | Older fallback.                                                                                                 |
| NVIDIA T4                              |      23 regions | `us-central1-a`, `us-central1-b`, `us-central1-c`, `us-central1-f` | Older cheap fallback.                                                                                           |
| NVIDIA P4                              |       7 regions | `us-central1-a`, `us-central1-c`                                   | Legacy fallback.                                                                                                |
| NVIDIA P100                            |       7 regions | `us-central1-c`, `us-central1-f`                                   | Legacy fallback.                                                                                                |

## Broad quota pattern across regions

The older Compute regional quota API returned 946 non-committed NVIDIA quota
rows across regions. Aggregated high-signal rows:

| Metric                         | Regions with limit >= 16 or usage > 0 | Total nominal limit | Total observed usage |
| ------------------------------ | ------------------------------------: | ------------------: | -------------------: |
| `PREEMPTIBLE_NVIDIA_A100_GPUS` |                                    43 |                2752 |                    0 |
| `NVIDIA_A100_GPUS`             |                                    43 |                 688 |                    0 |
| `PREEMPTIBLE_NVIDIA_L4_GPUS`   |                                    43 |                 688 |                    0 |
| `NVIDIA_L4_GPUS`               |                                    43 |                 688 |                    3 |
| `NVIDIA_K80_GPUS`              |                                    24 |                 384 |                    0 |
| `PREEMPTIBLE_NVIDIA_P100_GPUS` |                                    24 |                 384 |                    0 |
| `PREEMPTIBLE_NVIDIA_P4_GPUS`   |                                    24 |                 384 |                    0 |
| `PREEMPTIBLE_NVIDIA_V100_GPUS` |                                    24 |                 384 |                    0 |

Do not read the cross-region totals as one giant fungible pool. GPU quota is
regional, and many modern families require specific zones. For inference
deployment planning, treat the useful pool as `region + zone + machine family +
scheduling model`, then prove it with a create/delete or actual run.

## Practical recommendations for Khala / inference

1. Keep `us-central1` as the first owner-operated GPU lane. It already has live
   L4 capacity, previously proved G4 allocation, the default `gcloud` config
   points there, and the inference docs already frame it as the initial cloud
   supply lane.
2. Use L4 for cheap always-on serving and smaller open-weight models. There is
   on-demand quota available, and one L4 contributor is already running.
3. Re-create a G4 RTX PRO 6000 probe for the first Blackwell/Confidential G4
   inference smoke if that lane is intentionally needed. The original probe
   proved allocation once, but it is no longer visible in the current live
   instance list.
4. Treat H100/H200/B200 as Spot-first until a successful create test proves the
   exact shape. The quota is there, but quota is not capacity.
5. Do not promise A100 80GB availability from this project today. It is visible
   as an accelerator type in some zones, but the quota signal is not usable in
   this audit.
6. Record every real benchmark as decision-grade only when it includes the exact
   GPU family, zone, scheduling model, engine (`vLLM`, `SGLang`, etc.), driver
   stack, quantization disclosure, wall-clock, tokens, cost basis, and verifier
   result. This matches the Khala benchmark and quantization docs: faster tokens
   are not a product win unless accepted-outcome cost improves.

## Commands used

No secrets were printed. The Cloud Quotas command used an access token only in
an HTTP header.

```sh
gcloud config list --format=json
gcloud services list --enabled --format='value(config.name)'
gcloud compute accelerator-types list --format='table(zone,name,description)'
gcloud compute instances list --filter='guestAccelerators:*' \
  --format='table(name,zone,machineType,guestAccelerators[].acceleratorType,guestAccelerators[].acceleratorCount,status)'
gcloud compute regions list --format='value(name)'
gcloud compute regions describe us-central1 --format=json
curl -H "Authorization: Bearer $(gcloud auth print-access-token)" \
  "https://cloudquotas.googleapis.com/v1/projects/openagentsgemini/locations/global/services/compute.googleapis.com/quotaInfos?pageSize=1000"
```

`gcloud compute machine-types list --filter='name~^(a2|a3|a4|g2|g4)-'` crashed
locally with `TypeError: 'NoneType' object is not iterable`, so this inventory
uses accelerator visibility, quota APIs, and live instance descriptions instead
of relying on that machine-type listing.
