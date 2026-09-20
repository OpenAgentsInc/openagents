# LoRA merge precision and loading memory

The assembled Rust model now adds each LoRA update to the original base
weight in fp32, then casts the combined weight once. It decodes one base
shard in host memory and transfers one tensor at a time. Device
synchronization releases temporary Metal buffers between weights. The
loader never materializes a second complete fp32 model on the device.

Non-LoRA tensors are cast directly from their stored dtype. The pointer
head retains its stored dtype, fp32 in the tested bundles; hidden states
are converted to fp32 for the pointer readout. Discovery reports
`lora_merge: fp32-before-cast-v1` separately from the artifact digest.
The lower-level `apply_lora` API cannot recover precision already lost by
its caller; use `DecisionModel::load_with_dtype` for assembled models.

## Inputs and host

Measured on 2026-09-20 on an Apple M5 Max with **128 GiB** of unified
memory, Rust 1.97.1, Candle 0.11.0, PyTorch 2.8.0, Transformers 4.57.6,
and PEFT 0.21.0. These are historical artifacts recovered through their
immutable locks, not the replacement candidate:

- 4B adapter `1a0cb0a0c4ea77e259cd215a3fb85d29edcc499e`, base
  `906bfd4b4dc7f14ee4320094d8b41684abff8539`.
- 8B adapter `6466fbd425be23b9634b19295afa3ea220f5d11a`, base
  `49e3418fbbbca6ecbdf9608b4d22e5a407081db4`; loading memory only.
- Upstream reference code
  [`86db6d924cee68fa9a1319d2c3e6010b9b233d60`](https://github.com/jaredpalmer/kev/blob/86db6d924cee68fa9a1319d2c3e6010b9b233d60/kev/evaluate.py),
  `merge=True`, eager attention. The verified base directories resolve to
  the same immutable Hub cache snapshots the offline reference loads.

The runtime content digest for 4B is
`sha256:2559d7a66cd0f4077d7f47b5460eedaf8211a43b917588638cc2da1f459628f9`.
[Retained measurements](data/merge-precision/) include every probability,
loaded-file hash, loading time, and memory observation. Recorded timings
were collected during correctness work; they are not quiet-host serving
latency measurements.

## Probability differences

The probe reads all 13 committed encoding records: 20 question
distributions, including packed and separate requests and isolation cases.
These are mechanism fixtures, not a held-out accuracy sample. Every row
below compares with upstream's fp32 CPU merged path on those same records.

| Path | Maximum absolute probability difference | Argmax changes |
| --- | ---: | ---: |
| Rust CPU fp32 | 0.00000775 | 0 of 20 |
| Upstream CPU bf16 | 0.019537 | 0 of 20 |
| Upstream MPS bf16 | 0.017014 | 0 of 20 |
| Rust Metal bf16 | 0.013815 | 0 of 20 |

Rust Metal bf16 differs from upstream MPS bf16 by at most **0.019003**, with
zero argmax changes. Correct merge order does not establish identical
low-precision arithmetic across runtimes. Retain fp32 conformance as the
numerical control and evaluate bf16 on each intended workload before
admission. Rust CPU bf16 loading succeeded, but inference returned
`unsupported dtype BF16 for op matmul`; no CPU bf16 parity pass is claimed.

The unchanged historical 4B fp32 golden battery passes all six tests.
The maximum corpus probability difference from those older goldens is
`2.563e-6`; packed versus separate differs by at most `1.341e-6`.
Rounded answers and permutation winners remain unchanged. The 0.5B
release battery also passes, including all four Jev-client HTTP cases.

## Loading memory

`/usr/bin/time -l` measured a separate load-only process for each final
Metal bf16 bundle. Both macOS counters are retained because RSS alone
understates the observed footprint of this Metal process.

| Bundle | Peak RSS | Peak process footprint | Load time |
| --- | ---: | ---: | ---: |
| Historical 4B, Metal bf16 | 9.13 GiB | 22.32 GiB | 18.11 s |
| Historical 8B, Metal bf16 | 11.01 GiB | 43.63 GiB | 38.49 s |

Dropping tensors without synchronizing initially left Metal's pooled
buffers resident; the first 4B load reached a 38.84 GiB footprint.
Synchronization and transferring individual weights reduced that peak.
Live merge temporaries are now bounded by one projection, alongside the
adapter factors, final weights, and one source shard. File hashing and
shard decoding also temporarily hold the source bytes in host memory.
Allocator overhead remains material.

**No 32 GB host was tested.** The 4B result supports further deployment
measurement on that class of host; it does not reserve space for its OS,
other applications, or concurrent inference. The observed 8B footprint
exceeds 32 GiB, so this implementation does not establish upstream's
32 GB deployment claim for 8B.

## Reproduction and verification

Recover the historical bundles with the documented
[artifact acquisition](../artifacts.md) commands. In the repository:

```sh
export CARGO_TARGET_DIR="$PWD/target"
cargo build --locked -p kev --features metal --release --example precision_probe
/usr/bin/time -l target/release/examples/precision_probe \
  "$ARTIFACTS/qwen3-4b" "$ARTIFACTS/kev-4b" \
  crates/kev/fixtures/variants/kev-4b metal bf16 > rust-4b.json
```

Replace the fixture path with `load-only` for the memory measurement.
Use `cpu f32` for the fp32 control. Run the reference from a checkout at
the full upstream commit above, using its Python environment:

```sh
HF_HUB_OFFLINE=1 PYTHONPATH="$UPSTREAM" python \
  crates/kev/fixtures/probe_precision.py \
  --run "$ARTIFACTS/kev-4b" \
  --fixtures crates/kev/fixtures/variants/kev-4b \
  --dtype bf16 --device mps \
  --upstream-revision 86db6d924cee68fa9a1319d2c3e6010b9b233d60 \
  --out reference-4b.json
```

Two synthetic tests distinguish fp32 merging from premature rounding;
one writes a complete tiny base and adapter and checks the loaded weights.
Present but invalid conformance artifacts now fail loading instead of
silently becoming a skipped test.

The pinned manual gate completed with `--skip-postgres`, including both
workspace feature configurations, minimum compilers, and dependency
policy. This is partial coverage: the preceding full run stopped at the
unrelated PostgreSQL gateway socket-reset assertion. After the final
memory refinement, strict Kev Clippy with `serve,metal`, all Kev tests,
the Rust 1.94 library check, the real 4B fp32 conformance battery, and the
Metal probability and load probes passed. Routine tests without supplied
weights still skip their external-model cases. No soak or live Lev run
was performed for this change.
