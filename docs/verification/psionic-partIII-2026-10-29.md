# Psionic Part III verification — 2026-10-29

Chapters 18, 19, 22 and 23 extend the existing chapter 20/21 endpoint.
Focused checks only: full mutation runs and original whole-model CPU decisions
are omitted under owner instruction. This serving work does not resolve
chapter 17's independent original-model conformance/quality gates.

## What shipped

- **14, Time a decision:** real monotonic clocks, per-question phase records,
  observed file pages, process RSS, logical-token rates, nearest-rank tails,
  native copy/dot probes and a reproducible owned-workload driver.
- **15, Keep the model resident:** authenticated mapping and reserved arenas,
  host-payload admission, descriptor-bound verified files, native replacement
  race checks and removal of redundant CPU state on the device path.
- **17, Run it live:** three retained golden warm-ups before binding/readiness,
  build/checkpoint health, Coax restart policy, native supervisor and systemd
  notification, immutable user-level install and private usage records.
- **18, Make the numbers move:** exact CPU prefix reuse, isolated suffix state,
  per-request ownership of the device forks already landed on main, a joint
  single-question device path and measured storage/latency comparisons.

The endpoint and protobuf chapters 20/21 remain their existing contracts.
The original CPU-pool/decoded-cache ambition in row 18 is explicitly sequenced
into chapter 35 and later cache work. No CPU pool or general decoded-weight
cache is claimed by this delivery. Original device execution was already the
useful serving path, so that is what this part measures and operates.

## Producers and retained evidence

All data is in [this owned fixture directory](../../psionic/fixtures/serving/partIII-2026-10-29/README.md).
`manifest.json` binds the raw files; no model weights, private requests, access tokens
or credentials are committed. Every inference workload is the literal
`Water is wet.` fixture, repeated as declared, with owned yes/no questions.
The actual original checkpoint digest is
`682f02348dfe4617dd21bba2915c12e805de8cf16327a61e895a831ea681d0ba`.

The qualified existing Coax 0.24.0 producer used for C/CUDA was
`/tmp/psionic-placement/coax-metal`, SHA-256
`69f5cbac6cdce1b5c7c80e94bd453608eb876cb30daa31d778fbe8bb912ea4ba`.
The new matrix Metal emission needed a different already-built producer,
copied from the `openagents-wt-elephant-fast` checkout, SHA-256
`d7b8fb4d97ed5b90573c11a4321d70aa78e627a7798bdca206be6a1387971516`.
`COAX_METAL` selects that shader emitter independently. No compiler source was
changed or bootstrapped for this work. Earlier unsupported emitters failed
before producing a qualified candidate; no stale output from those attempts
is presented as a passing build.

| Measured artifact | Build identity | Native environment |
| --- | --- | --- |
| Final paired toy CPU | `44dc6ca8ce0e4afd9bc6e41902490ec075a496e35695b32079b628c0871280f7` | Apple clang 21, M5 Max 128 GiB, macOS 26.4 |
| Original CUDA, 100 samples per workload; managed deployment | `5f51c9ee090328ea588587e4784dc4743c01d9d8f728b22b89d164dcd0bf8a02` | GCC on Linux 6.18.49, RTX 4080, CUDA 13.2 |
| Original Metal, 100 samples; multi-question/operations checks | `2d54d91ccf6477e1eeb15d617fc46e6bb7ba36af7bc815862204b4e44aafc57b` | Apple clang 21, M5 Max 128 GiB, macOS 26.4 |

Each build record keeps hashes of emitted C/header, native sources, kernels,
compiler identity and relevant build configuration. The final build tooling
also hashes the generated original-weight inventory header. That additional
identity input was checked separately after the measurements; it changes a
new build's identity, not the measured model arithmetic. The retained build
records identify the actual binaries measured, not a substituted later binary.

