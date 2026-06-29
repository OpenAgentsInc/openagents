# Pylon Proven-Engine Serving Capability Evidence

Status: schema + self-benchmark + receipt machinery landed (book P1-6,
openagents#6089). The real vLLM/SGLang GPU benchmark path is
**compute/owner-gated** — see the flags below. Nothing here is a green product
promise; it is the supply-side evidence shape Khala's marketplace can verify
before it pays anyone.

## Why this exists (from the book)

The inference-engineering book's near-term advice for whole-small-model Pylon
serving: don't write a bespoke engine — use a proven runtime (vLLM/SGLang/
TensorRT-LLM/llama.cpp) and make Pylon capability evidence **precise**. Two
ideas drive the schema here:

- **GPUs are not fungible.** "GPU online" is not capability evidence. Usable GPU
  memory, memory-bandwidth class (decode is bandwidth-bound), and interconnect
  posture (single GPU vs NVLink node vs InfiniBand multi-node) all change what a
  node can actually serve.
- **A served model id is not a product** unless engine, version, quantization
  mode, GPU class, and warm/cold state travel with it. An FP8/MXFP8 serve is a
  different product than an unqualified model id, so the precision/backend are
  disclosed in every receipt and scoped into parity.

## What landed (buildable now, no GPU/network)

- `src/serving-capability.ts` — typed `PylonServingCapabilityEvidence`: engines,
  usable/total GPU memory, bandwidth class, interconnect posture, per-model
  residency (warm/paged/cold) with the book's four cold-start factors
  (GPU procurement, image load, weight load, engine startup/compile). A serving
  claim is published only when a self-benchmark receipt accompanies it
  (`publishableServingCapabilityRefs`); `stripUnreceiptedServingCapability` is
  wired into the presence path's `publishableCapabilityRefs`, so an unproven
  serving claim never leaves the device — the same no-overclaim rule as the
  Tassadar executor capability.
- `src/serving-benchmark.ts` — worker **self-benchmark before registration**.
  The default adapter is deterministic and fixture-backed: a pinned,
  digest-known workload produces a stable, public-safe self-benchmark receipt
  (same input → byte-identical metrics + parity digest), touching no GPU and no
  network. Quantization is part of the parity digest, so changing the quant mode
  changes the digest and fails parity against the original — the book's "FP8 is
  a different product" rule expressed mechanically. The real-engine path is an
  async OpenAI-compatible HTTP runner for owner-provided vLLM/SGLang endpoints;
  it remains fail-closed unless the real-GPU flag, approval ref, and endpoint
  are present.
- `src/serving-receipt.ts` — per-serve `PylonServingReceipt` carrying
  engine/version/quantization/GPU class/warm-cold/parity, plus the
  **canary + replay-challenge** shape needed to verify a worker **before**
  payout. `computeServingVerification` centralizes "no parity, no pay":
  `payoutEligible` is only ever true when parity passed and no attached canary
  or replay challenge failed (an identity mismatch on replay also fails the
  gate). **This module computes eligibility; it never moves money.** A product
  surface owns the payout decision.
- `preflightRealPylonServing` in `src/serving-capability.ts` — a read-only,
  public-safe readiness preflight for #6089. It classifies whether a candidate
  Pylon has all evidence needed before a real whole-small-model serving route
  can be armed: owner confirmation and approval ref, admitted Pylon ref, fabric
  transport readiness, real-GPU capability evidence, vLLM/SGLang warm
  residency, matching real-GPU self-benchmark receipt, and a live serving
  receipt with parity, canary, replay, and payout eligibility. Missing pieces
  become typed `blocker.pylon.serving_preflight.*` refs. The preflight touches
  no GPU, network, wallet, gateway route, or payout system.
- `apps/openagents.com/workers/api/src/inference/openagents-network-adapter.ts`
  now has the product-side paid-routing wrapper,
  `makeAdmittedOpenAgentsNetworkAdapter`. That wrapper checks the Khala Pylon
  admission gate **before** dispatch, and then requires the returned serving
  receipt to carry parity, canary, replay, and payout-eligibility evidence before
  the request can clear the paid Pylon lane. A parity-only receipt is still
  useful for fabric dry-runs, but it does **not** clear paid routing.

