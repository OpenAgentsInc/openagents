# 20. The road ahead

> **Plan.** This chapter follows spec §20. The stages are ordered by
> dependency and gated by evidence, not by dates.

## 20.1 The rule

Every stage produces something that can be tested on its own, and
working engine paths stay until a replacement is proven equal. A
compiler that can do something does not mean the app uses it yet.

## 20.2 Stages

| Stage | Work | Exit evidence |
| --- | --- | --- |
| 0. Bootstrap semantics | Lexer, parser, checker, C emitter, documented scalar grammar | Pass and fail fixtures, checked formats, overflow behavior. **Done.** |
| 1. Compile-time scalars | Constants, `#run`, `#assert`, `type_name`, `coax check`, bounded evaluation | Dependencies, cycles, limits, capture and I/O refusal, the same errors in debug and release, literal output. **Done in 0.2.0.** |
| 2. Data and the C boundary | Structs with defaults, fixed arrays and views, half-open `for`, `defer`, explicit mutation, fixed-width export and import, left-to-right operand order | One pulse kernel called by the native host, with outputs compared and frame cost measured. **Mostly done in 0.3.0:** everything but import; `examples/pulse` links a Coax kernel into a C host and matches the C reference byte for byte at the same frame cost. The aethyr-cpp kernel itself is next. |
| 3. Metadata and generation | Target layouts and stable declaration IDs, then reflection, `#derive`, hygienic typed code | A generated registry or inspector with provenance, invalid-generator fixtures, bounded expansion |
| 4. Build workspaces and tools | Dependency scheduler, capabilities, input manifests, compiler events, then plugins and `build.coax` | One real asset or registry workflow; generated CMake and Xcode lists match before they replace anything |
| 5. Effects and deterministic jobs | Region and alias model, effect summaries, strict math, scheduler integration | A real subsystem replays identically at 1, 2, 4 and 8 workers; bad effects and aliases are refused |
| 6. Protocol and GPU experiments | Stable layouts plus the stage 2 ABI proof | One bounded codec and one Metal pipeline, tested separately, before choosing a shader approach |
| 7. Browser vertical slice | wasm32 runtime and host ABI, a separate shader bridge | Geometry, memory growth, async resources, input, rendering and debugging in a real browser |
| 8. Resident compiler and reload | Dependency cache, stable code IDs, safe points, transactional migration | Mac-host edit and reload keeps state and resources, rolls back failures, and its latency is measured |
| 9. More backends | Earlier stages proved their value; the C backend stays as reference | Native debug and optimized backends pass the same suites and beat C output on a measured workload |

## 20.3 The first real subsystem

Stage 2's target is the `src/paynet` pulse update in aethyr-cpp. It
moves up to 100,000 payment pulses a second along channels and renders
them on Metal. It's small, hot and self-contained. The evidence it has
to produce:

- the same input and output as the existing C++ update, edge cases
  included;
- no hidden allocations per frame, with explicit scratch and ownership;
- exact determinism where the subsystem needs it;
- a verified layout and bounds for the Metal pipeline that reads it;
- frame time within an agreed budget on the same hardware;
- iOS and browser paths still working.

That example decides which struct, array, loop and C ABI features come
next. The language grows from what one real subsystem needs, not from
the whole design at once.

## 20.4 When to stop

- **After stage 2:** stop if the Coax subsystem is harder to maintain or
  less reliable than the C++ it replaced.
- **After stage 4:** pause if ordinary C++ code generation gives the same
  workflow with less infrastructure.
- **At any stage:** if Jai becomes publicly available under terms we can
  use, compare it against these requirements honestly. If it meets
  enough of them, move what's unique to Coax into libraries and plugins
  on Jai.

## 20.5 Questions still open

1. Which pulse-kernel ABI and data set best prove stage 2?
2. Which region and alias model makes field effects trustworthy without
   promising safety for arbitrary pointer graphs?
3. What stable IDs separate compiler declarations, engine registrations,
   GPU layouts and protocol names across builds?
4. Which generated registry or asset workflow should prove plugins are
   worth it before a build program replaces any source list?
5. What host capability interface allows useful file and process work
   while keeping inputs recorded and editor analysis safe?
6. When does measured tree-walking cost justify a bytecode VM?
7. Shared layouts feeding existing shader tools, or a Coax shader
   backend? (Chapter 13's experiment decides.)
