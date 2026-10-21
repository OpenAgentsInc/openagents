# Native Flint on GCP

Issues: [#12419](https://github.com/OpenAgentsInc/openagents/issues/12419) and
[#12425](https://github.com/OpenAgentsInc/openagents/issues/12425).
The Apple silicon counterpart, with Recursive's MLX INT4 weights on Metal,
is [Native Flint on Apple silicon](flint-metal.md) (#12422).

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
binary from the GGUF server. Pylon's `serve --text-profile flint` connects
to this native endpoint; it does not use a GGUF decoder.

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
export CARGO_INCREMENTAL=0
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

## Initial GCP results, 2026-10-21

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
`openagentsgemini`, zone `asia-northeast1-b`. The development run originally
expired at **2026-10-21 21:56:08 UTC**, with an estimated $6.65 of compute
and a separate $10 reserve within its $150 budget. The maintained-service
handoff below removed that expiry and settled the development entries.
Spot preemption remains possible. These costs are estimates, not an invoice.

To reach this running instance:

```sh
gcloud compute ssh oa-flint-12419 --project openagentsgemini \
  --zone asia-northeast1-b -- -N -L 18080:127.0.0.1:8080
# In another terminal:
curl http://127.0.0.1:18080/healthz
```

The receipt links the raw logits and traces in GCS and pins their archive by
SHA-256. The failed BF16-compute experiment remains in a separate report.
The initial smoke's longest prompt had 27 tokens. The public CUDA profile's
larger-context, sequential-load, relay, and app checks are recorded below.
The separate Metal work is tracked in #12422. Streaming and tools remain
unsupported. These checks do not establish Recursive's published coding
benchmark scores.

## Pylon and app integration (#12425)

The CUDA profile advertises `flint@sha256:e2e253557466548339ec3ba3d866ef6cd60440dfc895de7717b81f3ba320f58f`.
That hash binds the release manifest, including every checkpoint file's
SHA-256. Pylon checks the worker's revision, manifest hash, CUDA backend,
FP32 computation profile, context, and idle state before taking work.

The initial public profile has one slot, an 8,192-token context, a
6,144-token output allowance, and up to 2,048 prompt tokens after applying
the template. Pylon advertises an 8,192-byte request limit; the router keeps
recent history within it, and the worker checks the final token count.
Oversized prompts fail as request limits. Sampling is temperature 1,
top-p 0.95, top-k 20, and seed 42. A complete answer must stop normally;
truncated output, unfinished reasoning, and tool requests fail the job.

Flint uses `pylon.deadline.v1` with a 900-second deadline and processing
heartbeats. The buyer reads the complete model's ceiling from beacon
metadata. The HTTP client allows the full deadline. A timed-out job keeps
its Pylon slot until the worker finishes or restarts. The worker's
930-second process watchdog handles a stuck generation; systemd's
960-second watchdog also handles a frozen process. Both destroy the old
CUDA process before starting its replacement.

Install both commit-named executables on the GPU host:

```sh
bash scripts/psionic/flint-service.sh COMMIT /path/to/psionic-flint
bash scripts/psionic/flint-pylon-service.sh COMMIT /path/to/pylon
systemctl status psionic-flint pylon-flint
```

The Pylon is named **Flint on GCP**, with slug `flint-gcp`, and publishes
its signed beacon to `wss://relay.openagents.com`. Its local HTTP endpoint
stays on loopback. The homepage's existing live Pylon list draws that beacon.
At the shared terminal, choose **Flint** beside the composer to send `X-OpenAgents-Pylon-Model: flint`.
That request reaches only Flint; an unavailable Flint does not hand it to
hosted chat. Default terminal routing can also choose Flint, under the
extended deadline. Reasoning stays out of the displayed final answer.

The GPU VM's scheduling has no maximum run duration or termination time,
and preemption stops it while preserving its disk. An on-demand CPU VM,
`oa-flint-control-12425` in `us-central1-a`, runs the Rust
`pylon-vm-recovery` service. Every minute it checks the GPU VM and restarts
it only when its numeric instance ID matches, its labels say
`service=psionic-flint,lifecycle=maintained,desired=running`, and it has no
agent-run, budget, or expiry labels or scheduled expiry. It never creates
a replacement VM. This task grants its service account Compute access
on this GPU instance, creates no project-level role grant, and uses
short-lived metadata credentials.

The on-demand RTX PRO 6000 quota is currently zero. Request
`flint-12425-rtx-pro6000-tokyo` for one was denied; preemption recovery
depends on available spot capacity. Current Tokyo list prices
are about $2.225/hour for the GPU VM and $0.0084/hour for its CPU
controller: about $53.60/day before disks and network. On-demand GPU
capacity would cost about $5.85/hour. These ongoing service costs are
separate from the original $150 development budget.

To stop the service deliberately, stop the controller first so no restart
request remains in flight, then change the GPU VM's desired state and stop it:

```sh
gcloud compute instances stop oa-flint-control-12425 --project openagentsgemini --zone us-central1-a
gcloud compute instances add-labels oa-flint-12419 --project openagentsgemini --zone asia-northeast1-b --labels desired=stopped
gcloud compute instances stop oa-flint-12419 --project openagentsgemini --zone asia-northeast1-b
```

To resume, set `desired=running` and start the controller. Disks remain
available for restart; removing them ends their storage charges. Use
`CLOUDSDK_CONFIG=~/work/.secrets/gcloud-sa-config` with these commands.

## Live Pylon qualification, 2026-10-21

[Deployment receipt](verification/flint-pylon-2026-10-21/receipt.json) pins
the executables, evidence files, and raw GCS archive by SHA-256.

The maintained native worker, Pylon, and recovery controller use source
`9d34624102`. The website image uses `4f6cd516e0`, which contains the
composer and router integration. Its digest is
`sha256:0c1f43ae3e17ad7226d614ff15a60b94dd015fd7418053e729baa2f58cd54648`;
production revision `coder-web-0c1f43ae3e-20261021200638` serves it.

The [profile results](verification/flint-pylon-2026-10-21/profile-summary.json)
record 22 full-checkpoint checks. Arithmetic, Spanish, conversation
history, and a generated Rust function pass; the function compiles and
passes four assertions. Retrieval from a 2,039-token prompt passes in
73.01 seconds. A 2,003-token prompt plus 6,144 generated tokens reaches
8,147 total tokens without exceeding the 900-second deadline, in
345.66 seconds. The worker refuses an oversized prompt with
`context_length_exceeded`. Twelve sequential requests pass. A disconnected
caller keeps its generation slot: a second HTTP request receives 429,
then a fresh request succeeds after the original generation ends.
Maximum sampled process memory across this run is 79,018 MiB.

The [public relay check](verification/flint-pylon-2026-10-21/relay.json)
returns `42` from **Flint on GCP**, pinned to the CUDA artifact. Its long
request lasts 328.33 seconds, with 10 processing heartbeats and four relay
connections. It reaches the output cap and returns `limit_exceeded`, with
no partial answer delivered. Its advertised and admitted deadline is
900 seconds.

The [freeze check](verification/flint-pylon-2026-10-21/watchdog.json)
temporarily shortens systemd's watchdog to 15 seconds, starts a Pylon
generation, and sends `SIGSTOP` to the native process. Systemd kills the
frozen process before starting a replacement. Readiness returns in
62.95 seconds; sampling observes at most one GPU process. The interrupted
job fails visibly, a competing job receives `rate_limited: no free slot`,
and a follow-up returns `42`. The normal 960-second watchdog is restored.

The [VM recovery check](verification/flint-pylon-2026-10-21/vm-recovery.json)
stops the GPU VM with `desired=stopped`. The controller leaves it stopped
for 75 seconds. After `desired=running`, the same numeric instance ID is
running again in 30.21 seconds. Both enabled services recover without an
SSH start command; native readiness is observed at 249.34 seconds after
the restart request, including checkpoint verification and loading. A
fresh pinned relay request returns `42`. Both VMs have no scheduled expiry
or temporary-run ownership labels in the
[maintained settings](verification/flint-pylon-2026-10-21/vm-settings.json).

The [CUDA test log](verification/flint-pylon-2026-10-21/cuda-tests.log)
passes nine library tests and the server watchdog test, with no ignored
tests. Focused local checks pass: Pylon's library (161 passed, one ignored),
two Flint relay fixtures, nine serial-recovery tests, 13 web router tests,
eight composer tests, and both Pylons view crates. The composer and Pylons
layer also pass WASM checks. An initial concurrent serial fixture missed
its one-second startup limit; its complete serial rerun passes.

The older staging smoke reports 80 passed, 13 failed, and two skipped:
the failures inspect the previous homepage/composer layout. They are
recorded as failures, not a passing deployment gate. The targeted browser
and [public route checks](verification/flint-pylon-2026-10-21/public-routes.json)
exercise the Grid at `/`, chat at `/chat`, and the `/home` and `/new`
redirects. This qualification does not cover broad coding benchmarks,
streaming, tools, or the separate INT4 Metal profile.

The [production browser check](verification/flint-pylon-2026-10-21/browser-chat.json)
walks to the shared terminal and clicks **Flint**. The real composer sends
`X-OpenAgents-Pylon-Model: flint` and the Home surface header. Its first
answer is `42`; a follow-up asking to subtract five returns `37`, with
both answers attributed to **Flint on GCP · Flint**. The saved conversation
and rendered [first answer](verification/flint-pylon-2026-10-21/production-first-answer.png)
and [follow-up](verification/flint-pylon-2026-10-21/production-followup.png)
agree. Reasoning is absent from the displayed final content.

The isolated browser uses the declared dedicated account's app API key,
attached through DevTools only to same-origin chat/history requests,
including the unchanged EventSource stream. This checks the composer,
router, native generation, saved conversation, and live updates. It does
not test WorkOS login. An earlier fixture authenticated the POST but
missed the event stream; that setup was corrected and the test repeated.
[Cleanup](verification/flint-pylon-2026-10-21/cleanup.json) deletes both
disposable conversations with HTTP 204. No owner account is used.

A fresh visit to [the production homepage](https://openagents.com/)
shows **Flint on GCP**, **Online · GPU · 0 of 1 busy**, in its
[live Pylon list](verification/flint-pylon-2026-10-21/homepage.json) and
[Grid overview](verification/flint-pylon-2026-10-21/production-homepage.png).
The service remains running with the temporary expiry removed.