This work rebased over main's new CUDA/Metal kernels and prefix forks and
preserved them. Device improvements remain credited to
[the existing fast-device record](psionic-ch31-fast-2026-10-29.md), including
its own fully resident comparisons. Our CUDA rebuild loads the current 21
generated kernels; Metal loads 23. Older 19-kernel exploratory runs are not
mixed into the final 100-sample observations.

## Focused qualification

The retained `focused-checks.log` is the complete successful post-rebase gate:

```shell
COAX=/tmp/psionic-placement/coax-metal \
  PSIONIC_LIVE_OUT=/tmp/psionic-live-part3/rebased-qualification \
  psionic/tests/check_live_decision.sh
```

- `serve-proto.coax` and its dependency graph: **342/342 checks in debug and
  release**, including statistics, memory limits, exact prefix identities,
  readiness, frozen golden limits, retry limits and busy cleanup policy.
- Native **debug/release × plain/ASan+UBSan**: existing HTTP semantics,
  choice/noul/score parity, malformed/bounded input, protobuf carriage,
  deadlines, disconnect and busy refusal; new private phase/usage records,
  cache on/off exact parity, failed golden/budget startup, bounded crash loop,
  child kill/restart and clean supervisor shutdown.
- Descriptor replacement race, bounded busy sockets and optional native
  readiness datagrams passed in all four configurations.
- All four new runnable examples matched their committed stdout in debug
  and release. The gate's book check passed **64 programs / 243 source-linked
  listings**, with local links. The final book structure is checked again
  only for subsequent prose/index changes and main's new chapters: **67
  programs / 260 listings** passed after the final rebase (`final-book.log`).
- The final rebased Linux/CUDA build also passed `service_live.py` against its
  toy engine (`linux-checks.log`).
- The final inventory-header build-identity change passed the standalone
  `serve_build_id_test.py`: relocation leaves identity unchanged, changing the
  actual generated inventory bytes changes it (`build-identity-check.log`).
  `service_observations.py` independently verified retained digests, ranks,
  request hashes, reuse counters, memory arithmetic and operational parity.
  Shell syntax and Python compilation checks passed for the new tooling.

An early native attempt exposed missing log-directory setup and deprecated
fixture `sprintf` under Xcode's sanitizer warnings; those were fixed before
the four-configuration gate. The operational check exposed a supervisor
SIGTERM hang: `sigaction` without `SA_RESTART` now interrupts `waitpid`, so
the child receives the stop signal. These were real failed attempts, not
discarded successful samples. The complete passing gate is retained.

No full mutation run or original whole-model CPU inference was performed.
Those omissions follow the owner's instruction. This is focused changed-code
qualification, not a claim of a new full-library release gate.

## Authentication: page warmth is independent of verification reuse

`service_auth_probe.c` uses the exact production mapper/session/arena
constructor and **does not execute any model decision**. An APFS CoW copy of
the owned original checkpoint supplied a distinct mapping; `mincore` observed
the actual resident pages before authentication. Only our local verification
directory was changed; global caches were not dropped.

| Original-file start | Resident pages / total (16 KiB pages) | Authentication | Probe total | Peak process RSS |
| --- | --- | --- | --- | --- |
| Fresh file pages, fresh verified directory | 0 / 1,148,665 | 144.348274 s | 144.782470 s | 18,858,541,056 B |
| Warm file pages, fresh verified directory | 1,148,665 / 1,148,665 | 88.517700 s | 89.086801 s | 18,859,065,344 B |
| Warm file pages, reused verified record | 1,148,665 / 1,148,665 | 0.093089 s | 0.541583 s | 42,729,472 B |

The probe reserves the capacity-128 CPU execution payload, 461,912,048 bytes,
but does not touch all its lazy pages. Its total excludes placement, compilation
and golden warm-up. Hashing a warm file still costs time; a verified record
skips that hash while config/tokenizer/inventory checks remain. Cached evidence
is trusted local state, not remote attestation. An external writer must never
mutate the checkpoint throughout its borrowed session lifetime.

