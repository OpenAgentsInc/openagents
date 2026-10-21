# Native Flint on GCP

Issue: [#12419](https://github.com/OpenAgentsInc/openagents/issues/12419).

`psionic-flint` executes Recursive's original Flint checkpoint in Rust through
Candle on one CUDA GPU. Python supplies offline test references; it is absent
from the serving process. The first profile is
`bf16_weights_f32_compute_v1`: weights stay in BF16 on the device, and each
projection expands its weights to FP32 while it runs. Activations, attention,
expert accumulation, KV state, and logits use FP32.

The checkpoint is `RecursiveSI/flint` at
`2a247be0c0aec895ccb0ac7ca92229ec6df37288`. Startup verifies all 15 shards,
the tokenizer, configuration, index, and template against the independent
[release hashes](../../crates/psionic/crates/psionic-flint/testdata/artifact.json).
Keep the downloaded Apache-2.0 license beside the weights.

## Scope

The model implements all 49 layers: grouped-query attention, per-head Q/K
normalization, learned sinks, full attention without RoPE, sliding attention
with NeoX RoPE and a 513-token window, two dense FFNs, 47 sigmoid-routed MoE
layers with six selected experts, and the always-active shared expert.

The released tokenizer and text template preserve assistant reasoning history
and escape role markers in other messages. The four released stop IDs end
generation. Unfinished reasoning stays out of final content.

The HTTP endpoint accepts text messages, `max_tokens`, temperature, top-p,
top-k, and seed. Defaults are Recursive's temperature 1, top-p 0.95, and
top-k 20, with seed 42. Temperature zero is available for diagnostics.
Streaming, tools, and other request fields are refused. This is a separate
binary from the GGUF server; it does not add a Flint GGUF decoder or Pylon
catalog entry.

The initial service uses an 8,192-token context. Prompt and output allowance
must fit together. Each request has a 900-second deadline checked between
token steps. One request runs at a time; another receives HTTP 429. If a
client disconnects, its generation keeps the slot until it ends. Every
request starts with fresh KV state. Systemd restarts a failed process.

## Build and run

Use a dedicated RTX PRO 6000 GCP host with a 96 GB GPU. Allocate it through
[`cloud-budget.py`](../../scripts/dev/cloud-budget.py), with an expiry and
run labels. The tested host uses `g4-standard-48`, a 250 GB
`hyperdisk-balanced` boot disk, and the Deep Learning VM image
`common-cu129-ubuntu-2404-nvidia-580-v20261019` from
`deeplearning-platform-release`.

In a source checkout on that host:

```sh
bash scripts/psionic/flint-gcp-setup.sh
bash scripts/psionic/flint-service.sh COMMIT \
  "$HOME/work/agent-targets/openagents-crates-psionic/release/psionic-flint"
curl http://127.0.0.1:8080/healthz
curl http://127.0.0.1:8080/v1/chat/completions \
  -H 'Content-Type: application/json' \
  -d '{"model":"flint","messages":[{"role":"user","content":"What is 2 + 2?"}],"max_tokens":512}'
```

`flint-gcp-setup.sh` installs the pinned compiler, downloads and verifies the
checkpoint, and builds and tests CUDA support. It uses one shared Cargo
target outside the checkout, with source-keyed compiler state. The service
installer copies the executable to a commit-named directory and listens on
loopback. Reach it through SSH forwarding; no public firewall rule is needed.

## Verification

The independent NumPy fixture covers a complete three-layer model across a
sliding-window boundary, with both dense and routed/shared FFNs. A separate
fixture rounds weights to BF16 before computing FP32 expected results. CPU
error is bounded by `2e-6`; CUDA error by `2e-5`. Tokenizer fixtures come from
the official tokenizer and Jinja template, including multilingual text,
digits, NUL removal, escaped role markers, and reasoning history.

```sh
cd crates/psionic
export CARGO_TARGET_DIR="$HOME/work/agent-targets/openagents-crates-psionic"
. ../../scripts/dev/cargo-build-env.sh
FLINT_TOKENIZER="$HOME/flint-weights/tokenizer.json" \
  cargo test --release -p psionic-flint --features cuda -- --include-ignored
```

[`flint-reference.py`](../../scripts/psionic/flint-reference.py) independently
computes teacher-forced logits with eager PyTorch. Its default FP32 computation
matches the served profile. `--compute bf16` preserves the first diagnostic
profile. [`flint-compare.py`](../../scripts/psionic/flint-compare.py) compares
all 150,144 logits at every supplied position, requiring equal top-1, total
variation at most 0.02, and centered-logit maximum error at most 0.25. These
limits were fixed before reading the first full-model comparison and remain
unchanged. This is a short-prefix regression check, not broad task-quality
qualification.

The initial BF16-compute experiment failed those limits despite producing
correct short answers. At the first token, identical input normalization led
to a one-BF16-ULP attention difference; subsequent residuals and routes drifted.
The supported profile uses FP32 computation to avoid repeatedly rounding those
states. The failed result stays separate from the supported profile's results.

## GCP results, 2026-10-21

[Receipt](verification/flint-2026-10-21/receipt.json),
[CUDA test log](verification/flint-2026-10-21/cuda-tests.log),
[HTTP answers](verification/flint-2026-10-21/http-f32.json), and
[recovery check](verification/flint-2026-10-21/recovery.json).

- Source: `9c1128675b`; executable SHA-256:
  `850d5a867678c66303ef5cc6300d2d1f28d768731a3948a9d4c03ab31d3fbc14`.
- All nine CUDA-host tests pass, including the released tokenizer, with no
  skipped tests. CPU checks and the separate released-tokenizer check pass.
- The original-weight FP32 comparison passes all six positions and all
  150,144 logits per position. Top-1 agrees at every position; maximum total
  variation is `1.78e-6` and maximum centered-logit error is `4.52e-5`.
- Greedy arithmetic repeats the same 33 output tokens and returns `4` with
  reasoning separated. Recommended sampled generation also returns `4` and
  stops. The generated Rust `add` function compiles and passes four cases.
- The code answer uses 102 output tokens, takes 4.70 seconds including
  0.86 seconds of prefill, and reports 26.6 output tokens/s after prefill.
  These are short-request measurements.
- A simultaneous request receives 429. Streaming, tools, and an excessive
  output allowance are refused. Killing an active worker disconnects its
  request; systemd starts a new PID, which answers subsequent greedy and
  sampled requests. One worker remains on the GPU. The crash and follow-up
  checks finish in 54.9 seconds.
- Post-request GPU usage is 77,747 MiB, about 75.9 GiB; this is a snapshot,
  not a measured memory peak.

The service is `psionic-flint` on `oa-flint-12419`, project
`openagentsgemini`, zone `asia-northeast1-b`. Its spot VM expires at
**2026-10-21 21:56:08 UTC** and may be preempted earlier. The guard estimates
$6.65 of compute through that expiry and holds a separate $10 reserve for
storage and regional price differences, within the $150 cap. These are
estimates rather than a billing invoice.

To reach this running instance:

```sh
gcloud compute ssh oa-flint-12419 --project openagentsgemini \
  --zone asia-northeast1-b -- -N -L 18080:127.0.0.1:8080
# In another terminal:
curl http://127.0.0.1:18080/healthz
```

The receipt links the raw logits and traces in GCS and pins their archive by
SHA-256. The failed BF16-compute experiment remains in a separate report.
The longest full-checkpoint smoke prompt has 27 tokens. Long contexts, sustained-load
performance, quantized formats, Metal, streaming, tools, and public Pylon
routing need separate work. The native text smoke does not establish
Recursive's published coding benchmark scores.