## Compute/owner-gated split (honest bounds)

- The **real-GPU benchmark adapter** (`realGpuServingBenchmarkAdapter`) is
  reachable only when `PYLON_SERVING_REAL_GPU_BENCH=1`. Until then it refuses
  with `blocker.pylon.serving.real_gpu_adapter_gated`. The synchronous adapter
  still refuses with `blocker.pylon.serving.real_gpu_http_runner_required` so
  CI/default registration cannot accidentally perform network I/O.
- `runRealGpuServingSelfBenchmark` is the owner-gated HTTP runner. It requires:
  `PYLON_SERVING_REAL_GPU_BENCH=1`,
  `PYLON_SERVING_REAL_GPU_OWNER_APPROVAL_REF`, and
  `PYLON_SERVING_REAL_GPU_ENDPOINT`; `PYLON_SERVING_REAL_GPU_API_KEY` is
  optional for private endpoints. Endpoint URLs and API keys are never copied
  into the public receipt. Missing gates emit typed blocker refs instead of
  fabricating a measurement.
- Tests and CI always run the fixture path (`realGpuAdapter: false`).
- The real-serving preflight therefore fails closed on current fixture evidence:
  it will report missing real-GPU evidence, missing live fabric transport or
  admitted Pylon refs when they are absent, and missing canary/replay evidence
  for parity-only receipts. A passing preflight is a launch-readiness proof, not
  the live serving implementation itself.
- `apps/pylon/deploy/gcloud/setup-pylon.sh` is the GCE setup path for bringing
  an always-on Pylon host online through IAP/SSH without putting owner tokens in
  instance metadata. It can create CPU hosts for standing capacity or GPU hosts
  with `--accelerator type=count` for the #6089 proven-engine lane. The setup
  path installs and verifies the Pylon node; it still does not install vLLM or
  SGLang, arm paid routing, move money, or claim real-GPU benchmark evidence.
- `apps/pylon/scripts/real-serving-preflight.ts` is the owner-gated live
  evidence runner for an already-started vLLM/SGLang OpenAI-compatible
  endpoint. It emits a public-safe real-GPU self-benchmark receipt, capability
  evidence, serve receipt, replay challenge, and read-only preflight projection.
  Endpoint URLs, API keys, prompts, and raw outputs are not copied into the
  evidence.
- `apps/pylon/scripts/psionic-vllm-proxy.ts` is the bearer-protected local
  bridge from the Worker HTTP transport to the live vLLM endpoint. It accepts the
  Psionic serve request shape and returns the Psionic serve response shape with
  paid-traffic verification. Current scope is intentionally narrow: it clears
  the known-answer `OK` canary route smoke and keeps arbitrary/non-canary output
  unpayable until a fuller per-request verifier/proxy exists.

## Live GCloud bring-up evidence (2026-06-23)

- CPU standing-capacity host: `pylon-gcloud-khala-6089-check` in
  `openagentsgemini/us-central1-a`, Pylon ref
  `gcloud.pylon-gcloud-khala-6089-check`, systemd service
  `openagents-pylon` active, registered as
  `registration.gcloud.pylon-gcloud-khala-6089-check`, and heartbeating.
- Repurposed L4 host: `gswarm508-clean2-20260325044551-contrib` in
  `openagentsgemini/us-central1-b`, Pylon ref
  `gcloud.gswarm508-clean2-20260325044551-contrib`, systemd service
  `openagents-pylon` active, registered as
  `registration.gcloud.gswarm508-clean2-20260325044551-contrib`, and
  heartbeating.
