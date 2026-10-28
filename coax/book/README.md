# The Coax Programming Language

*A draft book, begun October 2026.*

This is a book about Coax, the language OpenAgents uses for code that has
to be fast and has to be right: the Metareal engine behind the Verse, the
iPhone app's home screen and Grid, and the money rules in its bitcoin
wallet. Coax descends from Jonathan Blow's Jai. It compiles to plain C you
can read, it checks arithmetic and array bounds while you develop, and it
lets you write checks that the compiler runs for you.

The [Introduction](introduction.md) explains why Coax exists. If you'd
rather start typing, go straight to [Chapter 1](ch01-getting-started.md).

## Who this book is for

We assume you've programmed before, in any language. You don't need to
know C, though it helps in a few places, and when it does we'll say so.
If you know C, C++, Rust or Jai, you'll find notes along the way that
compare Coax with what you already know, and two appendices written just
for you ([Jai](appendix-c-jai.md), [C++](appendix-d-cpp.md)).

The book is also written to be read by agents. Most Coax in OpenAgents
is written by models, and they read this book the way a new teammate
would. That's one reason every error code has an entry in
[Appendix B](appendix-b-error-codes.md), and every rule comes with the
reason for it.

## How to use this book

Read it in order the first time. Each chapter builds on the ones before,
and the early chapters introduce ideas lightly that later chapters cover
in depth.

Part I gets you running. Chapter 1 builds the compiler and a first
program. Chapter 2 is a small project, an ASCII Mandelbrot renderer, that
touches most of the everyday language at once. Chapters 3 and 4 then go
back over the basics properly and show how to read the compiler's errors.

Part II covers the language itself: structs and enums, memory, errors and
cleanup, generics, compile-time execution, and writing checks. Part III
is about data layout, jobs, determinism and the GPU. Part IV is about how
Coax fits into OpenAgents: the protocol boundary, builds and hot reload,
interop with C, C++ and Swift, targets, and tooling for agents. Part V
opens up the compiler. Part VI turns those pieces into larger programs:
project organization, asynchronous lifetimes, debugging and cost, verification
boundaries, and a runnable protocol-to-native capstone.

The best way to learn is to type the examples in and run them. Change
them, break them on purpose, and read what the compiler says.

## What runs today and what doesn't

Coax is young. Its first compiler, `coax0`, compiles a real but limited
subset of the language: procedures, integers and floats, strings,
structs, enums, fixed arrays and views, `defer`, compile-time `#run` and
`#assert`, checks with `#test` and `#forall`, source graphs with `#load`,
and a C boundary. A lot of
what later chapters describe is designed but not built yet.

The book is honest about which is which. Anything you can run today is
shown with its real output. When a chapter describes something `coax0`
can't compile yet, it says so in a note like this:

> Note: This feature is planned and not available in `coax0`. The syntax
> shown follows [the Coax specification](https://github.com/OpenAgentsInc/openagents/tree/main/coax/spec.md) and may change
> before it's built.

Code in planned sections has not been compiled. The runnable examples are
checked against the current compiler; elided and illustrative fragments
need the surrounding program described in their chapter.

When the book and the compiler disagree, reproduce the case and check the
implemented subset and its conformance tests. Either the book or the compiler
can have a bug. The specification, `coax/spec.md`, is the design of record;
planned features are not claims about the current compiler.

## Where the examples live

Every program the book runs is a file in
[`examples/`](https://github.com/OpenAgentsInc/openagents/tree/main/coax/book/examples/), next to its exact output in a `.out` file.
The script `check-examples.sh` in this folder compiles and runs each one
and compares the output in debug and release modes. It also runs authored
checks, library/host examples and local link checks. Selected source-linked
listings are compared with their example files; excerpts and illustrative
fragments are not all standalone programs. Programs shown only being
compiled have no `.out` file. Deliberate traps have explicit `.status`
files; mode-specific output or
status uses `.release.out` and `.release.status`. A matching error message
with the wrong exit status is a failure. See Chapter 25 for the limits.

Commands assume you're at the root of the openagents repository, and
`coax` means `coax/bin/coax`. Chapter 1 shows how to build it and put it
on your path.

## Contents at a glance

- [Foreword](foreword.md) and [Introduction](introduction.md)
- Part I. Getting started: [1. Getting started](ch01-getting-started.md),
  [2. A first project](ch02-a-first-project.md),
  [3. Common concepts](ch03-common-concepts.md),
  [4. Reading errors](ch04-reading-errors.md)
- Part II. The language:
  [5. Structs, enums and unions](ch05-structs-enums-unions.md),
  [6. Arrays, strings, memory and the context](ch06-memory.md),
  [7. Errors and cleanup](ch07-errors.md),
  [8. Generics and constraints](ch08-generics.md),
  [9. Compile time](ch09-compile-time.md),
  [10. Writing checks](ch10-writing-checks.md)
- Part III. Data and speed:
  [11. Data-oriented layout](ch11-data-layout.md),
  [12. Jobs: captures as effects](ch12-jobs.md),
  [13. Determinism](ch13-determinism.md),
  [14. GPU programs](ch14-gpu.md)
- Part IV. Coax in OpenAgents:
  [15. The protocol boundary](ch15-protocol.md),
  [16. Builds and hot reload](ch16-builds-hot-reload.md),
  [17. Living beside C, C++ and Swift](ch17-interop.md),
  [18. Targets: Apple first, then the browser](ch18-targets.md),
  [19. Built for agents](ch19-agents.md)
- Part V. Building Coax:
  [20. Inside coax0](ch20-inside-coax0.md),
  [21. The road ahead](ch21-road-ahead.md)
- Part VI. Building larger programs:
  [22. Organizing a larger program](ch22-organizing-programs.md),
  [23. Asynchronous work and lifetimes](ch23-asynchronous-work.md),
  [24. Debugging and measuring cost](ch24-debugging-and-cost.md),
  [25. What verification establishes](ch25-verification-boundaries.md),
  [26. A complete boundary](ch26-a-complete-boundary.md)
- Appendices: [A. Keywords, directives and types](appendix-a-keywords.md),
  [B. Error codes](appendix-b-error-codes.md),
  [C. Coax for Jai programmers](appendix-c-jai.md),
  [D. Coax for C++ programmers](appendix-d-cpp.md),
  [E. Glossary](appendix-e-glossary.md),
  [F. Sources](appendix-f-sources.md)
