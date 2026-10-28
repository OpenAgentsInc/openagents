# 5. Structs, enums and unions

So far every value we've worked with has been a single number, a bool or a
string. Real programs deal in things that have several parts: a position has
an x and a y, a ship has a position, a velocity and some health left. In this
chapter we'll build the beginnings of a small asteroid game and use it to
learn the three ways Coax lets you make your own types. A *struct* groups
named fields into one value. An *enum* names a fixed set of choices. A
*union* holds one of several alternatives and remembers which. Along the way
we'll look at how each one is laid out in memory, because in Coax that
layout is part of the type's meaning, not an accident of the compiler.

> Note: Structs and enums run on `coax0` 0.10.0, and every program in this
> chapter that prints something has its output checked. Unions, `switch`,
> `using`, `enum_flags` and `#derive` are planned: `coax0` reserves those
> words and refuses them with error `E0204` (or `E0210` for `#derive`).
> Where the chapter shows them, it says so, and it shows what you write
> today instead. The book's copy of each program is in
> `coax/book/examples/`, with a `ch05_` prefix: `ship.coax` below is
> `examples/ch05_ship.coax`.

## Why group values at all

Let's start with a ship that flies across the screen, written with the
tools we already have. Make a file called `ship.coax`.

Filename: ship.coax

```jai
#import "Basic";

moved_x :: (x: f32, vx: f32, dt: f32) -> f32 { return x + vx * dt; }

main :: () {
  ship_x: f32 = 0;
  ship_y: f32 = 0;
  ship_vx: f32 = 2;
  ship_vy: f32 = 1;
  ship_health: s32 = 100;

  ship_x = moved_x(ship_x, ship_vx, 0.5);
  ship_y = moved_x(ship_y, ship_vy, 0.5);   // moved_x, but for y
  print("ship at (%, %) with % health\n", ship_x, ship_y, ship_health);
}
```

*Listing 5-1: A ship as five loose variables*

```
$ coax run ship.coax
ship at (1.0, 0.5) with 100 health
```

It works, and for one ship it's even readable. But nothing in the program
says that these five variables belong together. The only clue is the
`ship_` prefix we typed. A second ship means five more variables, and a
procedure that moves a ship has to take every piece separately, in the right
order, with nothing to stop us passing `ship_vy` where `ship_vx` was meant.
The comment on the second call is a sign of the trouble: `moved_x` is really
"move one coordinate", and we're reusing it because there's no type that
means "a position".

What we want is to say "a ship has a position, a velocity and health" once,
and then pass a ship around as one value. That's what a struct is for.

## Defining a struct

Here's the same program with two structs.

Filename: entity.coax

```jai
#import "Basic";

Vec2 :: struct {
  x: f32;
  y: f32;
}

Entity :: struct {
  pos: Vec2;
  vel: Vec2;
  health: s32;
}

main :: () {
  ship := Entity.{
    pos = Vec2.{x = 0, y = 0},
    vel = Vec2.{x = 2, y = 1},
    health = 100,
  };
  print("%\n", ship);

  ship.pos.x += ship.vel.x * 0.5;
  ship.pos.y += ship.vel.y * 0.5;
  ship.health -= 25;
  print("ship at (%, %) with % health\n", ship.pos.x, ship.pos.y, ship.health);
}
```

*Listing 5-2: A Vec2 and an Entity made from it*

```
$ coax run entity.coax
Entity.{pos = Vec2.{x = 0.0, y = 0.0}, vel = Vec2.{x = 2.0, y = 1.0}, health = 100}
ship at (1.0, 0.5) with 75 health
```

Let's go through the new pieces.

`Vec2 :: struct { ... }` declares a type called `Vec2`. It uses the same
`name :: value` form as a constant from Chapter 3, because a type is a
constant: it's fixed when the program is compiled and can't change while it
runs. Inside the braces, each field is `name: type;`. Structs live at the top
level of a file, like procedures, and they can be declared in any order:
`Entity` could come before `Vec2` and the program would mean the same thing.

`Entity` has fields whose types are other structs. A struct's fields can be
any type Coax has: numbers, bools, strings, enums, other structs, fixed
arrays and views (Chapter 6).

`Entity.{ pos = ..., vel = ..., health = 100 }` is a *struct literal*. It
starts with the type's name, then `.{`, then `field = value` for the fields
we want to set, separated by commas. A trailing comma before the `}` is
allowed, which keeps diffs clean when the literal spans several lines.

`ship.pos.x` reads a field, and fields of fields, with a dot. The same
expression on the left of `=` or `+=` writes the field, as long as `ship` is
a variable we're allowed to change.

`print("%\n", ship)` prints the whole value, field by field, in the same
literal syntax we'd use to write it. That's handy while debugging: you can
paste a printed value back into source.

Notice what we no longer have: there's no `ship_` prefix, and the five
pieces travel together. If we add a second ship, it's one more declaration.

