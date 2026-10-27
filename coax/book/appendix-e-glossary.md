# Appendix E. Glossary

**AoS / SoA / blocked SoA.** Array of structs, struct of arrays, and SoA in
fixed-width blocks (`SOA(4)`). Storage choices for the same logical data
(Chapter 10).

**Arena.** An allocator that hands out memory by bumping a pointer and
frees everything at once on reset. Gives a group of objects one shared
lifetime.

**Budget.** A compile-time limit counted in work (steps, depth, nodes),
never in seconds, so validity doesn't depend on the machine.

**Capability.** A declared permission for build-time code to read files,
run processes or use the network. Pure `#run` code has none.

**Capture list.** The `[read a, write b]` list after a job's signature:
the data it may touch, and how.

**coax0.** The bootstrap compiler in `coax/`. Compiles a subset of Coax to
C11.

**Constraint.** A named set of operations a generic type parameter must
support, such as `Hashable`.

**Context.** The implicit value passed to ordinary procedures, carrying
the allocator, scratch arena and logger.

**Development / release build.** Development builds trap on overflow and
check bounds and handles. Release builds optimize and wrap integer
overflow. Compile-time evaluation behaves the same in both.

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
migration (Chapter 15).

**Provenance.** The recorded chain from a source call through a generator
to generated code and its emitted C.

**Resident host.** The part of the app that never reloads. It owns
persistent storage, platform services and GPU resources.

**Scratch.** Per-thread, short-lived memory from the context, reset in
bulk.

**Strict.** Pinned floating-point behavior: no contraction or
reassociation, fixed rounding, tested math functions.

**Trap.** A deliberate stop for a bug (overflow, division by zero, bad
index), as opposed to an error value.

**Unity build.** Compiling a program as one translation unit by including
every source file from one file. `coax0` is built this way.

**View.** A pointer and a count borrowing someone else's elements (`[] T`,
`string`).
