# Metal attention and sequence padding

The experimental Metal SDPA path passes fp32 conformance, but bf16 changes
one fixture's winning answer. Eager attention with exact sequence lengths
remains the default. Padding to 64 tokens is a separate option; neither
option is admitted by the prior workload rows or their calibration records.

## Implementation

`kev-serve --attention eager|sdpa --bucket-size 0|64` selects the two
independent experiments. SDPA requires Metal and a supported head width.
It receives Q, K, and V with their actual grouped-query head counts, the
full additive block-causal mask, and the existing per-branch rotary
positions. `do_causal` is false: an ordinary triangular mask would expose
sibling questions. The per-head mask is a broadcast view with zero stride,
not a materialized copy. Single-token library calls are refused because
Candle's vector kernel does not consume this mask.

Padding appends masked tokens after every real token, leaves all original
positions and pointer indices intact, and never adds options or reported
input tokens. All three forward routes admit the rounded allocation
length before building the mask. The per-variant memory estimate also
rounds the length. It retains the conservative eager score-tensor reserve
for SDPA; fewer kernel allocations do not silently increase concurrency.
There is no state cache in either path.

Discovery reports attention and bucket size separately from the artifact
digest. Gym therefore distinguishes the numerical execution choices and
cannot apply an eager calibration record to the SDPA or padded experiment.
The explicit zero bucket is also recorded; older records without that
field keep their historical identity and require a fresh match.

## Checkpoint and controls

All experiments use the `kev-4b-c4bfa11` bundle and the 128 GiB M5 Max from
[the workload evaluation](2026-09-20-candidate-4b.md). Content identity is
`sha256:5622bbba5cae1d35c4572ea382f76cbc4c4c63ef18452c916d80cf9e25a08991`.
The pointer head stays fp32; LoRA merges in fp32 before the compute cast.

The real-weight fp32 Metal run with SDPA and 64-token padding passed all
16 API, encoding, and conformance tests, including all golden probabilities,
packed/separate, sibling isolation, permutation, and forged delimiters.
The maximum probability difference from upstream CPU fp32 was `2.563e-6`;
packed/separate differed by at most `1.609e-6`. No fixture was regenerated
to make this experiment pass.

`attention_probe` additionally runs 13 fixture encodings and four benchmark
shapes under each attention/padding combination, using one loaded bf16
checkpoint. It checks that intrusive profiling does not alter probabilities.
Those results are retained in
[`data/metal-attention/`](data/metal-attention/).

| bf16 execution | Largest probability difference from eager exact | Argmax changes | Packed/separate delta | Sibling/absent delta |
| --- | ---: | ---: | ---: | ---: |
| Eager, exact | 0 | 0 | 0.005109 | 0.002393 |
| Eager, 64-token padding | 0.008227 | 0 | 0.006094 | 0.000742 |
| SDPA, exact | 0.035482 | 1 | 0.001931 | 0 |
| SDPA, 64-token padding | 0.035482 | 1 | 0.001931 | 0 |

The changed argmax is the near-tied Unicode request. Upstream fp32 assigns
its first and third options 0.48877 and 0.47863; eager bf16 assigns 0.50044
and 0.46703; SDPA bf16 assigns 0.46496 and 0.50192. This is a numerical
execution change, not evidence that either answer is a better language
judgment. Fp32 agreement does not establish bf16 decision parity.

The nonzero bf16 packed/sibling deltas already occur in eager execution as
shape changes alter rounded arithmetic. The exact mask tests and fp32
controls remain necessary alongside these measured bf16 differences.

## Where time goes

`DecisionModel::profile_probs` synchronizes the device between stages.
Its `submit_ms` is host time before the explicit synchronization;
`completed_ms` includes the wait and is not a GPU-only timer. Synchronization
changes scheduling and allocation lifetimes, so these timings locate costs
and cannot replace steady-state HTTP measurements.

The first profile ran while another task used the host's Apple inference
runtime. On the 723-token, five-question shape, eager's completed attention
kernel time was 408.8 ms across 36 layers; SDPA's was 63.9 ms. MLP and
residual work remained 679.9 and 641.6 ms, respectively, and rotary plus
contiguous copies remained 266.9 and 251.3 ms. Mask construction and upload
were about 1.5 ms. The kernel removes one substantial cost; projection,
MLP, and dispatch costs remain. These contended profiles do not establish
an end-to-end speedup.

In the same diagnostic sweep, padding often increased short-request time.
It does extra work to reach the next multiple of 64, and it also changed
bf16 probabilities by up to 0.00823. Upstream's bucketing result is not
assumed to transfer to this Candle runtime.

## Verification and the serving comparison

Portable tests verify rounded memory costs, invalid backend/bucket choices,
and HTTP refusal before a deliberately broken embedding lookup. Packed,
separate, and raw-predict routes all refuse a request that fits the raw
100-token bound but would pad to 128. Existing busy, variant-slot, and
working-memory refusal tests also pass.

