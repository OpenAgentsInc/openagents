# Google Confidential AI gcloud Access Audit

**STATUS (2026-07-08): RETIRED FOR NOW — not current direction.**
OpenAgents is focused on Khala Code and business-facing work
(`docs/fable/MASTER_ROADMAP.md` rev 6). This program is retired
until an explicit owner decision revives it (earliest
reconsideration: after cashflow-positive). Preserved for history;
do not route new work, issues, or copy from this document.


Date: 2026-06-23

Source prompt: evaluate the Google Cloud announcement "Verifiable, private AI:
Google Cloud expands Confidential Computing frontiers" and determine whether
OpenAgents appears to have local `gcloud` access to try the new Confidential
Computing surfaces.

## Bottom line

OpenAgents can provision and boot the newly announced Confidential G4 path now.
On 2026-06-23, a live `g4-standard-48` Spot VM in `us-central1-b` was created
successfully in project `openagentsgemini` with AMD SEV and one NVIDIA RTX PRO
6000 Blackwell Server Edition GPU. Guest-side checks confirmed AMD SEV memory
encryption and, after installing the NVIDIA 580 open kernel module driver and
the Google-documented LKCA/SPDM setup, `nvidia-smi` reported a working
Confidential GPU with DRAM encryption enabled and 97,131 MiB of protected GPU
memory.

The short-term service answer is therefore yes, with caveats that are now
operational rather than speculative: we need a reproducible SEV-compatible boot
image or bootstrap layer with the open 580 driver, LKCA config, persistence
mode, and our inference runtime already baked in. A standard Google Deep
Learning VM image with NVIDIA 580 was rejected as not SEV-compatible, so the
fast path is not "use DLVM as-is"; it is "start from supported Ubuntu/COS and
own the GPU bootstrap."

For Google's Prompt Encryption SDK specifically, the local evidence now points
more strongly at an A3/H100 Confidential Space probe than a plain G4 VM probe:
the checked-out SDK codelab provisions `a3-highgpu-1g`, TDX, Confidential Space,
Spot, an external TCP load balancer, and a vLLM container. The project has
`PREEMPTIBLE-NVIDIA-H100-GPUS` quota in `us-central1`, so the H100 SDK path
looks quota-feasible, but it is materially more expensive than G4 and still
needs an actual create/delete test.

The active local `gcloud` configuration targets project `openagentsgemini`,
region `us-central1`, zone `us-central1-a`, with an active human account that
has project Owner. The project can see ordinary G4 machine types and
`nvidia-rtx-pro-6000` accelerator types in `us-central1-b` and `us-central1-f`.
Google's current docs say Confidential VM on G4 is Preview, limited to
`g4-standard-48` in limited regions and zones. That exact machine type is
visible in both `us-central1-b` and `us-central1-f`.

The old blocker "can we create one?" is closed for the cheapest G4 Spot path.
The remaining blockers for a service launch are packaging, attestation receipt
capture, Prompt Encryption SDK integration, capacity/reservation planning, and
operator policy around retention and logs.

2026-06-24 follow-up: the confidential G4 probe remains a deleted/proven access
artifact, not current live serving capacity. The first live owner-operated
open-weight serving lane instead went up on a non-confidential
`g2-standard-8` L4 host:
`hydralisk-gptoss20b-l4-20260624000550` in `us-central1-a`. The production
OpenAgents Worker version `b5c74b67-32a9-4865-a28e-83a878d0b81b`, from commit
`009924bac5`, served GPT-OSS 20B through Hydralisk as `openai/gpt-oss-20b` and
wrote the public inference receipt
`receipt.inference.charge.chatcmpl_8434ec68f53249658d9f0d1f6bba1cba`. A live
catalog check showed `openai/gpt-oss-20b` present and
`openagents/khala-oss-20b` absent.
Visible-content follow-up receipts:
`receipt.inference.charge.chatcmpl_550afe2c0e894dec8c3624b664331353`
for non-streaming and
`receipt.inference.charge.chatcmpl_dcd97345b3f14699b672544138597c3d`
for streaming.

2026-06-24 slug-policy follow-up: that raw GPT-OSS public model exposure is now
superseded. Within the OpenAgents ecosystem the model slug is `khala`; external
OpenAI-compatible and MPP surfaces use `openagents/khala`. Hydralisk still sends
raw upstream model ids such as `openai/gpt-oss-20b` and
`openai/gpt-oss-120b` to vLLM, but those ids are no longer listed, quoted, or
MPP-payable as public products.