- Live local serving backend on the repurposed L4 host: `pylon-vllm.service`
  runs vLLM `0.10.2` with Torch `2.8.0+cu128`, bound to `127.0.0.1:8000`, using
  public model `Qwen/Qwen2.5-0.5B-Instruct` served under
  `model.psionic.qwen35.0_8b.q8_0`. The newer vLLM `0.23.0` install path was
  rejected because it pulled CUDA 13 / Torch `2.11`, while the repurposed host's
  working NVIDIA driver is `570.211.01` / CUDA `12.8`. The CUDA-12-compatible
  stack was installed in `/opt/pylon-vllm-cu128`.
- The live known-answer run passed the typed no-parity/no-pay gate on
  2026-06-23 using a one-token `OK` workload with quantization `bf16`.
  Representative refs:
  `receipt.pylon.serving.self_bench.*`,
  `receipt.pylon.serving.*`,
  `challenge.pylon.serving.*`, and
  `preflight.pylon.real_serving.ready.v0_1`. This proves a warm local
  vLLM-backed serving route and replayable receipt evidence; it still does not
  claim a public gateway route, deploy a product route, move sats, or arm MPP.

Rerun from the Pylon host after the vLLM service is up:

```bash
cd /opt/openagents-pylon
PYLON_SERVING_REAL_GPU_BENCH=1 \
PYLON_SERVING_REAL_GPU_OWNER_APPROVAL_REF=approval.owner.khala.6089.gcloud_l4_smoke.2026_06_23 \
PYLON_SERVING_REAL_GPU_ENDPOINT=http://127.0.0.1:8000/v1/chat/completions \
PYLON_SERVING_ADMITTED_PYLON_REF=gcloud.gswarm508-clean2-20260325044551-contrib \
PYLON_SERVING_FABRIC_TRANSPORT_READY=1 \
PYLON_SERVING_ENGINE_VERSION=vllm@0.10.2+torch2.8.0-cu128 \
bun apps/pylon/scripts/real-serving-preflight.ts
```
- The repurposed L4 host had stale Psion startup metadata and a
  kernel/module mismatch after boot. The setup path now has
  `--clear-startup-script` and existing-VM tag assurance; the live host was
  rebooted once into `6.8.0-1053-gcp`, where the installed
  `linux-modules-nvidia-570-server-open-6.8.0-1053-gcp` module makes
  `nvidia-smi` report one NVIDIA L4, driver `570.211.01`, CUDA `12.8`, and
  23034 MiB VRAM.
- This is Pylon setup, GPU inventory, local vLLM residency, and known-answer
  receipt evidence. #6089 remains open until a real public gateway transport is
  deployed and smoked through the Worker route without exposing endpoint URLs or
  secrets.
- Follow-up gateway-transport smoke on 2026-06-23 installed the local
  Psionic-vLLM proxy as `pylon-psionic-vllm-proxy.service`, protected by a
  host-local bearer token file, and ran the known-answer canary through both
  `127.0.0.1:8011` and a temporary `trycloudflare.com` route. The temporary
  route returned HTTP 200 with `content: "OK"`, `parityVerified: true`,
  `canaryPassed: true`, `replayPassed: true`, and `payoutEligible: true`;
  representative public refs were
  `serve.pylon.gateway_proxy.cAR4xZXQagyw7yBsjeO6IG` and
  `challenge.pylon.serving.GuUBPkgNgLRtTCgkkO-s`. This proves the
  proxy/tunnel transport path, but still does not claim a durable named tunnel,
  a production Worker secret deployment, live customer dispatch, sats movement,
  or a product-promise green flip.
