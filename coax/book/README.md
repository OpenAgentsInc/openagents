# The Coax Programming Language

*A draft book, begun 2026-10-27.*

Coax is a programming language for real-time software: the Verse, its
renderers and simulations, and the rest of OpenAgents' code that has to
run at 120 frames a second on a phone. It is a descendant of Jonathan
Blow's Jai, changed for a team where agents write most of the code, where
simulations must replay bit for bit, and where data crosses a public
protocol.

This book teaches the language and explains why it is the way it is.

## The state of things

Coax is young. The first compiler, `coax0`, lives in
[`coax/`](../README.md) at the root of the openagents repo and compiles a growing subset of the language
to C: scalars, structs, enums, arrays and views, `defer`, compile-time
execution and a C boundary. Most of what this book describes is designed but not built yet.
Every chapter says which kind it is:

> **Runs today.** Every example in a chapter marked this way compiles
> with `coax0` and prints what the book says. `check-examples.sh` in this
> folder checks that, so the book cannot drift from the compiler.

> **Design.** The chapter describes the language as specified in
> [the Coax specification](../spec.md).
> The syntax can change before it is built. Examples are not compiled.

| Part | Status |
| --- | --- |
| I. Getting started (chapters 1–4) | Runs today |
| II. The language (5–9) | Mixed: structs, enums, arrays, views, `defer` and Chapter 9's first half run today; each section says which |
| III. Data and speed (10–13) | Design, except Chapter 10's layout tools |
| IV. Coax in OpenAgents (14–18) | Design, except Chapter 16's `#export_c` and Chapter 18's tools |
| V. Building Coax (19–20) | Runs today (19), plan (20) |

The language moves faster than any one chapter. When the book and the
compiler disagree, the compiler and its tests are right, and the book has
a bug. The specification (`coax/spec.md`) is the design of record.

## Who this book is for

- **Programmers who know C, C++, Rust, Go or Jai** and want to write fast
  code without fighting the language.
- **Agents** writing Coax for OpenAgents. The book is written to be read
  start to finish by a model as well as a person. Every rule has a reason,
  and every error code is in [Appendix B](appendix-b-error-codes.md).
- **People who want to work on the compiler.** Chapter 19 walks through
  `coax0`, which is about 5,000 lines of C-style C++.

## Conventions

- Source files end in `.coax`. Commands assume you are at the root of the
  openagents repository, with the compiler built (`coax/build.sh`).
- `coax` means `coax/bin/coax`.
- `check-examples.sh` in this folder compiles and runs every listing in
  the chapters marked *Runs today*.
- Output is shown in a block after the program. Errors are shown exactly
  as the compiler prints them.
