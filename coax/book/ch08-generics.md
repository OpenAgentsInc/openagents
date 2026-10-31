# 8. Generics and constraints

Sooner or later you write the same procedure twice. Once for `int`, then
again for `f64`, then a third time for `u8`, each copy identical except
for the type names. *Generics* let us write that procedure once, with
the type left as a parameter, and have the compiler produce each
version we need. *Constraints* say what a type must be able to do for
the generic to make sense, so a mistake is reported where the generic
is used rather than somewhere deep inside it.

In this chapter we'll start with a comparison algorithm that runs today
and has exactly that duplication problem. We'll remove its first layer
with an ordinary procedure, add the built-in constraint that allows a
generic comparison, and then distinguish that working subset from the
remaining design.

<!-- capability-claim: coax.generics.current -->
Generic procedures bind `$T` from parameters, instantiate checked copies,
and bind fixed-array lengths through `[$N] T`. Unconstrained parameters
support copying, assignment, indexing and iteration. Since 0.40.0, built-in `equatable`,
`ordered`, `numeric` and `copyable` constraints now state additional
operations where a type parameter binds, in procedures and generic structs.
The compiler checks bodies against those bounds, rejects incompatible
bindings at their use, and reports concrete instance failures with a trace.
Generic structs still hold fixed storage: a view type, or an array or
struct containing one, is refused as a struct binding with E0802. This is
not general ownership safety, and `copyable` does not lift that restriction.
User-defined constraints, operator overloading and placeholder layout
queries remain outside the current subset.
<!-- /capability-claim -->

## Removing duplication by extracting a procedure

Let's begin with a small program that finds the largest number in a
list.

Filename: examples/ch08_largest1.coax

```jai
#import "Basic";

main :: () {
  scores := [5] int.[34, 50, 25, 100, 65];

  largest := scores[0];
  for n: scores {
    if n > largest { largest = n; }
  }
  print("the largest score is %\n", largest);
}
```

*Listing 8-1: Finding the largest number in an array*

We keep the first element in `largest`, then walk the array and replace
`largest` whenever we see something bigger.

```shell
$ coax run examples/ch08_largest1.coax
the largest score is 100
```

Now suppose we need the same thing for a second array. We could copy
those four lines and change the names, but then we'd have two copies to
keep in step, and a fix to one would have to be made to the other. The
usual answer is to pull the logic into a procedure that takes the data
as a parameter:

Filename: examples/ch08_largest2.coax

```jai
#import "Basic";

largest_int :: (values: [] int) -> int {
  result := values[0];
  for n: values {
    if n > result { result = n; }
  }
  return result;
}

main :: () {
  scores := [5] int.[34, 50, 25, 100, 65];
  print("the largest score is %\n", largest_int(scores[..]));

  depths := [4] int.[-12, -3, -40, -7];
  print("the shallowest depth is %\n", largest_int(depths[..]));
}
```

*Listing 8-2: Extracting the logic into a procedure that takes a view*

The parameter is a view, `[] int`, so the procedure works on an array of
any length. `scores[..]` makes a view of the whole array
([Chapter 6](ch06-memory.md) covers views).

```shell
$ coax run examples/ch08_largest2.coax
the largest score is 100
the shallowest depth is -3
```

That's the move we made: notice the duplication, pull the duplicated
code into a procedure, and replace each copy with a call. The procedure
abstracts over *which values*. Next we need to abstract over *which
type*.

> Note: `largest_int` reads `values[0]` before it looks at anything else,
> so an empty view traps with `index out of bounds`. That's a reasonable
> contract ("don't ask for the largest of nothing") as long as it's
> written down. [Chapter 7](ch07-errors.md) discusses when a trap is the
> right answer.

## The same code for another type

Our program grows. Now we also want the warmest temperature reading,
which is an `f64`, and the largest byte in a buffer, which is a `u8`.
`largest_int` won't take either: Coax has no implicit conversions, and
a `[] f64` isn't a `[] int`. Writing a concrete version for each type
would look like this:

Filename: examples/ch08_largest3.coax

```jai
#import "Basic";

largest_int :: (values: [] int) -> int {
  result := values[0];
  for n: values {
    if n > result { result = n; }
  }
  return result;
}

largest_f64 :: (values: [] f64) -> f64 {
  result := values[0];
  for n: values {
    if n > result { result = n; }
  }
  return result;
}

largest_u8 :: (values: [] u8) -> u8 {
  result := values[0];
  for n: values {
    if n > result { result = n; }
  }
  return result;
}

main :: () {
  scores := [5] int.[34, 50, 25, 100, 65];
  print("the largest score is %\n", largest_int(scores[..]));

  temperatures := [3] f64.[21.5, 19.25, 23.75];
  print("the warmest reading is %\n", largest_f64(temperatures[..]));

  name := [5] u8.['c', 'o', 'a', 'x', '0'];
  print("the largest byte is %\n", largest_u8(name[..]));
}
```

