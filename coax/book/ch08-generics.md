# 8. Generics and constraints

Sooner or later you write the same procedure twice. Once for `int`, then
again for `f64`, then a third time for `u8`, each copy identical except
for the type names. *Generics* let us write that procedure once, with
the type left as a parameter, and have the compiler produce each
version we need. *Constraints* say what a type must be able to do for
the generic to make sense, so a mistake is reported where the generic
is used rather than somewhere deep inside it.

In this chapter we'll start with code that runs today and has exactly
that duplication problem. We'll remove the first layer of duplication
with an ordinary procedure, hit the wall where ordinary procedures stop
helping, and then look at how Coax's generics and constraints are
designed to take us the rest of the way.

> Note: `coax0` doesn't implement generics yet. The first half of this
> chapter runs today and its examples are checked. From "Writing it once
> with a type parameter" onward, the code follows spec §7.2 and doesn't
> compile; where it helps, we'll show what `coax0` says when you try.

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

```
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

```
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
a `[] f64` isn't a `[] int`. So today we write it out again, for each
type:

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

```
$ coax run examples/ch08_largest3.coax
the largest score is 100
the warmest reading is 23.75
the largest byte is 120
```

(The largest byte is `'x'`, which is 120.)

The three bodies are character-for-character the same. Could we at
least give them one name? Let's try renaming all three to `largest`:

```
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

Here's the generic version in the design:

```jai
#import "Basic";