2026-06-24 H100 follow-up: the project also successfully allocated a non-
confidential Spot `a3-highgpu-1g` H100 80GB VM in `us-central1-b`,
`hydralisk-gptoss120b-h100-probe-20260623210841`. vLLM `0.23.0` loaded
`openai/gpt-oss-120b` with MXFP4 weights at `max_model_len=32768`, reporting
64.67 GiB GPU memory for model load before compile/warmup/autotune and about
76 GiB resident after first smoke. The bearer-protected Hydralisk proxy returned
HTTP 200 for a chat completion with visible content `READY_120B`; public-safe
run ref: `hydralisk-run-2969ab61fcf44c3085c5e9f2bc513f11`. This proves the
project can run GPT-OSS 120B on a live H100 host today. It does not change the
confidential-compute conclusion: the H100 serving host is not a Confidential VM,
and confidential H100/G4 serving still needs its own attestation-oriented
bootstrap.

## What Google announced

The June 23, 2026 Google Cloud post announces:

- Confidential G4 VMs and Confidential GKE Nodes in preview, using NVIDIA RTX
  PRO 6000 Blackwell Server Edition GPUs on AMD EPYC Turin CPUs.
- Prompt Encryption SDKs that establish an attested TLS channel from client to
  inference server running inside a TEE.
- Apple Private Cloud Compute collaboration on Google Cloud, using confidential
  compute, NVIDIA Confidential Computing, Titanium, and an open-source host
  stack.
- Intel TDX support for C4 Confidential VMs in preview soon.
- C3D Confidential VM live migration generally available.
- Confidential Space updates: Intel Trust Authority independent verification
  generally available, and H100 GPU support generally available.

References:

- Google Cloud announcement:
  <https://cloud.google.com/blog/products/identity-security/verifiable-trust-in-the-ai-era-whats-new-in-confidential-computing>
- Google Cloud "About Confidential VM":
  <https://docs.cloud.google.com/compute/docs/about-confidential-vm>
- Google Cloud accelerator-optimized machine docs:
  <https://docs.cloud.google.com/compute/docs/accelerator-optimized-machines>
- Prompt Encryption SDK:
  <https://github.com/google/prompt-encryption-sdk>
- Prompt Encryption SDK codelab:
  <https://codelabs.developers.google.com/prompt-encryption-sdk>
- Google Cloud "Create a Confidential VM instance with GPU":
  <https://docs.cloud.google.com/confidential-computing/confidential-vm/docs/create-a-confidential-vm-instance-with-gpu>

Local reference reviewed:

- `/Users/christopherdavid/work/projects/repos/prompt-encryption-sdk`

## Local access findings

Commands were run from `/Users/christopherdavid/work` with no secrets printed.

### gcloud account and project

`gcloud config list --format=json`

- `core.project`: `openagentsgemini`
- `core.account`: active OpenAgents human account
- `compute.region`: `us-central1`
- `compute.zone`: `us-central1-a`

`gcloud auth list --format=json`

- One active human account.
- Two inactive service accounts are locally known:
  `nexus-mainnet@openagentsgemini.iam.gserviceaccount.com` and
  `oa-vertex-inference@openagentsgemini.iam.gserviceaccount.com`.

`gcloud projects get-iam-policy openagentsgemini --flatten='bindings[].members' --filter='bindings.members:<active-account>' --format='table(bindings.role)'`

The active account has:

- `roles/owner`
- `roles/aiplatform.user`
- `roles/consumerprocurement.entitlementManager`
- `roles/serviceusage.serviceUsageConsumer`

### Enabled APIs

`gcloud services list --enabled --format='value(config.name)'`

Relevant enabled APIs observed:

- `aiplatform.googleapis.com`
- `compute.googleapis.com`
- `container.googleapis.com`
- `iamcredentials.googleapis.com`
- `serviceusage.googleapis.com`

`confidentialcomputing.googleapis.com` did not appear in the enabled-service
filter used for this audit. Compute Engine Confidential VM creation may not
require that API directly, but Confidential Space and attestation workflows
should be checked before implementation.

### G4 and RTX PRO 6000 visibility

