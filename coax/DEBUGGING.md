# Native source debugging and compiler timings

`coax0` 0.13.0 emits original Coax line locations through C11. The native
compiler, linker and debugger still supply machine code and debug information.
This does not introduce an interpreter into the target program.

## Build and inspect

From the repository root:

```sh
coax/bin/coax build coax/book/examples/ch24_debug/main.coax \
  -o /tmp/coax-debug --keep-c --source-map /tmp/coax-debug.map.json \
  --timings /tmp/coax-debug.timings.json
lldb /tmp/coax-debug
```

In LLDB, substitute the absolute path to this checkout:

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

The breakpoint stops before `total += 2`. `total` is 40 before the step and
42 afterward; the call stack includes `twice` in `adjust.coax` and `main` in
`main.coax`. The names and type labels come from the compiler's map, rather
than guesses about the generated C. Ordinary `bt` and `frame variable` also
work, using the native C symbols and types.

## Flags and artifacts

| Flag | Contract |
| --- | --- |
| `--debug-info` | Add escaped `#line` directives for physical Coax files. Supported by `c`, `build`, `lib`, `run`. |
| `--source-map FILE` | Also write schema-1 JSON: canonical source paths, SHA-256 source hashes, procedure names, physical locations and parameter/local/loop variable names and Coax type labels. Implies `--debug-info`. |
| `--timings FILE` | Write schema-1 JSON with status, generated C byte count and observed phase wall times. Does not change stdout or grant clocks to `#run`. |
| `--keep-c` | Preserve the emitted C beside an executable/object for inspection. |

`build`, `lib` and `run` enable line mapping in the default debug profile.
Release builds need an explicit debug flag. Plain `coax c` remains byte-stable
and has no absolute debug paths unless requested, preserving bootstrap seeds.
For a custom C/Xcode build use `coax c --debug-info`, then compile with `-g`.
Optimization and Coax's arithmetic policy are separate: `--release` changes
both the driver's C optimization and runtime integer overflow behavior; an
external host can optimize checked C without requesting wrapping arithmetic.

Keep the matching source, map, C, object, executable and dSYM/archive together.
The driver retains debug executable `.o` files because Darwin debug maps refer
to them. `coax-map` checks source hashes and refuses stale files. Maps contain
absolute checkout paths: review them before sharing. Relocated sources require
the original files or native debugger source-path remapping; the helper does
not rewrite or trust different source bytes automatically.

The LLDB helper uses the debugger's Python support only for presentation. The
compiler, normal bootstrap and app generation remain Python-free. SHA-256 in
the compiler comes from the existing checked Coax fingerprint module.

## What is verified and what remains limited

`coax/tests/debugging.py` runs actual LLDB sessions on macOS: breakpoints,
stepping, values, loaded-file stacks, multi-line statement lowering, stale
source refusal, escaped paths, duplicate private names in separate libraries,
native diagnostics and sanitizer abort stacks. The iOS Simulator Grid was
also attached to LLDB at `verse_cast_pose` in `verse/cast.coax`.

This is line-level debugging and a name/type presentation bridge. It is not
a Coax expression evaluator or a complete type visualizer. Native C types
remain visible; optimized-out values and inlining follow the native debugger.
Runtime helpers retain generated/native locations. Generated protobuf
procedures currently collapse to their `#import_proto` call site, as compiler
diagnostics already do; this is import provenance, not step-by-step authored
codec source. Inspect retained generated C for those bodies. Source hashes
cover physical Coax files, not descriptor contents or the whole build;
retain generated-code descriptor fingerprints too. There is no Metal shader source map,
validated physical-device crash-symbolication workflow, portable DWARF type
encoding, or browser debugger integration in this slice.

## Timing semantics

The driver observes monotonic wall time around `load`, `lex`, `imports`,
`parse`, `check`, `evaluate`, `authority` and `emit`; native builds also report
`native_compile` and `link`. Only executed phases appear. `load` includes
physical-unit lex/syntax validation used to discover the source graph; `lex`
and `parse` describe the later combined program passes. `emit` includes map
hashing/writing and output preparation. `total` includes remaining driver
overhead and, for `run`, child program execution. It ends before serializing
and writing the timing report itself.

Times are observations, not deterministic compiler values. A failed operation
records a failing status when a report can be written. An unwritable timing
report itself makes the command fail. Repeated single-shot invocations may
benefit from OS caches; they are not incremental compilation.

See [the real engine harness](../verse/benchmarks/README.md) and
[R0 evidence](../docs/verification/coax-r0-2026-10-28.md) for the measured scope.