### Literals name their fields

Every field in a Coax struct literal is named. There's no form that lists
values in order, which means a literal can't be wrong because someone
reordered the struct's fields. It also means that the compiler can tell you
exactly what you got wrong when you misspell one:

```jai
ship := Entity.{vel = Vec2.{x = 2, y = 1}, helth = 50};
```

```
$ coax check entity.coax
entity.coax:4:46: error[E0351]: struct 'Entity' has no field 'helth'
    ship := Entity.{vel = Vec2.{x = 2, y = 1}, helth = 50};
                                               ^
  fix: its fields are: pos vel health
1 error
```

The fix line lists the real fields, so you don't have to go and look. Giving
the same field twice is `E0352`, and giving a field a value of the wrong type
is `E0367` (`field 'y' is f32, given string`).

If you come from C or Jai you may reach for a positional literal:

```jai
v := Vec2.{2, 1};
```

```
point.coax:4:14: error[E0208]: expected a field name, found integer
    v := Vec2.{2, 1};
               ^
1 error
```

> Note: The design plans two shorter forms. Positional literals, such as
> `Vec2.{2, 1}`, would be allowed for small math types where the order *is*
> the meaning. Inferred literals, `.{x = 2, y = 1}`, would leave the type
> name out where the compiler already knows it, such as an argument or an
> initializer with a declared type. Neither is in `coax0`; today the type's
> name always comes first and every field is named.

## Defaults and initialization

A literal doesn't have to set every field. Let's give `Entity` some
defaults and see what a literal leaves alone.

Filename: defaults.coax

```jai
#import "Basic";

Vec2 :: struct { x: f32; y: f32; }

MAX_HEALTH :: 100;

Entity :: struct {
  pos: Vec2;
  vel: Vec2 = Vec2.{x = 0, y = -1};    // things drift down unless told otherwise
  health: s32 = MAX_HEALTH;
  target: s32 = -1;                     // -1: no target
}

main :: () {
  a: Entity;                                   // every default, zero elsewhere
  b := Entity.{health = 40};                   // defaults, except health
  print("%\n%\n", a, b);

  c := b;                                      // a copy of every field
  c.pos.x = 10;
  c.target = 3;
  print("b: % %\n", b.pos.x, b.target);
  print("c: % %\n", c.pos.x, c.target);

  d: Entity = ---;                             // no defaults, no zeroing
  d.health = 1;
  print("%\n", d.health);
}
```

*Listing 5-3: Field defaults, copies, and opting out with `---`*

```
$ coax run defaults.coax
Entity.{pos = Vec2.{x = 0.0, y = 0.0}, vel = Vec2.{x = 0.0, y = -1.0}, health = 100, target = -1}
Entity.{pos = Vec2.{x = 0.0, y = 0.0}, vel = Vec2.{x = 0.0, y = -1.0}, health = 40, target = -1}
b: 0.0 -1
c: 10.0 3
1
```

A field can have a default after its type: `health: s32 = MAX_HEALTH;`. The
default can be a literal, a constant, or another struct literal, as `vel`
shows.

The rule for a new struct value is: every field gets its declared default,
and every field without one gets zero. That's true whether you declare a
variable with no initializer (`a: Entity;`) or write a literal that only
names some fields (`Entity.{health = 40}`). A literal starts from the
defaults and then overwrites the fields it names. `Entity.{}` is a valid
literal that means "all the defaults".

This is why we say *default initialized* rather than *zeroed*. In Listing
5-3, `target` starts at `-1` and `vel` starts pointing down, and neither is
zero. Choose defaults so that a freshly declared value is a sensible value.
`-1` for "no target" is a common example; an index of `0` would be a real
entity, and a zeroed struct would silently target it.

`d: Entity = ---;` opts out of all of it: no defaults are applied and
nothing is zeroed. The fields hold whatever was in that memory before. It's
for the rare case where you're about to overwrite every field anyway and
have measured that the initialization costs something. Reading a field of
`d` you haven't written is a bug that `coax0` doesn't catch at run time, so
use `---` sparingly.

### Structs are values

Look at the `b:` and `c:` lines of the output. We copied `b` into `c`, then
changed `c.pos.x` and `c.target`, and `b` didn't change. Assigning a struct
copies every field. So does passing one to a procedure and returning one.
There's no hidden reference to a shared object.

The copy is *shallow*. A field that is itself a view or a string (Chapter 6)
is a pointer and a count, and copying the struct copies that pointer and
count, not the bytes they point at. Two copies of a struct with a `name:
string` field share the same characters. That's cheap and usually what you
want, and it's also the first thing to check when two values seem to change
together.

## Procedures that work on structs

Coax has no methods. There's no `impl` block and no member functions inside
a struct. Instead, we write ordinary procedures that take a struct as a
parameter. Let's give our entities some motion.

