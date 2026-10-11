# One interleaved gate/up product

This source-only probe combines each pair of 128 gate/up output columns into
one 64×256 full-K TensorOps product with four SIMD groups. It is not built or
run. It changes no product code. The previous pair kernel issued two separate
matmuls; this candidate instead gives one matmul both weight blocks. Input
reuse within that operation is a hypothesis, not a measured hardware fact.

The original and candidate paths both write f32 projection values. A separate
SiLU kernel uses the identical expression and rounds its result to half once.
The down projection, including its f32 residual accumulation, uses the exact
current production weight-shape selector and unchanged kernel in both paths.
No new K partition, input quantization, precision relaxation or variant sweep
is included. The smaller synthetic baseline also follows the production
selector, including its fixed-64 fallback.

## Row mapping and tests

For logical FFN column c, the packed gate row is
`256 * floor(c / 128) + (c % 128)`; its up row is 128 rows later.
The packed matrix has `256 * ceil(F / 128)` rows. Every real half weight is
copied unchanged, and all padded rows contain positive zero. The host checks
every packed row against its original bytes before any candidate dispatch.
The kernel computes all padded columns, which must become finite zero, and
the SiLU writes only the logical F columns. The candidate has the same parent
K stride and dynamic full-K reduction as the baseline; static slices are used
only for complete token tiles.

Nine small shapes run both overwrite and nonzero-residual FFNs. They cover
query/output/K tails, K=4096/12288, zero and nonsymmetric inputs, correlated
inputs, cancellation, and mixed ranges. Every logical gate/up/down output
gets an independent f64 reference. Five actual Flash layer-zero weight cases
use complete output scans plus 96 fixed f64 samples per projection. Down's
oracle uses each variant's actual half activation. All source buffers,
redzones, output padding and immutable expectations are checked.

The f32 gate/up/down limit is absolute 0.001. Half activation requires finite
values and absolute difference at most 0.001 OR at most one half ULP, with
signed zeros at the same ordered point. This is the same criterion fixed
before the preceding independent-SIMD experiment, not a post-result change.
Absolute differences, ULP counts and per-element violations are all retained.
A full-model raw-logit limit of 0.001 remains a separate required gate.

## Timing and memory

All initial correctness checks precede timing. Each timed operation must
match its own previously finite-checked output bytes exactly. Immutable
expectations are owned CPU Data copies, never borrowed Metal storage. Five
fixed ABBA rounds cover 512/1024/2048 tokens and gate+up, SiLU, accumulated
down, and complete FFN scopes. Standalone down uses the same baseline half
activation and kernel for both labels; it is a temporal/control measurement.
SiLU uses each variant's own projection layout and values. Every intermediate
of a timed full FFN is compared. A material result requires at least 10%
paired improvement in complete FFN and projection sum at all three sizes.
The shared-machine timing does not establish model latency or a hardware cause.

Packing occurs once outside GPU timing and its CPU time is reported. The
extra packed half weights occupy 192 MiB for Flash. At 2048 tokens, all four
weight buffers use 480 MiB, input uses 16 MiB, one state uses 272 MiB, common
down input uses 48 MiB, and two owned expected states use 544 MiB. The total
payload is 1360 MiB plus redzones and Metal allocation overhead. During
expectation capture, a completed baseline workspace is freed before the
candidate workspace is allocated. Timing reuses one state because the two
real layouts have identical sizes. Mapped weight-read temporaries end before
expectations exist. No separate residual seed array is allocated.

Every allocation checks explicit buffers at 1 GiB and Metal
`currentAllocatedSize` plus owned CPU expectations and mapped temporaries at
1.5 GiB. The external guard also checks process physical footprint/RSS at
2 GiB, requires 14 GiB available before launch, preserves a 12 GiB host floor,
and enforces 60 seconds. The internal deadline stays 50 seconds. Compilation
requires build+memory leases and the existing 4 GiB/start16/floor12/180-second
limits. Quiet leases are refused. Failures stop without changing caps or
rerunning automatically. No desktop app, system service or unrelated process
is controlled.

Check mode expects 46 command buffers and 161 dispatches. Bench mode adds
six expectation commands and 240 timing commands: 292 commands and 602
dispatches total. Receipts bind the source and executable hashes. The summary
rejects missing, repeated or malformed correctness and timing records.
