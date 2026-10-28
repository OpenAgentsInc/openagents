# 1. Why Coax

> **Design and history.** This chapter explains where Coax comes from.
> Nothing in it needs the compiler.

## 1.1 What we needed

The Verse has two ways to reach a screen. The web build goes C++ →
WebAssembly → JavaScript glue → WebGL2 → the browser's GPU process →
ANGLE → Metal → GPU. The native OpenAgents app for iPhone goes C++ →
metal-cpp → Metal → GPU, with nothing in between: a Swift view controller
owns a `CAMetalLayer`, a display link fires 120 times a second, and each
tick calls straight into the engine's C entry points
(`src/metal/home_app.h`). Three frames are in flight. The render targets
use tile memory for memoryless transient targets; resolved outputs and other
resources still have explicit backing storage.

That native path is as close to the hardware as a phone allows. What's
left between us and the speed we want is mostly in the code itself, and
in how fast we can change it. The [performance stack](../../docs/verse/metareal-engine.md)
lists five things to do in C++ now: a hot-reloadable game layer, C-style
C++ with unity builds, data-oriented layout with a job system, GPU-driven
rendering, and SIMD only where the profiler says. Coax is the sixth
thing: a language where those five are the default instead of a
discipline.

The specification turns that into requirements, each with a concrete
test (spec §1). In short, Coax has to give us:

1. **A short development loop**, measured end to end: edit, check,
   generate, link, launch or reload.
2. **Control of representation**: field offsets, padding, ownership and
   SoA or AoS layout you can inspect, down to the generated C.
3. **Compile-time programming**: bounded, typed computation now, and code
   that generates checked code later.
4. **Native Apple integration**: the Swift host, the C boundary, metal-cpp
   and the native Metal path stay as they are.
5. **Browser delivery**, proved separately on WebAssembly.
6. **Deterministic simulation**: the same replay gives the same output on
   every architecture and worker count.
7. **Explicit lifetimes**: world, level, scratch and pool storage each have
   a visible owner and a visible reset point.
8. **Safe parallel scheduling**, claimed only after real aliasing is
   checked.
9. **Stable interchange** with the OpenAgents Protocol's public schemas.
10. **Inspectable tools**: errors point at your source, and generated code
    can always be read.
11. **Incremental adoption**: Coax code sits beside the C++ engine without
    replacing it or its build.

None of these is a speed number. A small compiler that makes one real
engine workflow better is worth more than a large language that only
exists on paper.

## 1.2 What the compiler should expose

The compiler already knows declaration types, field layouts, dependencies
and the places where code originated. Coax aims to expose that information
through checked metadata and bounded compile-time APIs. Registries,
inspectors, codecs and build tools should share those declarations.

The scalar bootstrap already evaluates `#run` and checks `#assert` without
embedding an interpreter in the target program. General reflection, typed
code generation and compiler plugins remain planned. A generator's output
must pass ordinary checking and retain provenance back to source.

## 1.3 Prove the workflow

The useful measurement is edit to observable result, including checking,
generation, linking, assets and GPU preparation. Development throughput and
release optimization have different budgets. Coax keeps inspectable C output
until another backend demonstrates a benefit on actual engine work.

The native Apple path and browser path require separate tests. A scalar
compiler does not prove shader generation, ownership safety, parallel effects
or transactional reload. Those features enter through explicit gates.

## 1.4 What Coax adds

Chapter by chapter, this book covers the Coax design and its implementation gates:

| Area | Coax | Chapter |
| --- | --- | --- |
| Checks | Overflow, division, bounds, cast and shift checks today; handle checks next | 2, 3 |
| Diagnostics | Stable codes, the source line, and a fix | 4 |
| Equality and hashing | `#derive(eq, hash)` and explicit constraints, instead of hand-written code found by name | 5, 8 |
| Parallelism | A job's capture list is its effects, checked by the compiler and the scheduler | 11 |
| Determinism | `strict` floating point, `deterministic` procedures, `fixed32`, ordered tables | 12 |
| GPU | Shaders in Coax, compiled to Metal first, sharing struct layouts with the CPU | 13 |
| Protocol | Field numbers, limits, data classes and versions on the type | 14 |
| Hot reload | Persistent state behind a versioned C boundary, with explicit, transactional layout migrations | 15 |
| Compile time | Bounded `#run` and `#assert` today; typed code generation with provenance later | 9 |
| Tooling | The compiler as a server that agents can query | 18 |

## 1.5 What Coax will never have

No garbage collector. No exceptions. No constructors and destructors
(RAII). No inheritance. No header files. No preprocessor. No borrow
checker: Chapter 11 shows why capture-list effects cover the case a game
engine has. And no loading code at run time on iOS, because Apple doesn't
allow it and we don't fight the platform.