`gcloud compute machine-types list --filter='name~^g4-standard' --format='table(zone,name,guestCpus,memoryMb)' --limit=80`

Visible in `us-central1`:

- `us-central1-b`: `g4-standard-6`, `g4-standard-12`, `g4-standard-24`,
  `g4-standard-48`, `g4-standard-96`, `g4-standard-192`, `g4-standard-384`
- `us-central1-f`: `g4-standard-48`, `g4-standard-96`,
  `g4-standard-192`, `g4-standard-384`

`gcloud compute accelerator-types list --filter='name~nvidia-rtx-pro-6000' --format='table(zone,name)' --limit=80`

Visible in `us-central1`:

- `us-central1-b`: `nvidia-rtx-pro-6000`,
  `nvidia-rtx-pro-6000-vws`
- `us-central1-f`: `nvidia-rtx-pro-6000`,
  `nvidia-rtx-pro-6000-vws`

`gcloud compute zones describe us-central1-b --format='json(availableCpuPlatforms,status)'`

- Zone status: `UP`
- Includes `AMD Turin`

`gcloud compute zones describe us-central1-f --format='json(availableCpuPlatforms,status)'`

- Zone status: `UP`
- Includes `AMD Turin`

### Quota signal

`gcloud compute regions describe us-central1 --format='json(quotas)'`

The region has substantial general CPU quota and existing standard GPU quota
entries for older accelerator families such as L4 and A100. The current output
did not expose a named RTX PRO 6000, G4, or similarly obvious Blackwell quota
metric. That means visibility of machine and accelerator types should not be
treated as proof that a `g4-standard-48` Confidential VM can be allocated.

Follow-up checks on 2026-06-23 used the Cloud Quotas API:

```sh
curl -H "Authorization: Bearer $(gcloud auth print-access-token)" \
  "https://cloudquotas.googleapis.com/v1/projects/openagentsgemini/locations/global/services/compute.googleapis.com/quotaInfos?pageSize=1000"
```

Relevant `us-central1` quota results:

| Quota                                                         | Limit shown | Meaning for probe                                                                                                    |
| ------------------------------------------------------------- | ----------: | -------------------------------------------------------------------------------------------------------------------- |
| `PREEMPTIBLE-NVIDIA-RTX-PRO-6000-GPUS-per-project-region`     |          16 | Enough quota for a one-GPU `g4-standard-48` Spot probe, subject to preview entitlement and capacity.                 |
| `PREEMPTIBLE-NVIDIA-RTX-PRO-6000-VWS-GPUS-per-project-region` |          16 | Virtual Workstation variant quota exists, but this is not the primary inference probe shape.                         |
| `NVIDIA-RTX-PRO-6000-VWS-GPUS-per-project-region`             |          16 | On-demand VWS quota exists, but normal non-VWS on-demand RTX PRO 6000 quota did not appear in the quota-info result. |
| `PREEMPTIBLE-NVIDIA-H100-GPUS-per-project-region`             |          64 | Enough quota for a one-GPU A3 High Spot/Confidential Space-style H100 probe, subject to capacity.                    |
| `COMMITTED-NVIDIA-H100-GPUS-per-project-region`               |        `-1` | Commitment quota is effectively not the constraint for an ad hoc probe.                                              |

Quota preference requests:

```sh
gcloud beta quotas preferences list --project=openagentsgemini --format=json
```

The command returned no parseable quota preference records for the target GPU
families, so the project does not appear to have an in-flight quota preference
request for these probes.

### Pricing signal

Pricing was checked against the Google Cloud Billing Catalog API and the public
Confidential VM pricing docs on 2026-06-23. Prices are USD list prices before
taxes, network egress, boot disk, Hyperdisk, logs, images, reservations, and
other ancillary charges. Spot prices can change.

Billing catalog command:

```sh
curl -H "Authorization: Bearer $(gcloud auth print-access-token)" \
  "https://cloudbilling.googleapis.com/v1/services/6F81-5844-456A/skus?pageSize=5000&currencyCode=USD"
```

`g4-standard-48` in Iowa / `us-central1`:

| Component                        |                On-demand |         Spot/preemptible |      DWS defined duration |
| -------------------------------- | -----------------------: | -----------------------: | ------------------------: |
| G4 vCPU                          | `$0.04891` per vCPU-hour | `$0.01027` per vCPU-hour | `$0.024455` per vCPU-hour |
| G4 RAM                           |  `$0.00587` per GiB-hour | `$0.001235` per GiB-hour |  `$0.002935` per GiB-hour |
| RTX 6000 96GB GPU                |  `$1.09565` per GPU-hour |  `$0.20810` per GPU-hour |   `$0.54783` per GPU-hour |
| Estimated `g4-standard-48` total |            `$4.49993/hr` |            `$0.92336/hr` |             `$2.24997/hr` |

`g4-standard-48` total math: 48 vCPU + 180 GiB RAM + 1 RTX 6000 96GB GPU.
Google's public Confidential VM pricing page says additional Confidential
Computing charges and the NVIDIA Confidential Computing license fee for G4 are
currently not charged during Preview. After GA, the listed adders are `$0.45/hr`
on-demand, `$0.09/hr` Spot, `$0.22/hr` DWS Flex-Start, plus a `$0.08/hr` NVIDIA
Confidential Computing license fee per RTX PRO 6000 GPU.

Estimated G4 Confidential VM probe cost while Preview surcharge/license are
waived:

| Runtime    | Spot estimate | On-demand estimate |
| ---------- | ------------: | -----------------: |
| 10 minutes |    `$0.15389` |         `$0.74999` |
| 1 hour     |    `$0.92336` |         `$4.49993` |
| 24 hours   |   `$22.16064` |       `$107.99832` |

`a3-highgpu-1g` in Americas / `us-central1`:

| Component                       |                    On-demand |         Spot/preemptible |      DWS defined duration |
| ------------------------------- | ---------------------------: | -----------------------: | ------------------------: |
| A3 vCPU                         | `$0.025498003` per vCPU-hour | `$0.01263` per vCPU-hour | `$0.010934` per vCPU-hour |
| A3 RAM                          |   `$0.00222034` per GiB-hour |  `$0.00110` per GiB-hour |  `$0.000952` per GiB-hour |
| H100 80GB GPU                   |  `$9.796550569` per GPU-hour |  `$4.85340` per GPU-hour |  `$4.200761` per GPU-hour |
| Estimated `a3-highgpu-1g` total |               `$10.97906/hr` |            `$5.43918/hr` |             `$4.70781/hr` |

`a3-highgpu-1g` total math: 26 vCPU + 234 GiB RAM + 1 H100 80GB GPU. Google's
Confidential VM GPU creation docs say A3 High Confidential GPU VMs support Spot
and Flex-start, not standard on-demand. Google's Confidential VM pricing page
lists additional Confidential Computing adders for A3 as `N/A` on-demand,
`$0.4391592/hr` Spot, and `$0.512173102/hr` DWS Flex-Start.

Estimated A3 High Confidential VM probe cost including the Confidential
Computing adders:

| Runtime    | Spot estimate | DWS defined-duration estimate |
| ---------- | ------------: | ----------------------------: |
| 10 minutes |    `$0.97972` |                    `$0.87000` |
| 1 hour     |    `$5.87834` |                    `$5.21999` |
| 24 hours   |  `$141.08014` |                  `$125.27961` |

Practical cost read:

- G4 Spot is the cheapest meaningful confidential-GPU probe path, and it did
  work in `openagentsgemini` on 2026-06-23.
- A3/H100 is materially more expensive and should be reserved for a
  Hopper-specific Confidential Space/H100 test or a workload that actually
  needs H100.
- A failed create attempt should not materially bill beyond transient API and
  logging noise, but any successful probe should be deleted immediately after
  describe/attestation capture.

### Live G4 Spot probe on 2026-06-23

Result: success for provision, boot, AMD SEV, GPU enumeration, NVIDIA driver,
and Confidential GPU memory/encryption signal.

Instance:

- Name: `oa-confidential-g4-probe-20260623-1`
- Project: `openagentsgemini`
- Zone: `us-central1-b`
- Machine: `g4-standard-48`
- Provisioning: Spot / preemptible
- Confidential type: `SEV`
- GPU: one `nvidia-rtx-pro-6000`
- Image that worked: `ubuntu-os-cloud/ubuntu-2404-lts-amd64`
- Lifecycle: created, tested, and deleted in the same work session

Create command that succeeded:

```sh
gcloud compute instances create oa-confidential-g4-probe-20260623-1 \
  --project=openagentsgemini \
  --zone=us-central1-b \
  --machine-type=g4-standard-48 \
  --confidential-compute-type=SEV \
  --maintenance-policy=TERMINATE \
  --provisioning-model=SPOT \
  --instance-termination-action=STOP \
  --boot-disk-type=hyperdisk-balanced \
  --boot-disk-size=50GB \
  --image-family=ubuntu-2404-lts-amd64 \
  --image-project=ubuntu-os-cloud \
  --labels=purpose=confidential-g4-probe,owner=openagents,ttl=manual-delete
```

Compute Engine describe output confirmed:

```text
confidentialInstanceType: SEV
machineType: g4-standard-48
guestAccelerators: nvidia-rtx-pro-6000 x 1
provisioningModel: SPOT
status: RUNNING
```

Guest evidence before GPU driver:

```text
Vendor ID: AuthenticAMD
Model name: AMD EPYC 9B45
Memory Encryption Features active: AMD SEV
SEV: Status: SEV
PCI-DMA: Using software bounce buffering for IO (SWIOTLB)
05:00.0 3D controller: NVIDIA Corporation Device 2bb5 (rev a1)
```

The stock Ubuntu image did not include `nvidia-smi`. Installing the default
non-open `nvidia-driver-580-server` path caused:

```text
NVRM: GPU 0000:05:00.0: RmInitAdapter failed! (0x22:0x38:897)
No devices were found
```

The working path was the Google-documented Confidential GPU setup:

```sh
sudo apt-get update --yes
sudo apt-get install -y \
  linux-headers-$(uname -r) \
  build-essential \
  libxml2 \
  libncurses5-dev \
  pkg-config \
  libvulkan1 \
  gcc-12 \
  linux-modules-nvidia-580-server-open-gcp-6.17 \
  nvidia-headless-580-server-open \
  nvidia-utils-580-server

echo "install nvidia /sbin/modprobe ecdsa_generic; /sbin/modprobe ecdh; /sbin/modprobe --ignore-install nvidia" \
  | sudo tee /etc/modprobe.d/nvidia-lkca.conf

sudo test -f /usr/lib/systemd/system/nvidia-persistenced.service \
  && sudo sed -i "s/no-persistence-mode/uvm-persistence-mode/g" \
    /usr/lib/systemd/system/nvidia-persistenced.service

sudo systemctl daemon-reload
sudo update-initramfs -u
sudo reboot
```

After purging the non-open module packages and rebooting, `modinfo nvidia`
showed:

```text
filename: /lib/modules/6.17.0-1018-gcp/kernel/nvidia-580srv-open/nvidia.ko
version: 580.159.03
license: Dual MIT/GPL
```

Final `nvidia-smi` evidence:

```text
GPU 0: NVIDIA RTX PRO 6000 Blackwell Server Edition
Driver Version: 580.159.03
CUDA Version: 13.0
Attached GPUs: 1
Product Architecture: Blackwell
FB Memory Usage Total: 97887 MiB
Conf Compute Protected Memory Usage Total: 97131 MiB
DRAM Encryption Mode Current: Enabled
Persistence Mode: Enabled
```

The attempted shortcut with Google Deep Learning VM image family
`common-cu129-ubuntu-2404-nvidia-580` failed at create time:

```text
Confidential Instance Config can only be set when using an SEV compatible image.
```

Cost model for this live probe: the instance ran for roughly half an hour. At
the G4 Spot estimate of `$0.92336/hr`, compute/GPU cost should be roughly
`$0.46` before small disk/network/logging charges. The important service cost
number remains about `$0.92336/hr` per running one-GPU G4 Spot worker while the
Preview confidential surcharge/license are waived.

## Feasibility assessment

### Confidential G4 VM

Status: proven provisionable and GPU-usable with the right driver path.

Why:

- The required API baseline for Compute Engine is enabled.
- The active account has Owner on the project.
- `g4-standard-48` is visible in `us-central1-b` and `us-central1-f`.
- `nvidia-rtx-pro-6000` is visible in the same two zones.
- Cloud Quotas shows 16 `PREEMPTIBLE-NVIDIA-RTX-PRO-6000-GPUS` in
  `us-central1`.