- Durable named-tunnel follow-up on 2026-06-23 created Cloudflare Tunnel
  `pylon-khala-6089-gcloud-l4`
  (`055cc100-c9a8-46c9-85f6-90ebf1accf09`) with remote ingress to the same
  host-local proxy at `127.0.0.1:8011`, installed its token as a root-readable
  file on the admitted L4 host, and enabled
  `pylon-khala-6089-named-tunnel.service`. The service is active/enabled,
  `cloudflared` prechecks pass with QUIC, and Cloudflare reports the named
  tunnel `healthy`. A local proxy canary after the named-tunnel install still
  returned HTTP 200, content `OK`, parity true, canary true, replay true, and
  payout eligible true. This proves the durable connector is running, but the
  public DNS route is not yet installed: the current Wrangler OAuth token can
  manage tunnels and read the `openagents.com` zone, but `dns_records` calls
  return Cloudflare API `403` authentication errors. Do not deploy Worker
  `OPENAGENTS_NETWORK_FABRIC_SERVE_URL` / bearer secrets, claim a public
  gateway route, or retire the temporary tunnel until an owner supplies DNS
  write authority or creates the required CNAME to this tunnel target.
- Durable HTTPS route-around follow-up on 2026-06-24 attached a reserved GCloud
  external IP to the admitted L4 host, opened host-level HTTP/HTTPS only to the
  `pylon-hosted` tag, and placed Caddy in front of the bearer-protected local
  proxy. The public route canary returned HTTP 200 with `content: "OK"`,
  `parityVerified: true`, `canaryPassed: true`, `replayPassed: true`, and
  `payoutEligible: true`. Public-safe refs:
  `serve.pylon.gateway_proxy.cAR4xZXQagyw7yBsjeO6IG`,
  `challenge.pylon.serving.GuUBPkgNgLRtTCgkkO-s`, and
  `approval.owner.khala.6089.gateway_route.2026_06_24`. This proves a durable
  route without requiring Cloudflare DNS write authority. Endpoint URL and
  bearer values remain deploy-time Worker secrets, not committed config.
- The public model surface remains collapsed to the single customer-facing
  `openagents/khala` id. The internal `openagents/khala-pylon-mini` canary id
  stays out of `/v1/models`, `/v1/quote`, MPP discovery, and the public
  `/v1/chat/completions` model gate. Operators prove the Pylon route with the
  admin-token-gated
  `POST /api/operator/inference/pylon-fabric/smoke` route instead; it runs one
  fixed known-answer prompt through the secret-backed admitted Pylon adapter and
  returns only public-safe status fields. That route exists so production smokes
  can prove the real Pylon route rather than another open-model provider without
  widening public model selection or creating a public load generator.
- Current Worker admission wiring still uses a deploy-time Pylon serving
  snapshot for heartbeat freshness. For production smokes, refresh
  `OPENAGENTS_NETWORK_PYLON_HEARTBEAT_AT` immediately before the route test.
  A follow-up hardening should replace that static snapshot with a live
  Pylon-store registration lookup before broad, unattended paid routing.

## Where this plugs in next (not in this change)

The fabric supply adapter behind `InferenceProviderAdapter`
(`apps/openagents.com/workers/api/src/inference/provider-adapter.ts`, P1-5 lane)
now owns the canaryable paid-routing gate for a registered Pylon. The Worker can
also register a secret-backed HTTP Psionic serve transport when the route refs,
transport URL/token, and Pylon admission snapshot are present. Any payout still
goes through the separate settlement gates; this evidence path does not move
money.

## Worker Gateway Route Arming

The OpenAgents.com Worker serving policy can now arm the `openagents-network`
catalog lane only when a deploy supplies all of the following public-safe
route/evidence refs:

```bash
OPENAGENTS_NETWORK_GATEWAY_ROUTE_READY=ready
OPENAGENTS_NETWORK_GATEWAY_APPROVAL_REF=approval.owner.khala.6089.gateway_route.2026_06_24
OPENAGENTS_NETWORK_SERVING_PREFLIGHT_REF=preflight.pylon.real_serving.ready.v0_1
OPENAGENTS_NETWORK_SERVING_RECEIPT_REF=serve.pylon.gateway_proxy.cAR4xZXQagyw7yBsjeO6IG
OPENAGENTS_NETWORK_REPLAY_CHALLENGE_REF=challenge.pylon.serving.GuUBPkgNgLRtTCgkkO-s
OPENAGENTS_NETWORK_ADMITTED_PYLON_REF=gcloud.gswarm508-clean2-20260325044551-contrib
```

