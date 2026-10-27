# 11. Jobs: captures as effects

> **Design.** Spec §12. No job syntax, scheduler integration, atomics or
> race checking exists yet. This chapter explains the idea and the
> conditions it has to meet before Coax can claim it.

## 11.1 The problem

The Verse runs its systems one after another on one thread today. Moving
them onto several cores is the obvious speedup and the classic source of
bugs that appear once a week on one customer's phone. Rust answers with a
borrow checker over every reference. C++ answers with nothing. Engines in
practice use a **frame graph**: each system declares what it reads and
writes, and a scheduler runs non-conflicting systems in parallel.

Coax makes that declaration part of the language.

## 11.2 Jobs declare their effects

```coax
move :: (dt: f32) job [write transforms.position, read velocities] {
  for_parallel transforms grain 1024 {
    it.position += velocities[it_index] * dt;
  }
}

animate :: () job [write poses, read transforms.orientation, read clips] { ... }
```

The square brackets are a **capture list**. Jai's primer introduced
capture lists as a way to limit what a block can see. Coax gives each
entry a mode, `read` or `write`, and makes the list a contract:

1. A job may touch only what it declares. Touching anything else,
   including a global, is a compile error that names the missing entry.
2. Effects can be **per field**. `move` writes `transforms.position` and
   `animate` reads `transforms.orientation`. Those don't conflict, and
   SoA storage (Chapter 10) makes them separate memory.
3. **The scheduler reads the lists.** Two jobs run at the same time only
   if neither writes what the other reads or writes. With a static frame
   graph that's checked at compile time, and otherwise when the frame is
   scheduled.
4. **A job calls only jobs with a subset of its effects, or `pure`
   procedures.** Effects are summarized transitively, including
   allocation (Chapter 6).

## 11.3 Splitting a loop

```coax
for_parallel transforms grain 1024 { ... }
```

`for_parallel` splits a range into chunks of at least `grain` elements,
spread across workers. Each chunk may write only its own index range of
the arrays it writes, so writes must be indexed by `it_index`. A write to
`transforms[it_index + 1]` is refused.

## 11.4 Deterministic merges

```coax
total := for_parallel pulses grain 4096 reduce(sum) { yield it.value; }
```

Reductions such as sums, minimums and appends combine in a fixed
index-tree order, not in whichever order the workers finish. The result
is the same with 1 worker or 8. The test for this is literal: replay the
same input at 1, 2, 4 and 8 workers and compare the output bytes.

## 11.5 What this is not

Capture-list effects are **not a borrow checker**, and Coax won't claim
more than it checks:

- **Field names aren't a proof.** A view inside one field can point into
  another field's memory. Effects mean something only when the regions
  they name are actually disjoint. That needs an alias and region
  discipline, plus validation of disjoint ranges, before parallel
  execution is enabled.
- **Foreign calls need declared effects.** A C function says nothing about
  what it touches, so its effect summary is declared and reviewed by
  hand.
- **Atomics need a memory model.** A future atomic API states its ordering
  for every operation and target. Running on native threads doesn't
  define one automatically.
- **Allocators are shared state.** Each thread has its own context and
  scratch arena, but the general-purpose allocator is still synchronized.

The guarantee Coax aims for is narrower than "no data races" and more
useful for an engine: **two jobs never touch conflicting data at the same
time, and the compiler tells you which declaration would have to change
for them to do so.** That is what a frame graph needs.

## 11.6 Effects and the protocol

This mirrors the OpenAgents Protocol's `Effects` object, which describes
what an agent action reads, writes and is allowed to do, with child
effects narrowed from their parent's. In Coax, the same idea governs
memory inside one process. Effects grant no authority on their own: a
`write` declaration doesn't give code network, file or payment access.
Authority stays external.
