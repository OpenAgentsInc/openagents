# Appendix E. Glossary

**AoS / SoA / blocked SoA.** Array of structs, struct of arrays, and SoA in
fixed-width blocks (`SOA(4)`). Storage choices for the same logical data
(Chapter 13).

**Arena.** An allocator that hands out memory by bumping a pointer and
frees everything at once on reset. Gives a group of objects one shared
lifetime. `coax0` allocates its syntax tree this way (Chapter 27).

**Authority file.** A file that starts with `#authority;`. Every procedure
in it except `main` and `#foreign` imports must be executed by some check,
and its checks run on every build (Chapter 11).

**Budget.** A compile-time limit counted in work (steps, depth, nodes),
never in seconds, so whether a program compiles doesn't depend on the
machine. A compile gets 100,000 evaluation steps by default, configurable up to
250,000,000; each check gets its own 20,000,000 by default. Aggregate storage
has a separate fixed limit.

**Capability.** A declared permission for build-time code to read files,
run processes or use the network. Pure `#run` code has none.

**Capture list.** The `[read a, write b]` list after a job's signature:
the data it may touch, and how (Chapter 14).

**Check.** A `#test` or `#forall` block. It runs in the compile-time
evaluator under `coax test` (and on every build of an authority file) and
is never compiled into the program (Chapter 11).

**coax0.** The bootstrap compiler in `coax/`. Compiles a subset of Coax to
C11.

**Constraint.** A named set of operations a generic type parameter must
support, such as `Hashable`.

**Context.** The implicit value passed to ordinary procedures, carrying
the allocator, scratch arena and logger.

**Development / release build.** Development builds trap on integer
overflow. Release builds optimize and wrap integer overflow. Bounds, casts,
shifts and division by zero are checked in both, and compile-time
evaluation behaves the same in both.

**Effect.** What a procedure may do: read or write certain data, allocate,
perform I/O. Summarized transitively and checked for jobs, shaders and
deterministic code.

**Fingerprint.** A hash of a struct's layout on one target, recorded by
everything that depends on that layout.

**Frame graph.** The schedule of a frame's jobs, built from their effects,
running non-conflicting jobs in parallel.

**Handle.** An index plus a generation that refers to a pooled object and
fails safely once the slot is reused.

**Hot reload.** Replacing code in a running program while keeping its
state. In Coax, a resident-host design with safe points and transactional
migration (Chapter 19).

**Mutant.** A copy of a program with one small change, such as `<` turned
into `<=` or a statement removed. `coax mutate` builds every mutant it can
and reports the ones no check fails (Chapter 11).

**Place.** Something that can be assigned to: a local variable, a field of a
place, or an element of a place or a view. Parameters, constants and loop
variables are not places.

**Provenance.** The recorded chain from a source call through a generator
to generated code and its emitted C.

**Resident host.** The part of the app that never reloads. It owns
persistent storage, platform services and GPU resources.

**Scratch.** Per-thread, short-lived memory from the context, reset in
bulk.

**Seed.** One of the generated C files in `coax/bootstrap/` that let the
compiler build its own Coax lexer and parser. A normal build requires the
seeds to match freshly generated output byte for byte (Chapter 27).

**Strict.** Pinned floating-point behavior: no contraction or
reassociation, fixed rounding, tested math functions.

**Trap.** A deliberate stop for a bug (overflow, division by zero, bad
index), as opposed to an error value.

**Unity build.** Compiling a program as one translation unit by including
every source file from one file. `coax0`'s C++ frontend is built this way.

**Untyped literal.** A literal such as `200` or `1.5` that has no type
until its use gives it one. It takes that type if the value fits, so
`x: u8 = 200;` needs no cast and `x: u8 = 300;` is error E0350.

**View.** A pointer and a count borrowing someone else's elements (`[] T`,
`string`).
