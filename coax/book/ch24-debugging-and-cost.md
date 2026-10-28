# 24. Debugging and measuring cost

Readable C output is useful only if you know how to follow a failure into
it and back to the Coax source. This chapter gives a working path for the
current compiler, then explains what to measure before calling code fast.

## Keep the generated program

For a standalone example, ask the compiler to keep its C:

```sh
coax/bin/coax build coax/book/examples/ch23_requests.coax --keep-c -o /tmp/coax-request-demo
```

The normal debug build uses the host C compiler with debug information.
For a library, generate the C and header explicitly, then compile them
with the host and the flags you need. The capstone's `run.sh` does exactly
that, so it can enable sanitizers for both the library and its host.

`coax0` does not yet emit Coax-to-C debugger source mappings. A debugger
will normally show generated C, and native compiler errors refer to that
C too. Preserve the matching `.coax`, generated C, header and compiler
revision when investigating a failure. Do not edit the C as a fix: it
will be generated again.

On a Mac, you can open the standalone executable in LLDB:

```sh
lldb /tmp/coax-request-demo
```

Inside LLDB, use `breakpoint set --name request_complete`, `run`, `bt`
and `frame variable`. An exported procedure keeps its C symbol; an
internal procedure generally has a generated name such as
`coax_p_main`. At optimization level `-O2`, inlining and removed locals
can make this harder. Reproduce in a debug build first, then confirm the
fix with optimized code too.

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
