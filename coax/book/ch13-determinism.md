# Determinism

A deterministic replay requires the same input to produce the same output
under its specified execution contract. For a game world, record the
player's inputs and require the replay to reach the same state, including
the bits of its floats. Extending that promise to a phone, another backend
or a compiler upgrade needs qualification of those implementations and
numerical profiles; it does not follow from a matching run on one laptop.

Determinism is easy to lose without noticing, because most of the ways
to lose it are decisions the C compiler makes on your behalf. In this
chapter we'll look at what actually breaks it, see which of those
`coax0` closes off today, and then build a small simulation whose replay
we can check, at run time and at compile time, against a golden
fingerprint. We'll finish with the parts of the design that aren't
built yet.

> Note: these example transcripts were captured with `coax0` 0.10.0.
> Current execution-profile selection is described in Chapter 24. The
> `strict` and `deterministic` annotations, fixed-point types and
> `#test replay` near the end remain planned (spec §13). The retained
> examples do not qualify every CPU, GPU or compiler version.

## What breaks determinism

Here are the usual suspects, roughly in the order they bite:

The order in which a call's arguments are evaluated. C leaves it
unspecified, so `f(a(), b())` may call `b` first on another compiler or
at another optimization level. If `a` and `b` both draw from the same
random generator, the world diverges.

Floating-point contraction. The C compiler may turn `a * b + c` into one
fused multiply-add instruction that rounds once, instead of a multiply
and an add that round twice. The two answers can differ in the last bit,
and the compiler decides per build which one you get.

Reassociation and approximate instructions, which fast-math flags
enable: `(a + b) + c` becomes `a + (b + c)`, and `1 / sqrt(x)` becomes a
hardware estimate.

Math libraries. `exp` and `sin` come from the platform, and two
platforms' versions can differ in the last bit.

Memory that was never initialized, iteration over a hash table, clocks,
ambient randomness, and the order in which threads finish.

Each one is small. Any one of them is enough to make a replay drift, and
once it drifts it never comes back.

## Operands run left to right

Coax evaluates arguments, operands and indices in the order they appear
in the source, at run time and at compile time. Let's see it. We'll
write a procedure that prints a label before it returns its value, so
the output shows the order of the calls.

Filename: operand_order.coax

```jai
#import "Basic";

note :: (label: string, value: int) -> int {
  print("% ", label);
  return value;
}

pair :: (a: int, b: int) -> int { return a * 10 + b; }

main :: () {
  total := note("a", 1) + note("b", 2) * note("c", 3);
  print("= %\n", total);
  p := pair(note("first", 1), note("second", 2));
  print("= %\n", p);
}
```

*Listing 13-1: Calls that announce themselves*

```shell
$ coax run operand_order.coax
a b c = 7
first second = 12
```

The first line is the interesting one. Precedence says the
multiplication happens before the addition, and it does: `1 + 2 * 3` is
7. But precedence only decides how the results combine. It doesn't
decide when the calls run, and in Coax the calls always run in the order
you wrote them, `a` then `b` then `c`.

The C that `coax0` generates can't simply write `f(a(), b())` and hope,
because C makes no such promise. Instead, wherever an operand list
contains a call and another operand that could also call or trap, the
emitter stores each operand in a temporary, in order. Run
`coax c operand_order.coax` and look near the end of `main`:

```c
int64_t l_p = (coax_t4 = coax_p_note(COAX_STR("first", 5), INT64_C(1)), coax_t5 = coax_p_note(COAX_STR("second", 6), INT64_C(2)), coax_p_pair(coax_t4, coax_t5));
```

*Listing 13-2: The generated C for `pair(note("first", 1), note("second", 2))`*

C's comma operator sequences its left side before its right, so
`coax_t4` is computed before `coax_t5`, and only then is `pair` called.
Where the order can't be observed, as in `a + b * c` on plain locals,
the emitter doesn't bother with temporaries, so ordinary arithmetic
stays ordinary.

