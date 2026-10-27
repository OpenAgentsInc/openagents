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
live in tile memory and never touch RAM.

That native path is as close to the hardware as a phone allows. What's
left between us and the speed we want is mostly in the code itself, and
in how fast we can change it. The [performance stack](https://github.com/OpenAgentsInc/aethyr-cpp/blob/main/docs/performance-stack.md)
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

## 1.2 The dream

Jai meets more of that list than any language we can use. These are the
reasons we call it the dream.

**Compile speed is the design goal.** Jai has no header files and no
textual includes, so each file is parsed once per build. Its compiler is
C++, with a handwritten recursive-descent parser, multithreaded work, an
internal bytecode for compile-time execution, and two backends: a custom
x64 backend built to produce code quickly, and LLVM for optimized builds.
The evidence is specific and worth keeping attached to its workloads:

| Evidence | Result | What it doesn't show |
| --- | --- | --- |
| Blow's 2025 demonstration | ~300,000 lines, clean rebuild, 2.3 s | Incremental builds (that compiler had none; it makes full rebuilds fast instead) |
| Smári McCarthy, 2024 | ~44,000 lines, three executables, ~1.3 s | That the front end is the whole story: over two-thirds of that time was LLVM |

C++ parses `ecs.h` again in every file that includes it, and Rust's
compile times are the reason the C++ engine exists. The lesson we take is
not a lines-per-second number. It's that a development build should be
designed for feedback time and a release build for runtime speed, and
that the thing to measure is edit to visible result.

**The build is a program, and it can watch the compiler.** In Forrest
Smith's Advent of Code repository, `modules/autorun.jai` is 53 lines of
ordinary Jai that the compiler loads as a plugin. It receives a message
for each compiler event. When the compile completes, it reads the build
options and runs the program it just built:

```jai
message :: (_p: *Plugin, message: *Compiler.Message) {
    p := cast(*AutorunPlugin) _p;
    if message.kind == .COMPLETE {
        complete := cast(*Compiler.Message_Complete) message;
        if complete.error_code != .NONE { p.should_run = false; }
    }
}
```

The same messages carry every typechecked declaration, so a plugin can
generate code, enforce a project rule, or write bindings, all in the same
language as the program. No CMake, no Python generators.

**Code is a value at compile time.** The same repository builds
assertion macros that print the source text of what failed. The macro
takes parameters of type `Code` (unevaluated expressions). It asks the
compiler for their syntax tree at compile time with `#run`, splices them
back in with `#insert`, and reports the caller's location with
`#location`. Rust needs a separate procedural-macro crate for this. C++
needs the preprocessor.

**`#run` runs anything.** Any procedure can run at compile time, file IO
included. Lookup tables, baked assets and generated code are just code.

**Memory is explicit but not painful.** Every procedure gets an implicit
`context` with an allocator in it, so a caller decides where a library's
memory comes from without the library taking allocator parameters. A
built-in temporary allocator makes scratch memory a pointer bump that is
freed all at once. Dynamic arrays carry their allocator with them.

**The compiler's knowledge is application infrastructure.** An engine
keeps several views of the same data: CPU structs, editor properties,
saves, network messages, debug views, asset processing. Without one
source of truth, each needs its own declarations, parsers or hand-kept
registries. Jai lets the program ask the compiler what it already knows,
through `type_info`, `Code` and compile-time messages, and generate the
rest. Blow's 2025 demonstration showed generated command registries,
generated code you can step through in a debugger, and visual analysis of
a large game. Current Jai builds structure-of-arrays layouts this way too,
with reflection and `#insert`, instead of a dedicated keyword.

**The small things are right.** `ifx` conditional expressions, multiple
return values, distinct integer types (`RoomIndex :: #type,distinct
int;`), struct literals, `defer`, and `using` for composition without
inheritance.

## 1.3 Why we can't depend on it

- **We can't get it.** As of October 2026 the official compiler is a
  closed beta. The studio's 1 September 2026 announcement describes public
  binaries, and later source, as work that follows the game's release, and
  there is no verified public release yet. Unofficial implementations
  (Jacket, and OpenJai's early Zig bootstrap) are separate projects that
  can't stand in for the official compiler's behavior or license.
- **We can't plan around its license.** A stated intent to publish source
  is not a license. We'd need terms for shipping commercial software and
  for running the compiler on every CI and agent machine, at a pinned
  version.
- **We can't change it.** Its team builds it for their own games. If we
  need an iOS or WebAssembly target, a Metal shader story, or a bug fixed
  before a launch, we wait.
- **It doesn't do several things we need.** It has no shader language: a
  Jai program can be compiled to WebAssembly, but WebGPU takes WGSL, so
  shaders stay a separate language. Public material describes no formal
  floating-point or concurrency memory model. Its plain `union` overlays
  storage without a tag. The hash tables in Forrest Smith's code iterate
  in hash order, which breaks replays.
- **It's proven in places we aren't.** The strongest platform evidence is
  per project: *Order of the Sinking Star* on PC, PlayStation 5 and
  Nintendo Switch 2, Polygon's procedural engine on desktop and in the
  browser through WebAssembly and WebGPU, and the Focus editor. None of
  that is the iOS and Metal path we ship first.
- **Agents can't learn it reliably.** There is little public Jai, and what
  exists spans compiler generations: older code dereferences with `<<p`,
  newer code with `p.*`. An agent produces convincing obsolete syntax.
  For a language of our own, we write the material agents learn from:
  this book, the specification, the tests and the grammar, all pinned to
  the compiler that's actually in the repository.

So we took the design, not the dependency. The report that informed this
chapter put it well: study Jai's compiler-as-a-programming-interface
architecture even if you never adopt the language. That's what Coax is.

## 1.4 What Coax adds

Chapter by chapter, this book covers what Coax does that Jai doesn't:

| Area | Coax | Chapter |
| --- | --- | --- |
| Checks | Overflow and division checks today; bounds and handle checks in development builds next | 2, 3 |
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
