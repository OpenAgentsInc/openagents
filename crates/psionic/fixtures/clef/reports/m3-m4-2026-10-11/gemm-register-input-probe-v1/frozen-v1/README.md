# Cooperative input projection probe

This is one source-only experiment. It has not been compiled or run. It changes no product files and loads only three existing Flash projection matrices, not a complete model.

## Fixed candidate

Four independent SIMD groups form a 64×64 output tile. Each group owns a 32×32 tile, represented by two 16×32 f32 cooperative accumulators. A strict 16×32×16 NT primitive consumes cooperative half inputs. Each K32 step loads four 16×16 X fragments and two 16×32 weight fragments. The two weight fragments each serve both output row halves. Outputs visit K in increasing order; a nonzero residual is loaded once before multiplication. Every group reaches the scheduling barrier after each K128 block, including inactive output groups.

The previous rejected independent-SIMD probe passed direct device operands to a 32×32×128 operation. This candidate explicitly loads and reuses input fragments. It keeps that probe's four-group arrangement and K128 synchronization cadence. Compared with production, it also changes the combined output tile from 64×128 to 64×64 and divides the full reduction into smaller operations. It is not an isolated test of SIMD count, precision, or one compiler flag.

This is an independent implementation informed by the public [MLX source at a pinned commit](https://github.com/ml-explore/mlx/tree/06eb7483f03bca49a80e596f4089d02a507a2cf9) under the MIT license. It uses public Metal cooperative coordinate, validity, load, and store APIs instead of MLX's lane mapping. The source review identified in manifest.json records the references. No private code or Apple guide text is copied. Relaxed precision remains false; the specification's float-operand truncation option is not a claimed speed mechanism for these half inputs.

## Checks fixed before execution

[The exact table](small-cases.json) contains 12 shapes with K=1,15,16,17,31,32,33,127,128,129,4096,12288. Each runs in overwrite and nonzero-residual mode: 24 small projection checks. This supersedes the preliminary plan's inconsistent promise of 18 checks covering both modes for all 12 K values. These are single projection checks, not complete small FFNs. They include one-hot, zero, nonsymmetric, correlated, cancellation and mixed-range inputs, plus row/column/K tails. Every output is finite, within absolute 0.001 of production, and within absolute 0.001 of an independent f64 dot product.

Before any GEMM, 12 exact input round trips exercise the same load and residual helpers. Each lane exports its valid slots, public coordinates and bits into separate guarded records. The host requires unique complete coordinate coverage for all eight fragments, unchanged record holes, exact input and residual bits, exact zero padding, and unchanged inputs/redzones. This checks both interior collective loads and tail coordinate loads without assuming the SDK's lane ownership. A capacity above the fixed 64-slot export bound is refused.

Five complete FFNs use unchanged real Flash half weights: 65 tokens in both modes, plus 512/1024/2048 in accumulate mode. Every gate/up/half-activation/down value is scanned. F32 output differences must be at most 0.001; half activations must be finite and satisfy absolute difference at most 0.001 OR at most one Float16 ULP. Signed zeros have distance zero. Both errors are recorded. Gate/up/down also have 96 fixed f64 samples per projection. This is not a full weighted f64 comparison. Down references use each variant's actual half activation.

All these checks precede timing. Each timed size then computes and fully checks both complete outputs and a separate candidate down output using the common baseline half activation. Expectations are copied into owned immutable CPU Data; GPU workspaces are released. Each timed output must match its own expectation byte for byte. Final hashes check all expectation bytes, inputs and weights. Full FFNs use their own activations; standalone down comparisons share the baseline activation.

The half-intermediate rule was already fixed before the earlier independent-SIMD experiment and is unchanged here. A different f32 reduction can straddle a half rounding boundary larger than 0.001 above magnitude 2. The absolute branch preserves the existing near-zero allowance. No f32, f64, or full-model raw-logit threshold changes; full-model raw logits would still require 1e-3 and all top answers before integration.

## Fixed timing and bounds

Five ABBA rounds at each of 512/1024/2048 tokens cover gate, up, down-accumulate and complete FFN: 240 timing rows. Check mode has 70 command buffers and 100 dispatches; bench mode has 319 and 547. Summaries require exact unique coverage, positive finite GPU/wall times, every numerical and repeat check, and successful owned-process cleanup. Material improvement requires a paired median candidate/baseline ratio at most 0.90 for both complete FFN and the sum of projections at every size. Temporal drift and cache effects remain possible on the shared machine. This is not model latency evidence.

The GPU process cap is 2 GiB, with 14 GiB available before launch, a 12 GiB host floor, a 60-second external deadline and a 50-second internal deadline. Compilation uses 4 GiB, 16 GiB start, the same floor, 180 seconds and two compiler threads. There are no automatic retries, cap increases, group-count variants or quiet leases. The guard controls only its newly launched process group.

At 2048 tokens, three weights and X use 318,769,152 guarded bytes. One FFN workspace uses 285,214,720 bytes; the common half activation uses 50,332,160 bytes. Two complete owned CPU expectations plus the separate down expectation use 603,979,776 bytes. Their largest overlap totals 1,258,295,808 bytes, including buffer redzones. Only one GPU workspace is live at a time. Metal allocations remain below 1 GiB, and Metal `currentAllocatedSize` plus owned CPU expectations and temporary mapped assets must stay below 1.5 GiB. The 100,663,296-byte weight loading temporary ends before expectations exist. Every allocation and completed command checks these bounds. Driver overhead must fit the same bounds.

Logical cooperative storage per SIMD group is 2,048 half values and 1,024 f32 values, or 8 KiB: an average 256 bytes per thread. That is a logical payload, not a register-allocation measurement. Capacity padding, compiler temporaries or spills can increase storage. There is no declared shared-memory scratch or additional device kernel workspace.

## Files and reproduction

`candidate.metal` is the new shader. The baseline and production extracts are unchanged. `probe.swift` owns validation and dispatch; `guard.py` enforces owned-process bounds. `build.py` records source and executable hashes. `source_check.py` checks source identity, Python syntax, case coverage, indexing/count/memory arithmetic and fragment coverage without compiling or reading weight payloads. `summarize.py` refuses incomplete output. `weights.json` pins existing half assets in `../gemm-tile-probe`; do not publish those assets.

After source review and explicit scheduling authorization, run `python3 guard.py build --output build-1` under build+memory leases, then `python3 guard.py bench --output run-1` under GPU+memory leases. Run `python3 summarize.py run-1` afterward. Preserve every failed receipt. No benchmark is authorized by this document alone.
