# 12. Jobs: captures as effects

A game frame is a list of systems: move everything, animate the
characters, update the camera, build the draw list. Most engines start
out running them one after another on one thread. Phones and laptops
have six, eight or more cores, so spreading the systems across them is
the obvious speedup. It's also the classic way to get bugs that show up
once a week on one customer's phone, when two systems happen to touch
the same memory at the same moment.

Languages answer this differently. Rust has a borrow checker that tracks
every reference. C++ leaves it to you. Engines in practice mostly use a
*frame graph*: each system declares what it reads and what it writes,
and a scheduler runs systems side by side only when their declarations
don't conflict. Coax plans to make that declaration part of the
language, as a capture list on a *job*.

In this chapter we'll first look at what `coax0` already tells you about
what a procedure can touch, and where that falls short, with a program
that runs today. Then we'll go through the planned job syntax: capture
lists, parallel loops and deterministic merges. We'll build a runnable
model of a deterministic parallel sum and check it with a `#forall`.
Last, we'll be precise about what capture lists would and wouldn't
guarantee.

> Note: jobs are design (spec section 12). `coax0` has no job syntax, no
> scheduler, no atomics and no race checking. The two programs in this
> chapter run today, single-threaded, and are in the book's examples.
> Everything written with `job` or `for_parallel` is design.

## What a procedure can touch today

Before adding anything, it's worth seeing how much `coax0` already
narrows what a procedure can change. Three rules from earlier chapters
combine here.

There are no global variables yet. If you try to declare one:

```jai
#import "Basic";

frames: int = 0;

main :: () {
  frames += 1;
  print("%\n", frames);
}
```

the compiler refuses:

```
$ coax check frames.coax
frames.coax:3:1: error[E0212]: global variables are not supported by coax0 yet
  frames: int = 0;
  ^
1 error
```

Top-level declarations are constants, and constants can't change.

Parameters are immutable. A procedure that tries to assign to one is
told to copy it into a local:

```
p.coax:1:22: error[E0344]: parameters cannot be assigned
  bump :: (x: int) { x += 1; }
                       ^
  fix: copy it into a local variable
```

Structs and arrays are passed by value, and there are no pointers. The
only way for a procedure to change memory that belongs to its caller is
through a view parameter, `[] T`, whose elements are the caller's array
(Chapter 6).

Put together, a `coax0` procedure's signature already lists everything
it can write: the views it's given. That's most of what a frame graph
needs. So why not just run two procedures in parallel whenever they're
given different arrays?

Because "different views" and "different memory" aren't the same thing.
Two views can point into the same array. Here's a `move` system, run
first-to-last and last-to-first, which is the kind of difference two
schedules of parallel work produce:

Filename: overlap.coax

```jai
#import "Basic";

// Move each position by its velocity, first element to last. The
// signature says what this can write: the elements of `position`.
move :: (position: [] f32, velocity: [] f32, dt: f32) {
  for i: 0..<position.count { position[i] += velocity[i] * dt; }
}

// The same update, last element to first, as another worker might
// happen to run it.
move_backward :: (position: [] f32, velocity: [] f32, dt: f32) {
  for k: 0..<position.count {
    i := position.count - 1 - k;
    position[i] += velocity[i] * dt;
  }
}

main :: () {
  p: [3] f32;
  v: [3] f32;
  for i: 0..<3 { p[i] = 1; v[i] = 1; }
  move(p[..], v[..], 1);
  print("separate, forward:  % % %\n", p[0], p[1], p[2]);
  for i: 0..<3 { p[i] = 1; }
  move_backward(p[..], v[..], 1);
  print("separate, backward: % % %\n", p[0], p[1], p[2]);

  // Two views of one array that overlap by two elements.
  a: [4] f32;
  for i: 0..<4 { a[i] = 1; }
  move(a[0..<3], a[1..<4], 1);
  print("overlap, forward:   % % % %\n", a[0], a[1], a[2], a[3]);
  for i: 0..<4 { a[i] = 1; }
  move_backward(a[0..<3], a[1..<4], 1);
  print("overlap, backward:  % % % %\n", a[0], a[1], a[2], a[3]);
}
```

*Listing 12-1: The same update in two orders, on separate and overlapping views*

```
$ coax run overlap.coax
separate, forward:  2.0 2.0 2.0
separate, backward: 2.0 2.0 2.0
overlap, forward:   2.0 2.0 2.0 1.0
overlap, backward:  4.0 3.0 2.0 1.0
```

With separate arrays, the order doesn't matter: each position reads only
its own velocity. With overlapping views, `position[1]` and
`velocity[0]` are the same element, so each step reads what the
previous step wrote, and the answer depends on the order. Split that
loop across workers and you'd get either answer, or a mixture, from one
run to the next.