## Steady-state original decisions

These are **100 serial loopback samples per row**, after warm-up, on shared
hosts. All raw latencies, exact request hashes, retained answers, health/build
identities, load observations and private phase records remain available.
Percentiles use `ceil(p*n/100)` nearest ranks. No failure is silently excluded.

| Engine and state repetitions; one question | Logical input tokens | P50 | P95 | P99 |
| --- | --- | --- | --- | --- |
| CUDA, 1 | 86 | 809.599 ms | 1,027.923 ms | 1,093.358 ms |
| CUDA, 32 | 210 | 742.567 ms | 858.098 ms | 889.184 ms |
| CUDA, 80 | 402 | 914.661 ms | 963.704 ms | 996.279 ms |
| Metal, 1 | 86 | 373.605 ms | 571.622 ms | 670.752 ms |

The CUDA plan budget was explicitly 1,024 MiB, with streamed weights, alongside
the existing resident GPT-OSS service. The GPU was not exclusive; Linux load
was approximately 81 at the start and 85 at the end. The Mac was also shared
(load approximately 25 down to 16), with visible warming/contended early
samples. These are not cross-hardware rankings, quiet-host benchmarks, an
improvement over main's fully resident latency, or a production P99 SLA.
The shorter CUDA workload being slower than the next one illustrates why
shared-host numbers must retain time order and load.

The fixture driver verifies every measured answer against its warmed retained
answer exactly. The first seed additionally meets the pre-existing 0.0005
CPU/device ceiling: CUDA p_yes `0.757202773`; Metal `0.757192428`, compared with
the recorded CPU `0.757196285`. No original CPU inference was rerun.
These decisions output probabilities, not generated text. Logical input
tokens/s therefore does not mean generation throughput.

Startup observations from these same runs were: CUDA authentication 0.240 s,
compile/setup 4.347 s, placement 9.585 s, server total through warm-up 17.684 s;
Metal authentication 0.188 s, compile/setup 0.749 s, placement 21.577 s and
total 24.704 s. Do not combine these mixed/warm starts with the cold auth-only
experiment as if they were a single controlled run.

## Before and after: CPU prefix reuse

`service_prefix_bench.py` starts two owned toy processes from **one binary**,
one with reuse disabled, one enabled; only one call is active. It alternates
the order on each of **100 pairs per row**, and retains every pair, the load,
exact requests and answers. Each request has three identical choice questions.
Every serialized answer matched exactly across both variants and all samples.

| State repetitions | P50 without → with | P95 without → with | P99 without → with | P50 reduction |
| --- | --- | --- | --- | --- |
| 1 | 83.765 → 55.081 ms | 93.477 → 61.955 ms | 96.396 → 65.381 ms | 34.2% |
| 4 | 130.629 → 82.686 ms | 219.163 → 134.231 ms | 231.849 → 165.770 ms | 36.7% |
| 8 | 282.753 → 170.964 ms | 381.092 → 232.847 ms | 505.330 → 255.340 ms | 39.5% |

The reused path has one miss and two hits per request; a new request starts
with a miss. Both identity and every prefix token are checked. A complete
hybrid snapshot is immutable while each suffix executes in separate state.
The CPU prefix miss still performs the existing discarded prefix readout,
which is counted in prefix time and remains an optimization opportunity.
This toy observation is not an Elephant CPU throughput claim.

## Before and after: original Metal forks and host storage

The same final Metal binary, original checkpoint and three-question workload
with 32 state repetitions, run with/without `--no-prefix-reuse`, returned
identical serialized answers. Ten sequential samples per variant gave P50
**2,602.223 → 1,317.730 ms**, a **49.4% reduction**. Two hits restored **346
prefix tokens per request**. The ten-sample maxima were 3,081.688 and
1,573.766 ms; they are not meaningful P99 qualifications. Shared load and
sequential order limit the comparison. Main supplied the GPU fork kernels;
this check verifies their host integration, request reset and counters.

