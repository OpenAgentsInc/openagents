# Introduction

Welcome to *The Coax Programming Language*. Before we write any code,
let's talk about what Coax is for, where it already runs, and how it
differs from the languages you probably know. Nothing in this chapter
needs the compiler, so if you'd rather start typing, skip to
[Chapter 1](ch01-getting-started.md) and come back later.

## What Coax is for

OpenAgents builds a 3D world called the Verse, and the engine that runs
it carries the same name. On an iPhone, a frame of the Verse goes like this:
Swift owns the window and the Metal layer, a display link fires every
frame, and Swift calls into the engine through a C interface. The engine
updates the scene, builds draw data, and hands it back to a thin Metal
layer that talks to the GPU. Three frames can be in flight at once, so
the engine has a few milliseconds to do its work before it falls behind.

That's the kind of code Coax is for. It needs to be fast, which means
you control exactly how data is laid out and nothing allocates behind
your back. It needs to be quick to change, because a game is tuned by
editing, rebuilding and looking, hundreds of times a day. And it needs to
be right in ways that are easy to get wrong: arithmetic that doesn't
silently overflow, arrays that aren't read past their end, simulations
that replay the same way every time.

This isn't hypothetical. The OpenAgents iPhone app already runs Coax.
The files are in [`verse/`](https://github.com/OpenAgentsInc/openagents/tree/main/verse/) at the root of the
repository:

- `home.coax` owns home configuration and HUD admission through generated
  protobuf codecs. The current home shows an overview of the Grid with Start;
  the earlier flat-storage graph remains in the module.
- `grid.coax`, `player.coax` and `camera.coax` run the Grid: walking,
  running and jumping, bumping into robots, and the camera that circles
  above the spawn and swoops down when you press Start.
- `cast.coax` poses your character and the robots from packed animation
  data.
- `light.coax` holds shader math that compiles to both C and Metal.
- `haunt.coax`, `haunt-buffer.coax`, `haunt-light.coax` and `hud.coax` run
  the local Haunted House, its editor/documents, GPU span admission, shader
  math and reusable hotbar layout.
- `wallet.coax` decides the rules for the bitcoin wallet: whether a
  quote may be paid, how a payment's state moves, what the keypad holds.

Here's a small piece of `camera.coax`, the procedure that works out how
far through its swoop the camera is:

```jai
// How far through the dolly `now` is, 0 to 1.
progress :: (d: Dolly, now: f64) -> f32 {
  if !(d.length > 0) { return 1; }
  return clamp01(f32((now - d.start) / f64(d.length)));
}
```

You can probably read most of it already. `progress` takes a `Dolly`
struct and a time, and returns a 32-bit float. The conversions between
`f64` and `f32` are written out, because Coax never converts between
number types on its own. The Swift app calls code like this every frame
through a C function that `coax0` generates.

The wallet shows a different side of the language. It starts like this:

```jai
#import "Basic";
#authority;

// All the bitcoin there will ever be, in satoshis.
MAX_SAT :: 2_100_000_000_000_000;

// The SDK counts satoshis; payment.v1 counts millisatoshis.
verse_wallet_sat_to_msat :: (sat: int) -> int #export_c {
  if sat < 0 || sat > MAX_SAT { return -1; }
  return sat * 1000;
}
```

The `#authority` line tells the compiler that this file makes decisions
that matter, so every procedure in it must be exercised by a check, and
the checks run on every build. We'll write checks of our own in
Chapter 2 and cover them fully in [Chapter 10](ch10-writing-checks.md).

Coax is meant to grow beyond the engine. OpenAgents' web, mobile,
desktop and cloud code share one public protocol, and Coax types are
designed to carry that protocol's field numbers and limits
([Chapter 15](ch15-protocol.md)). Most of that is still design work.

## How Coax compares

If you know C, C++, Jai or Rust, it helps to see where Coax sits.

**C.** Coax compiles to C today. Every Coax program becomes one C11
file, which any C compiler can build, including Xcode's clang for the
iPhone app. So Coax has C's cost model: structs are plain
data, calls are calls, nothing is garbage collected. What Coax adds is
everything C leaves to you: integer overflow and out-of-bounds indexing
stop the program with the file and line instead of being undefined
behavior, strings carry their length, there are no header files or
preprocessor, and the compiler can run your code while compiling.

**C++.** Coax deliberately leaves out most of what makes C++ big:
classes, inheritance, constructors and destructors, exceptions,
templates as a separate language. Cleanup is written with `defer` at
the place you acquire something, so you can see it. Generic code is
planned with explicit constraints instead of templates that fail deep
inside their expansion.

**Jai.** Coax is a descendant of Jai, and if you know Jai most of the
syntax will look familiar: `name :: value`, `:=`, `#run`, `print` with
`%`, `defer`, `---` for uninitialized storage. The differences come from
our situation. Errors have stable codes and fix text that agents can act
on. Planned job syntax lists exactly what each job reads and writes.
Determinism and protocol metadata are part of the language design.
[Appendix C](appendix-c-jai.md) has the details.

**Rust.** Rust makes memory safety its central promise and enforces it
with a borrow checker. Coax doesn't make that promise. Pointers, when
they arrive, will be managed by hand as in C, and there is no general
borrow checker. Instead Coax checks the specific things that go wrong in
engine code: arithmetic, bounds, casts, and (planned) which jobs touch
which data at the same time, which [Chapter 12](ch12-jobs.md) covers.
Coax also aims to build much faster, and it treats compile-time
execution as an everyday tool.

> Note: Several comparisons above describe plans. Generics, pointers and
> jobs aren't in `coax0` yet. Each chapter says what runs today.

## What Coax is trying to achieve

The specification (`coax/spec.md`) states what Coax has to deliver, each
with a concrete way to test it. Here they are in plain terms:

1. A short loop from editing code to seeing the result, measured end to
   end, including checking, linking, launching and reloading.
2. Control of representation: you can see field offsets, padding,
   ownership and memory layout, down to the generated C.
3. Compile-time programming: bounded, typed computation today, and code
   that generates checked code later.
4. Native Apple integration that keeps the Swift host, the C interface
   and the Metal path as they are.
5. Browser delivery, proved separately on WebAssembly.
6. Deterministic simulation: the same replay gives the same output on
   every architecture and with any number of worker threads.
7. Explicit lifetimes: world, level, scratch and pool memory each have
   a visible owner and a visible reset point.
8. Safe parallel scheduling, claimed only after the real aliasing rules
   are checked.
9. Stable interchange with the OpenAgents Protocol's public schemas.
10. Inspectable tools: errors point at your source, and generated code
    can always be read.
11. Incremental adoption: Coax code lives beside the existing C and C++
    without replacing their builds.

None of these is a speed number. A small compiler that makes one real
workflow better is worth more to us than a large language that exists
only on paper, which is why `coax0` is small and why the book is careful
to say what it can do.

## Where the language is going

This table maps what Coax adds to the chapters that cover it:

| Area | What Coax does | Chapter |
| --- | --- | --- |
| Runtime checks | Overflow, division, bounds, cast and shift checks today; handle checks planned | [1](ch01-getting-started.md), [3](ch03-common-concepts.md) |
| Diagnostics | Stable codes, the source line, and a fix | [4](ch04-reading-errors.md) |
| Equality and hashing | `#derive(eq, hash)` and explicit constraints (planned) | [5](ch05-structs-enums-unions.md), [8](ch08-generics.md) |
| Compile time | Bounded `#run` and `#assert` today; typed code generation later | [9](ch09-compile-time.md) |
| Checks | `#test`, `#forall`, `#authority` and `coax mutate` today | [10](ch10-writing-checks.md) |
| Parallelism | A job's capture list is its effects (planned) | [12](ch12-jobs.md) |
| Determinism | Plain IEEE floats today; `strict` mode, `fixed32` and ordered tables planned | [13](ch13-determinism.md) |
| GPU | A restricted Metal emitter today; shaders in Coax sharing struct layouts planned | [14](ch14-gpu.md) |
| Protocol | Field numbers, limits and versions on the type | [15](ch15-protocol.md) |
| Hot reload | Persistent state behind a versioned C boundary (planned) | [16](ch16-builds-hot-reload.md) |
| Tooling | Errors as JSON today; the compiler as a server agents can query, planned | [19](ch19-agents.md) |

And some things Coax will not have: no garbage collector, no exceptions,
no constructors and destructors, no inheritance, no header files and no
preprocessor. It also won't load new code at run time on iOS, because
Apple doesn't allow it. Hot reload is for development machines
([Chapter 16](ch16-builds-hot-reload.md)).

With that, let's build the compiler and write some code.