The policy treats these as presence-only, public-safe refs. It rejects raw URLs,
secret-shaped strings, blank values, and truthy-but-not-`ready` route flags. It
also requires the secret-backed transport URL and bearer token to be present via
Worker secrets:

```bash
OPENAGENTS_NETWORK_FABRIC_SERVE_URL=<secret Pylon proxy URL>
OPENAGENTS_NETWORK_FABRIC_SERVE_BEARER_TOKEN=<secret proxy bearer token>
OPENAGENTS_NETWORK_PYLON_HEARTBEAT_AT=<fresh heartbeat ISO timestamp>
OPENAGENTS_NETWORK_PYLON_HEARTBEAT_STATUS=ok
OPENAGENTS_NETWORK_PYLON_SERVING_CAPABILITY_REF=pylon.capability.serving.whole_small_model.v0.6
OPENAGENTS_NETWORK_PYLON_SERVING_LANE_REF=lane.openagents.pylon.vllm.whole_small_model.v1
OPENAGENTS_NETWORK_SPARK_PAYOUT_TARGET_REF=payout.spark.aab6617b16f096dfe02fc6b4
```

The URL/token values are read only for presence in catalog/readiness policy and
are never returned from public routes. The actual dispatch path posts a Psionic
serve request to the proxy with `Authorization: Bearer ...`, consumes the
Psionic serve response, then runs the admitted Pylon gate and full
parity/canary/replay/payout-eligibility receipt gate before returning success.
This lets `/v1/gateway/readiness` expose the Pylon lane arming state only after
the operator has a real gateway route plus the serving preflight, serving
receipt, replay challenge, and admitted-Pylon refs. Public `/v1/models`,
`/v1/quote`, and `/v1/chat/completions` still expose/accept only
`openagents/khala`; the operator smoke route is the direct Pylon adapter proof.
It still does not expose the private GCloud endpoint, move sats, or green a
product promise by itself.

Run the Worker smoke after refreshing the heartbeat snapshot and setting the
Worker URL/token secrets:

```bash
curl -fsS https://openagents.com/api/operator/inference/pylon-fabric/smoke \
  -H "Authorization: Bearer $OPENAGENTS_ADMIN_API_TOKEN" \
  -X POST
```

Expected public-safe success shape:

```json
{
  "status": "ok",
  "routeRef": "route.operator.inference.pylon_fabric_smoke.v0_1",
  "model": "openagents/khala-pylon-mini",
  "servedModel": "model.psionic.qwen35.0_8b.q8_0",
  "canaryPassed": true,
  "content": "OK"
}
```

Run the local proxy on the Pylon host:

```bash
cd /opt/openagents-pylon
PYLON_PSIONIC_PROXY_BEARER_TOKEN=<secret proxy bearer token> \
PYLON_PSIONIC_PROXY_NODE_REF=gcloud.gswarm508-clean2-20260325044551-contrib \
PYLON_PSIONIC_PROXY_REPLAY_CHALLENGE_REF=challenge.pylon.serving.GuUBPkgNgLRtTCgkkO-s \
PYLON_PSIONIC_PROXY_SERVED_MODEL=model.psionic.qwen35.0_8b.q8_0 \
PYLON_PSIONIC_PROXY_UPSTREAM_MODEL=model.psionic.qwen35.0_8b.q8_0 \
PYLON_PSIONIC_PROXY_UPSTREAM_URL=http://127.0.0.1:8000/v1/chat/completions \
PYLON_PSIONIC_PROXY_PORT=8011 \
bun apps/pylon/scripts/psionic-vllm-proxy.ts
```

Use the model id returned by the local vLLM `/v1/models` endpoint for
`PYLON_PSIONIC_PROXY_UPSTREAM_MODEL`; on the live L4 host that is the served
alias `model.psionic.qwen35.0_8b.q8_0`, not the Hugging Face source id.

