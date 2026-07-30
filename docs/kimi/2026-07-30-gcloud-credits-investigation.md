# Can our Google Cloud credits host Kimi K3? (investigation)

Status: **first live pass complete, owner answers partially in**
(2026-07-30). All findings below were gathered against project
`openagentsgemini` with the workspace automation service account
(`oa-mvp-automation@…`) plus the owner session where billing permissions
required it.

## Owner answers (2026-07-30, same day)

- **Credits: $48,000** (owner-reported; expiry and SKU-scope column still
  unread).
- **Console K3 page**: reports that Google "provides deployment
  instructions that allow you to securely retrieve the Kimi K3 model
  weights directly from the publisher and deploy them to a GKE cluster
  within your own Google Cloud project. **Model Garden self-deployment
  will be available soon.**" So today the "Model Garden path" for K3 is
  really the GKE recipe with publisher-hosted weights; the one-click
  Vertex deploy does not exist yet, matching the API probe.

### $48k runway against the recipe shapes

The announcement's recommended A4X config (4× `a4x-highgpu-4g`, 16×
GB200) has **no public SKU at all** — see §4b. The priceable equivalent is
the A4 B200 variant (2× `a4-highgpu-8g`, 16× B200, same TP=16):

| Consumption model | 16-GPU $/hr | $48k lasts |
| --- | --- | --- |
| On-demand | 257.76 | ~7.8 days |
| DWS Flex Start | 128.88 | ~15.5 days |
| Spot | 79.27 | ~25 days |
| Vertex MaaS (K2-Thinking rates) | per-token | ~19B output tokens |

## Questions and answers

### 1. What credits do we hold, and what do they exclude?

**Not readable programmatically.** Facts established:

- `openagentsgemini` bills to `billingAccounts/01D15C-64524A-1062EA`
  ("My Billing Account", open, USD, org `831063912314`), billing enabled.
- Credit balances and credit-program exclusions are only visible in the
  Cloud Console (Billing → Credits). There is no gcloud/API surface for
  them, and this billing account has **no BigQuery billing export**
  configured (the only dataset, `psion_training_finops`, is our own price
  snapshot table, not an export).
- Only one budget exists: "Psion Google Single-Node Pilot" ($150).

→ Owner console step recorded in workspace `NEEDS_OWNER.md`: read the
Credits page for that billing account and note remaining balance, expiry,
and any SKU exclusions. Everything else below can proceed without it.

### 2. Does the Model Garden path work for K3, and would credits cover it?

Split answer — the K3 listing is real but not yet actionable via API, while
the Moonshot **serverless (MaaS) lane already works** from our project:

- `publishers/moonshotai/models/kimi-k3` exists and is **GA** on the Vertex
  v1 API, but returns no `supportedActions`, and
  `gcloud ai model-garden models list-deployment-config` says the model
  "does not support deployment". The Day-0 "one-click" path lives in the
  new Gemini Enterprise Agent Platform console and is not yet exposed to
  API/gcloud deployment from our project.
- No `kimi-k3*` MaaS model is live: `moonshotai/kimi-k3-maas` returns a
  distinct "Requested entity was not found" 404 (the entity name appears
  reserved but not yet enabled); `-thinking-maas` / `-instruct-maas`
  variants do not exist.
- **`moonshotai/kimi-k2-thinking-maas` served a real completion from our
  project** on the global endpoint (`traffic_type: ON_DEMAND`), proving
  the Moonshot MaaS lane is open to us with zero quota work.
- Billing catalog: Kimi K2 Thinking MaaS bills as **native Vertex AI
  SKUs** (service `C7E2-9256-1C43` "Vertex AI"):
  - input $0.60 / 1M tokens
  - output $2.50 / 1M tokens
  - cached input $0.06 / 1M tokens
  These are ordinary Google-billed service SKUs, not Marketplace items —
  the most credit-friendly shape, subject to the credit program's terms
  (owner check above). **No K3 SKUs exist in the catalog yet.**
- Vertex self-deploy quota: `CustomModelServingB200GPUsPerProjectPerRegion`
  is **8** in us-central1, us-east1, us-west2, asia-southeast1. K3 needs
  **16** B200s (TP=16), so even when Model Garden self-deploy for K3
  appears, we would need a quota increase to run it under Vertex.

### 3. Do we have quota for the required GPU capacity?

**No.** The project has effectively zero modern-accelerator capacity today:

- No `NVIDIA_B200` / `GB200` / `H100` / `H200` quota metrics appear in any
  candidate region (us-central1, us-east1, us-east4, europe-west4) — the
  legacy region quota list tops out at A100/L4 with single-digit limits.
