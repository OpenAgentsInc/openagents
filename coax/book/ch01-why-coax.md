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

Fourteen requirements came out of that work. They are listed in full in
§1 of the specification. In short:

1. A one-file change compiles in well under a second, and stays that way
   as the code grows.
2. Hot reload keeps the world's state, even when a struct's layout
   changes.
3. Data-oriented layout is the default.
4. Simulations are deterministic: the same inputs give the same bits on
   every machine and every core count.
5. Parallel code is safe without a borrow checker.
6. CPU and GPU share one definition of every buffer.
7. SIMD comes from the data layout, not from hand-written intrinsics.
8. It targets Metal on Apple devices first, then WebAssembly, then
   everything else.
9. The OpenAgents Protocol's schemas are types in the language.
10. Reflection drives saves, replication and the editor.
11. It lives beside 40,000 lines of C++ for years.
12. Agents can write it well.
13. The build is part of the source.
14. Decoders refuse oversized input before they allocate.

## 1.2 The dream

Jai meets more of that list than any language we can use. These are the
reasons we call it the dream.

**Compile speed is the design goal.** Jai has no header files and no
textual includes, so each file is parsed once per build. It has no
template instantiation model that multiplies work. It has a fast debug
backend that does not go through LLVM. C++ parses `ecs.h` again in every
file that includes it, and Rust's compile times are the reason this
repository exists.

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

**Layout is in the type system.** Marking a struct `SOA` changes how its
arrays are stored without changing the code that uses them. Every engine
that cares about the cache writes this by hand.

**The small things are right.** `ifx` conditional expressions, multiple
return values, distinct integer types (`RoomIndex :: #type,distinct
int;`), struct literals, `defer`, and `using` for composition without
inheritance.

## 1.3 Why we can't depend on it

- **We can't get it.** Jai is a closed beta by invitation, with no public
  download and no release date.
- **We can't plan around its license.** There are no published terms for
  shipping commercial software with it or putting the compiler on CI and
  agent machines.
- **We can't change it.** Its team builds it for their own games. If we
  need an iOS or WebAssembly target, a Metal shader story, or a bug fixed
  before a launch, we wait.
- **It doesn't do several things we need.** It has no shader language, no
  stated floating-point model, and its hash tables iterate in hash order,
  which breaks replays.
- **Agents can't learn it.** There is very little public Jai. Models write
  a language about as well as the amount of it they've read. For a
  language of our own, we write that material: this book, the
  specification, the tests, and eventually a translated engine.

So we took the design, not the dependency.

## 1.4 What Coax adds

Chapter by chapter, this book covers what Coax does that Jai doesn't:

| Area | Coax | Chapter |
| --- | --- | --- |
| Checks | On by default in debug and dev builds; every unchecked block is listed | 3, 17 |
| Diagnostics | Stable codes, the source line, and a fix | 4 |
| Equality and hashing | `#derive(eq, hash)` and explicit constraints, instead of hand-written code found by name | 5, 8 |
| Parallelism | A job's capture list is its effects, checked by the compiler and the scheduler | 11 |
| Determinism | `strict` floating point, `deterministic` procedures, `fixed32`, ordered tables | 12 |
| GPU | Shaders in Coax, compiled to Metal first, sharing struct layouts with the CPU | 13 |
| Protocol | Field numbers, limits, data classes and versions on the type | 14 |
| Hot reload | `#persistent` state and compiler-written layout migrations | 15 |
| Tooling | The compiler as a server that agents can query | 17 |

## 1.5 What Coax will never have

No garbage collector. No exceptions. No constructors and destructors
(RAII). No inheritance. No header files. No preprocessor. No borrow
checker: Chapter 11 shows why capture-list effects cover the case a game
engine has. And no loading code at run time on iOS, because Apple doesn't
allow it and we don't fight the platform.