Filename: step.coax

```jai
#import "Basic";

Vec2 :: struct { x: f32; y: f32; }

Entity :: struct {
  pos: Vec2;
  vel: Vec2;
  health: s32 = 100;
}

add :: (a: Vec2, b: Vec2) -> Vec2 { return Vec2.{x = a.x + b.x, y = a.y + b.y}; }
scale :: (v: Vec2, k: f32) -> Vec2 { return Vec2.{x = v.x * k, y = v.y * k}; }

// One entity in, the same entity a moment later out.
moved :: (e: Entity, dt: f32) -> Entity {
  next := e;
  next.pos = add(e.pos, scale(e.vel, dt));
  return next;
}

// Every entity in the caller's array, changed where it lies.
step_all :: (all: [] Entity, dt: f32) {
  for i: 0..<all.count { all[i] = moved(all[i], dt); }
}

same_place :: (a: Entity, b: Entity) -> bool {
  return a.pos.x == b.pos.x && a.pos.y == b.pos.y;
}

main :: () {
  ship := Entity.{vel = Vec2.{x = 2, y = 1}};
  later := moved(ship, 0.5);
  print("% -> %\n", ship.pos, later.pos);

  world: [3] Entity;
  world[0] = ship;
  world[1].vel = Vec2.{x = -1, y = 0};
  step_all(world[..], 1);
  step_all(world[..], 1);
  for e, i: world { print("% %\n", i, e.pos); }

  print("% %\n", same_place(world[2], Entity.{}), same_place(world[0], world[1]));
}
```

*Listing 5-4: Value in, value out, and changing many in place through a view*

```
$ coax run step.coax
Vec2.{x = 0.0, y = 0.0} -> Vec2.{x = 1.0, y = 0.5}
0 Vec2.{x = 4.0, y = 2.0}
1 Vec2.{x = -2.0, y = 0.0}
2 Vec2.{x = 0.0, y = 0.0}
true false
```

There are two styles here, and both are common in real Coax.

The first is *value in, value out*. `moved` takes an `Entity` and returns a
new one. It copies its parameter into a local, changes the local and returns
it. The caller decides what to do with the result: keep both, as `main`
does with `ship` and `later`, or overwrite the old one with
`ship = moved(ship, dt);`. Small helpers like `add` and `scale` work the
same way and build the result with a literal.

Why not change the parameter directly? Let's try:

```jai
nudge :: (e: Entity) {
  e.pos.x += 1;
}
```

```
$ coax check nudge.coax
nudge.coax:4:11: error[E0344]: parameters cannot be assigned
    e.pos.x += 1;
            ^
  fix: copy it into a local variable
1 error
```

Parameters in Coax are immutable, fields included. Because a struct
parameter is a copy, changing it would only change the procedure's own copy,
and the caller would never see it. Coax makes that mistake impossible to
write instead of letting it quietly do nothing.

A related mistake is calling a value-in, value-out procedure and forgetting
to use what it returns:

```jai
ship: Entity;
moved(ship, 0.5);
```

```
forgot.coax:10:3: error[E0323]: the result of 'moved' (Entity) is not used
    moved(ship, 0.5);
    ^
  fix: use it: result := moved(...);
1 error
```

The result of a procedure call can't be dropped on the floor. Here that
catches the bug of thinking `moved` changed `ship`.

The second style is *changing things where they lie*, through a view.
`step_all` takes `all: [] Entity`, a view of the caller's array (Chapter 6
covers views properly). The view itself can't be reassigned, but the
elements it points at can, so `all[i] = moved(all[i], dt)` writes into
`main`'s `world`. That's why the second and third lines of the output show
positions after two steps. Use this style when you have many values to
update, or when the values are too large to copy back and forth cheaply.

> Note: The procedures in the native Metareal home use the first style
> almost everywhere. `metareal/player.coax` advances the player with
> `metareal_player_advance_among :: (p: Player, frame: f32, ...) -> Player`,
> and the host keeps the returned `Player`. A procedure that hands back the
> whole new state is easy to test, because the old state is still there to
> compare against.

> Note: In C++ or Rust you'd write `ship.moved(dt)`. Coax keeps procedures
> and data separate, as C and Jai do, and the spec doesn't describe a
> method-call syntax.

### Structs don't compare with `==`

`same_place` compares two entities' positions field by field. Why not write
`a == b`?

```jai
a: Entity;
b := a;
if a == b { }
```

```
compare.coax:6:8: error[E0311]: cannot compare Entity values with '=='
    if a == b { }
         ^
  fix: compare the fields or elements you mean
1 error
```

Whole-struct equality hides decisions. Should padding bytes be compared?
Are two `string` fields equal when they point at the same bytes, or when
their bytes match? Is `-0.0` equal to `0.0`? Should two entities with the
same position but different health be "equal"? Coax makes you write the
comparison you mean, which is often narrower than "every field", as
`same_place` is.