The program is legal, and it should be: overlapping slices are
sometimes exactly what you want. But it shows what a parallel scheduler
would need beyond names. It has to know that two regions really don't
overlap, and today nothing tells it that.

## Jobs declare their effects

> Note: design. `coax0` doesn't parse `job` or capture lists.

Here's how the planned syntax looks for two systems:

```jai
move :: (dt: f32) job [write transforms.position, read velocities] {
  for_parallel transforms grain 1024 {
    it.position += velocities[it_index] * dt;
  }
}

animate :: () job [write poses, read transforms.orientation, read clips] { ... }
```

The square brackets are a *capture list*. Jai's primer introduced
capture lists as a way to limit what a block can see. Coax gives each
entry a mode, `read` or `write`, and makes the list a contract that the
compiler and the scheduler both rely on.

A job may touch only what it declares. Reading or writing anything
else, including a global once Coax has them, would be a compile error
that names the missing entry, so the fix is always "add this to the
list" or "don't touch that".

Effects can be per field. `move` writes `transforms.position` and
`animate` reads `transforms.orientation`. Those don't conflict, and
with SoA storage (Chapter 11) they are separate arrays in memory, so
the two jobs can run at the same time even though both mention
`transforms`.

The scheduler reads the lists. Two jobs run at the same time only if
neither writes something the other reads or writes. With a frame graph
known at compile time, that's checked while compiling; otherwise it's
checked when the frame is scheduled.

Effects compose. A job may call only jobs whose effects are a subset of
its own, or `pure` procedures that touch nothing. The compiler
summarizes effects through calls, including allocation (Chapter 6), so
a helper deep in the call tree can't quietly write something the job
didn't declare.

## Splitting a loop

> Note: design.

```jai
for_parallel transforms grain 1024 { ... }
```

`for_parallel` would split a range into chunks of at least `grain`
elements and spread them across workers. The grain is the smallest
piece of work worth sending to another core: too small and the
scheduling costs more than the work, too large and some cores sit idle.

Each chunk may write only its own index range of the arrays it writes,
so writes have to be indexed by `it_index`. A write to
`transforms[it_index + 1]` would be refused, because it reaches into the
next chunk, which another worker may be running. That's exactly the
overlap Listing 12-1 showed, ruled out by the loop's own shape.

## Deterministic merges

A parallel loop that computes one answer, such as a total, has to
combine the workers' partial results. The obvious way is to add each
worker's result as it finishes. That's a problem for anything that has
to replay the same way twice (Chapter 13), because floating-point
addition isn't associative: `(a + b) + c` and `a + (b + c)` can round
differently. If the order of the adds depends on how many workers there
were, or which finished first, so does the answer.

The plan for Coax:

```jai
total := for_parallel pulses grain 4096 reduce(sum) { yield it.value; }
```

Reductions such as sums, minimums and appends combine in a fixed
index-tree order, not in whichever order the workers finish. The tree is
built over blocks of `grain` elements, so its shape depends on the data,
not on the number of workers. The result is the same with 1 worker or 8.

We can't write `for_parallel` yet, but we can model both strategies in
ordinary code and see the difference. In the program below, a block is
`GRAIN` values summed left to right, the smallest piece a worker takes.
`by_worker` gives block `b` to worker `b % workers`, lets each worker
keep a running total, and adds the totals at the end. `by_tree` puts
each block's sum in its own slot, whichever worker computed it, and
combines the slots pairwise up a tree.

Filename: workers.coax

```jai
#import "Basic";

COUNT :: 64;
GRAIN :: 8;
BLOCKS :: COUNT / GRAIN;

// One block's sum, added left to right. A block is the smallest piece
// of work a worker takes.
block_sum :: (values: [] f32, block: int) -> f32 {
  sum: f32 = 0;
  for i: block * GRAIN..<(block + 1) * GRAIN { sum += values[i]; }
  return sum;
}

// Each worker keeps a running total of the blocks it took, and the
// totals are added at the end. The answer depends on how many workers.
by_worker :: (values: [] f32, workers: int) -> f32 {
  totals: [8] f32;
  for b: 0..<BLOCKS { totals[b % workers] += block_sum(values, b); }
  sum: f32 = 0;
  for w: 0..<workers { sum += totals[w]; }
  return sum;
}

// Each block's sum goes in its own slot, whichever worker computed it,
// and the slots are combined pairwise: (0+1)+(2+3), and so on up.
by_tree :: (values: [] f32, workers: int) -> f32 {
  sums: [BLOCKS] f32;
  for w: 0..<workers {
    b := w;
    while b < BLOCKS {
      sums[b] = block_sum(values, b);
      b += workers;
    }
  }
  width := BLOCKS;
  while width > 1 {
    width = width / 2;
    for i: 0..<width { sums[i] = sums[2 * i] + sums[2 * i + 1]; }
  }
  return sums[0];
}

// One big block and many small values: the order of the adds shows.
sample :: () -> [COUNT] f32 {
  values: [COUNT] f32;
  for i: 0..<COUNT { values[i] = ifx i < GRAIN then 100000000 else 3; }
  return values;
}

#forall "the tree gives one answer for any number of workers" workers: 1..<9 {
  values := sample();
  #assert by_tree(values[..], workers) == by_tree(values[..], 1);
}

main :: () {
  values := sample();
  workers := 1;
  while workers <= 8 {
    print("% workers: by worker %, by tree %\n",
          workers, by_worker(values[..], workers), by_tree(values[..], workers));
    workers *= 2;
  }
}
```

