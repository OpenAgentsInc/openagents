# Announcing Day 0 support for Kimi K3 on Google Cloud (digest)

- Source: <https://discuss.google.dev/t/announcing-day-0-support-for-kimi-k3-on-google-cloud/385392>
- Forum: Google Developer forums → Google Cloud → Community Articles
- Author: Injae_Kwak (Googler article), with dbartoletti and Shakhizat_Nurgaliyev
- Tags: `gke`, `ai-infrastructure`, `googler-article`
- Captured: 2026-07-30. This is a digest for internal evaluation, not a
  verbatim mirror; consult the source post for authoritative text.

## The model

[Kimi K3](https://huggingface.co/moonshotai/Kimi-K3) is a
**2.8-trillion-parameter Mixture-of-Experts model** from Moonshot AI, built on
the novel **Kimi Delta Attention (KDA)**. It activates only **16 of 896
experts per token**, which makes it computationally efficient per token, but
its size demands extreme network throughput and highly optimized decoding to
serve under high concurrency without stalling accelerators.

Kimi K3 is subject to specific license terms and restrictions:
<https://huggingface.co/moonshotai/Kimi-K3/blob/main/LICENSE>

## Flexible deployment paths on Google Cloud

1. **Model Garden on Gemini Enterprise Agent Platform** — the easiest path for
   enterprises: one-click templates and production-grade endpoints.
   - Console: <http://console.cloud.google.com/agent-platform/publishers/moonshotai/model-garden/kimi-k3>
2. **AI Hypercomputer recipes** — validated infrastructure configurations for
   teams running custom orchestration.
   - Recipes: <https://github.com/AI-Hypercomputer/gpu-recipes/tree/main/inference/a4x/multi-host-serving/sglang>
3. **GKE with llm-d** — Kubernetes-native distributed serving at massive
   scale, with intelligent KV-aware routing, disaggregated prefill-decode
   serving, and multi-tier KV cache offloading.
   - llm-d: <https://llm-d.ai/> · GitHub: <https://github.com/llm-d/llm-d>

## Architecture: multi-node serving with SGLang and DSPARK

- Model server: **SGLang**, with native support for Kimi Delta Attention and
  Stable LatentMoE.
- The model exceeds single-machine memory, so serving spans **2 nodes (A4 VM,
  8× NVIDIA B200 each) or 4 nodes (A4X VM, GB200 NVL72)** with **Tensor
  Parallelism 16**.
- Recommended: NVLink domains on A4X (GB200 NVL72) and A4X Max (GB300 NVL72).
  Alternative: A4 (B200) using **RDMA** for accelerated all-reduce.
- **Speculative decoding via DSPARK** with the
  [RadixArk/Kimi-K3-DSpark](https://huggingface.co/RadixArk/Kimi-K3-DSpark)
  draft model (block size 7) to raise decode throughput.

## Deploying Kimi K3 on GKE (B200 recipe)

### 1. Pre-upload model weights and draft model to a GCS bucket

```sh
pip install -U huggingface_hub[cli] hf_transfer

export HF_HUB_ENABLE_HF_TRANSFER=1
export HF_TOKEN=<yourKey>

huggingface-cli download moonshotai/Kimi-K3 \
  --local-dir ./Kimi-K3 \
  --local-dir-use-symlinks False

huggingface-cli download RadixArk/Kimi-K3-DSpark \
  --local-dir ./Kimi-K3-DSpark \
  --local-dir-use-symlinks False

cp ./Kimi-K3/generation_config.json ./Kimi-K3-DSpark/

gcloud storage cp -r ./Kimi-K3 gs://${GCS_BUCKET}/Kimi-K3
gcloud storage cp -r ./Kimi-K3-DSpark gs://${GCS_BUCKET}/Kimi-K3-DSpark
```

### 2. Deploy an AI-optimized GKE cluster

Create the cluster with RDMA connectivity using either:

- Cluster Toolkit: <https://docs.cloud.google.com/ai-hypercomputer/docs/create/gke-ai-hypercompute>
- Custom deployment with the gcloud CLI: <https://docs.cloud.google.com/ai-hypercomputer/docs/create/gke-ai-hypercompute-custom>

### 3. Deploy Kimi K3 with SGLang on GKE with B200

The post publishes a verified manifest: a headless master Service
(`sglang-master-pod-k3`, port 20000), a serving Service
(`sglang-serving-k3`, port 30000), and a **StatefulSet with 2 replicas**
(one per A4 node), each requesting **8× `nvidia.com/gpu`, 206 CPU, 1500Gi
memory**, node-selected onto `nvidia-b200` / `a4-highgpu-8g-a4-pool`.

Key elements of the manifest:

- Image `docker.io/lmsysorg/sglang:kimi-k3`.
- Ten network interfaces per pod: `eth0` default, `eth1` gVNIC, `eth2`–`eth9`
  on `rdma-0` … `rdma-7` networks.
- GCSFuse CSI ephemeral volume mounts the weights bucket at `/bucket` with
  parallel-download file cache and streaming writes.
- An NCCL shim (`libnccl_dev_shim.so`) is compiled at startup and
  `LD_PRELOAD`ed, with `NCCL_NET=gIB` and the gIB env script sourced, so the
  container's NCCL rides Google's RDMA stack.
- Weights load through the RunAI model streamer
  (`--load-format runai_streamer`, 32GiB memory limit, concurrency 16).
- Serve command (per node rank):

```sh
sglang serve \
    --trust-remote-code \
    --model-path /bucket/Kimi-K3 \
    --served-model-name moonshotai/Kimi-K3 \
    --load-format runai_streamer \
    --enable-metrics \
    --tp-size 16 \
    --nnodes 2 \
    --node-rank $POD_INDEX \
    --dist-init-addr sglang-master-pod-k3:20000 \
    --mem-fraction-static 0.85 \
    --disable-flashinfer-autotune \
    --watchdog-timeout 3600 \
    --reasoning-parser kimi_k3 \
    --tool-call-parser kimi_k3 \
    --model-loader-extra-config '{"memory_limit": 34359738368}' \
    --mamba-full-memory-ratio 0.43 \
    --host 0.0.0.0 \
    --port 30000 \
    --speculative-algorithm DSPARK \
    --speculative-draft-model-path /bucket/Kimi-K3-DSpark \
    --speculative-dspark-block-size 7 \
    --enable-linear-replayssm-spec \
    --max-running-requests 512 \
    --cuda-graph-max-bs 512
```

- Shared memory: 250Gi `emptyDir` (Memory medium) at `/dev/shm`; host paths
  for the NVIDIA libraries and gIB tooling; workload identity service account
  `workload-identity-k8s-sa`; optional `hf-secret` for `HF_TOKEN`.

### 4. Test the model

```sh
kubectl exec -i distributed-sglang-k3-0 -c sglang-container -- \
  curl -s -X POST http://localhost:30000/v1/chat/completions \
  -H "Content-Type: application/json" \
  -d '{
    "model": "moonshotai/Kimi-K3",
    "messages": [
      {"role": "user", "content": "What is Google Cloud AI Hypercomputer in 2 sentences."}
    ],
    "max_tokens": 256,
    "temperature": 0.7
  }' | jq .
```

## Moving to production readiness

The post directs readers to
[Best practices for GKE](https://docs.cloud.google.com/kubernetes-engine/docs/best-practices)
for cost optimization, performance tuning, security hardening, and
reliability engineering.

## What's next (per the post)

- Expanding llm-d ecosystem support for massive-scale inference (addressing
  memory-bandwidth limits and request queuing).
- Broader hardware support: A4X-Max (GB300 NVL72) variants and Google Cloud
  TPUs.

## Acknowledgements (per the post)

Collaboration between Moonshot AI and RadixArk for community infrastructure,
including the DSPARK draft model.

## All external links from the post

| Link | URL |
| --- | --- |
| Kimi K3 model card | <https://huggingface.co/moonshotai/Kimi-K3> |
| Kimi K3 license | <https://huggingface.co/moonshotai/Kimi-K3/blob/main/LICENSE> |
| Kimi K3 DSpark draft model | <https://huggingface.co/RadixArk/Kimi-K3-DSpark> |
| Model Garden listing (console) | <http://console.cloud.google.com/agent-platform/publishers/moonshotai/model-garden/kimi-k3> |
| AI Hypercomputer GPU recipes (SGLang multi-host, a4x) | <https://github.com/AI-Hypercomputer/gpu-recipes/tree/main/inference/a4x/multi-host-serving/sglang> |
| llm-d project | <https://llm-d.ai/> |
| llm-d GitHub | <https://github.com/llm-d/llm-d> |
| Cluster Toolkit GKE AI Hypercompute guide | <https://docs.cloud.google.com/ai-hypercomputer/docs/create/gke-ai-hypercompute> |
| Custom gcloud CLI cluster deployment | <https://docs.cloud.google.com/ai-hypercomputer/docs/create/gke-ai-hypercompute-custom> |
| GKE best practices | <https://docs.cloud.google.com/kubernetes-engine/docs/best-practices> |
| SGLang Kimi K3 image | `docker.io/lmsysorg/sglang:kimi-k3` |
