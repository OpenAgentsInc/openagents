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

## Verification and pending serving comparison

Portable tests verify rounded memory costs, invalid backend/bucket choices,
and HTTP refusal before a deliberately broken embedding lookup. Packed,
separate, and raw-predict routes all refuse a request that fits the raw
100-token bound but would pad to 128. Existing busy, variant-slot, and
working-memory refusal tests also pass.

The pinned manual gate with `--skip-postgres --with-metal` passed formatting,
strict Clippy, default and runtime-feature tests, minimum compiler checks,
dependency policy, and artifact-acquisition regressions. PostgreSQL and the
long-running relay soak were explicitly skipped. Model-free gate results
do not substitute for the real-weight run above.

A quiet-host HTTP comparison, including load/serving memory, warm-up, and
p50/p95 for all four attention/padding combinations, is pending while the
other task performs Lev measurements. Until it completes, the supported
conclusion is the negative bf16 parity result and retention of the eager
exact default, not a production latency claim.

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
BASE ADAPTER FIXTURES bf16` to regenerate the diagnostic probe. Use
`scripts/benchmark-kev-http.py` against a fresh server for serving latency;
do not use stage-synchronized profiling times as HTTP latency.
