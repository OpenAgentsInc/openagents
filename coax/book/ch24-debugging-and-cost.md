# 24. Debugging and measuring cost

Readable C output is useful only if you know how to follow a failure into
it and back to the Coax source. This chapter gives a working path for the
current compiler, then explains what to measure before calling code fast.

## Step through the original source

Version 0.13.0 maps generated native code back to physical Coax files. Build
this two-file example from the repository root:

```sh
coax/bin/coax build coax/book/examples/ch24_debug/main.coax \
  -o /tmp/coax-debug --keep-c --source-map /tmp/coax-debug.map.json \
  --timings /tmp/coax-debug.timings.json
lldb /tmp/coax-debug
```

The loaded procedure is small enough to inspect directly:

<!-- checked-source: examples/ch24_debug/adjust.coax -->
```jai
twice :: (value:int)->int {
  total:=value*2;
  total+=2;
  return total;
}
```

In LLDB, replace the checkout path with your own absolute path:

```text
command script import /absolute/openagents/coax/tools/coax_lldb.py
coax-map /tmp/coax-debug.map.json
breakpoint set --file adjust.coax --line 3
run
coax-vars
coax-bt
thread step-over
coax-vars
continue
```

Before the step, `value` is 20 and `total` is 40. Afterward, `total` is 42.
The stack shows `twice` in `adjust.coax`, called by `main` in `main.coax`.
The optional helper presents authored names and Coax type labels. Ordinary
`bt` and `frame variable` remain useful, showing the native C names/types.

`--source-map` writes schema-1 names, locations and SHA-256 source hashes and
implies `--debug-info`. The helper refuses files changed since compilation.
Default debug `build`, `lib` and `run` commands enable original-file line
mapping. Plain `coax c` opts in explicitly, preserving reproducible bootstrap
C without absolute paths. Custom native builds also need `-g` when compiling
the mapped C. Release builds can request mapping explicitly.

This is not a Coax expression evaluator or complete native type visualizer.
Inlining and optimized-out variables follow the native debugger. Runtime
helpers keep native/generated locations. Generated protobuf procedures map to
the `#import_proto` site, not an authored codec body; inspect their generated
C and descriptor fingerprints. Source hashes cover physical Coax files, not
every build input. Keep matching sources,
maps, objects and executable/dSYM together; Darwin's debug map needs the
object file retained by the driver. Maps contain absolute checkout paths.
Do not edit generated C as a fix: it will be generated again.

The regression suite uses real Mac LLDB sessions, including stepping after a
statement lowered to several C statements. It also checks escaped paths,
source-hash refusal, duplicate private names in separate libraries, native
compiler diagnostics and sanitizer stacks. The running iOS Simulator app was
attached at `verse_cast_pose` in `cast.coax` with a stack through Swift.
That does not establish every device/target debugging workflow.

A Coax trap, an AddressSanitizer report and a compiler diagnostic describe
different failures. A bounds trap is a runtime violation of a checked
operation. A sanitizer report may reveal native memory misuse that the
Coax evaluator never sees. A diagnostic rejects source before execution.
Keep their stderr and exit status; an empty stdout is not success.

## Exercise the native boundary

From the repository root:

```sh
BOOK_SANITIZE=1 coax/book/examples/ch26_workflow/run.sh
```

This compiles the emitted C and its actual C host with
`-fsanitize=address,undefined`, debug information and frame pointers. Both
debug and optimized executions run the same assertions. It checks exact
wire bytes, short output buffers, malformed input and late completions.

Sanitizers do not prove absence of bugs. They examine executed native
paths and have their own coverage and platform limits. They complement
the Coax checks, especially for views, overlapping storage and ABI layout.
Thread races require a separate native concurrency test strategy; running
ASan does not establish thread safety.

For an iOS failure, preserve the exact build number, executable, archive
and matching dSYM. Symbolicate the actual crash report before guessing at
a fix. The debugger attached to a simulator is useful, but it is not a
substitute for a device report when the failure depends on device memory,
audio routing or GPU behavior.

## No allocation is not the same as cheap

Coax makes storage visible, but explicit work can still be expensive.
These are costs to look for:

| Operation | Cost or risk to inspect |
| --- | --- |
| Passing a large fixed-array struct by value | Copying or stack pressure; the optimizer may remove some copies, but the source does not promise it |
| Returning a view | A small descriptor copy, with the backing storage's lifetime still your responsibility |
| A large local array | Stack reservation even though there is no heap allocation |
| Repeated checked indexing or conversions | Branches and checks unless the native optimizer can prove them redundant |
| Generated protobuf field getters | Validation/scanning work over the message, not guaranteed constant-time field access |
| Generated protobuf setters and appenders | Rewriting into another caller-owned buffer; repeatedly growing a message can do quadratic total copying |
| CPU-to-GPU transfer | Bytes copied or synchronized and resources retained until completion |

Let `B` be message bytes and `k` the number of field reads. Revalidating a
message for each getter can cost on the order of `k * B`. A loop that
appends one item by rewriting the whole accumulated packet can copy the
same prefix many times. Zero allocation does not remove either cost.

The current codecs favor bounded, simple access. For a hot consumer,
parse at the boundary, check domain requirements, then copy the needed
fields into a bounded internal representation once. Retain original
bytes separately if signatures, unknown fields or forwarding need them.
Do not introduce an unchecked accessor by bypassing generated validation
merely to improve a benchmark.