- Both zones are UP and include the AMD Turin CPU platform.
- Google docs state that Confidential VM with G4 uses AMD SEV and RTX PRO
  6000 in Preview, and that G4 Confidential VM is limited to
  `g4-standard-48` in limited regions/zones.
- A live `g4-standard-48` Spot VM was created in `us-central1-b`, booted with
  `confidentialInstanceType=SEV`, and exposed the RTX PRO 6000 to the guest.
- With NVIDIA 580 open kernel modules plus LKCA/SPDM configuration, the GPU
  initialized and reported Confidential Compute protected memory and DRAM
  encryption enabled.

Remaining unknowns:

- Whether non-Spot/on-demand quota exists for normal non-VWS RTX PRO 6000
  probes. Spot quota exists.
- Whether current zonal capacity is reliable enough without reservations.
- Whether we should use Ubuntu or COS for the first service image.
- Whether Google will keep the exact G4 Preview driver/LKCA path stable through
  GA.
- How long our final inference bootstrap takes after we bake the driver path,
  model/runtime, attestation service, and log redaction into an image.

Recommended repeatable service bootstrap:

```sh
gcloud compute instances create INSTANCE_NAME \
  --project=openagentsgemini \
  --zone=us-central1-b \
  --machine-type=g4-standard-48 \
  --confidential-compute-type=SEV \
  --maintenance-policy=TERMINATE \
  --provisioning-model=SPOT \
  --instance-termination-action=STOP \
  --boot-disk-type=hyperdisk-balanced \
  --boot-disk-size=100GB \
  --image-family=ubuntu-2404-lts-amd64 \
  --image-project=ubuntu-os-cloud \
  --labels=purpose=confidential-g4-worker,owner=openagents
```

Then install the open 580 driver, LKCA config, persistence mode, inference
runtime, and Prompt Encryption SDK server into a reusable image. Do not rely on
a standard Deep Learning VM image for this path; the tested DLVM family was not
accepted as SEV-compatible.

### Confidential GKE Nodes on G4

Status: likely later than VM probe.

Why:

- `container.googleapis.com` is enabled.
- G4 machine types are visible.
- Google announced Confidential GKE Nodes on G4 in preview.

Recommendation:

Start with the VM probe. GKE adds cluster version, node pool, GPU driver,
workload identity, and scheduling complexity. A direct VM create/delete probe
will answer the entitlement/quota question faster and with less blast radius.

### Prompt Encryption SDK

Status: available to evaluate now, but production use depends on a TEE server
deployment and a successful attested TLS request.

Why:

- The SDK is now present locally at
  `/Users/christopherdavid/work/projects/repos/prompt-encryption-sdk`.
- The SDK's architecture uses post-handshake Attested TLS. A normal TLS 1.3
  channel is established first, then the client calls `/_attest-connection`,
  derives TLS Exported Keying Material, and verifies Google Cloud Attestation
  evidence against the live TLS session.
- The client-side policy is a protobuf `AttestationPolicy`; the validator can
  pin hardware model, container image digest, GCE project, zone, instance ID,
  and instance name. It maps TDX to `GCP_INTEL_TDX`, SEV to `GCP_AMD_SEV`, and
  SEV-SNP to `GCP_AMD_SEV_SNP`.
- The server side rotates an ephemeral ECDSA P-256 instance key, fetches an
  attestation token from either the Confidential Space TEE server Unix socket
  or `gotpm`, embeds the public-key fingerprint as a nonce, and signs the
  session payload over `SHA256(EKM)` and `SHA256(attestation_token)`.
- Client connections revalidate by default after 55 minutes, which matters for
  long-running inference sessions and connection pooling.
- The checked-out codelab's concrete happy path is not a generic G4 VM. It
  provisions `a3-highgpu-1g` in `us-central1-a`, `--confidential-compute-type=TDX`,
  `--image-family=confidential-space-debug`, `--provisioning-model=SPOT`, and
  metadata including `tee-install-gpu-driver=true` and
  `tee-experiment-enable-confidential-gpu-support=true`.
- The codelab enables `compute.googleapis.com`,
  `confidentialcomputing.googleapis.com`, `logging.googleapis.com`, and
  `storage.googleapis.com`; builds and pushes a Docker image; creates a service
  account; grants Confidential Computing workload/viewer, Storage object
  viewer, Logging writer, and Artifact Registry reader roles; uploads the
  gated Gemma model to GCS; and fronts the VM with a regional external TCP load
  balancer on port 8000.