*Listing 8-3: Three procedures that differ only in their types*

```shell
$ coax run examples/ch08_largest3.coax
the largest score is 100
the warmest reading is 23.75
the largest byte is 120
```

(The largest byte is `'x'`, which is 120.)

The three bodies are character-for-character the same. Could we at
least give them one name? Let's try renaming all three to `largest`:

```shell
$ coax check largest.coax
largest.coax:11:1: error[E0334]: procedure 'largest' is already declared at line 3
  largest :: (values: [] f64) -> f64 {
  ^
largest.coax:32:59: error[E0317]: argument 1 of 'largest' is [] int, given [] f64
    print("the warmest reading is %\n", largest(temperatures[..]));
                                                            ^
2 errors
```

`coax0` has no overloading: every procedure name is unique (`coax
explain E0334` says "There are no overloads yet"). The second error
follows from the first. The compiler kept the first `largest`, the
`int` one, and so the call with temperatures now passes the wrong type.

Even if overloading existed, it wouldn't solve the real problem. We'd
still have three bodies to keep in step. What we want is one body, with
the type as a parameter.

## Writing it once with a type parameter

A type parameter can already abstract over copying and indexing. To
abstract over `largest`, we also need a rule that permits comparison.
Here is the unconstrained form. It is an intentional rejected example:
`>` needs the built-in `ordered` bound shown below.

```jai
#import "Basic";

largest :: (values: [] $T) -> $T {
  result := values[0];
  for n: values {
    if n > result { result = n; }
  }
  return result;
}

main :: () {
  scores := [5] int.[34, 50, 25, 100, 65];
  print("the largest score is %\n", largest(scores[..]));

  temperatures := [3] f64.[21.5, 19.25, 23.75];
  print("the warmest reading is %\n", largest(temperatures[..]));
}
```

*Listing 8-4: Unconstrained `largest`; add `: ordered` at its first `$T` binding*

The first `$T` in the parameter list declares a *type parameter* and says
"bind it from the argument here". With a `[] int` argument, `$T` binds to
`int`, and a result written `-> $T` has that same type. Later uses of
`$T` must match the binding; they do not introduce another type parameter.
The current compiler already implements those binding rules.

For each distinct `T`, the compiler produces a *specialized* procedure,
compiled exactly like `largest_int` would be. There's no runtime
dispatch, no boxed value and no table of function pointers. After adding
the bound in Listing 8-6, these calls produce two instances, one for `int`
and one for `f64`.

