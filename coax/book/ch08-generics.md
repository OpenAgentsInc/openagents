# 8. Generics and constraints

> **Design.** `coax0` has no generics. This chapter follows spec §7.2.

## 8.1 Type parameters

```jai
max :: (a: $T/Ordered, b: T) -> T {
  return ifx a > b then a else b;
}

m := max(3, 9);          // T is int
n := max(1.5, 0.25);     // T is f64
```

`$T` declares a type parameter and binds it from the first argument.
Later uses of `T` must match. Each distinct `T` produces a specialized
procedure, compiled like any other. There's no runtime dispatch, no boxed
object and no vtable.

`$N` does the same for a compile-time value:

```jai
Pool :: struct ($T: Type, $N: int) {
  items: [N] T;
  free:  [N] u32;
  count: u32;
}

particles: Pool(Particle, 4096);
```

## 8.2 Constraints

`/Ordered` is a **constraint**: the operations a type must have for the
procedure's body to make sense.

```jai
Hashable :: constraint ($T) {
  hash: (T) -> u64;
  eq:   (T, T) -> bool;
}

Table :: struct ($K: Type/Hashable, $V: Type) { ... }
```

Constraints are checked when a generic is used, and the error names the
call and the missing operation:

```
error: Table needs Hashable for 'Grid_Key', which has no 'hash'
  fix: add #derive(hash) to Grid_Key
```

Without a constraint, the error appears deep inside the generic's body,
in code the caller never wrote, which is the C++ template experience.
Jai has constraints too (`$T/Base`, `$T/interface Matchable`). Coax makes
them the normal way to write a generic, so a generic's signature tells
you everything it needs.

A constraint is a compile-time requirement only. It never becomes a
runtime interface object.

## 8.3 Specialization costs something

Specialization removes runtime decisions and gives the optimizer concrete
code. It also multiplies code: ten key types make ten tables. The costs
are build time, binary size and instruction cache.

So Coax makes them visible:

- Instances are cached by their full key: resolved types, constant
  arguments, target, and relevant effects and options.
- The compiler reports how many instances each generic produced, their
  code size, and the time spent generating them.
- Recursive specialization has a limit, and failing prints the
  instantiation trace from your source.

Measure instance counts in a real codebase instead of assuming generics
are free.

## 8.4 Baking arguments

```jai
draw_64 :: #bake_arguments draw(limit = 64);
```

`#bake_arguments` makes a specialized copy with some arguments fixed at
compile time. The baked ones become constants, and the rest stay
ordinary parameters. It's explicit on purpose. A compiler that
specialized every call with constant arguments would hide its code-size
and build-time costs. Baked copies use the same cache and budgets as
other specializations.

## 8.5 Computing types (later)

Forrest Smith's typesafe-units experiment in Jai uses `#modify` and type
inspection to compute result types from physical dimensions, so that
metres divided by seconds has the type metres-per-second. That's real
semantic power. Coax may add a hook like it: a pure, bounded type
computation with an explicit input and output, not permission to change
arbitrary compiler state. Ordinary constraints and their diagnostics come
first, and a separate `#modify` will have to show it's needed.

## 8.6 Operators

```jai
operator + :: (a: Money, b: Money) -> Money pure { ... }
```

User types can define arithmetic, comparison and indexing operators.
Forrest Smith's Jai code shows how much a small vector type gains from
them. In Coax an operator must be **pure**: no allocation, no I/O and no
failure. Math syntax should cost what math costs. Assignment, field
access, `&&`, `||` and calls can't be overloaded. The built-in math types
(`v2 v3 v4 quat m3 m4`) already have their operators.