## One rounding per operation

Now for contraction. We need an expression where a fused multiply-add
gives a different answer from a separate multiply and add. Here's one.
Take `x` just above 1, square it, and subtract a number we chose to be
almost exactly the square:

Filename: ch13_residue.coax

```jai
#import "Basic";

residue :: (x: f32, c: f32) -> f32 {
  return x * x - c;
}

main :: () {
  x := 1.0f32 + 1.0f32 / 4096.0f32;
  c := 1.0f32 + 1.0f32 / 2048.0f32;
  print("%\n", residue(x, c));
  print("%\n", #run residue(1.0f32 + 1.0f32 / 4096.0f32, 1.0f32 + 1.0f32 / 2048.0f32));
}
```

*Listing 13-3: A subtraction that exposes how `x * x` was rounded*

The exact square of `x` is 1 + 2⁻¹¹ + 2⁻²⁴. An `f32` near 1 can't hold
that last 2⁻²⁴, so a multiply that rounds gives exactly 1 + 2⁻¹¹, which
is `c`, and the residue is zero. A fused multiply-add keeps the full
product until after the subtraction and gives 2⁻²⁴, about 6e-8.

```shell
$ coax run ch13_residue.coax
0.0
0.0
$ coax run ch13_residue.coax --release
0.0
0.0
```

Zero every time, at run time and at compile time, in both build modes.
Now here is the same function in plain C, in
`examples/lib/ch13_residue.c` (the `volatile` stops the compiler from
working out the answer at compile time):

```c
#include <stdio.h>

float residue(float x, float c) { return x * x - c; }

int main(void) {
  volatile float x = 1.0f + 1.0f / 4096.0f, c = 1.0f + 1.0f / 2048.0f;
  printf("%g\n", residue(x, c));
  return 0;
}
```

*Listing 13-4: The same residue in C*

On an Apple silicon Mac with Apple clang 21:

```shell
$ cc -O2 ch13_residue.c -o residue && ./residue
5.96046e-08
$ cc -O2 -ffp-contract=off ch13_residue.c -o residue && ./residue
0
```

Clang fused the multiply and the subtract by default, even though
nothing in the source asked for it. The flag `-ffp-contract=off` turns
that off. A C++ engine that wants deterministic floats has to remember
to pass that flag for every file that matters, and a function that moves
to another file can silently lose it.

Coax puts the rule in the code instead. Every C file `coax0` generates
starts with:

```c
/* Coax float arithmetic is plain IEEE, one rounding per operation: no fused
   multiply-add, so results match compile-time evaluation and other compilers. */
#pragma STDC FP_CONTRACT OFF
```

So `x * x - c` is always a multiply, rounded, then a subtract, rounded.
Look at what `coax c ch13_residue.coax` produces for `residue`:

```c
static float coax_p_residue(float l_x, float l_c) {
  return ((l_x * l_x) - l_c);
}
```

That's the same expression clang fused in Listing 13-4, but the pragma
at the top of the file forbids the fusion. `coax0` also passes no
fast-math flags to the C compiler. The legacy `--release` preset selects
`-O2` instead of `-O0` and wrapping runtime integer arithmetic, along with
release build-profile branches. Version 0.25.0 lets you select arithmetic,
native optimization and the build profile independently (Chapter 24);
none of those choices enables fast math.

## Two places the build mode still matters

That last point deserves a closer look, because it's one place where a
debug and a release build of the same Coax program behave differently.
In [Chapter 1](ch01-getting-started.md) we added one to the largest
`int`:

```shell
$ coax run overflow.coax
before: 9223372036854775807
overflow.coax:6: integer overflow in '+'
$ coax run overflow.coax --release
before: 9223372036854775807
after: -9223372036854775808
```

A debug build traps; a release build wraps. Either way, a program that
overflows has a bug, and the debug build is there to find it. But if
you run a replay in release and it happens to overflow, you'll get the
wrapped result without a warning. If you mean to wrap, say so by
widening and reducing, as we'll do in a moment for a hash function.