> Note: `#derive(eq, hash)` is the planned way to say "compare every field
> in declaration order" when that really is what you mean. It would be
> library code over reflection (Chapter 9), so you could read what it
> generated. `coax0` refuses it:
> `error[E0210]: #derive on structs is not supported by coax0 yet`.

### A struct can't contain itself

A linked list node that holds the next node by value seems natural if you
come from a garbage-collected language:

```jai
Node :: struct {
  value: int;
  next: Node;
}
```

```
node.coax:1:1: error[E0353]: struct 'Node' contains itself
  Node :: struct {
  ^
  fix: hold it through a view '[] Node' or an index
1 error
```

A struct holds its fields inline, so a `Node` containing a `Node` would be
infinitely large. The fix suggests the two ways out. Hold related values
through a view, or, more often in game code, store all the nodes in one
array and have each node hold the *index* of the next one.

## How a struct is laid out

A struct's fields sit in memory in the order you declare them, each at an
offset that suits its alignment. Coax never reorders them. That matters
because other code reads structs by offset: the C compiler on the other side
of an `#export_c` procedure (Chapter 17), a GPU shader, a file. If the
compiler reordered fields to save space, adding a field could move every
other one, and that other code would read garbage without any error.

The cost of that promise is that field order is your job. Here are two
structs with the same fields in a different order.

Filename: layout.coax

```jai
#import "Basic";

Kind :: enum u8 { ship = 1; rock = 2; bullet = 3; }

// Fields in the order they occurred to us.
Spark :: struct {
  alive: bool;
  born: f64;
  kind: Kind;
  ttl: s32;
  team: u8;
}

// The same fields, largest first.
Spark_Packed :: struct {
  born: f64;
  ttl: s32;
  alive: bool;
  kind: Kind;
  team: u8;
}

main :: () {
  print("% %\n", size_of(Spark), size_of(Spark_Packed));
}
```

*Listing 5-5: The same fields in two orders*

```
$ coax run layout.coax
32 16
```

Half the size, just from the order. `coax layout` shows where the bytes go:

```
$ coax layout layout.coax
Spark  size 32  align 8
       0     1  alive: bool
       1     7  (padding)
       8     8  born: f64
      16     1  kind: Kind
      17     3  (padding)
      20     4  ttl: s32
      24     1  team: u8
      25     7  (padding)
Spark_Packed  size 16  align 8
       0     8  born: f64
       8     4  ttl: s32
      12     1  alive: bool
      13     1  kind: Kind
      14     1  team: u8
      15     1  (padding)
Kind  enum u8  size 1
  ship = 1
  rock = 2
  bullet = 3
```

The first column is the offset, the second is the size. An `f64` has to
start at a multiple of 8, so after the one-byte `alive`, seven bytes are
wasted. The struct's own alignment is its largest field's, 8 here, and its
size is rounded up to a multiple of that so that an array of them keeps
every `born` aligned; that's the seven bytes at the end. Putting the largest
fields first lets the small ones pack together.

For one spark, 16 bytes don't matter. For the 50,000 sparks in a particle
system, the difference is 800 KB of cache traffic every frame. Chapter 11
goes much further into layout, including when to split a struct into
separate arrays.

> Note: The design lets you ask for a different layout explicitly with a
> layout mode on the struct, rather than having the compiler choose one
> silently. `coax0` has only declaration order with natural alignment.

## Enums

Our game has three kinds of thing: the ship, rocks and bullets. We could
store the kind as a number and remember that 1 means ship. An enum lets us
name the choices instead.

Filename: kinds.coax

```jai
#import "Basic";

Kind :: enum u8 {
  ship   = 1;
  rock   = 2;
  bullet = 3;
}

Size :: enum u16 { tiny; small; large = 10; huge; }

main :: () {
  k := Kind.rock;
  print("% % %\n", k, k == Kind.rock, k != Kind.ship);
  print("% %\n", cast(u8) k, size_of(Kind));
  print("% % % %\n", Size.tiny, Size.small, Size.large, cast(u16) Size.huge);

  nothing: Kind;                       // zero, and no member is zero
  print("%\n", nothing);

  code: u8 = 3;
  print("%\n", cast(Kind) code);       // 3 names a member: bullet
  code = 7;
  print("%\n", cast(Kind) code);       // 7 names nothing: this traps
  print("never printed\n");
}
```

*Listing 5-6: Defining enums, converting them, and a bad conversion*

```
$ coax run kinds.coax
rock true true
2 1
tiny small large 11
Kind(0)
bullet
kinds.coax:23: value is not a member of enum Kind
```

There's a lot in this listing, so let's take it a piece at a time.

`Kind :: enum u8 { ... }` declares an enum whose values are stored as a
`u8`. Every enum in Coax names its underlying integer type. Leave it out and
the compiler stops you:

```
kind.coax:1:14: error[E0219]: an enum names its underlying integer type
  Kind :: enum { ship; rock; bullet; }
               ^
  fix: Outcome :: enum u8 { ... }
1 error
```

(The fix text uses a different enum's name as its example; the shape is
what matters.) In C, an enum's size is up to the compiler and can depend on
the values in it, so adding a member can change the layout of every struct
that holds one. In Coax, `enum u8` is one byte on every target, forever.

Each member is `name;` or `name = value;`. A member without a value is the
previous value plus one, and the first one is zero. That's how `Size.huge`
ends up as 11: it follows `large = 10`. Two members can't share a value, and
every value must fit the underlying type; both mistakes are `E0363`
(`'rock' has the same value as 'ship'`, or
`value 256 of 'rock' does not fit in u8`).

We name a member through its type, `Kind.rock`. Misspelling one lists the
real members, the same way a misspelled field does:

```
boat.coax:3:12: error[E0351]: enum 'Kind' has no member 'boat'
    k := Kind.boat;
             ^
  fix: its members are: ship rock bullet
1 error
```

Enums print by name, which is why the first line of output says `rock`, not
`2`. Log lines and test failures read as words.

### An enum is not an integer

You can compare two values of the same enum with `==` and `!=`, and that's
all. There's no ordering and no arithmetic:

```
order.coax:4:8: error[E0310]: '>' compares two numbers of one type, found Kind and Kind
    if k > Kind.ship { }
         ^
1 error
```

```
next.coax:3:18: error[E0309]: '+' needs two numbers of one type, found Kind and integer literal
    k := Kind.rock + 1;
                   ^
1 error
```

And an enum doesn't quietly turn into its number:

```
toint.coax:4:11: error[E0325]: 'n' is u8 but is initialized with Kind
    n: u8 = k;
            ^
1 error
```

These rules exist because the numbers behind an enum are an encoding, not a
meaning. `Kind.bullet > Kind.rock` happens to be true, but nothing about
bullets makes them greater than rocks, and code that relied on it would
break the day someone renumbered the members.

When you do need the number, for example to write it to a file, say so
with `cast(u8) k`. Going the other way, `cast(Kind) code` turns a number
into an enum, and this is where Coax is careful. The last two conversions in
Listing 5-6 show it: 3 names `bullet` and converts, 7 names nothing and the
program stops with `value is not a member of enum Kind`. This check runs in
every build, release included. A bad value read from a file or a network
packet is caught at the line where it enters the program, not three
procedures later where a lookup goes wrong.

### The zero value of an enum

`nothing: Kind;` printed `Kind(0)`. A new enum variable is zeroed like any
other value, and `Kind` has no member equal to zero, so the value names no
member; `print` shows the type and the number. `coax0` doesn't stop you
declaring such a variable. If an enum has a natural "nothing" or "unknown"
state, make it the zero member, so a fresh value means something:

```jai
Pay :: enum u8 { idle; quoted; sending; succeeded; not_sent; unknown; }
```

That's the payment state from `metareal/wallet.coax`. Its first member,
`idle`, is zero, so a zeroed payment is an idle one.

If an enum has no sensible zero, as `Kind` doesn't, give it a field default
wherever a struct holds one (`kind: Kind = Kind.rock;`) so that a new
struct doesn't start out holding a non-member.

### Numbers that persist are written out

`Kind` spells out `ship = 1; rock = 2; bullet = 3;` even though the default
numbering would have worked. That's deliberate. If we save a level file with
`cast(u8) kind` in it, those numbers become part of the file format. If the
values came from declaration order, then someday someone would add `mine`
between `ship` and `rock`, every rock in every saved file would load as a
mine, and nothing would report an error.

So the rule is: any enum whose numbers leave the program (saved, sent,
stored in a registry, shared with C) gets explicit values. `Size` above,
which never leaves the program, can use the defaults.

> Note: In Jai this is the opt-in `#specified`. The Coax design means to
> require explicit values wherever an enum crosses a boundary. `coax0`
> doesn't enforce that yet, so for now it's a rule you follow by hand.

## Matching on an enum

Most code that holds an enum wants to do something different for each
member. Many languages have a `switch` or `match` statement for this. Coax
plans one, but `coax0` doesn't have it:

```
sw.coax:4:3: error[E0204]: 'switch' is reserved and not supported by coax0 yet
    switch k {
    ^
1 error
```

Today we match with `if`/`else if` chains and `ifx`. Let's give each kind of
entity a collision radius and a description.

Filename: match.coax

