# 18. Make the numbers move

An optimization has three obligations: identify the work removed, preserve
the previous answer, and measure before/after with chapter 14's harness.
This chapter combines the [GPU prefix-fork work](../../docs/verification/psionic-ch31-fast-2026-10-29.md)
already landed on main with exact CPU prefix reuse, explicit per-request
snapshot ownership and removal of redundant CPU model-state arenas. Its
underlying model arithmetic retains its existing numerical profile.

## Keep a prefix snapshot separate from suffix work

Previously `sv_answers` prepared and executed each question from the beginning.
The [new CPU path](../lib/serve-run.coax) prepares each question, computes or
restores its prefix, and executes its suffix in a separate working state.
The checked `qp_fork` copies all live recurrence, convolution and KV state.
A suffix never overwrites the retained snapshot.

A hit requires the same authenticated checkpoint identity, equal prefix length
and equality of every prefix token. A common string prefix is insufficient:
the tokenizer's safe block boundary and exact IDs matter. Metadata starts empty
for every HTTP request, so there is no cross-request or tenant cache here.
Changing any prefix token, the identity, the length, or disabling reuse causes
a miss. The [example](../examples/reuse-decision-prefix.coax) shows that rule:

<!-- source: psionic/examples/reuse-decision-prefix.coax example -->
```jai
main :: () {
    cached:=[4]int.[10,20,30,40];next:=[4]int.[10,20,31,41];record:=LsPrefix.{valid=true,identity="checkpoint-A",count=2};
    print("Same checkpoint and exact prefix: %\n",ls_prefix_match(record,"checkpoint-A",cached[..],next[..],2,true));
    next[0]=99;print("Changed prefix token: %\n",ls_prefix_match(record,"checkpoint-A",cached[..],next[..],2,true));
    print("Changed checkpoint: %\n",ls_prefix_match(record,"checkpoint-B",cached[..],cached[..],2,true));
    print("Only one request owns the snapshot; suffix state never replaces it.\n");
}
```

The native endpoint checks compare cached and uncached answers exactly as
serialized and compare the usual mixed choice/noul/score request with direct
chapter-10 calls. Timing records prove the three-identical-question workload
has one prefix miss and two hits, including a fresh miss after the next
request changes its state. Unused snapshot capacity is not live history.

The CUDA/Metal host retains main's complete per-layer prefix forks. A request
with more than one question may use them; a single-question request uses the
joint program because it has no second question to benefit from a snapshot.
The fork metadata is reset at every request boundary. `--no-prefix-reuse`
disables the CPU or device path for a controlled comparison. On a miss, a
device prefix pass and suffix pass are both included in its observed model
time; hit/reused-token counters distinguish later suffixes. Streaming weights
can make the extra first pass expensive, so a short streamed workload need
not improve. The original fully resident CUDA/Metal comparison stays credited
to its own record.

## Do not reserve a CPU model that will run on the device

The old host used `sv_plan`'s CPU state/scratch allocation even for CUDA/Metal
requests, although device execution used neither. `ls_memory_for` now selects
the correct ownership profile: the device plan owns model state and scratch;
the host retains only bounded text/control/probability buffers.
`ls_stage_device` admits the prepared tokens directly against the device's
configured capacity. The device plan independently checks placement and the
request's operation schedule. There are no fake nonempty null spans or hidden
CPU fallbacks.

This removes reserved host execution payload. Since `calloc` pages can be
lazy, it does not imply the same RSS reduction. The retained comparison reports
planned payload and process virtual/RSS observations separately. Original
Elephant answers at every measured prompt length are compared with the retained
baseline; the seed also stays within the pre-existing CPU/device probability
ceiling. Host-allocation removal is not presented as a faster GPU kernel.

## Change one thing, keep the raw samples

```shell
python3 -I psionic/tests/service_prefix_bench.py \
  --binary /absolute/psionic-serve --samples 100 --out /tmp/decision-pairs
```

Both variants use one binary and the same workload, with two toy processes
and only one active call. The only CPU switch is prefix reuse. Each pair flips
which variant runs first. All 100 pairs retain exact serialized answer parity.

| State repetitions, three questions | CPU P50 without reuse | CPU P50 with reuse | Reduction |
| --- | --- | --- | --- |
| 1 | 83.765 ms | 55.081 ms | 34.2% |
| 4 | 130.629 ms | 82.686 ms | 36.7% |
| 8 | 282.753 ms | 170.964 ms | 39.5% |

Original Elephant on Metal, three questions with 32 state repetitions, took
2,602.223 ms P50 without forks and 1,317.730 ms with them (49.4% reduction).
All answers were identical and two hits restored 346 prefix tokens per
request. Those ten-sample sequential batches are a smoke comparison, weaker
than the alternating CPU pairs, and do not qualify tail latency. They measure
the existing device forks under this chapter's per-request ownership policy.

Raw requests, identities, answers, phase/token accounting and all samples
remain with the [verification record](../../docs/verification/psionic-partIII-2026-10-29.md).
Both hosts were shared; load and that limitation are retained.

## Follow the actual execution path

The roadmap originally placed all-core CPU products and a decoded hot-weight
cache before device work. Original CPU decisions proved too expensive for
routine use, while the Coax CUDA path already serves Elephant. This chapter
therefore measures that existing resident/streamed device path first and makes
the two changes above. It does not claim a new CPU worker pool or general decoded
weight cache: deterministic CPU parallelism remains chapter 24, wider cache
policy chapter 25 and later device/kernel tuning Part V.

Next useful experiments are broader GPU prefix-fork workloads,
less repeated prefix readout, tokenization/allocation attribution, kernel and
matrix improvements under the frozen parity profile, and batching/fairness
under the later serving chapters. Each needs its own workload and before/after
record. A faster toy CPU workload is not an Elephant throughput claim.