The pinned manual gate with `--skip-postgres --with-metal` passed formatting,
strict Clippy, default and runtime-feature tests, minimum compiler checks,
dependency policy, and artifact-acquisition regressions. PostgreSQL and the
long-running relay soak were explicitly skipped. Model-free gate results
do not substitute for the real-weight run above. The complete gate output
is retained as `data/metal-attention/manual-gate.txt`.

## Quiet-host serving matrix

The HTTP matrix runs `scripts/benchmark-kev-http.py` against a fresh server
per configuration: twenty measured serial requests per case after three
warm-ups, on the same pinned bundle, Metal device, bf16 compute, 4 GiB
memory budget, and 4096-token bound. An early SDPA attempt stopped when
its monitor detected a build; that attempt is excluded and retained as
`data/metal-attention/http-sdpa-0-interrupted.*`. Both SDPA configurations
were re-measured on 2026-09-21 with no build or measurement client on the
host. The server reports the pinned bundle as `kev-latest`; the content
digest `sha256:5622bbba…` matches the checkpoint section. Host and process
captures sit beside each result as `http-*-quiet-host.json` and
`http-*-quiet-process.txt`.

| Case, p50 (p95) ms | Eager, exact | SDPA, exact | Eager, 64-token | SDPA, 64-token |
| --- | ---: | ---: | ---: | ---: |
| short, 1 question, repeated | 82.0 (83.8) | 80.9 (86.9) | 84.7 (89.6) | 79.6 (82.7) |
| short, 1 question, new | 84.6 (85.8) | 87.8 (89.1) | 91.2 (97.0) | 90.1 (95.1) |
| short, 5 questions, repeated | 185.5 (187.1) | 203.4 (218.4) | 223.2 (243.6) | 232.6 (247.8) |
| short, 5 questions, new | 199.8 (201.0) | 209.7 (218.0) | 220.9 (228.5) | 229.9 (234.8) |
| long, 1 question, repeated | 613.8 (614.4) | 620.4 (700.8) | 730.3 (774.3) | 684.8 (806.0) |
| long, 1 question, new | 671.2 (722.2) | 633.8 (649.9) | 764.1 (798.0) | 701.4 (722.6) |
| long, 5 questions, repeated | 862.0 (928.6) | 757.3 (800.3) | 951.6 (1010.0) | 859.1 (881.9) |
| long, 5 questions, new | 880.6 (935.0) | 788.6 (820.5) | 953.7 (1001.8) | 878.4 (919.9) |

SDPA's kernel-time advantage reaches HTTP latency only at the long states.
Long five-question requests improve 10–13 percent at p50 under either
bucket policy; long one-question requests improve 6–8 percent under
padding and are at parity without it. Short requests are at parity or
slightly slower — attention is a minority of request time at these
shapes, so the 6.4× attention-kernel ratio does not transfer end to end.
Peak memory is unchanged within noise: 9.1–9.8 GiB maximum resident set
and 23.8–24.4 GB peak footprint across all four configurations. Warm-up
differs once: the first request under the first SDPA server cost 1661 ms —
consistent with a one-time Metal pipeline build — while the second SDPA
server's first request cost 82 ms, so the cost did not repeat across
processes. Every eager configuration's first request cost 82–91 ms.

Padding still costs time on this workload — every padded row is slower
than its exact counterpart under both backends — and it retains the bf16
probability drift the table above records. The measured outcome is a
modest positive, not a default change: SDPA buys roughly a tenth off
long-state latency and carries one changed argmax on a near-tied
fixture. The eager exact default remains unchanged; whether the
long-state gain justifies that numerical difference is a serving-policy
decision this record informs rather than makes.

Reproduce the correctness run:

```sh
KEV_VARIANT=kev-4b-c4bfa11 \
KEV_ARTIFACT_DIR=/path/to/artifacts/kev-4b-c4bfa11 \
KEV_BASE_DIR=/path/to/artifacts/qwen3-4b \
KEV_TEST_DEVICE=metal KEV_TEST_ATTENTION=sdpa KEV_TEST_BUCKET=64 \
RUST_TEST_THREADS=1 cargo test --release --locked -p kev \
  --features serve,metal --test conformance --test encode --test api -- --nocapture
```

Run `cargo run --release -p kev --features metal --example attention_probe --
BASE ADAPTER FIXTURES bf16` to regenerate the diagnostic probe. Reproduce
one serving row with a fresh server and the benchmark script:

```sh
target/release/kev-serve --adapter-dir ADAPTER --base-dir BASE \
  --port 18454 --device metal --dtype bf16 \
  --attention sdpa --bucket-size 0 --memory-budget-mib 4096 --max-tokens 4096 &
python3 scripts/benchmark-kev-http.py --url http://127.0.0.1:18454 \
  --model kev-latest --repeats 20 --output http-sdpa-0-quiet.json
```

Use the script's request timing for serving latency; do not use
stage-synchronized profiling times as HTTP latency.
