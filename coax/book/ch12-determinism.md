# 12. Determinism

> **Partly runs today.** Integer arithmetic in `coax0` is already exact
> and identical in debug and release at compile time. `strict`,
> `deterministic` and fixed point are designed (spec §13).

A deterministic simulation gives the same output for the same input, on
every machine, every build and every worker count. The Verse needs it
for replays, for saves that load identically, and for tests that compare
bytes instead of eyeballing screenshots. Ice Cream Man's rules and the
text reading are already held to it in the C++ engine, with
`-ffp-contract=off` on their files.

## 12.1 What breaks determinism

- **Floating-point contraction.** `a * b + c` may become one fused
  multiply-add on one CPU and two rounded operations on another.
- **Reassociation** by fast-math flags.
- **Approximate instructions** for reciprocals and square roots.
- **Platform math libraries** whose `sin` differs in the last bit.
- **Iteration order** of hash tables.
- **Uninitialized memory** that happens to hold different values.
- **Clocks and ambient randomness.**
- **Thread scheduling** deciding the order results are combined.

## 12.2 `strict`

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

## 12.3 `deterministic`

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

## 12.4 Fixed point

`fixed32` (Q16.16) and `fixed64` (Q32.32) are for values that must agree
bit for bit across languages and machines. Fixed point still needs
explicit scale, rounding and overflow rules, and it doesn't reproduce a
floating-point algorithm's output. It's a different algorithm. Coax adds
it once those rules have test vectors.

## 12.5 Compile time is already deterministic

`coax0`'s compile-time evaluator is the first deterministic piece of
Coax:

- It has no clock, randomness, files or output.
- Its limits count work, not time, so whether a program compiles doesn't
  depend on the machine.
- Overflow and division by zero are errors in debug and release alike,
  so a constant is never computed differently in two build modes.

## 12.6 Proving it

Determinism is a claim about a whole pipeline: serialization, replay
order, compiler flags, SIMD lowering and scheduling. The proof is always
a replay. Record real input, run it on two architectures at 1, 2, 4 and
8 workers, and compare outputs byte for byte. In Coax that is meant to
be one declaration:

```coax
#test replay "fixtures/icecream/*.trace" step_ice_cream;
```
