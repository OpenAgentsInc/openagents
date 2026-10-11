# Reproduce the Mac experiments

Use the repository's Rust 1.97.1 toolchain and a persistent target directory
outside the checkout. Build before measuring. Do not suspend desktop applications,
kill the window server, or change display settings to satisfy an admission check.
If the machine remains busy, retain the refusal or label an exploratory run as
contended.

## Artifacts

The baseline is `1d16475293b1645796a873fc6877ed6d842fd07c`. Apply
[`candidate.patch`](candidate.patch) to that revision to reproduce the candidate.
The two Rust files in this patch are the only changes needed by the candidate
server. The benchmark tools are included with this report's commit.

Use `ggml-org/Clef-Flash-GGUF`, file `Clef-Flash-Q4_K_M.gguf`, with SHA-256
`fd3e90605e8103307dca37cb5a8cdb036267e2fe3cb2d908d80a8ceb9ec0638c`.
Store it in persistent local storage, outside Git. Both variants use the same
file. `environment.json` records the server binary hashes used in this experiment.

In each source checkout, build with:

```sh
export CARGO_TARGET_DIR="$HOME/work/openagents-target-agent1"
openagents lease build --keep-target-dir -- cargo build --release \
  --manifest-path crates/psionic/Cargo.toml \
  -p psionic-serve --bin psionic-openai-server
```

Copy the resulting `release/psionic-openai-server` into a different persistent
scratch filename for each variant before rebuilding. Keep separate target
directories for separate worktrees. Do not replace a binary during a run.

## Paired HTTP comparison

Use `crates/psionic/fixtures/clef/tools/mac_quiet_bench.py` with a JSON config.
The retained configuration gives the exact variants, requests, repetitions,
chunk sizes, and environment overrides. Replace `$SCRATCH` and other path
placeholders in copied evidence with real paths; they are documentation
placeholders, not environment expansion performed by the harness.

For an idle-machine attempt, configure:

```json
{
  "model_path": "/persistent/models/Clef-Flash-Q4_K_M.gguf",
  "output_dir": "/persistent/scratch/new-results",
  "requests": ["requests/len1k.json", "requests/len4k.json", "requests/len16k.json"],
  "rounds": 3,
  "runs": 5,
  "settle_seconds": 600,
  "max_wait_seconds": 1800,
  "max_start_load": 5,
  "max_run_load": 8,
  "max_start_gpu_percent": 10,
  "request_timeout": 120,
  "variants": [
    {"name": "baseline", "binary": "/persistent/bin/baseline-server", "chunk": 2048},
    {"name": "merged", "binary": "/persistent/bin/merged-server", "chunk": 2048},
    {"name": "ollama", "url": "http://127.0.0.1:11434/v1/systemone", "model": "clef-flash"}
  ]
}
```

```sh
openagents lease quiet --receipt /persistent/scratch/lease.json -- \
  caffeinate -i python3 crates/psionic/fixtures/clef/tools/mac_quiet_bench.py config.json
```

The output directory must be empty. The harness retains startup events, every
warmup and measured response, request hashes, paired nonces, token counts, host
samples, binary hashes, and a summary. Workload paths resolve against the config
file. Owned servers bind loopback and run serially. Existing external servers
are never stopped by the harness. Record the external server's version, model
identity, and any uncontrolled settings separately.

A diagnostic desktop run can explicitly set `max_start_gpu_percent` to 100 and
use a shorter settling period. Such a run is automatically marked
`diagnostic_only`; it cannot establish an idle-machine speed gate. A completed
HTTP run also does not establish model quality. Profiling and skipped-kernel
variants receive the same diagnostic classification.

`usable_comparison` identifies completed, unprofiled measurements under the
configured thresholds. It is not an idle-machine acceptance verdict. The current harness summary
explicitly states that GPU interference during inference cannot be detected.

