# 18. Time a decision

Before changing the engine, establish what a decision costs. The
[measurement harness](../tests/service_bench.py) starts the real server, waits
for its ready address, warms each owned workload, and retains every latency
sample and answer. The same harness works on the toy CPU model and original
Elephant on CUDA or Metal. It downloads nothing and is verification tooling; Python is
absent from the serving runtime.

## Read clocks around real work

The host observes a monotonic clock in microseconds. Coax validates elapsed
intervals, computes rates and nearest-rank percentiles, and writes the bounded
[private record](../lib/serve-run.coax). Backwards clocks, invalid counts and
impossible phase totals cannot become a successful measurement.

| Observation | Meaning |
| --- | --- |
| `authenticate_us` | Open and authenticate the pinned model session; once at startup |
| `compile_us`, `place_us` | Compile generated device kernels and place resident/pinned weights |
| `prepare_us` | Render, normalize, tokenize and admit the question |
| CPU `prefix_us` | Compute or restore the shared-prefix state, including the existing discarded prefix readout on a miss |
| CPU `suffix_us` | Execute the suffix and its numerical readout |
| Device `joint_model_us` | Execute both blocks together, including the device LM head |
| Device `readout_us` | Coax maps the downloaded logits to the requested decision statistics |
| `question_phases_us` | Separate observations for each question, without its ID or content |
| `total_us` | Server time through answer preparation, including phase and process/HTTP overhead |
| Client samples | Time through reading the complete HTTP response body |

These intervals have different boundaries. A single device question computes prefix
and suffix in a joint masked program; a forked multi-question call is timed
across its actual prefix/suffix passes: its separate prefix/suffix fields are
unused, not measurements of zero-cost work. CPU suffix includes readout;
separate CPU readout timing is not claimed. Transport and process overhead
make client and phase totals different. Initialization is never folded into
a warm token-rate claim.

`tokens` counts logical input tokens across questions. `reused_tokens` counts
prefix tokens restored from a snapshot. Logical input tokens per second can
rise when work is skipped; it is not the number of newly executed tokens.
Decisions return probabilities, so this is not generated output tokens/s.

## Observe the file cache

The verified-file mapper observes mapped pages with `mincore` before
session authentication. Zero resident pages is `cold`, every page resident
is `warm`, a partial inventory is `mixed`, and failed/unavailable observations
are `unknown`. Asking for a cold run does not establish that it was cold.
A fresh verified-record directory forces hashing, which is a different
variable from the operating system's file cache.

The retained record separates a fresh verification-cache start from reuse.
An opt-in `posix_fadvise(DONTNEED)` on our checkpoint is only an eviction hint;
the measured page inventory decides the label. We do not drop the machine's
global caches or claim exclusive use of a shared host.

The [authentication-only probe](../tests/service_auth_probe.c) calls the
production mapper and session constructor without executing a model decision.
On the M5 Max, a fresh CoW copy had zero of 1,148,665 pages resident and took
144.348 seconds to authenticate. With every page resident but a fresh
verification directory, hashing still took 88.518 seconds. Reusing its verified
record took 0.093 seconds. File warmth and trusted verification evidence are
different observations; these numbers isolate startup, not inference.

`peak_host_bytes` is peak process RSS, the larger parent/worker observation
for CPU requests. Linux's KiB observation is converted to bytes; macOS
already reports bytes. It is not an aggregate process-tree or GPU-memory
meter. Device placement separately reports actual resident/pinned payloads.

## Retain the workload before publishing a number

```shell
COAX=/path/to/qualified/coax psionic/platform/serve-build.sh /tmp/decision-build release plain
python3 -I psionic/tests/service_bench.py \
  --binary /tmp/decision-build/psionic-serve --samples 100 \
  --repeats 1,4,8 --questions 3 --out /tmp/decision-observations
```

For original Elephant, build with `gpu`, set the NVRTC library path, and use
`--engine elephant-gpu --model-dir ORIGINAL --table TOKENIZER_PST`. A fixed
`--device-mib 1024` makes the residency budget explicit when sharing a card.
The harness writes the exact owned requests, their hashes, retained answers,
binary identity, host/load observations and raw client/phase samples.
Repeated answers must reproduce the retained answer; failures are not discarded.

P50/P95/P99 use nearest ranks: sort `n` samples and take position
`ceil(percent*n/100)`, counting from one. A five-sample P99 is merely its
maximum. The candidate record uses 100 serial requests per workload; those
are first shared-host observations, not a production tail-latency SLA or a
concurrency/capacity qualification.

The [native host probe](../tests/service_host_probe.c) measures actual
Coax-generated copy and ordered wide-dot code on 16,777,216-element buffers.
It records five raw intervals, byte/operation counts and exact output checks.
Observed bandwidth/compute rates help explain bottlenecks and form an empirical
roofline estimate: the slower of the memory and arithmetic estimates matters.
These particular kernels, their cache behavior and host contention are not
absolute hardware ceilings. GPU rooflines and packaged-engine comparisons
remain [Part V](PLAN.md#v-make-execution-measurable-parallel-and-portable).

## A small statistics example

The [example](../examples/time-decision.coax) uses explicit teaching clock
observations, not a fictitious benchmark:

<!-- source: psionic/examples/time-decision.coax example -->
```jai
main :: () {
    // Teaching observations in microseconds; native harnesses read real clocks.
    samples:=[5]int.[100,180,110,130,120];work:[5]int;p:=ls_percentiles(samples[..],work[..]);
    #assert p.ok && p.p50==120 && p.p95==180 && p.p99==180;
    print("Observed samples: %, P50 %, P95 %, P99 % us\n",p.count,p.p50,p.p95,p.p99);
    print("100 tokens in 500000 us: % logical input tokens/s\n",ls_rate(100,500000));
    print("A requested cold run is cold only when zero mapped pages are resident: %\n",ls_cache_label(0,100));
}
```

Run `psionic/check.sh --only live-decision` for the focused gate. The
[verification record](../../docs/verification/psionic-partIII-2026-10-29.md)
and [retained observations](../fixtures/serving/partIII-2026-10-29/README.md)
name the qualified producer, raw samples, limitations and results.
Next, [keep the model resident](ch019-keep-the-model-resident.md).
