# 21. The road ahead

Most of this book has had two kinds of material in it: what `coax0` does
today, which you can run, and what the specification plans, which is
marked as not yet available. This chapter is about the second kind. The
practical chapters in Part VI then return to what can be built with the
current subset.
We'll look at where things stand, the order in which the rest is planned to
arrive, what each step has to prove before the next one starts, and the
points at which the plan says to stop.

> Note: This chapter follows section 20 of `coax/spec.md`. The stages there
> are ordered by what depends on what, and each one is gated by evidence.
> None of them has a date.

## Where things stand

`coax0` 0.11.0 compiles bounded source graphs to C11 using `#load`. It has sized
integers, floats, `bool` and strings; structs with defaults, enums with a
declared width, fixed arrays and views; `for`, `defer` and `ifx`; checked
casts and left-to-right evaluation; constants, `#run` and `#assert` with
bounded compile-time evaluation; checks with `#test`, `#forall`,
`#authority`, `coax test` and `coax mutate`; and the C boundary: `#export_c`
with generated headers, `#foreign` for scalar C functions, and `coax metal`
for a small f32/bool subset as Metal Shading Language. It imports protobuf
descriptors with `#import_proto` and generates bounded codecs for the
supported proto3 subset.

Coax is also in use. The `metareal/` folder of the OpenAgents repository
holds the product Coax sources that the iOS app compiles and calls (see
`apps/openagents-ios/README.md`): the
home screen (`home.coax`, which also uses generated protobuf codecs), the
Grid world, the player controller, the animated cast, the camera, the
lighting math shared with Metal, the wallet's payment rules, and the
Haunted House gameplay/editor, buffer admission and reusable HUD layout. The
wallet file is `#authority`, so every procedure in it has a check and
survives mutation testing. In each case Swift or C owns the platform,
memory and GPU resources, and the Coax code is the logic in between.

The compiler itself has been moving into Coax piece by piece. The lexer, the
parser, type checker, protobuf descriptor importer, wire runtime and SHA-256
fingerprinting are Coax now (Chapter 20). Native C++ bridges transport their
inputs and results. The evaluator and C emitter remain C++.

Much of the language in Parts II to IV isn't built yet: pointers and
dynamic arrays, tagged unions and `switch`, `try` and error values,
generics and constraints, the context and allocators, reflection and
macros, jobs and effects, `strict` and `deterministic`, general source modules
and visibility,
importing C headers, `build.coax`, hot reload and the browser target. Each
of those chapters says so where it matters.

## The rule that orders everything

The plan has one rule underneath it: every stage has to produce something
that can be tested on its own, and a working engine path is kept until its
replacement has been shown to behave the same. A compiler feature existing
doesn't mean the app uses it. The iOS home moved to Coax only after the
scalar subset and the C boundary it needed had been tested, and that move
still doesn't count as finishing stage 2 (we'll see why below).

## The stages

Here are the stages from the spec, with what each one must show before it
counts as done, and where each one is today.

| Stage | Work | Exit evidence | Status |
| --- | --- | --- | --- |
| 0. Bootstrap semantics | Lexer, parser, checker, C emitter, documented scalar grammar | Pass and fail fixtures, checked formats, overflow behavior | Done |
| 1. Compile-time scalars | Constants, `#run`, `#assert`, `type_name`, `coax check`, bounded evaluation; later the checks (`#test`, `#forall`, `#authority`, `coax test`, `coax mutate`) | Dependencies, cycles, limits, refusal of captures and I/O, the same errors in debug and release, literal output | Done |
| 2. Data and the C boundary | Structs with defaults, enums, fixed arrays and views, `for`, `defer`, `ifx`, checked casts, ordered operands, `#export_c` with headers and layout assertions, `#foreign` | A pulse kernel called by the native host, outputs compared and frame cost measured | Mostly done; see below |
| 3. Metadata and generation | Target layouts and stable declaration IDs, then reflection, `#derive`, hygienic typed code | A generated registry or inspector with provenance, invalid-generator fixtures, bounded expansion | Not started |
| 4. Build workspaces and tools | Dependency scheduler, capabilities, input manifests, compiler events, then plugins and `build.coax` | One real asset or registry workflow; generated CMake and Xcode lists match the hand-written ones before replacing them | Source graph and `deps` prerequisites ship; workspaces, plugins and `build.coax` remain planned |
| 5. Effects and deterministic jobs | Region and alias model, effect summaries, strict math, scheduler integration | A real subsystem replays identically at 1, 2, 4 and 8 workers; bad effects and aliases are refused | Not started |
| 6. Protocol expansion and GPU experiments | Stable layouts, plus the stage 2 ABI proof | More domains on the selected-profile codec; a Metal pipeline proven separately before choosing a shader approach | Started: descriptor imports and codecs ship, and the iOS home uses them |
| 7. Browser vertical slice | A wasm32 runtime and host ABI, a separate shader bridge | Geometry, memory growth, async resources, input, rendering and debugging in a real browser | Not started |
| 8. Resident compiler and reload | Dependency cache, stable code IDs, safe points, transactional migration | Edit and reload on a Mac host keeps state and resources, rolls back failures, and has measured latency | Not started |
| 9. More backends | Only after earlier stages have shown their value; C stays as the reference | Native debug and optimized backends pass the same suites and beat the C output on a measured workload | Not started |