The second place is memory you asked not to initialize. Coax starts
scalars at zero and structs at their field defaults, so `w: World;`
gives you a known value. You can opt out with `---`, which is useful for
a large buffer you're about to overwrite. Here's what happens if you
read such a value before writing it:

```jai
#import "Basic";

total :: () -> int {
  sum: int = ---;
  for i: 0..<4 { sum += i; }
  return sum;
}

main :: () {
  print("%\n", total());
}
```

*Listing 13-5: Reading a variable that was never initialized*

```shell
$ coax run garbage.coax
4294967303
$ coax run garbage.coax --release
8362049776
```

The answer depends on whatever was on the stack, so the two builds
disagree, and your machine may print other numbers again. At run time
`coax0` doesn't detect this. At compile time it does: add the line
`FOLDED :: #run total();` just below `total` and the compiler refuses.

```shell
$ coax run garbage.coax
garbage.coax:5:22: error[E0405]: compile-time read of an uninitialized variable
    for i: 0..<4 { sum += i; }
                       ^
  note: compile-time call at garbage.coax:9:16
1 error
```

The fix is to leave off the `---`. Use it only when the next thing you
do is write every element.

## Compile time has a bounded execution contract

The compile-time evaluator you met in [Chapter 9](ch09-compile-time.md)
removes several sources of variation. Its results still belong to the
selected compiler and host math environment:

- It has no clock, no randomness, no files and no output. A `#run` that
  tries to print is an error.
- Its limits count work, not time. One compilation gets 100,000 steps by default,
  so whether a program compiles doesn't depend on how fast the machine
  is.
- Overflow, division by zero, bounds and bad casts are errors in debug
  and release alike, so a constant can't come out differently in the
  two build modes.
- It uses the language's integer widths and ordered floating-point
  operations. Compile-time integers remain checked even when a runtime
  profile wraps; host math calls are not a portable cross-target oracle.

For a compatible operation/profile, compare the `#run` result with the
actual emitted execution. This fixture does that for a float recurrence
and an integer scrambler:

Filename: same_answer.coax

```jai
#import "Basic";

// A small float recurrence: every * and + rounds once, and none are fused.
settle :: (steps: int) -> f32 {
  x: f32 = 0.1;
  v: f32 = 0;
  for i: 0..<steps {
    v = v * 0.9 + (1 - x) * 0.05;
    x += v;
  }
  return x;
}

// xorshift32: << drops high bits, so nothing here can overflow.
shuffle :: (seed: u32, rounds: int) -> u32 {
  h := seed;
  for i: 0..<rounds {
    h ^= h << 13;
    h ^= h >> 17;
    h ^= h << 5;
  }
  return h;
}

AT_COMPILE :: #run settle(20);
SHUFFLED :: #run shuffle(7, 100);

main :: () {
  at_run := settle(20);
  print("% % %\n", AT_COMPILE, at_run, AT_COMPILE == at_run);
  print("% %\n", SHUFFLED, SHUFFLED == shuffle(7, 100));
}
```

*Listing 13-6: The same computations at compile time and at run time*

```shell
$ coax run same_answer.coax
1.1090176 1.1090176 true
1513575512 true
```

The output is the same with `--release`. `AT_COMPILE` reaches the C as
the exact bits the compiler computed, written as a hexadecimal float
literal (`0x1.1be894p+0f`) so nothing is lost by printing and parsing a
decimal. In this retained native fixture, the runtime loop lands on the
same bits at `-O0` and `-O2`. That is evidence for these inputs and
operations, not a general evaluator/native/GPU equivalence result. Here
exact float equality is the specific contract being tested.

## Building a replay

Now let's put these pieces together and build something we can replay.
Our world is eight motes on a line. Each frame, every mote's velocity
decays a little and picks up some random jitter, and the frame's input
may nudge one mote. We'll run 120 frames from a recorded input sequence
and reduce the whole world to one 32-bit fingerprint.