```jai
#import "Basic";

Kind :: enum u8 { ship = 1; rock = 2; bullet = 3; }

radius :: (k: Kind) -> f32 {
  if k == Kind.ship { return 1.5; }
  else if k == Kind.rock { return 4; }
  else if k == Kind.bullet { return 0.25; }
  return 0;                            // only an unchecked cast gets here
}

describe :: (k: Kind) -> string {
  return ifx k == Kind.ship then "the player"
         else ifx k == Kind.rock then "an asteroid"
         else "a shot";
}

#forall k: Kind {
  #assert radius(k) > 0;
}

main :: () {
  kinds := [3] Kind.[Kind.ship, Kind.rock, Kind.bullet];
  for k: kinds { print("% is %, radius %\n", k, describe(k), radius(k)); }
}
```

*Listing 5-7: Matching on an enum with `if` chains, and a check that covers
every member*

```
$ coax run match.coax
ship is the player, radius 1.5
rock is an asteroid, radius 4.0
bullet is a shot, radius 0.25
```

Two things in `radius` are worth a closer look.

The final `return 0;` looks unreachable: we've covered all three members.
But the compiler doesn't reason about which members an `if` chain covers,
so without it we get:

```
radius.coax:2:1: error[E0338]: 'radius' can reach its end without returning f32
  radius :: (k: Kind) -> f32 {
  ^
  fix: add a return at the end
1 error
```

And the compiler is right in a narrow sense: a value that came from `---`
memory isn't guaranteed to be a member. Checked casts are why that case is
rare.

The bigger problem with an `if` chain is that nothing tells you when it's
incomplete. Add a `mine` member to `Kind` next month and `radius` will
quietly return 0 for every mine. A `switch` that has to cover every member
would catch that at compile time. Until Coax has one, the `#forall` block at
the bottom of Listing 5-7 does the job. `#forall k: Kind` runs its body once
for every member of `Kind`, while compiling, and `#assert radius(k) > 0`
fails if any member falls through to the `return 0`. `coax run` doesn't run
checks; `coax test` does:

```
$ coax test match.coax
ok   #forall k: Kind (3 cases, 54 steps)
1 check, 1 passed, 0 failed
```

Let's see it catch a mistake. Delete the `bullet` line from `radius` and
test again:

```
$ coax test match.coax
FAIL #forall k: Kind
match.coax:18:3: error[E0406]: assertion failed
    #assert radius(k) > 0;
    ^
  note: in #forall k: Kind
  counterexample: k = Kind.bullet
1 check, 0 passed, 1 failed
```

The check names the member we forgot. Chapter 10 is about writing checks
like this one.

`describe` uses `ifx`, Coax's expression form of `if`, so the whole match is
one `return`. The last branch is a plain `else`, which makes the chain
complete by construction, at the cost of treating any new member as "a
shot". Choose the final `else` with that in mind.

> Note: `metareal/wallet.coax` drives a real payment through its states
> this way. `metareal_wallet_step :: (f: Flow, e: Event) -> Flow` is an
> `if e == Event.quote { ... } else if e == Event.confirm { ... }` chain,
> and a `#forall state: Pay, e: Event, sends: 0..<2` check walks every state
> and event pair to prove things like "a payment is never sent twice".

### What `switch` will look like

> Note: The spec plans an exhaustive `switch` but doesn't fix its syntax
> yet. Something like this, which `coax0` does not accept:
>
> ```jai
> radius :: (k: Kind) -> f32 {
>   switch k {
>     case .ship:   return 1.5;
>     case .rock:   return 4;
>     case .bullet: return 0.25;
>   }
> }
> ```
>
> A `switch` on an enum must handle every member or say `case else`, so
> adding `mine` turns every incomplete `switch` into a compile error.

## Flags

Some properties aren't one choice out of many but any combination: a rock
is solid and visible and wraps around the screen edges; a bullet is visible
and does damage. The classic representation is one bit per property in an
integer. An enum is the wrong tool here, because `|` on two members gives a
value that isn't a member:

```
flagsbad.coax:3:21: error[E0370]: '|' needs two integers of one type, found Kind and Kind
    both := Kind.ship | Kind.rock;
                      ^
1 error
```

Coax's design has a separate form, `enum_flags`, for exactly this case, and
`coax0` doesn't implement it yet:

```
layers.coax:1:10: error[E0204]: 'enum_flags' is reserved and not supported by coax0 yet
  Layer :: enum_flags u32 { solid; visible; damaging; wraps; }
           ^
1 error
```

Until it arrives, typed integer constants do the job.

Filename: flags.coax

```jai
#import "Basic";

// One bit each. cast(u32) gives the constants a type, so they can't mix
// with some other set of bits by accident.
SOLID     :: cast(u32) 1;      // 0b0001
VISIBLE   :: cast(u32) 2;      // 0b0010
DAMAGING  :: cast(u32) 4;      // 0b0100
WRAPS     :: cast(u32) 8;      // 0b1000

has :: (flags: u32, bit: u32) -> bool { return (flags & bit) != 0; }

main :: () {
  rock := SOLID | VISIBLE | WRAPS;
  shot := VISIBLE | DAMAGING;
  print("% %\n", rock, shot);
  print("% % %\n", has(rock, SOLID), has(shot, SOLID), has(shot, DAMAGING));

  rock ^= WRAPS;                       // flip one bit: now it's off
  print("% %\n", rock, has(rock, WRAPS));
  rock ^= WRAPS;                       // and back on
  print("% %\n", rock, has(rock, WRAPS));
}
```