If no named Cloudflare Tunnel credential is available, a temporary
`trycloudflare.com` tunnel can smoke the route while the proxy's bearer token
protects the origin:

```bash
cloudflared tunnel --url http://127.0.0.1:8011
```

Use the emitted HTTPS URL only as `OPENAGENTS_NETWORK_FABRIC_SERVE_URL` secret
input; do not paste it into issues, public docs, commits, or product promises.

## Khala M6 shadow-run preflight

`apps/pylon/src/khala-m6-shadow-preflight.ts` adds the read-only M6 readiness
projection `openagents.khala.m6.shadow_run_preflight.v0.1`. It composes the
existing Psionic training boundary, the real Pylon serving preflight, owner
approval and spend-cap inputs, the live verdict source, the shadow candidate,
the baseline router, and paid-shadow/publication refs into one public-safe
blocker list.

The projection distinguishes two states:

- `canStartShadowRun` may become true once owner approval, caps, Psionic
  boundary evidence, live serving evidence, verdict source, candidate, baseline,
  and live rollout refs are present.
- `canPublishM6Claim` remains false until the paid shadow win and publication
  refs are also present.

This is still not a dispatcher. It does not run Psionic, call a Pylon, spend
sats, promote runtime artifacts, or green the M6 public claim by itself.

### Worker HTTP buy-mode eval adapter

`POST /api/operator/buy-mode/eval` is the owner/admin-authenticated HTTP target
for Psionic's `HttpBuyModeDispatch` client. It accepts Psionic's compact
`BuyModeEvalJob` shape (`worker_id`, `role_index`, `sample_id`,
`amount_msats`) and returns the matching compact settled result
(`verdict.class`, `verdict.passed`, `settled_msats`) only after the request has
passed the existing OpenAgents buy-mode campaign, per-job cap, daily cap, and
relay-publication boundaries.

The endpoint deliberately remains fail-closed unless the live eval bridge is
configured in the Worker runtime. An admitted dispatch without that bridge
returns `blocker.buy_mode.eval_bridge_unconfigured`; it does not fabricate a
verification pass, move sats, or publish the M6 shadow-win claim.

## Khala M7 Conductor preflight

`apps/pylon/src/khala-m7-conductor-preflight.ts` adds the read-only M7 readiness
projection `openagents.khala.m7.conductor_preflight.v0.1`. It consumes the M6
shadow-run preflight and mirrors Psionic's `ConductorReadiness` close gates:
7B policy backend, GRPO training run, armed paid verdict source, paid M6
shadow-win, Verse fan-out, crossy-road composition proof, and publication refs.

The projection distinguishes:

- `canStartConductorTraining`, which may become true when M6 is publishable,
  the 7B policy backend is wired, the paid verdict source is armed, and owner
  caps approve the run.
- `canPublishM7Claim`, which remains false until the actual GRPO training run,
  Verse fan-out, verified crossy-road composition result, and publication refs
  exist.

This is not the Conductor implementation or a training launcher. It does not run
GRPO, serve a 7B model, dispatch workers, spend sats, or publish the benchmark
claim.

`apps/pylon/src/khala-m7-conductor-composition.ts` adds the companion public
composition-proof projection `openagents.khala.m7.conductor_composition_proof.v0.1`.
It replaces the old "bare composition ref" shape with a structural proof:

- the planner evidence must name the executed GRPO/DPPO run, FP32 LM head, and
  zero-std filtering recipe;
- the topology must include the crossy-road plan -> implement -> verify -> refine
  chain, worker ids, and access lists for the selected frontier, open Pylon,
  verifier, and optional Tassadar/module workers;
- the M2 crossy-road verdict must be accepted, the Verse fan-out must be visible,
  and the composition cost must be lower than the single-model baseline at
  comparable quality.

When that proof is publishable, the M7 preflight can derive
`compositionProofRef`, `crossyRoadCompositionRef`, and `verseFanoutRef` from the
proof. If the proof is missing, unsafe, equal-or-higher cost, unverifiable, or
topologically invalid, the preflight still fails closed.
