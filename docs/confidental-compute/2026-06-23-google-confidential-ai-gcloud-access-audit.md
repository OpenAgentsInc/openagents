# Google Confidential AI gcloud Access Audit

Date: 2026-06-23

Source prompt: evaluate the Google Cloud announcement "Verifiable, private AI:
Google Cloud expands Confidential Computing frontiers" and determine whether
OpenAgents appears to have local `gcloud` access to try the new Confidential
Computing surfaces.

## Bottom line

OpenAgents appears to have enough local Google Cloud access to start a
non-production probe for the newly announced Confidential G4 path, but access
is not yet proven as provisionable capacity.

The active local `gcloud` configuration targets project `openagentsgemini`,
region `us-central1`, zone `us-central1-a`, with an active human account that
has project Owner. The project can see ordinary G4 machine types and
`nvidia-rtx-pro-6000` accelerator types in `us-central1-b` and `us-central1-f`.
Google's current docs say Confidential VM on G4 is Preview, limited to
`g4-standard-48` in limited regions and zones. That exact machine type is
visible in both `us-central1-b` and `us-central1-f`.

The unresolved blocker is quota/preview entitlement/capacity, not basic IAM.
The quota listing for `us-central1` did not expose a named RTX PRO 6000 or G4
quota metric in the current `gcloud` output, and I did not create a VM during
this audit. Treat the next step as a deliberately bounded create/delete probe,
or as a quota/preview confirmation with Google Cloud support, before making any
product or operator promise.

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

## Feasibility assessment

### Confidential G4 VM

Status: likely testable, not proven provisionable.

Why:

- The required API baseline for Compute Engine is enabled.
- The active account has Owner on the project.
- `g4-standard-48` is visible in `us-central1-b` and `us-central1-f`.
- `nvidia-rtx-pro-6000` is visible in the same two zones.
- Both zones are UP and include the AMD Turin CPU platform.
- Google docs state that Confidential VM with G4 uses AMD SEV and RTX PRO
  6000 in Preview, and that G4 Confidential VM is limited to
  `g4-standard-48` in limited regions/zones.

Remaining unknowns:

- Whether the project is admitted to the Preview feature.
- Whether quota exists under the newer RTX PRO 6000/G4 metric.
- Whether current zonal capacity is available.
- Whether the right image and disk choices satisfy both G4 and Confidential VM
  constraints.

Recommended bounded probe:

```sh
gcloud compute instances create oa-confidential-g4-probe-20260623 \
  --project=openagentsgemini \
  --zone=us-central1-b \
  --machine-type=g4-standard-48 \
  --confidential-compute-type=SEV \
  --maintenance-policy=TERMINATE \
  --provisioning-model=SPOT \
  --boot-disk-type=hyperdisk-balanced \
  --image-family=ubuntu-2404-lts-amd64 \
  --image-project=ubuntu-os-cloud \
  --labels=purpose=confidential-g4-probe,owner=openagents,ttl=manual-delete \
  --no-address
```

If creation succeeds, immediately verify and delete unless a live test is
intended:

```sh
gcloud compute instances describe oa-confidential-g4-probe-20260623 \
  --project=openagentsgemini \
  --zone=us-central1-b \
  --format='json(confidentialInstanceConfig,machineType,guestAccelerators,status)'

gcloud compute instances delete oa-confidential-g4-probe-20260623 \
  --project=openagentsgemini \
  --zone=us-central1-b
```

If creation fails, capture only the structured error category, not raw logs with
private metadata. Likely outcomes are preview entitlement missing, quota missing,
or capacity unavailable.

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
deployment.

Why:

- The SDK repository is public.
- The SDK's stated purpose is an end-to-end attested TLS channel to a TEE
  workload, including vLLM/FastAPI-style inference servers.
- The codelab says the client verifies server identity, software hash,
  hardware model, and launch configuration before sending prompt data.

Recommendation:

Use this as the client/server channel layer only after a minimal Confidential
Space or Confidential VM inference server exists. Do not claim prompt privacy
for OpenAgents until the attestation policy pins the expected project, zone,
hardware model, image hash, and launch configuration.

### Confidential Space with H100 and Intel Trust Authority

Status: accessible for research; not the shortest path for this audit's G4
question.

Why:

- The announcement says H100 GPU support for Confidential Space is generally
  available and Intel Trust Authority integration is generally available.
- The project has Compute, Container, and AI Platform enabled, but this audit
  did not verify Confidential Space deployment templates, Confidential
  Computing API enablement, ITA account setup, or H100 quota.

Recommendation:

Use Confidential Space for multi-party data-sharing or federated-learning
flows, not as the first proof for OpenAgents private inference. If the product
need is single-tenant private agent inference, Confidential G4 VM plus Prompt
Encryption SDK is the cleaner first probe.

## OpenAgents product implications

This is infrastructure readiness, not a product claim.

Do not update product promises, public copy, or agent-readable capabilities to
say OpenAgents has Confidential AI until:

1. A VM or Confidential Space workload is actually provisioned.
2. Attestation evidence is captured and stored as a public-safe receipt.
3. Prompt/response encryption is tested end-to-end with a pinned policy.
4. Runtime logs prove prompts, raw responses, keys, private repo contents, and
   customer data do not leak into public projections, docs, fixtures, or
   operator logs.
5. The relevant invariant or product-promise ledger is updated if the claim
   broadens beyond an internal readiness note.

Near-term useful claim after a successful probe:

> OpenAgents has an internal Google Cloud Confidential G4 probe path under
> evaluation.

That remains weaker than:

> OpenAgents provides Confidential AI inference.

The stronger claim needs deployment, attestation, prompt encryption, retention,
and public-safe evidence gates.

## Next actions

1. Run the bounded `oa-confidential-g4-probe-20260623` create/describe/delete
   flow in `us-central1-b`.
2. If entitlement or quota fails, request/confirm Preview access and RTX PRO
   6000/G4 quota for `openagentsgemini`.
3. If VM creation succeeds, run a no-secret attestation proof and capture only
   public-safe metadata.
4. Clone and test `google/prompt-encryption-sdk` against a local/dev server,
   then against the confidential server once one exists.
5. Write a second audit with exact create result, attestation result, cost
   envelope, teardown state, and whether a product-promise change is justified.