For a large state, compare passing a view to stable storage with returning
a complete value every frame. Measure emitted native code rather than
assuming the C compiler eliminated all value copies. Never trade lifetime
clarity for a small benchmark win without native checks of the new path.

## Measure a workload, not a slogan

Choose a representative workload and write down its limits: entity count,
message size, pending operations, draw calls, GPU bytes and frame budget.
Measure the target and configuration that ship. Record hardware, OS,
compiler revision, optimization flags and whether the debugger was attached.

Separate at least four measurements:

1. Source-to-C generation and check time.
2. Native compile, link, install and launch time.
3. CPU frame or operation time, including boundary copies.
4. GPU time, upload/synchronization cost and end-to-end latency.

The driver can record its own phases without changing program stdout:

```sh
coax/bin/coax build coax/book/examples/ch24_debug/main.coax \
  -o /tmp/coax-debug --timings /tmp/coax-debug.timings.json
```

The report separates load, lex, proto imports, parse, check, evaluation,
authority checks, emit, native C compilation and link, with status and emitted
byte count. Only executed phases appear. Source discovery's physical-file
syntax validation is included in load; later lex/parse buckets describe the
combined program. Emit includes source-map hashing/writing when requested.
These are host clock observations, not new capabilities available to `#run`.

A quick shell timing is useful for tool iteration. Use repeated runs and
report the distribution, not just the fastest sample. A frame-time
comparison should include warm-up, sustained load and tail latency.
Chapter 16's older generation timings identify their smaller source set;
they are not measurements of today's whole app or of the runtime.

Use Instruments' Time Profiler and Allocations for the native app. Inspect
large transient allocations and stack use as well as total CPU time.
For rendering, use Metal validation and a GPU frame capture to inspect
buffer strides, bindings, resource lifetimes and expensive passes. Keep
captures and traces with the revision and reproducible input that made
them useful.

## Turn a regression into a durable check

A good performance report includes the input and a correctness oracle.
First show that old and new implementations produce the required result;
then compare cost. Otherwise a faster implementation that skips work can
look like an improvement.

Keep semantic checks deterministic and exact where the contract is exact.
Keep timing thresholds in a separate target-specific harness, with enough
tolerance for noise and an explicit workload. Do not make a finite
`#forall` claim a frame-rate guarantee: it runs in the compiler's evaluator,
not in the phone's renderer.

## A real engine baseline

The repository's `verse/benchmarks/` harness uses the app's actual cast,
with 65 player joints and 402 robot joints. It compares player palette values
against a pinned historical native evaluator, measures robots/body capsules
and movement/collision, records allocation calls, and retains raw samples,
compiler phases, peak compiler RSS and generated sizes. Run it with:

```sh
python3 verse/benchmarks/run.py --output /tmp/coax-r0 --repeats 9
python3 verse/benchmarks/run.py --output /tmp/coax-r0-asan --repeats 1 --sanitize
```

The benchmark separates checked `-O0`, app-style checked `-O2` and wrapping
release `-O2` profiles. Both sides receive matching native flags. A Debug
app launched with `-OABenchmark` also records the normal Grid's CPU stages,
host buffer copies, Metal encoding and completed-command GPU clocks in
`Documents/R0-frame.json`. Simulator clocks are not iPhone performance.

The first comparison missed the proposed 5% median timing budget. A Coax-only
follow-up hoists invariant clip/skeleton/yaw work, caches decoded rows in bounded
stack storage, and removes vector/matrix temporaries and identity initialization
from scratch rows. It keeps exact-frame normalization and checked access. The
same frozen reference, asset, workload, flags and threshold now pass with median
ratios 1.034× at checked `-O0`, 0.807× for app-style checked `-O2` and 0.740×
for wrapping release. Palette tolerance and zero observed kernel allocations
still pass. This preserves the numeric contract, not signed-zero bit identity.

The default harness records any missed median gate; `--enforce-budget` makes
it fail. These are named-Mac results, not universal speed guarantees. A large
debug outlier remains in the raw report, and the caches increase bounded stack
use. Inspect tails and memory as well as medians. R0 remains open for
physical-device/tail budgets, full-loop evidence, status generation and
independent reference/gate review. The benchmark README and `docs/verification/`
retain both the original miss and the follow-up, including their actual inputs.

## Timing evidence for secret paths

Fast code and constant-time code are different properties. The Nostr timing
harness at `tools/coax-nostr/timing/` compares fixed and seeded random public
inputs with Welch's t-test. Coax owns moments, bounds and classification;
platform code observes clocks and invokes emitted callbacks. Deliberately
leaky branch and memory-index controls check that the experiment can detect
those differences on the selected target.

The retained run has a million observations per fixture on native arm64,
native x86-64 and real Chrome/Wasm, with exact class counts, raw-row hashes,
compiler flags, timer resolution and noise statistics. Browser results name
the engine and requested tier. Test tooling recomputes the statistics through
an independent two-pass calculation. Instrumented sanitizer timings stay
separate from optimized release timings.

`no-difference-detected` is an observation under that experiment's inputs,
noise and threshold. It does not establish constant time for another target,
optimizer, JIT tier, secret distribution or algorithm. Source restrictions,
generated-object inspection, erasure, independent review and key custody remain
separate gates. See the [harness method and scoped evidence](https://github.com/OpenAgentsInc/openagents/blob/main/tools/coax-nostr/timing/README.md).