*Table 21-1: The implementation stages*

Stage 6 started before stages 3 to 5, which might look out of order. The
protocol plan needed a first bounded importer and codec before any product
code could move onto generated protocol types, and that didn't depend on
reflection, build programs or effects. Stage 6 expands that foundation; it
didn't wait for everything ahead of it in the table.

Source composition removes the shell-concatenation workaround in Chapter 22
and keeps physical-file authority scopes. It is a prerequisite for later
build workspaces, not a replacement for stage 2's remaining C-header, real
engine and measured target evidence.

## The rest of stage 2

Stage 2 is the stage that decides whether Coax is worth continuing, so it's
worth being precise about what's missing.

Two things are done. `coax/examples/pulse` links a Coax pulse kernel into a
C host and matches a C reference byte for byte at the same frame cost, and
the iOS home runs its scene and HUD logic in Coax over the scalar C ABI.

Two things are not. `coax0` can't import C headers; `#foreign` declares
scalar C functions one at a time. And the stage's real target, the
`src/paynet` pulse update from the Metareal source archive, hasn't been
ported. That update moves up to 100,000 payment pulses a second along
channels and renders them on Metal. It's small, hot and self-contained,
which makes it a fair test. To pass, the Coax version has to show:

- the same output as the existing C++ update for the same input, edge
  cases included;
- no hidden allocations per frame, with scratch memory and ownership
  explicit;
- exact determinism where the subsystem needs it;
- a verified layout and bounds for the Metal pipeline that reads its data;
- frame time within an agreed budget, on the same hardware;
- the iOS and browser paths still working.

Frame-time parity against the old engine hasn't been measured yet for the
iOS home either. Until it is, "Coax is as fast as the C++ it replaced" is a
goal, not a result.

## When to stop

The spec doesn't assume the project succeeds. It names the points where it
should stop or pause:

- After stage 2: stop if the Coax subsystem is harder to maintain, or less
  reliable, than the C++ it replaced.
- After stage 4: pause if ordinary C++ code generation would give the same
  workflow with less machinery.
- At every stage: compare the whole workflow with simpler tools, and keep a
  language feature only if its measured benefit pays for its maintenance
  and platform costs.

A language is expensive to keep. The spec's own list of costs includes
diagnostics, debuggers, ABIs, standard libraries, platforms, documentation,
reproducible builds and maintenance, and it points out that parsing a few
examples is the cheap part. These stop points are how the project avoids
paying those costs for features nobody measured.

## Questions still open

These are the questions the spec lists as undecided. Each one will be
answered by building something, not by argument.

1. Which pulse-kernel ABI and data set best prove stage 2?
2. Which region and alias model makes field effects trustworthy, without
   promising safety for arbitrary pointer graphs?
3. What stable IDs separate compiler declarations, engine registrations,
   GPU layouts and protocol names across independent builds?
4. Which generated registry or asset workflow should prove that compiler
   plugins are worth it, before a build program replaces any source list?
5. What host capability interface allows useful file and process work
   while keeping inputs recorded and editor analysis safe?
6. When does the measured cost of tree-walking evaluation justify a
   bytecode VM?
7. Should GPU work share layouts with existing shader tools, or does a
   restricted Coax shader backend earn its cost? Chapter 14's experiment
   decides.

## Summary

`coax0` today is a small, working compiler: an explicit source graph in, C out, with
compile-time evaluation, checks, a C boundary and protobuf imports, and
real code in the iOS app built with it. The rest of the language is a plan
in nine stages, each with evidence it must produce and two places where the
plan says to stop. The next concrete step is finishing stage 2 with the
`src/paynet` kernel, and that result decides how much of the rest gets
built.

If you want to help, Chapter 20 shows where the code lives and how it's
tested, and the appendices that follow collect the reference material:
keywords, error codes, comparisons with Jai and C++, a glossary and the
sources this book draws on.