The first decision is where randomness comes from. A global generator
that anything can call is the classic way to lose a replay: add one
extra call somewhere and every later number shifts. So we'll make the
generator a value. Whoever steps the world passes it in and gets the
advanced generator back.

Filename: ch13_replay.coax

```jai
#import "Basic";

// The random generator is a value: whoever steps the world passes it in,
// and gets the advanced generator back.
Rng :: struct {
  state: u32;
}

Draw :: struct {
  value: f32;
  rng: Rng;
}

next :: (r: Rng) -> Draw {
  h := r.state;
  h ^= h << 13;
  h ^= h >> 17;
  h ^= h << 5;
  return Draw.{value = f32(h >> 8) / 16777216.0f32, rng = Rng.{state = h}};
}

World :: struct {
  x: [8] f32;
  v: [8] f32;
  rng: Rng;
}

// One frame. `push` is this frame's input: the mote the player nudged,
// or -1 for none.
step :: (before: World, push: int) -> World {
  w := before;
  for i: 0..<8 {
    d := next(w.rng);
    w.rng = d.rng;
    w.v[i] = w.v[i] * 0.98f32 + (d.value - 0.5f32) * 0.01f32;
    if i == push { w.v[i] += 0.05f32; }
    w.x[i] += w.v[i];
  }
  return w;
}

// One step of FNV-1a. The multiply is meant to wrap at 32 bits, so we do it
// in 64 bits and keep the low half; a plain u32 multiply would overflow.
mix :: (h: u32, bits: u32) -> u32 {
  return cast(u32) ((cast(u64) (h ^ bits) * 16777619) % 4294967296);
}

// FNV-1a over the bits of every position and velocity.
fingerprint :: (w: World) -> u32 {
  h: u32 = 2166136261;
  for i: 0..<8 {
    h = mix(h, f32_bits(w.x[i]));
    h = mix(h, f32_bits(w.v[i]));
  }
  return h;
}

INPUTS :: [12] int.[3, -1, -1, 5, 5, -1, 0, -1, 7, -1, -1, 2];

replay :: (seed: u32, frames: int) -> u32 {
  w: World;
  w.rng = Rng.{state = seed};
  for f: 0..<frames {
    w = step(w, INPUTS[f % 12]);
  }
  return fingerprint(w);
}

AT_COMPILE :: #run replay(2026, 120);

main :: () {
  first := replay(2026, 120);
  second := replay(2026, 120);
  print("run 1:      %\n", first);
  print("run 2:      %\n", second);
  print("compiled:   %\n", AT_COMPILE);
  print("other seed: %\n", replay(2027, 120));
}

#test "the recorded replay lands on its golden fingerprint" {
  #assert replay(2026, 120) == 1882684683;
}
```

*Listing 13-7: A small world, replayed from recorded input*

There's a fair amount here, so let's take it a piece at a time.

`next` is the same xorshift32 scrambler as `shuffle` in Listing 13-6.
It takes an `Rng` by value and returns a `Draw` holding both the random
number and the generator's new state. Coax procedures can't assign to
their parameters, so `next` copies `r.state` into a local `h` first.
The top 24 bits of `h`, divided by 2²⁴, give a float in [0, 1).

`step` follows the same shape. It takes the world before the frame and
returns the world after it. Inside, it draws one random number per mote
in a fixed order, 0 through 7, and threads the generator through `w.rng`
as it goes. Because the draws happen in loop order and Coax evaluates
operands left to right, there is exactly one sequence of draws for a
given seed and input.

`fingerprint` reduces the world to one number with FNV-1a, a simple
hash: for each value, XOR it in and multiply by a prime. We hash the
bits of each float with `f32_bits`, so the input to the hash includes
differences hidden by decimal printing. A 32-bit FNV fingerprint can
collide: equal fingerprints alone do not prove equal complete states.
Compare the state bytes when that exact claim matters. FNV's multiply
is supposed to wrap at 32 bits, and that is where we have to be careful. If we wrote the obvious line,

