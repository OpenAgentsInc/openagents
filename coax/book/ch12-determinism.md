# 12. Determinism

> **Partly runs today.** On `coax0` 0.3.0, left-to-right evaluation,
> unfused IEEE float arithmetic and compile-time arithmetic that matches
> run time (12.2 and 12.6) run, and their examples are checked. `strict`,
> `deterministic`, fixed point and replay tests (12.3 to 12.5, 12.7) are
> *Design* (spec §13).

A deterministic simulation gives the same output for the same input, on
every machine, every build and every worker count. The Verse needs it
for replays, for saves that load identically, and for tests that compare
bytes instead of eyeballing screenshots. Ice Cream Man's rules and the
text reading are already held to it in the C++ engine, with
`-ffp-contract=off` on their files.

## 12.1 What breaks determinism

- **Evaluation order.** C leaves the order of a call's arguments, and of
  most operands, unspecified. `f(a(), b())` may call `b` first on another
  compiler or at another optimization level.
- **Floating-point contraction.** `a * b + c` may become one fused
  multiply-add on one CPU and two rounded operations on another.
- **Reassociation** by fast-math flags.
- **Approximate instructions** for reciprocals and square roots.
- **Platform math libraries** whose `sin` differs in the last bit.
- **Iteration order** of hash tables.
- **Uninitialized memory** that happens to hold different values.
- **Clocks and ambient randomness.**
- **Thread scheduling** deciding the order results are combined.

## 12.2 What `coax0` pins today

Two of the causes above are fixed in every program `coax0` compiles.

**Operands run left to right.** Arguments, operands and indices are
evaluated in source order, at compile time and at run time:

```coax
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

```
a b c = 7
first second = 12
```

Precedence decides how the results combine, not when the calls run.
Generated C can't rely on C's order, so where an operand list has a call
and another operand that could call or trap, the emitter stores each one
in a temporary, in order:

```c
int64_t l_p = (coax_t4 = coax_p_note(COAX_STR("first", 5), INT64_C(1)), coax_t5 = coax_p_note(COAX_STR("second", 6), INT64_C(2)), coax_p_pair(coax_t4, coax_t5));
```

The comma operator sequences them. Where order can't be observed, such
as `a + b * c` on plain locals, there are no temporaries.

**Floats round once per operation.** Every generated C file starts with:

```c
/* Coax float arithmetic is plain IEEE, one rounding per operation: no fused
   multiply-add, so results match compile-time evaluation and other compilers. */
#pragma STDC FP_CONTRACT OFF
```

So `v * 0.9 + w` is a multiply, rounded, then an add, rounded, never a
fused multiply-add that rounds once. That is the engine's
`-ffp-contract=off`, moved from a per-file build flag into the code the
compiler writes. `coax0` passes no fast-math flags, and `--release`
changes only the optimization level and integer overflow.

## 12.3 `strict` (design)

```coax
step_ice_cream :: (s: *Sim, input: Input) strict { ... }
```

Inside `strict` code, floating point is pinned: no contraction, no
reassociation, no approximate reciprocal or square-root instructions, and
fixed rounding. Math functions come from a pinned implementation tested
against exact vectors. For the engine that means its existing fdlibm
behavior, not a new claim of correctly rounded math.

The point is that the rule **travels with the code**. Today it's a
per-file flag in CMake, and moving a function to another file silently
drops it.

## 12.4 `deterministic` (design)

```coax
replay_step :: (w: *World, input: Input) deterministic strict { ... }
```

`deterministic` is checked, transitively. A deterministic procedure
can't:

- read a clock;
- read uninitialized memory;
- iterate a hash-order table;
- use ambient randomness (an `Rng` is a value you pass in);
- call anything whose effects allow any of the above.

## 12.5 Fixed point (design)

`fixed32` (Q16.16) and `fixed64` (Q32.32) are for values that must agree
bit for bit across languages and machines. Fixed point still needs
explicit scale, rounding and overflow rules, and it doesn't reproduce a
floating-point algorithm's output. It's a different algorithm. Coax adds
it once those rules have test vectors.

## 12.6 Compile time is already deterministic

`coax0`'s compile-time evaluator is the first deterministic piece of
Coax:

- It has no clock, randomness, files or output.
- Its limits count work, not time, so whether a program compiles doesn't
  depend on the machine.
- Overflow, division by zero, bounds and casts are errors in debug and
  release alike, so a constant is never computed differently in two
  build modes.
- Its arithmetic is the run-time arithmetic: the same integer widths,
  and the same one-rounding-per-operation floats.

The last point is what lets a `#run` result stand in for a run-time one:

```coax
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

```
1.1090176 1.1090176 true
1513575512 true
```

The output is the same with `--release`. `AT_COMPILE` reaches the C as
the exact bits the compiler computed (`0x1.1be894p+0f`, a hex float
literal, so nothing is lost in printing and parsing), and the run-time
loop lands on the same bits whether the C compiler builds it at `-O0`
(debug) or `-O2` (release). Comparing floats with `==` is usually a
mistake; here it's the claim being tested.

One difference is on purpose: a release build wraps integer overflow at
run time, but compile-time overflow is always `E0407`. A table built
with `#run` never depends on which mode built it.

## 12.7 Proving it (design)

Determinism is a claim about a whole pipeline: serialization, replay
order, compiler flags, SIMD lowering and scheduling. The proof is always
a replay. Record real input, run it on two architectures at 1, 2, 4 and
8 workers, and compare outputs byte for byte. In Coax that is meant to
be one declaration:

```coax
#test replay "fixtures/icecream/*.trace" step_ice_cream;
```