*Listing 5-8: Flags as typed bit constants*

```
$ coax run flags.coax
11 6
true false true
3 false
11 true
```

`|` combines bits, `&` tests them, and `^` flips them. Each constant is
written `cast(u32) 1` so it's a `u32` rather than an untyped literal; a
`u16` set of flags can't then be mixed into it by accident. `coax0` has no
unary bitwise-not (`~` is `E0108: unexpected character`), so clearing a bit
you don't know is set takes `flags & (ALL ^ bit)` with an `ALL` constant
holding every bit.

What we lose compared with `enum_flags` is printing by name (the output says
`11`, not `solid | visible | wraps`) and any check that a value only uses
bits that have names.

## Tagged data

Our entities need a collision shape. Ships and rocks are circles, but we'd
like a box for the walls of the level. A shape is a circle *or* a box, and
the two need different data: a circle has a radius, a box has a width and a
height.

This is what a union is for, and Coax's design makes unions *tagged*: the
value records which alternative it holds, and the language won't let you
read the wrong one. `coax0` doesn't have unions yet:

```
un.coax:2:10: error[E0204]: 'union' is reserved and not supported by coax0 yet
  Shape :: union { circle: f32; box: Vec2; }
           ^
1 error
```

What we can write today is the same idea by hand: an enum for the tag, and
a struct that holds the tag and the fields for every alternative.

Filename: shapes.coax

```jai
#import "Basic";

Vec2 :: struct { x: f32; y: f32; }

// Which alternative a Shape holds. The tag is first, and every reader
// checks it before touching the fields that belong to one alternative.
ShapeKind :: enum u8 { circle = 1; box = 2; }

Shape :: struct {
  kind: ShapeKind = ShapeKind.circle;
  radius: f32;          // circle only
  half: Vec2;           // box only: half the width and half the height
}

circle :: (r: f32) -> Shape { return Shape.{kind = ShapeKind.circle, radius = r}; }
box :: (w: f32, h: f32) -> Shape {
  return Shape.{kind = ShapeKind.box, half = Vec2.{x = w / 2, y = h / 2}};
}

area :: (s: Shape) -> f32 {
  if s.kind == ShapeKind.circle { return 3.14159 * s.radius * s.radius; }
  return 4 * s.half.x * s.half.y;
}

// Is p inside s, with s centred on the origin?
contains :: (s: Shape, p: Vec2) -> bool {
  if s.kind == ShapeKind.circle { return p.x * p.x + p.y * p.y <= s.radius * s.radius; }
  return abs(p.x) <= s.half.x && abs(p.y) <= s.half.y;
}

// Only a circle has a radius. Asking a box for one is a bug, so stop.
radius_of :: (s: Shape) -> f32 {
  #assert s.kind == ShapeKind.circle;
  return s.radius;
}

main :: () {
  shapes := [2] Shape.[circle(2), box(4, 1)];
  p := Vec2.{x = 1.5, y = 0.25};
  for s: shapes {
    print("% area % contains (1.5, 0.25): %\n", s.kind, area(s), contains(s, p));
  }
  print("%\n", radius_of(shapes[0]));
  print("%\n", radius_of(shapes[1]));
}
```

*Listing 5-9: A tagged shape built from an enum and a struct*

```
$ coax run shapes.coax
circle area 12.56636 contains (1.5, 0.25): true
box area 4.0 contains (1.5, 0.25): true
2.0
shapes.coax:33: assertion failed
```

The pattern has three parts, and all three matter:

The *constructors* `circle` and `box` are the only places that build a
`Shape`. Each sets the tag and the fields that go with it, so the tag and
the data can't disagree as long as everyone uses them.

The *readers* `area` and `contains` test the tag before touching any
alternative's fields.

The *accessor* `radius_of` is for code that believes it already knows the
alternative. It checks with `#assert`, which stops the program in every
build when the belief is wrong, as the last line of output shows. Without
that check, `radius_of` on a box would return 0, the radius field's default,
and the bug would surface somewhere else as a collision that never happens.

### What the hand-written version costs

Let's look at the layout:

```
$ coax layout shapes.coax
...
Shape  size 16  align 4
       0     1  kind: ShapeKind
       1     3  (padding)
       4     4  radius: f32
       8     8  half: Vec2
...
```