```jai
  return (h ^ bits) * 16777619;
```

the compiler would stop us:

```shell
$ coax run ch13_replay.coax
ch13_replay.coax:45:21: error[E0407]: integer overflow in u32 during compile-time evaluation
    return (h ^ bits) * 16777619;
                      ^
  note: compile-time call at ch13_replay.coax:52:9
  note: compile-time call at ch13_replay.coax:66:10
  note: compile-time call at ch13_replay.coax:69:20
1 error
```

The notes trace the call chain back to the `#run` on line 69. Without
that `#run`, a debug build would trap at run time and a release build
would quietly wrap, which is the build-mode difference from earlier.
So `mix` says what it means: multiply in 64 bits, where the product of
a `u32` and a 25-bit prime can't overflow, then keep the remainder
modulo 2³², the low 32 bits.

`replay` builds a world (zeroed, because it's declared without `---`),
seeds its generator, and steps it through 120 frames of `INPUTS`,
repeating the 12 recorded inputs.

Then we run it four ways: twice at run time, once at compile time, and
once with a different seed to make sure the fingerprint actually depends
on the simulation.

```shell
$ coax run ch13_replay.coax
run 1:      1882684683
run 2:      1882684683
compiled:   1882684683
other seed: 241112748
```

The retained `--release` run prints the same four lines. The evaluator
and emitted native code are separate execution paths agreeing on this
fixture's fingerprint. They can still share assumptions or bugs, and the
fingerprint is a compact regression check rather than a proof of complete
state equality or another target's behavior.

### Locking the replay in with a test

A replay is only useful if someone notices when it changes. The `#test`
at the bottom of Listing 13-7 records the golden fingerprint, and
`coax test` runs it (see [Chapter 10](ch10-writing-checks.md) for the
whole checking toolkit):

```shell
$ coax test ch13_replay.coax
ok   #test the recorded replay lands on its golden fingerprint (1 case, 70272 steps)
1 check, 1 passed, 0 failed
```

Suppose someone changes the damping from `0.98` to `0.981`, or reorders
the draws in `step`. The fingerprint moves and the test says so. To see
what that looks like, change the golden number to `1882684684`:

```shell
$ coax test ch13_replay.coax
FAIL #test the recorded replay lands on its golden fingerprint
ch13_replay.coax:81:3: error[E0406]: assertion failed
    #assert replay(2026, 120) == 1882684683;
    ^
  note: in #test the recorded replay lands on its golden fingerprint
1 check, 0 passed, 1 failed
```

When the change was intended, you update the golden number in the same
commit, and the review shows that the simulation's output changed on
purpose.

### The step budget

Why 120 frames and not 600? Try it. Change both `120`s in `AT_COMPILE`
and `main` to `600`:

```shell
$ coax run ch13_replay.coax
ch13_replay.coax:35:54: error[E0400]: compile-time evaluation exceeded 100000 steps
      w.v[i] = w.v[i] * 0.98f32 + (d.value - 0.5f32) * 0.01f32;
                                                       ^
  note: compile-time call at ch13_replay.coax:64:9
  note: compile-time call at ch13_replay.coax:69:20
1 error
```

The test above already used 70,272 steps for 120 frames, so 600 frames
doesn't fit in one compilation's default 100,000. Because the limit counts
steps rather than seconds, this error comes out the same on every
machine; a slow build server can't fail where your laptop passed. For
longer checks, `coax test` takes `--steps N`. A compile-time build can
select `--run-steps N` independently, up to 250,000,000. For a long replay in a shipping build, leave it at run time
and keep the golden fingerprint in a test.

## What isn't pinned yet

Everything so far is real and checked. Several causes from the start of
the chapter are still open, and it's worth knowing exactly which.

Math functions. Native built-ins and compile-time evaluation use host
math implementations; a matching fixture does not establish that all
`exp`, `sqrt` or `sin` calls agree, even before moving to another target.
The replay above uses only `+`, `-`, `*` and `/`. Disabling contraction
and avoiding transcendental functions remove two sources of difference,
but do not by themselves qualify cross-backend bit identity. Storage and
intermediate precision, subnormal handling, conversions, signed zero,
NaNs and the selected compiler flags also matter. A portable exact replay
needs a named numerical contract and actual target comparisons.

The GPU. Each shader or compute backend has its own compiler, options,
precision rules and device behavior. Turning fast math off is useful but
is not a CPU/GPU identity proof. Keep replay state on its qualified
execution path; moving it to a GPU requires the selected workload and
numerical profile to pass actual device comparisons. Chapter 14 separates
CPU math checks from shader execution evidence.

Threads. The Coax job/effect scheduler remains planned. Native hosts can
still call Coax libraries concurrently; those hosts own synchronization
and ordering today. [Chapter 12](ch12-jobs.md) describes the additional
language contract needed for worker-count-independent replay.

Hash tables. There is no general built-in ordered-table contract. Any
library or host collection used by a replay must state its iteration order.

## `strict` and `deterministic` (planned)

> Note: Nothing in this section is implemented in `coax0`. It describes
> the design in spec §13.

Today the guarantees above apply to every Coax program. The plan adds
two annotations for code that must promise more.

```jai
step_ice_cream :: (s: *Sim, input: Input) strict { ... }
```

Inside `strict` code, floating point is pinned completely: no
contraction, no reassociation, no approximate reciprocal or square-root
instructions, fixed rounding, and math functions from a pinned
implementation tested against exact vectors. For OpenAgents that means
keeping the fdlibm behavior the old C++ engine already relied on, not a
new claim of correctly rounded math. Like the `#pragma` in today's
output, the rule travels with the function instead of living in a
per-file build flag.

```jai
replay_step :: (w: *World, input: Input) deterministic strict { ... }
```

`deterministic` is a checked promise, and the check is transitive. A
deterministic procedure can't read a clock, read uninitialized memory,
iterate a hash-ordered table, or use ambient randomness, and it can't
call anything that might. Our `Rng` value in Listing 13-7 is exactly the
style this annotation will enforce. Today it's a convention you follow;
later the compiler will reject code that breaks it.

## Fixed point (planned)

> Note: Not implemented. `fixed32` and `fixed64` are planned types.

`fixed32` (Q16.16) and `fixed64` (Q32.32) are meant for values that
must agree bit for bit across languages and machines, even where float
behavior can't be pinned. Fixed point isn't a free fix, though. It still
needs explicit rules for scale, rounding and overflow, and it doesn't
reproduce a float algorithm's output: switching a simulation to fixed
point changes the simulation. Coax will add these types once those rules
have test vectors.

## Replay tests (planned)

> Note: Not implemented. Today, write a golden `#test` as in Listing
> 13-7.

Determinism is a claim about a whole pipeline: how input is serialized,
the order of replay, compiler flags, SIMD lowering and job scheduling.
The only convincing proof is a replay of real input on two
architectures at 1, 2, 4 and 8 workers, comparing outputs byte for
byte. The plan is to make that one declaration:

```jai
#test replay "fixtures/icecream/*.trace" step_ice_cream;
```

## Summary

Coax closes off two of C's biggest sources of nondeterminism in every
program: operands are evaluated left to right, and floats round once per
operation with no fused multiply-adds, enforced by a pragma in the
generated C rather than a build flag you might forget. The compile-time
evaluator uses the same arithmetic, which let us check a 120-frame
replay against native code and lock its fingerprint into a `#test`. We
also saw where you still need care: integer overflow traps in debug and
wraps in release, `---` memory holds whatever was there, math functions
come from the platform, and the GPU uses fast math.

In the next chapter we'll take the same kind of f32 code to the GPU,
compiling Coax procedures to Metal for the iOS app's shaders.