At capacity 512, both original device runs report host execution payload
**3,288,560 B**, versus the conservative CPU state/scratch/snapshot plan's
**691,746,800 B**. The removed reservation is **688,458,240 B (656.6 MiB)**.
This is payload arithmetic, not equal measured RSS savings: `calloc` is lazy.
The CUDA process still observed VmSize 39,909,172 KiB and peak/RSS
30,878,560 KiB at readiness, including pinned/mapped weights and driver costs.
The 1 GiB device-plan cap excludes additional fork buffers/driver overhead;
it is not a whole-process GPU usage promise.

The host copy probe moved 134,217,728 counted read/write bytes per interval
(16,777,216 f32 elements), with a median 0.002008 s: **66.84 GB/s**. Its ordered
dot counted 33,554,432 operations, median 0.017012 s: **1.972 Gop/s**. Five raw
intervals and exact checks are retained. These empirical kernels explain some
memory/arithmetic costs; they do not establish hardware ceilings or GPU rooflines.

## Original device operations and managed deployment

`service_device_live.py` checks the original Metal model through real sockets.
A competing request received typed 503 busy in **23.085 ms while the first
decision was still active**, rather than silently queuing model work. Two
subsequent three-question calls each had a fresh miss and two hits. Closing a
client during device work produced a cancellation event; the next original
golden succeeded within the frozen ceiling. No stale answer was published.

`install-systemone.sh` installed the reviewed CUDA build as the user unit
`psionic-systemone.service` on **coderos-4080, 127.0.0.1:8798**. It preserves
versioned binaries, kernels, build record and tokenizer under
`~/.local/share/psionic/systemone/BUILD/`; metrics are under
`~/.local/state/psionic/systemone/`. Unit verification and native `READY=1`
passed. There is no root install, Python serving process or boot enablement.

An initial attempt lacked GPU memory because another idle Elephant occupied
the shared card. The child failed placement and the Coax supervisor exhausted
five retries instead of becoming ready. Only idle Elephant processes, after
checking no established clients, were terminated under the owner's existing
permission. GPT-OSS was left running. A new start then passed.

The [managed test's raw record](../../psionic/fixtures/serving/partIII-2026-10-29/managed/operations.json)
records supervisor 740951, killed child 740958 and replacement 780138. After
SIGKILL, the same supervisor re-authenticated, placed, warmed and became ready
in **13.735 s**. Checkpoint/build identity and serialized original answer were
unchanged (`yes`, p_yes `0.757202773`). Clean stop passed. The unit is **now
stopped, installed, not boot-enabled**, releasing the shared GPU between uses.
Public ingress, sustained GPU allocation, domain, TLS/access and budget remain
owner choices in `NEEDS_OWNER.md`; the private operational checks are complete.

## Reproduce only the relevant checks

```shell
psionic/check.sh --only live-decision
python3 -I psionic/tests/service_prefix_bench.py \
  --binary BUILD/psionic-serve --samples 100 --out /tmp/prefix-pairs
python3 -I psionic/tests/service_bench.py --binary BUILD/psionic-serve \
  --engine elephant-gpu --model-dir ORIGINAL --table TABLE \
  --device-mib 1024 --repeats 1,32,80 --samples 100 --out /tmp/decisions
python3 -I psionic/tests/service_device_live.py \
  METAL_BUILD/psionic-serve ORIGINAL TABLE /tmp/device-operations
# On the Linux host, after installing/starting our unit; this check stops it:
python3 -I psionic/tests/service_managed.py 8798 /tmp/managed-operations
python3 -I psionic/tests/service_observations.py
```

These opt-in original-device experiments need an available device/model and
are not part of routine original CPU testing. Future work includes exclusive
GPU measurements, broader mixed-question fork workloads, removal of discarded
prefix readout, CPU pools and decoded-cache qualification, fair batching and
public service operations. Chapter 17's model-quality and independent whole
original-model conformance gates are unchanged.