A `Shape` is 16 bytes, but a circle only uses `kind` and `radius`, and a
box only uses `kind` and `half`. Every circle carries a box's worth of dead
space, and every new alternative adds its fields to every shape. A real
union would overlap the alternatives, so a `Shape` would be the tag plus the
largest alternative, which with natural alignment would be 12 bytes here. With two small alternatives, the
hand-written form is fine. With many alternatives of different sizes, the
waste adds up, and that's one of the reasons unions are on the plan.

The other cost is that the discipline lives in comments and in `#assert`s
that run at run time. Nothing at compile time stops a new procedure from
reading `s.radius` without testing `s.kind`.

### Planned: tagged unions

> Note: This is the planned design, and the syntax may change before
> `coax0` implements it.
>
> ```jai
> Shape :: union {
>   circle: struct { radius: f32; };
>   box:    struct { half: Vec2; };
> }
>
> area :: (s: Shape) -> f32 {
>   switch s {
>     case .circle: (c) { return 3.14159 * c.radius * c.radius; }
>     case .box:    (b) { return 4 * b.half.x * b.half.y; }
>   }
> }
> ```
>
> A Coax `union` records which alternative it holds. The only way to read an
> alternative is a `switch` case that binds it (`(c)` above), so reading the
> wrong one can't be written. The `switch` must handle every alternative or
> say `case else`, so adding a third shape turns every `area`-like
> procedure that forgot it into a compile error.
>
> This differs from C and Jai, where a plain `union` overlays storage and
> the program tracks the active member itself. Overlapping raw storage has
> real uses, such as reinterpreting the bits of a float. If Coax adds it,
> it will be a separate, visibly unsafe form, and the safe tagged form stays
> the one you get by writing `union`.

## Composition with `using` (planned)

Games often build a big struct out of smaller ones: a player has a body
(position and velocity) plus a score. With plain fields, you write
`p.body.pos.x`. The design adds `using`, which makes an embedded field's
members reachable from the outer struct, so `p.pos.x` means
`p.body.pos.x`. `coax0` refuses it:

```
player.coax:4:3: error[E0204]: 'using' is reserved and not supported by coax0 yet
    using body: Body;
    ^
1 error
```

When it lands, `using` will be a naming convenience and nothing more. It
won't create a class hierarchy, there will be no virtual dispatch, and a
`Player` won't silently convert to a `Body`. A conversion between them will
have to be declared on its own (the design calls it `#as`) and checked.
Most code doesn't need either: a procedure that works on bodies takes the
`Body` field, `move_body(p.body)`, and that's clear today.

## A struct is not a file format

Now that we have an `Entity`, it's tempting to save a level by writing out
each entity's bytes and reading them back later. Don't. A struct's bytes
hold padding, which can be anything (look back at the seven-byte gaps in
`Spark`). They hold views and strings as pointers, which mean nothing in
another run of the program. They follow one target's byte order and
alignment. And they carry no version, so next month's `Entity`, with one
more field, can't read this month's file.

Saves, network messages and protocol records go through explicit, versioned
encodings that say what every field means on the wire (Chapter 15). Even
when reflection generates the encoder, a person still has to decide what
some fields mean. A field that refers to another entity might be saved as an
entity ID, as an asset reference, or not at all; the type alone can't say.

The Metareal cast loader is a real example of the other direction. It
reads a binary asset file into Coax structs one field at a time, at offsets
the file format defines, with a little-endian helper:

```jai
u32_at :: (b: [] u8, at: int) -> u32 {
  return cast(u32) b[at] | (cast(u32) b[at + 1] << 8) | (cast(u32) b[at + 2] << 16) | (cast(u32) b[at + 3] << 24);
}
```

That's from `metareal/cast.coax`. Every read is bounds-checked, and the
struct it fills can change shape without changing the file. Chapter 6 looks
at byte views like `b` in detail.

## Summary

Structs let us group related values into one named type with named fields.
Literals name every field they set and start from the defaults, so a new
value is default initialized rather than merely zeroed, and `---` opts out.
Structs are values: assigning, passing and returning copy them, shallowly.
Coax has no methods; we write procedures that take a struct and return a new
one, or that change many values in place through a view, and the compiler
refuses both changing a parameter and ignoring a returned value. Structs
don't compare with `==` and can't contain themselves, and their fields are
laid out in the order we write them, so field order is a size decision we
make.

Enums name a fixed set of choices with a declared integer width. They
compare only with `==` and `!=`, convert to numbers only with a cast, and
convert back only through a check that traps on non-members in every build.
Values that leave the program are written out explicitly. Until `switch`
arrives, we match with `if` chains and use `#forall` to prove that every
member is handled. Flags are typed bit constants today and `enum_flags`
later. Tagged data is a tag plus fields, guarded by constructors and
asserts, until tagged unions arrive.

Several of these types held arrays, views and strings without our looking
closely at what those are. That's the subject of Chapter 6: where values live
in memory, what a view points at, and what Coax checks for you when you
index into one.