*Listing 12-2: Two ways to merge partial sums, at 1, 2, 4 and 8 workers*

The sample data is chosen to make rounding visible: eight values of
100,000,000 followed by 56 values of 3. At that size, an `f32` can only
represent multiples of 64, so whether the small values are added to each
other first or to the big total first changes the result.

```
$ coax run workers.coax
1 workers: by worker 800000000.0, by tree 800000128.0
2 workers: by worker 800000128.0, by tree 800000128.0
4 workers: by worker 800000192.0, by tree 800000128.0
8 workers: by worker 800000000.0, by tree 800000128.0
```

`by_worker` gives three different answers for the same data. With one
worker, each small block (worth 24) is added to the big total one at a
time, and each add rounds away to nothing. With four, the small blocks pile up in other
workers' totals before meeting the big one, and they survive. The exact
sum is 800,000,168, and none of the answers is it.

`by_tree` gives 800,000,128 every time. It isn't the most accurate of
the four (800,000,192 is closer); what it is, is the same. The tree
adds blocks 0 and 1, 2 and 3, and so on, regardless of which worker
computed which block. For replay, the same answer every time is what
matters.

The `#forall` in the listing states that as a rule, using the checks
from Chapter 10. It tries every worker count from 1 to 8:

```
$ coax test workers.coax
ok   #forall the tree gives one answer for any number of workers (8 cases, 18790 steps)
1 check, 1 passed, 0 failed
```

Change the assertion to use `by_worker` instead, and the check finds the
problem at once:

```
$ coax test workers.coax
FAIL #forall the tree gives one answer for any number of workers
workers.coax:53:3: error[E0406]: assertion failed
    #assert by_worker(values[..], workers) == by_worker(values[..], 1);
    ^
  note: in #forall the tree gives one answer for any number of workers
  counterexample: workers = 2
1 check, 0 passed, 1 failed
```

That is the test planned for real parallel reductions too: replay the
same input at 1, 2, 4 and 8 workers and compare the output bytes.

## What this is not

Capture lists aren't a borrow checker, and Coax won't claim more than it
checks. Before parallel jobs can be turned on, several things have to
hold, and Listing 12-1 already showed why the first one matters.

Field names aren't a proof. A view stored in one field can point into
another field's memory, just as our two views pointed into one array.
Effects mean something only when the regions they name are actually
disjoint. That needs a discipline for aliases and regions, plus checks
that ranges really are disjoint, before parallel execution is enabled.

Foreign calls need declared effects. A C function says nothing about
what it touches, so its effect summary would be declared by hand and
reviewed.

Atomics need a memory model. A future atomic API would state its
ordering for every operation and every target. Running on native
threads doesn't define one automatically.

Allocators are shared state. Each thread has its own context and scratch
arena, but the general-purpose allocator is still shared between them
and has to be synchronized.

The guarantee Coax aims for is narrower than "no data races", and more
useful for an engine: two jobs never touch conflicting data at the same
time, and when they would, the compiler tells you which declaration
would have to change. That's what a frame graph needs.

## Effects and the protocol

The same idea appears in the OpenAgents Protocol (Chapter 15). Its
`Effects` object describes what an agent action reads, writes and is
allowed to do, and a child action's effects are narrowed from its
parent's. In Coax, the idea governs memory inside one process.

The two are kept separate on purpose. A job's effects grant no authority
on their own: declaring `write` doesn't give code network, file or
payment access. Authority stays outside the program, held by the host.

## Summary

Running systems in parallel needs to know what each one touches. In
`coax0`, with no globals, immutable parameters and no pointers, a
procedure can write only through the views it's given, but views can
overlap, so names alone don't prove anything. The planned jobs add a
capture list of `read` and `write` effects, per field, that the compiler
enforces and the scheduler reads. `for_parallel` would split loops into
chunks that may write only their own range, and reductions would merge
in a fixed tree over blocks so the answer doesn't depend on the number
of workers. We modeled that merge in ordinary code and checked it with a
`#forall`.

That last property, the same answer every time, is the subject of the
next chapter: determinism, and what Coax does to make a replay come out
the same on every machine.
