# Kev release review and integration priorities

Reviewed on 2026-09-20 against OpenAgents `b61fc0f496`, upstream Kev
[`86db6d9`](https://github.com/jaredpalmer/kev/tree/86db6d924cee68fa9a1319d2c3e6010b9b233d60),
and the immutable Hub revisions linked in [the current model
table](model-cards.md#current-upstream-checkpoints-reviewed-2026-09-20).
This review examines source and artifact metadata. It does not benchmark
the replacement checkpoints.

**Start with the new 4B checkpoint as an evaluation candidate.** The Rust
port already implements the Qwen3 family. The missing work is reproducible
checkpoint updates, measurements on Coder's current questions, and serving
improvements. The existing results do not establish how the new weights
perform here.

## What changed upstream

The announcement's 79.6% for Kev-8B and 85.7% for Jev are development
results on upstream's transfer suite. They are not OpenAgents scores or
locked-test results. The current 4B and 8B cards report locked transfer
accuracy of 80.6% and 78.0%, respectively. The development ranking alone
does not establish an 8B advantage. [The model tables](model-cards.md)
separate these results from the older checkpoints measured here.

Upstream's serving benchmark uses a 4B model in bf16 on an M5, five
three-option questions, and reports median model time:

| State | Before | Optimized |
| --- | --- | --- |
| New short state, 232 tokens | 442 ms | 296 ms |
| New long state, 772 tokens | 1,290 ms | 861 ms |
| Repeated long state | 1,214 ms | 242 ms |

The same README reports about 40 ms on an H100. These figures describe
upstream PyTorch, request shapes, and timing boundaries; they are not Rust
HTTP latency. The repeated-state row combines optimizations and is not a
measurement of the cache alone. Source:
[upstream serving benchmark](https://github.com/jaredpalmer/kev/blob/86db6d924cee68fa9a1319d2c3e6010b9b233d60/README.md#serving-performance).

## Findings in this repository

### Artifact downloads no longer reproduce the fixture set

**Resolved in [#9454](https://github.com/OpenAgentsInc/openagents/issues/9454):**
the downloader now uses immutable artifact locks and checks raw inputs,
converted heads, and base files. [Artifact acquisition](artifacts.md)
records the verified historical recovery path. The observations below
describe the pre-fix behavior.

[`fetch-kev-artifacts.sh`](../../scripts/fetch-kev-artifacts.sh) downloads
adapter files from Hub `main`, while the fixture manifests pin earlier
contents. All three Qwen3 adapters now differ. The Hub's LFS SHA-256
metadata, compared with the committed manifests, establishes the change
without downloading weights:

| Variant | Fixture adapter SHA-256 prefix | Current Hub adapter SHA-256 prefix | Historical Hub revision with matching adapter and config |
| --- | --- | --- | --- |
| `kev-0.6b` | `6e95ccb14926` | `deaab63b61f9` | [`30902c2`](https://huggingface.co/jaredpalmer/kev-0.6b/tree/30902c2bbc113145cc150199b2b999f64ef7f0b8) |
| `kev-4b` | `c3101a8a563c` | `3fa10d8646f5` | [`1a0cb0a`](https://huggingface.co/jaredpalmer/kev-4b/tree/1a0cb0a0c4ea77e259cd215a3fb85d29edcc499e) |
| `kev-8b` | `9de59714a572` | `fee56a07b92a` | [`6466fbd`](https://huggingface.co/jaredpalmer/kev-8b/tree/6466fbd425be23b9634b19295afa3ea220f5d11a) |

Only the adapter and `adapter_config.json` were checked at those historical
revisions; this is not verification of each complete bundle. The original
0.5B adapter still matches current Hub weights, but that alone does not
verify its configuration or converted head.

On a fresh Qwen3 download, the script reaches a digest mismatch. With
existing nonempty files, its fetch helper skips the download and can keep
the historical artifacts. These two machines therefore do different work
with the same command. The final digest check prevents a silent fixture
update, but it happens after head conversion and base downloads.

Model identity also needs attention before an update.
[`kev_serve.rs`](../../crates/kev/src/bin/kev_serve.rs) derives the default
model ID from the adapter directory; `load_variant` reads the base revision
from head metadata. [`/v1/models`](../../crates/kev/src/serve.rs)
publishes that revision and the run path, but no adapter/head content
digest. Replacing an adapter in place can preserve both its model ID and
base signature. A checkpoint name alone cannot identify a measurement.

### The bf16 merge differs from upstream

[`DecisionModel::load_with_dtype`](../../crates/kev/src/decision.rs)
loads the backbone in the requested dtype, then applies LoRA.
[`lora::merge`](../../crates/kev/src/lora.rs) computes the delta in fp32,
casts it to the backbone dtype, and adds it to the already-cast base.
Upstream's
[`evaluate.load`](https://github.com/jaredpalmer/kev/blob/86db6d924cee68fa9a1319d2c3e6010b9b233d60/kev/evaluate.py)
adds the adapter to the base in fp32 before casting the combined weights.
These operations can round differently. Existing fp32 fixture conformance
does not measure that difference in bf16.

This is a precision follow-up, not a missing LoRA-merge optimization: our
runtime already merges the adapter at load time.

### Repeated state is recomputed

[`DecisionModel::probs`](../../crates/kev/src/decision.rs) builds a full
mask and runs the complete packed sequence for every request.
[`Attention::forward`](../../crates/kev/src/model.rs) materializes attention
scores with matmul, softmax, and another matmul. There is no state KV cache,
fused attention path, or sequence-length bucketing.

Upstream now has all three. Its
[`serve.py`](https://github.com/jaredpalmer/kev/blob/86db6d924cee68fa9a1319d2c3e6010b9b233d60/kev/serve.py)
defaults to four cached states and only considers states of at least 384
tokens eligible. Reuse requires identical encoded state, not merely a
shared document inside a changed state object. Coder's changing transcript
may limit hits; repeated questions over a fixed document are a clearer
candidate. Measure the hit rate before promising the announcement's gain.

### A shared API does not make the doors interchangeable

- **Jev:** [`crates/jev`](../../crates/jev/README.md) already supports an
  explicit base URL and model. The Rust Kev server advertises `jev-latest`
  as an alias for its default variant. Set an explicit model when comparing
  checkpoints; `kev-latest` chooses the largest loaded variant in bundle
  mode unless the operator selects a default.
- **Kev:** its HTTP validator accepts up to 255 Score levels, while the
  Jev client, Lev, and [TypeSafe's documented API](https://docs.typesafe.ai/api)
  limit Score to 2–10. Use that common range. Kev's Score confidence is a
  stand-in formula, so an identical response shape does not establish
  identical confidence semantics.
- **Lev:** [`estimator.rs`](../../crates/lev/src/estimator.rs) estimates a
  distribution from sampled choices or a certainty band; it cannot read
  Apple's logits. Its [admission rules](../lev/disposition.md) remain
  specific to the estimator, question family, and calibration record.
  The Kev update supplies no new evidence about Lev's admitted workloads.

## Recommended work, in order

1. **Make checkpoint identity reproducible.** Add immutable Hub revisions
   to the artifact manifest and downloader, check downloaded inputs before
   head conversion, and verify base files as well as adapter/head files.
   Preserve the historical fixtures. Put new candidates in separate
   artifact directories and generate new fixtures from pinned upstream
   code. Carry an adapter/head digest into model discovery and measurement
   identity so old and new rows cannot collapse into one checkpoint.
2. **Evaluate the new 4B before changing Coder's default.** Run the complete
   fixture and HTTP round-trip battery with real weights, then measure
   `coder-turns-v2` (`action` and `shell_outcome`), support, external labels,
   and program selection. Retain the v1 program-selection baseline; build
   a separately digested suite for the existing v2 question before claiming
   current program coverage. Compare against hosted Jev's recorded rows
   where inputs match and against constant answers. Report misses,
   spurious program selections, refusals, calibration, and p50/p95 latency.
   Reserve locked items for a declared candidate. Include 8B as a control
   if 4B leaves a material quality gap.
3. **Align bf16 merging, then measure optimized attention.** Preserve the
   original base precision through the fp32 merge and cast once. Bound
   peak load memory so the implementation remains usable on a 32 GB Mac.
   Test a rounding-sensitive merge case, probability drift, and argmax
   changes on real fixtures. Investigate a Metal attention path that
   honors the arbitrary branch mask and position IDs; retain the eager
   path as the reference and update memory admission estimates with any
   new allocation pattern. Benchmark shape bucketing separately.
4. **Add a bounded state cache if reuse pays.** Scope entries to the exact
   model artifacts, tokenizer, dtype, device, encoding mode, and state
   tokens. Bound bytes as well as entry count and account for retained KV
   memory in admission. Test cache hits against uncached answers with
   changed question sets, sibling isolation, eviction, and concurrent
   requests. Measure misses and short states as well as long-state hits.
5. **Fine-tune only after the workload baseline.** The current 4B/8B
   records use lr `5e-5`; the new 0.6B uses `1e-4`. Preserve a general
   transfer suite alongside task labels, and compare each candidate with
   the untuned checkpoint. Use calibration data separately from development
   and locked data. Upstream's low training cost makes an experiment
   plausible; it does not establish a budget or acceptance result here.
   Keep exact date calculations and known rules in Rust; use the model for
   judgments that need language understanding.

Checkpoint identity, the bf16 merge correction, and the current
program-selection suite are prerequisites for the new 4B evaluation.
Attention and cache experiments follow that baseline. The existing Rust
client and Gym provide the integration and evaluation path.

## GitHub follow-up issues

Each issue records the evidence, dependencies, and acceptance criteria.
The performance and fine-tuning issues require measurements before an
implementation or training decision.

| Issue | Scope | Prerequisites |
| --- | --- | --- |
| [#9454](https://github.com/OpenAgentsInc/openagents/issues/9454) | Pin artifact downloads and verify inputs before conversion. | None. |
| [#9455](https://github.com/OpenAgentsInc/openagents/issues/9455) | Carry checkpoint content identity through discovery and Gym. | #9454. |
| [#9456](https://github.com/OpenAgentsInc/openagents/issues/9456) | Merge LoRA in fp32 before casting combined weights. | Implement independently; use #9454 and #9455 for measurements. |
| [#9457](https://github.com/OpenAgentsInc/openagents/issues/9457) | Freeze and score the current program-selection question. | None. |
| [#9458](https://github.com/OpenAgentsInc/openagents/issues/9458) | Evaluate the replacement 4B on current Coder workloads. | #9454–#9457. |
| [#9459](https://github.com/OpenAgentsInc/openagents/issues/9459) | Measure Metal attention and shape bucketing. | #9458 baseline. |
| [#9460](https://github.com/OpenAgentsInc/openagents/issues/9460) | Measure exact-state reuse and add a bounded cache if useful. | #9458 baseline; coordinate with #9459. |
| [#9461](https://github.com/OpenAgentsInc/openagents/issues/9461) | Decide whether a Coder-specific fine-tune is justified. | #9458 results. |
| [#9462](https://github.com/OpenAgentsInc/openagents/issues/9462) | Stabilize the Coderbench timeout fixture found during verification. | Independent of the model upgrade. |

## Review verification

All 139 local documentation links checked resolved, including nine heading
anchors. The new Jev client example compiled and constructed a client
without a network request. The current development-metric table was checked
against the three pinned upstream result files. `git diff --check` passed.

The manual gate ran with `CARGO_TARGET_DIR` set to this checkout's `target`
and `KEV_VARIANT=kev-0.5b` to bound the optional weight-backed tests.
Formatting, both Clippy configurations, and the default-feature workspace
tests passed, including 0.5B conformance. The runtime-feature test run
stopped at Coderbench's `a_timed_out_run_is_not_a_clean_run`: its one-second
fixture run produced no trace and returned 3 instead of the expected 1.
That case passed when rerun alone with the same feature selection. The
remaining gate stages were not reached, so this is not a full gate pass.
The replacement checkpoints were not evaluated.