- Cloud Quotas API shows `PREEMPTIBLE-NVIDIA-B200-GPUS-per-project-region`
  with a default value of 64 (unverified as an actual grant — treat as
  "must confirm via a quota request or a small provisioning test before
  relying on it"). On-demand B200 falls under `GPUS-PER-GPU-FAMILY` with
  no explicit grant.
- No reservations, no committed-use discounts, and **no GKE clusters** —
  the self-host paths start from scratch.
- Machine availability if quota were granted: `a4-highgpu-8g` (8× B200) in
  us-central1-b, us-east1-b, us-east4-b and others; `a4x-highgpu-4g`
  (GB200 NVL72) in us-central1-a/b, us-east1-d, us-east4-b,
  europe-west4-b.

### 4. What would self-hosting cost?

Compute Engine catalog, A4 B200 per-GPU-hour (Americas), 2026-07-30:

| Consumption model | $/GPU/hr | 16-GPU K3 $/hr | ~$/day | ~$/month |
| --- | --- | --- | --- | --- |
| On-demand | 16.11 | 257.76 | 6,186 | ~188k |
| DWS calendar mode | 11.277 | 180.43 | 4,330 | ~132k |
| 1-yr commitment | 11.1159 | 177.85 | 4,268 | ~130k |
| DWS Flex Start (defined duration) | 8.055 | 128.88 | 3,093 | ~94k |
| 3-yr commitment | 7.0884 | 113.41 | 2,722 | ~83k |
| Spot | 4.9542 | 79.27 | 1,902 | ~58k |

(Plus GKE cluster fee, 2× a4-highgpu-8g CPU/RAM components, ~2TB of GCS
for weights, and egress — the GPU line dominates.)

### 4b. The recommended A4X (GB200) config has no public price

The AI Hypercomputer recipe recommends **4× `a4x-highgpu-4g` (16× GB200,
multi-node NVLink, TP=16, LeaderWorkerSet Helm chart)**. Findings
(2026-07-30):

- Two full sweeps of the Compute Engine billing catalog (31,577 SKUs)
  found **zero** SKUs matching GB200 / A4X / Blackwell-superchip terms —
  unlike A4/B200, which has the six on-demand/Spot/DWS/CUD SKUs above.
- Google's public pricing pages list no A4X price, and the Spot VM docs
  exclude A4X from Spot.
- Conclusion: A4X GB200 capacity is **reservation-channel only** (AI
  Hypercomputer / Cluster Director reservations, DWS calendar mode, or a
  sales-negotiated commitment). There is no self-serve hourly path, so a
  credit-funded ad-hoc A4X run is not currently possible for us.
- Practical fallback with identical GPU count and a published price: the
  A4 B200 2-node variant from the same announcement (the GKE B200
  manifest in the digest doc), priced in the table above.

A bounded experiment is a different story: an 8-hour Spot window on 16
B200s is roughly **$630–700 all-in**, and a Flex-Start window about
**$1,050**.

### 5. Recommendation

1. **Do not self-host K3 as a standing service on credits.** Even the
   cheapest consumption model (Spot) is ~$58k/month for one 16-GPU
   serving group, we have zero granted GPU quota, no GKE cluster, and
   Spot preemption makes it a poor serving substrate anyway.
2. **The credit-plausible Kimi lane is Vertex MaaS.** Kimi K2 Thinking
   already serves from our project per-token under normal Vertex AI SKUs.
   Watch for the `kimi-k3` MaaS listing/SKUs (the `kimi-k3-maas` entity
   name already resolves differently from nonexistent models, so it looks
   staged). When it lands, we get K3 with zero ops and per-token billing.
3. **If we want hands-on K3 self-host experience**, the right shape is a
   bounded experiment, not a service: request B200 Spot/Flex-Start quota
   (16 GPUs, us-central1), build the announcement's GKE B200 recipe
   (see [the announcement digest](2026-07-30-google-cloud-day0-announcement.md)),
   run for hours, tear down. Budget a few hundred to ~$1k per run.
   llm-d is overkill at this scale; the plain SGLang StatefulSet recipe is
   the right starting point.
4. **Owner console steps** (tracked in workspace `NEEDS_OWNER.md`): read
   the Credits page for billing account `01D15C-64524A-1062EA` (balance,
   expiry, exclusions — this decides whether even MaaS burn is covered),
   and open the K3 Model Garden page on the agent-platform console to see
   what the one-click deploy actually offers (it may reveal a managed/MaaS
   option not yet visible via API).

## Method notes / evidence trail

- Billing link: `gcloud billing projects describe openagentsgemini`.
- Quota sweeps: `gcloud beta quotas info list/describe` on
  `compute.googleapis.com` and `aiplatform.googleapis.com`;
  `gcloud compute regions describe` for legacy region metrics.
- Model Garden: v1/v1beta1 `publishers/moonshotai/models/kimi-k3`
  (`?view=PUBLISHER_MODEL_VIEW_FULL`), `gcloud ai model-garden models
  list` / `list-deployment-config`.
- MaaS probes: `POST …/locations/global/endpoints/openapi/chat/completions`
  with `moonshotai/kimi-k2-thinking-maas` (success) and `kimi-k3*`
  variants (404s).
- Pricing: Cloud Billing Catalog API sweep of the Vertex AI (8,446 SKUs)
  and Compute Engine (31,577 SKUs) services.
- IAM changes made during this pass (durable, for future automation):
  granted `roles/cloudquotas.viewer`, `roles/aiplatform.user`, and
  `roles/container.viewer` to
  `oa-mvp-automation@openagentsgemini.iam.gserviceaccount.com`.