largest :: (values: [] $T) -> T {
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

*Listing 8-4: One `largest` for every element type (design; doesn't compile in `coax0`)*

The new piece is `$T`. The dollar sign declares a *type parameter* named
`T` and says "bind it from the argument here". When we call
`largest(scores[..])`, the argument is a `[] int`, so `T` is `int`, and
the result type, written `T`, is `int` too. Later uses of `T` in the
signature must match what was bound; they don't bind again.

For each distinct `T`, the compiler produces a *specialized* procedure,
compiled exactly like `largest_int` would be. There's no runtime
dispatch, no boxed value and no table of function pointers. The program
in Listing 8-4 would contain two procedures, one for `int` and one for
`f64`, and nothing else.

Today, `coax0` stops at the dollar sign:

```
$ coax check largest.coax
largest.coax:3:24: error[E0108]: unexpected character '$'
  largest :: (values: [] $T) -> T {
                         ^
1 error
```

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

```
$ coax check points.coax
points.coax:8:10: error[E0310]: '>' compares two numbers of one type, found Vec2 and Vec2
      if v > largest { largest = v; }
           ^
1 error
```

That's a good error: it names the operator and the type that can't
handle it. There's no obvious answer to "which of two points is larger",
so Coax doesn't invent one.

Now picture the same mistake with a generic. The caller writes
`largest(points[..])`, one line that looks perfectly fine. The `>` that
fails is inside `largest`, in code the caller didn't write and may never
have read. If the compiler reports the error there, the caller gets a
message pointing into someone else's library, at an operator they never
typed. That's the experience C++ templates are famous for, where one
wrong call produces pages of errors from deep inside a header.

## Constraints

The fix is to put the assumption into the signature, where the caller
can see it. A *constraint* names the operations a type must have:

```jai
largest :: (values: [] $T/Ordered) -> T {
  result := values[0];
  for n: values {
    if n > result { result = n; }
  }
  return result;
}
```

*Listing 8-6: `largest` constrained to types that can be ordered (design)*

`$T/Ordered` reads "a type `T` that is `Ordered`". Now the signature
tells you everything `largest` needs from its type, and the compiler
checks it at each call. When `largest(points[..])` is compiled, the
constraint is checked first, and per spec §7.2 the error is reported
*at the call*, naming the call and the missing operation (here, that
`Vec2` has no ordering). The body of `largest` is never involved.

> Note: The exact wording of constraint errors isn't settled, and no
> compiler prints them yet, so this book doesn't show one. What the spec
> fixes is where the error points (the call) and what it names (the
> missing operation).

A constraint is written like a small list of required procedures:

```jai
Hashable :: constraint ($T) {
  hash: (T) -> u64;
  eq:   (T, T) -> bool;
}

Table :: struct ($K: Type/Hashable, $V: Type) { ... }
```

A `Table` keyed by some type works only if that type can be hashed and
compared for equality, and the constraint says so up front. If you try
to build a `Table` keyed by a struct with no `hash`, the error names the
struct and the missing `hash`, at the line where you wrote the table's
type.

A constraint is a compile-time requirement only. It never becomes a
runtime interface object, a vtable or a box; it just decides whether a
specialization is allowed.

> Note: Jai has constraints too (`$T/Base`, `$T/interface Matchable`),
> but they're optional and many Jai generics go without. Coax makes them
> the normal way to write a generic. Rust programmers will recognize the
> idea from trait bounds, without the trait objects.

## Values as parameters

`$N` works the same way as `$T`, but binds a compile-time *value*
instead of a type. It's how a generic struct takes a size:

```jai
Pool :: struct ($T: Type, $N: int) {
  items: [N] T;
  free:  [N] u32;
  count: u32;
}

particles: Pool(Particle, 4096);
```

`Pool(Particle, 4096)` is a struct with 4096 `Particle`s inline. A
different `N` is a different type, with its own fixed size.

Today the nearest thing is a struct with a fixed array whose length is
a named constant, which works fine as long as you only need one size:

```jai
PARTICLES :: 4096;

Particle_Pool :: struct {
  items: [PARTICLES] Particle;
  free:  [PARTICLES] u32;
  count: u32;
}
```

A second pool of a different size or element type means a second
struct, which is Listing 8-3's problem again.

## Specialization costs something

Specializing removes decisions from the running program and gives the
optimizer concrete code to work with. It also multiplies code: ten key
types make ten tables. You pay in build time, binary size and
instruction cache. So the design makes those costs visible instead of
pretending generics are free:

Instances are cached by their full key, which includes the resolved
types, constant arguments, the target, and relevant effects and
options. Asking for `largest` on `int` in fifty places produces one
instance.

The compiler reports how many instances each generic produced, their
code size and the time spent generating them, so you can see which
generic is responsible for a slow build or a large binary.

Recursive specialization has a limit, and running into it prints the
chain of instantiations starting from your source, not from inside the
compiler.

When generics land, measure the instance counts in a real codebase
before assuming they don't matter.

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

```
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

```
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

User types can define arithmetic, comparison and indexing operators.
Forrest Smith's Jai code shows how much a small vector type gains from
them: `a + b * t` reads like the math it is. In Coax an operator must be
`pure`, which means no allocation, no I/O and no failure, so math
syntax costs what math costs. Assignment, field access, `&&`, `||` and
calls can't be overloaded. Defining `>` for a type is also how it would
come to satisfy a constraint like `Ordered`, which ties this section
back to Listing 8-6.

> Note: The built-in math types `v2 v3 v4 quat m3 m4` are planned to come
> with their operators. Neither they nor `operator` declarations exist in
> `coax0` yet.

## Computing types (later)

Forrest Smith's typesafe-units experiment in Jai uses `#modify` and type
inspection to compute result types from physical dimensions, so that
metres divided by seconds has the type metres-per-second. That's real
expressive power. Coax may add a hook like it: a pure, bounded type
computation with an explicit input and output, not permission to change
arbitrary compiler state. Ordinary constraints and their diagnostics
come first, and a separate `#modify` will have to show it's needed
before it's added.

## In the real world: five copies of `clamp`

The Coax code behind the OpenAgents iOS world shows exactly the problem
generics are for. `metareal/grid.coax`, `cast.coax` and `player.coax`
each carry the same three helpers:

```jai
lo :: (a: f32, b: f32) -> f32 { if a < b { return a; } return b; }
hi :: (a: f32, b: f32) -> f32 { if a > b { return a; } return b; }
clamp :: (a: f32, low: f32, high: f32) -> f32 { return lo(high, hi(low, a)); }
```

`light.coax` has its own `lo` and `hi`, and `home.coax` has the same
three again for `f64`, named `minimum`, `maximum` and `clamp`. A
tolerance check, `near :: (a: f32, b: f32, tolerance: f32) -> bool`,
appears in four files. Part of that is that `coax0` has no modules yet
(`#import "Basic";` is the only import), so one file can't share
procedures with another. The `f32` versus `f64` split is the part
generics fix: with `$T/Ordered`, one `clamp`
covers both, and the two copies can't drift apart.

## Summary

We started with a duplicated loop and removed it the usual way, by
extracting a procedure. That procedure was still tied to one type, and
in `coax0` today the only option for a second type is a second copy
with a second name, because there are no overloads. Coax's planned
generics fix this with type parameters (`$T`) and value parameters
(`$N`), specialized at compile time with no runtime dispatch.
Constraints like `$T/Ordered` put a generic's requirements in its
signature, so a type that can't do what the body needs is rejected at
the call that asked for it. Specialization, `#bake_arguments` and
operators all come with their costs made visible.

In [Chapter 9](ch09-compile-time.md) we'll look at the part of Coax that
does run today and that much of the rest of the language leans on:
running ordinary code while the program compiles.