Today, the type parameter is accepted, but `n > result` is refused: an
unconstrained `$T` does not promise that comparison exists. This keeps
valid copy/index algorithms usable without pretending that every type
is ordered. The current spelling is `values: [] $T: ordered`; the next
section explains its bound. The retained [0.40.0 verification record](https://github.com/OpenAgentsInc/openagents/blob/main/docs/verification/coax-generic-constraints-2026-10-31.json)
records the initial built-in constraint delivery.

> Note: This is the Jai syntax, and Jai programmers will recognize it. In
> C++ terms, `$T` is a template parameter that's deduced from the
> argument, written in the place where it's deduced instead of in a
> separate `template<typename T>` header.

## When a type can't do what the body needs

Listing 8-4 has a quiet assumption: that whatever `T` turns out to be,
`n > result` means something. That's true for `int`, `f64` and `u8`.
What about a struct?

We can see what happens today, without generics, by writing the
`Vec2` version by hand:

```jai
#import "Basic";

Vec2 :: struct { x: f32; y: f32; }

largest_vec2 :: (values: [] Vec2) -> Vec2 {
  largest := values[0];
  for v: values {
    if v > largest { largest = v; }
  }
  return largest;
}

main :: () {
  points := [2] Vec2.[Vec2.{x = 1, y = 2}, Vec2.{x = 3, y = 0}];
  print("%\n", largest_vec2(points[..]));
}
```

*Listing 8-5: Comparing structs with `>`*

```shell
$ coax check points.coax
points.coax:8:10: error[E0310]: '>' compares two numbers of one type, found Vec2 and Vec2
      if v > largest { largest = v; }
           ^
1 error
```

That's a good error: it names the operator and the type that can't
handle it. There's no obvious answer to "which of two points is larger",
so Coax doesn't invent one.

A generic needs two checks: its declaration must justify the operations
in its body, and each call must bind a type that meets that requirement.
That keeps an invalid body from hiding until a caller first instantiates
it, and gives the caller a diagnostic that names the bound it cannot meet.

## Built-in constraints

The current signature puts that assumption where the caller can see it:

```jai
largest :: (values: [] $T: ordered) -> $T {
  result := values[0];
  for n: values {
    if n > result { result = n; }
  }
  return result;
}
```

*Listing 8-6: The implemented `ordered` signature (illustrative fragment)*

`$T: ordered` reads "bind a type `T` that supports built-in number
comparisons". It belongs where `$T` is bound. The current built-ins are:

| Constraint | Operations it permits | Concrete types admitted |
| --- | --- | --- |
| `copyable` | Assignment, passing and returning | Every current type, subject to existing binding restrictions |
| `equatable` | `==` and `!=` | Scalars, strings and enums |
| `ordered` | Equality and `<`, `<=`, `>` and `>=` | Integers and floats |
| `numeric` | Ordering, `+`, `-`, `*`, `/`, their compound assignments and integer literals | Integers and floats |

These are compiler-defined bounds. `ordered` does not derive an order for
`Vec2`, or promise a total order for floating-point NaNs. A body using
arithmetic with only `ordered` is refused at its declaration. Calling a
constrained procedure or instantiating a constrained struct with an
incompatible type is E0806 at the use, naming the generic declaration and
its requirement. Unknown bounds are E0805; writing a bound away from the
parameter's binding position is E0808. If a concrete instance exposes a
further error, such as adding `300` with `$T` bound to `u8`, its errors are
followed by an instantiation trace at the first call (E0807).

The executable coverage is the `CONSTRAINED` fixture and `Constraints`
checks in [the compiler's generic regression suite](https://github.com/OpenAgentsInc/openagents/blob/main/coax/tests/generics.py),
including ordered maxima, numeric sums, string and enum equality, and a
constrained `Range` struct. Listing 8-6 is an explanatory fragment, not an
additional runnable book example.

## User-defined constraints (later)

General user-defined requirements remain planned. A future constraint
could list required procedures, for example:

```jai
Hashable :: constraint ($T) {
  hash: (T) -> u64;
  eq:   (T, T) -> bool;
}

Table :: struct ($K: Type/Hashable, $V: Type) { ... }
```

This is design syntax: `constraint` declarations, slash-form `$T/Ordered`
bounds, custom `Hashable` requirements and user-defined operators are not
implemented. The design
would let a table require hashing and equality together, with errors at
its use. The four current built-ins do not provide hashing or an owning
container implementation.

Both current built-in bounds and the planned richer constraints are
compile-time requirements. They do not introduce runtime interface
objects, vtables or boxed values.

## Values as parameters

`$N` already binds a fixed array argument's compile-time length in a
procedure parameter such as `[$N] int` or `[$N] $T`. Another parameter
using the same `$N` must have the same length. A view does not bind a fixed
array length.

Generic structs now declare their own parameter list. A parameter used
as an array length in a field is a length; another parameter is a type.
For example, this declaration uses both:

```jai
Pool :: struct($T, $N) {
  items: [$N] $T;
  free:  [$N] u32;
  count: u32;
}
```

With a concrete fixed-storage `Particle` type already declared, a local
`Pool(Particle, 4096)` has 4096 particles and free-list entries inline.
Different arguments give distinct concrete instances. A struct literal can
deduce bindings from its fields, or a declared instance type can supply
them. Missing or inconsistent arguments are errors.

A procedure parameter such as `p: Pair($T)` can bind a struct instance's
type argument. Views and arrays of those concrete struct instances also
work. The current subset refuses a view as a type argument, including an
array or struct containing a view (E0802). It supplies fixed storage and
ordinary value copies, not dynamic storage ownership or lifetime inference.

A concrete declaration with one named capacity remains useful when the
program needs just that one type:

```jai
PARTICLES :: 4096;

Particle_Pool :: struct {
  items: [PARTICLES] Particle;
  free:  [PARTICLES] u32;
  count: u32;
}
```

When several element types or capacities share that representation, a
generic struct can now replace the repeated declarations. Its fields and
each admitted instance still go through ordinary checking and layout.

## Specialization costs something

Specializing removes decisions from the running program and gives the
optimizer concrete code to work with. It also multiplies code: ten key
types make ten tables. You pay in build time, binary size and
instruction cache. The current compiler shares an instance for each
template and binding within a program. Struct and procedure instances
share a limit of 256. Repeating the
same call binding does not make another copy. `--timings` reports total
`instances` and `instance_generated_bytes`; use those observations when a
generic increases build time or output size.

Recursive generic calls are currently refused. Constraint failures now
name the declaration and bound, and concrete body failures carry the
E0807 instantiation trace described above. Richer specialization controls
remain planned; target and relevant semantic options must stay part of
specialization identity as those facilities grow.

## Baking arguments

Sometimes you want a specialized copy of an ordinary procedure, with
one argument fixed:

```jai
draw_64 :: #bake_arguments draw(limit = 64);
```

`#bake_arguments` makes a new procedure from `draw` in which `limit` is
the constant 64. The baked arguments become constants inside the copy,
so the optimizer can unroll or fold with them, and the remaining
arguments stay ordinary parameters.

It's explicit on purpose. A compiler that quietly specialized every
call with a constant argument would hide its code-size and build-time
costs, and the program you read would not tell you how many copies of
`draw` exist. Baked copies go through the same cache and the same
limits as other specializations. `coax0` doesn't parse it yet. Given
this file:

```jai
draw :: (limit: int) -> int { return limit; }
draw_64 :: #bake_arguments draw(limit = 64);
main :: () {}
```

it reports:

```shell
$ coax check bake.coax
bake.coax:2:12: error[E0205]: expected an expression, found directive
  draw_64 :: #bake_arguments draw(limit = 64);
             ^
1 error
```

## Operators for your own types

Here's another wall we hit today. A `Money` struct can't be added with
`+`:

```jai
#import "Basic";

Money :: struct { cents: int; }

main :: () {
  a := Money.{cents = 150};
  b := Money.{cents = 275};
  total := a + b;
}
```

```shell
$ coax check money.coax
money.coax:8:14: error[E0309]: '+' needs two numbers of one type, found Money and Money
    total := a + b;
               ^
1 error
```

For now we write `add_money(a, b)` or `Money.{cents = a.cents +
b.cents}`. The design lets a type define its own operators:

```jai
operator + :: (a: Money, b: Money) -> Money pure { ... }
```

The planned user operators cover arithmetic, comparison and indexing.
Forrest Smith's Jai code shows how much a small vector type gains from
them: `a + b * t` reads like the math it is. In Coax an operator must be
`pure`, which means no allocation, no I/O and no failure, so math
syntax costs what math costs. Assignment, field access, `&&`, `||` and
calls can't be overloaded. A future user-defined ordering requirement
could connect such operators to generic code. Today's built-in `ordered`
bound admits integers and floats; it does not admit custom struct operators.

> Note: The built-in math types `v2 v3 v4 quat m3 m4` are planned to come
> with their operators. Neither they nor `operator` declarations exist in
> `coax0` yet.

## Computing types (later)

Forrest Smith's typesafe-units experiment in Jai uses `#modify` and type
inspection to compute result types from physical dimensions, so that
metres divided by seconds has the type metres-per-second. That's real
expressive power. Coax may add a hook like it: a pure, bounded type
computation with an explicit input and output, not permission to change
arbitrary compiler state. Built-in constraints and their diagnostics now
ship; user-defined requirements and a separate `#modify` still need to
show that the existing facilities are insufficient.

## In the real world: five copies of `clamp`

The Coax code behind the OpenAgents iOS world shows exactly the problem
generics are for. `verse/grid.coax`, `cast.coax` and `player.coax`
each carry the same three helpers:

```jai
lo :: (a: f32, b: f32) -> f32 { if a < b { return a; } return b; }
hi :: (a: f32, b: f32) -> f32 { if a > b { return a; } return b; }
clamp :: (a: f32, low: f32, high: f32) -> f32 { return lo(high, hi(low, a)); }
```

`light.coax` has its own `lo` and `hi`, and `home.coax` has the same
three again for `f64`, named `minimum`, `maximum` and `clamp`. A
tolerance check, `near :: (a: f32, b: f32, tolerance: f32) -> bool`,
appears in four files. Source loading, logical modules and explicit
exports now let those files share a helper (Chapter 10). The built-in
`ordered` bound now permits a generic comparison-based `clamp` over both
`f32` and `f64`; arithmetic helpers can state `numeric` when needed. This
is an available refactoring path, not a claim that these consumers have
been migrated. Preserve each helper's comparison order, NaN and signed-zero
behavior, and qualify the actual call sites before replacing them.

## Summary

We started with a duplicated loop and removed it the usual way, by
extracting a procedure. Type parameters (`$T`) and fixed-array length
parameters (`$N`) remove type and capacity duplication without runtime
dispatch. The built-in `equatable`, `ordered`, `numeric` and `copyable`
bounds state the operations a generic body may use, with diagnostics at
its declaration or the incompatible call. General user-defined
constraints, argument baking, procedure overloading and user operators
remain planned; generic
structs still reject view-containing bindings.

In [Chapter 9](ch09-compile-time.md) we'll look at the part of Coax that
does run today and that much of the rest of the language leans on:
running ordinary code while the program compiles.