The GPU threshold applies before the run. Total GPU utilization during inference
contains both the model and any desktop work; it does not identify interference
by itself. Inspect the host trace and repeat with no background desktop GPU work before
making an absolute hardware or thermal claim. CPU load is not a power or clock
measurement. The lease excludes cooperating builds, not all applications.

Recompute the retained medians and baseline/candidate agreement with the report's
`summarize.py`, passing `comparison` or `sweep` as its directory argument. It
writes `analysis.json` from `records.jsonl`, `host.jsonl`, and `summary.json`.

## Correctness

The mock harness check uses no model or GPU:

```sh
python3 crates/psionic/fixtures/clef/tools/mac_quiet_bench.py --self-test
```

Compile the Clef unit tests before taking the quiet lease:

```sh
openagents lease build --keep-target-dir -- cargo test --release \
  --manifest-path crates/psionic/Cargo.toml -p psionic-serve --lib \
  clef:: --no-run
```

Run the emitted test executable's `clef::` filter for the unit check. The
opt-in weighted test must run separately with these variables:

```sh
openagents lease quiet -- env \
  RAYON_NUM_THREADS=4 \
  PSIONIC_CLEF_GGUF=/persistent/models/Clef-Flash-Q4_K_M.gguf \
  PSIONIC_CLEF_CUDA_CHECK=1 PSIONIC_CLEF_TEST_DEVICE=metal \
  /persistent/target/release/deps/psionic_serve-TEST_HASH \
  clef::tests::cuda_chunks_and_cpu_agree --exact --nocapture --test-threads=1
```

The test name and enable variable retain their historical CUDA names. The
explicit device selector chooses Metal. The test checks repeated results,
whole-input and 2,048/512/64-token chunks, the separately submitted observer
path, and a CPU reference on a short request.

For the 40-request HTTP correctness corpus, serve the candidate under a quiet
lease, send `crates/psionic/fixtures/clef/e2e/requests.jsonl`, and use
`tools/e2e.py compare` against `e2e/psionic-cpu-q4_k_m.jsonl`. Confirm that all
40 requests returned HTTP 200; the legacy comparison utility can otherwise skip
errors. These are numerical agreement checks on synthetic inputs, not product
answer-accuracy evaluations.

## Synthetic GEMM probe

`metal_sustained_gemm.swift` is a diagnostic tool over the existing Metal kernel,
not product code. Compile it before measuring:

```sh
openagents lease build --keep-target-dir -- swiftc -O \
  crates/psionic/fixtures/clef/tools/metal_sustained_gemm.swift \
  -o /persistent/scratch/metal-sustained-gemm
openagents lease quiet -- /persistent/scratch/metal-sustained-gemm \
  crates/psionic/crates/psionic-backend-metal/src/kernels/clef_prefill.metal
```

It measures 1, 4, 16, and 48 GEMMs per command buffer, using either repeated or
distinct weight buffers, over five rounds with reversed order. Each product is
2,048 by 12,288 by 4,096. All inputs are initialized, and two output samples must
equal 16. Report every sample. GPU command-buffer timestamps can include effects
of scheduling and competing work; they are not clock or power telemetry.

## Final admission refusal

The last staged command would run the GEMM probe, then the 40-request corpus,
under one quiet lease. Its wrapper reused the sweep's admission settings:
30 seconds below load 5, a 300-second wait deadline, a run limit of 8, and
explicit diagnostic GPU admission of 100%. It monitored load while each child
ran, bounded each child to 180 seconds, and stopped only its owned children on
cancellation. Admission timed out, so neither child started. The retained
`final-admission/summary.json` has an empty `steps` list. Its configuration lists
the sweep variants because the wrapper loaded that file; it did not rerun them.
The probe and HTTP corpus remain reproduction instructions, not completed checks.

## Cleanup

Normal completion or SIGTERM stops only the local server process that the
harness created. It never signals desktop applications, system services, or the
external comparator. The quiet lease is released when its command exits. Check
`openagents lease list` and the saved receipt. No watcher or recurring automation
is installed by these experiments.