- The example vLLM server runs FastAPI behind the SDK server middleware,
  downloads model weights from GCS into the enclave workload, sets
  `VLLM_ATTENTION_BACKEND=FLASHINFER`, and serves `/v1/completions`.

Recommendation:

Use this as the client/server channel layer only after a minimal Confidential
Space or Confidential VM inference server exists. For the SDK itself, the most
faithful first probe is the codelab-shaped A3/H100 Confidential Space deployment
with a strict budget and immediate cleanup. The cheaper G4 Spot VM remains the
best entitlement/capacity probe for the new G4 announcement, but it is not the
same as proving the Prompt Encryption SDK codelab path.

Do not claim prompt privacy for OpenAgents until the acceptance test is an
actual SDK client request that succeeds only when the policy pins the expected
hardware model, project, zone, and image digest, and fails when any one of
those fields is intentionally changed.

### Confidential Space with H100 and Intel Trust Authority

Status: accessible for research; not the shortest path for this audit's G4
question.

Why:

- The announcement says H100 GPU support for Confidential Space is generally
  available and Intel Trust Authority integration is generally available.
- The project has Compute, Container, and AI Platform enabled, but this audit
  did not verify Confidential Space deployment templates, Confidential
  Computing API enablement, ITA account setup, or H100 capacity.
- Cloud Quotas shows 64 `PREEMPTIBLE-NVIDIA-H100-GPUS` in `us-central1`.

Recommendation:

Use Confidential Space for multi-party data-sharing or federated-learning
flows, not as the first proof for OpenAgents private inference. If the product
need is single-tenant private agent inference, Confidential G4 VM plus Prompt
Encryption SDK is the cleaner first probe.

## OpenAgents product implications

This is stronger than infrastructure readiness, but still not a public product
claim.

Do not update product promises, public copy, or agent-readable capabilities to
say OpenAgents has Confidential AI until:

1. A reusable Confidential G4 worker image is built from the successful Ubuntu
   plus open 580/LKCA path.
2. Attestation evidence is captured and stored as a public-safe receipt.
3. Prompt/response encryption is tested end-to-end with a pinned policy.
4. Runtime logs prove prompts, raw responses, keys, private repo contents, and
   customer data do not leak into public projections, docs, fixtures, or
   operator logs.
5. The relevant invariant or product-promise ledger is updated if the claim
   broadens beyond an internal readiness note.

Near-term true internal claim:

> OpenAgents can provision a Google Cloud Confidential G4 Spot VM and initialize
> its RTX PRO 6000 GPU in confidential mode.

That remains weaker than:

> OpenAgents provides Confidential AI inference.

The stronger claim needs deployment, attestation, prompt encryption, retention,
and public-safe evidence gates.

## Next actions

1. Convert the working manual bootstrap into a startup script or Packer/image
   build: Ubuntu 24.04 SEV-compatible base, open NVIDIA 580 server module,
   LKCA config, persistence mode, CUDA/runtime dependencies, and redacted boot
   evidence capture.
2. Re-run the G4 Spot worker from that image and measure cold-start time from
   create to healthy `nvidia-smi`.
3. Add a minimal inference container and run one no-secret local prompt through
   the GPU.
4. Add the Prompt Encryption SDK server wrapper and client policy check against
   the G4 worker. For SDK acceptance, require policy failure when hardware
   model, project, zone, or image digest is changed.
5. Run a separate bounded A3/H100 Prompt Encryption SDK codelab probe only if the expected spend
   is approved. Use the local codelab as source material, prefer a short
   `a3-highgpu-1g` Spot/TDX/Confidential Space run, record the image digest and
   load balancer IP, run `examples/test_client.py`, then delete the VM, load
   balancer, firewall rules, instance group, bucket contents, and image.
6. If capacity becomes unreliable, request/confirm Preview access and RTX PRO
   6000/G4 capacity for `openagentsgemini`. If quota fails, cite the Cloud
   Quotas evidence showing 16 preemptible RTX PRO 6000 GPUs in `us-central1`
   or 64 preemptible H100 GPUs in `us-central1`, depending on which probe
   failed, and ask Google which quota dimension the create path consumes.
7. Write a second audit with exact attestation result, inference result, cost
   envelope, teardown state, and whether a product-promise change is justified.
