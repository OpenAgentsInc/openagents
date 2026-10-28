# 11. Data-oriented layout

At 120 frames a second, a frame lasts 8.3 milliseconds. In most engine
loops, a large share of that time goes to waiting for memory rather than
doing arithmetic. The processor can add and multiply far faster than it
can fetch bytes it doesn't already have in its cache, so how data sits in
memory often matters more than how clever the loop is. *Data-oriented
design* means laying data out for the loops that read it.

Coax treats layout as something you choose and can see, not something
the compiler decides for you behind your back. In this chapter we'll look
at how `coax0` lays out a struct and how to inspect it with `coax layout`.
Then we'll pin a layout down so it can't change by accident, and see how
the same layout is enforced across the boundary to C. Then we'll build
the same small simulation two ways, as an array of structs and as a
struct of arrays, and compare what each loop has to read. The chapter
ends with the layout features that are designed but not yet built:
`SOA` storage, per-target layouts, hot and cold fields, and SIMD loops.

> Note: `coax layout`, `size_of`, `#assert` on layouts and the layout
> checks in generated headers all run on `coax0` 0.10.0, and every
> runnable listing here is in the book's examples. `SOA`, per-target
> layouts, `@hot` and `@cold`, `for_simd` and layout fingerprints are
> design (spec section 4) and are marked where they come up.

## How a struct is laid out

Every type has a *size*, the number of bytes it takes, and an
*alignment*: its address must be a multiple of that number. An `f64` has
size 8 and alignment 8, a `u16` has 2 and 2, and a `u8` has 1 and 1. When
a field's natural position doesn't meet its alignment, the compiler puts
unused bytes, *padding*, in front of it. A struct's own alignment is the
largest alignment of its fields, and its size is rounded up to a multiple
of that, so that every element of an array of them is aligned too.

`coax0` lays fields out in the order you write them, each at its natural
alignment. That means the order you choose decides how much padding
there is. Here are two structs with the same three fields in different
orders.

Filename: padding.coax

```jai
#import "Basic";

Kind :: enum u8 { rock; tree; door; }

// Declaration order, natural alignment: 13 bytes of padding.
Loose :: struct {
  kind: Kind;
  position: f64;
  flags: u16;
}

// The same fields, largest first: 5 bytes of padding, all at the end.
Tight :: struct {
  position: f64;
  flags: u16;
  kind: Kind;
}

main :: () {
  print("% %\n", size_of(Loose), size_of(Tight));
  print("% %\n", size_of([4] Loose), size_of([4] Tight));
}
```

*Listing 11-1: The same fields in two orders*

`size_of` takes a type and gives its size in bytes as a constant. It
works on structs, enums and arrays alike. Run it:

```
$ coax run padding.coax
24 16
96 64
```

Loose is half again as big as Tight. To see why, ask the compiler to
show you. `coax layout` prints every struct's size, alignment, field
offsets and padding, and every enum's width and values, without
building anything:

```
$ coax layout padding.coax
Loose  size 24  align 8
       0     1  kind: Kind
       1     7  (padding)
       8     8  position: f64
      16     2  flags: u16
      18     6  (padding)
Tight  size 16  align 8
       0     8  position: f64
       8     2  flags: u16
      10     1  kind: Kind
      11     5  (padding)
Kind  enum u8  size 1
  rock = 0
  tree = 1
  door = 2
```

The columns are offset, size and field. In `Loose`, `kind` takes one
byte at offset 0, but `position` needs an 8-aligned address, so seven
bytes of padding come before it. After `flags`, six more bytes round the
struct up to 24, a multiple of its alignment of 8. Thirteen of its 24
bytes carry nothing.

`Tight` puts the largest field first. Each following field is already at
a position its alignment accepts, so the only padding is the five bytes
at the end that round 11 up to 16.

The difference grows with the data. An array of a million `Loose` values
is 8 MB bigger than the same array of `Tight`, and a loop over it pulls
those 8 MB of padding through the cache for nothing.

### Why the compiler doesn't reorder for you

Some languages are free to reorder fields to reduce padding. Coax won't,
for the reason given in Chapter 5: a struct's layout is part of its
contract with C code, with GPU shaders and with saved data, and those
can't follow a layout that changes when someone adds a field. So Coax
keeps your order and shows you the cost. You decide.

`coax layout` is also useful when someone else changes the struct,
including an agent. Run it before and after a change and compare the
output, and you see exactly which offsets moved, without reading the
generated C.

## Pinning a layout

Seeing a layout is good. Making sure it doesn't change without anyone
noticing is better. The top-level `#assert` from Chapter 9 can check a
size while compiling. Here's the start of a program we'll build later in
this chapter: a payment pulse moving along an edge of a network.

```jai
Pulse :: struct {
  t: f32;      // how far along its edge, 0 to 1
  speed: f32;  // edges per second
  edge: u32;   // which edge it is on
}

#assert size_of(Pulse) == 12;
```

*Listing 11-2: A struct with its size pinned*

Now suppose someone adds a field, say `owner: u64` for who sent the
pulse. An 8-byte field after three 4-byte ones needs four bytes of
padding before it, and the struct grows to 24 bytes. The build stops:

```
$ coax run pulses.coax
pulses.coax:10:24: error[E0406]: compile-time assertion failed
  #assert size_of(Pulse) == 12;
                         ^
1 error
```

`coax layout` stops at the same assertion, since it compiles the file
too. The person making the change now has to decide on purpose: update
the assertion and everything that relies on 12 bytes, or put `owner`
somewhere else.

> Note: `coax0` has `size_of` but not yet `align_of` or `offset_of`. To
> pin individual offsets today, rely on the generated header checks
> below, or on `coax layout` output reviewed in a diff.

### Layouts the C compiler must agree with

When a struct crosses into C (Chapter 17), both sides have to agree on
every offset. If they don't, C reads `speed` where Coax wrote `edge`,
and nothing crashes until the numbers look strange. So the header that
`coax header` generates asserts Coax's layout. Here are the checks it
writes for the `Pulse` in `coax/examples/pulse/pulse.coax`, which has
four 4-byte fields:

```
$ coax header examples/pulse/pulse.coax
...
COAX_LAYOUT(sizeof(coax_t_Pulse) == 16 && COAX_ALIGNOF(coax_t_Pulse) == 4, "layout of Pulse");
COAX_LAYOUT(offsetof(coax_t_Pulse, position) == 0, "offset of Pulse.position");
COAX_LAYOUT(offsetof(coax_t_Pulse, speed) == 4, "offset of Pulse.speed");
COAX_LAYOUT(offsetof(coax_t_Pulse, channel) == 8, "offset of Pulse.channel");
COAX_LAYOUT(offsetof(coax_t_Pulse, hops) == 12, "offset of Pulse.hops");
...
```

`COAX_LAYOUT` is defined as `_Static_assert` in C and `static_assert` in
C++, so these run in the C compiler. A packing pragma, a different
compiler or a struct edited by hand on the C side can't silently
disagree with Coax: the C build stops at the assertion that names the
field. The iOS wallet relies on exactly this. Its Swift code reads the
`Flow` struct from `metareal/wallet.coax`, and the header's layout
checks pin it.

## Arrays of structs and structs of arrays

Now let's look at the layout of many values instead of one. The usual
way to store a thousand pulses is an array of `Pulse`: each pulse's
fields sit together, then the next pulse's, and so on. That's called an
*array of structs*, or AoS.

```
t speed edge | t speed edge | t speed edge | ...
```

The other way is to keep one array per field: all the `t` values, then
all the `speed` values, then all the `edge` values. That's a *struct of
arrays*, or SoA.

```
t t t t ... | speed speed speed ... | edge edge edge ...
```

Which is better depends on the loop. Suppose each frame we move every
pulse along its edge. That reads `t` and `speed` and writes `t`. It
never touches `edge`. With AoS, the cache brings in whole pulses,
`edge` included, so a third of every fetch is wasted on this loop. With
SoA, the loop reads exactly the two arrays it needs.

`coax0` doesn't have SoA storage built in yet, but you can write it by
hand, with one array per field and view parameters for the loop. Let's
write both versions side by side and check that they agree.

Filename: pulses.coax

```jai
#import "Basic";

Pulse :: struct {
  t: f32;      // how far along its edge, 0 to 1
  speed: f32;  // edges per second
  edge: u32;   // which edge it is on
}

#assert size_of(Pulse) == 12;

COUNT :: 8;

// A pulse stops at the end of its edge.
finish :: (t: f32) -> f32 {
  if t > 1 { return 1; }
  return t;
}

// Array of structs: each pulse's fields sit together.
advance_aos :: (pulses: [] Pulse, dt: f32) {
  for i: 0..<pulses.count {
    p := pulses[i];
    p.t = finish(p.t + p.speed * dt);
    pulses[i] = p;
  }
}

// Struct of arrays: one array per field. The loop never reads edges.
advance_soa :: (t: [] f32, speed: [] f32, dt: f32) {
  for i: 0..<t.count {
    t[i] = finish(t[i] + speed[i] * dt);
  }
}

main :: () {
  aos: [COUNT] Pulse;
  t: [COUNT] f32;
  speed: [COUNT] f32;
  edge: [COUNT] u32;
  for i: 0..<COUNT {
    s := 0.125 * cast(f32) (i + 1);
    aos[i] = Pulse.{t = 0, speed = s, edge = cast(u32) i};
    t[i] = 0;
    speed[i] = s;
    edge[i] = cast(u32) i;
  }

  for frame: 0..<4 {
    advance_aos(aos[..], 0.5);
    advance_soa(t[..], speed[..], 0.5);
  }

  for i: 0..<COUNT {
    print("pulse %: % %\n", i, aos[i].t, t[i]);
  }
  print("aos loop reads % bytes, soa loop reads %\n",
        size_of([COUNT] Pulse), size_of([COUNT] f32) * 2);
}
```

*Listing 11-3: The same update over AoS and SoA storage*

`advance_aos` takes a view of pulses. Loop variables and elements are
copies, so it copies each pulse into `p`, updates it and writes it back.
`advance_soa` takes two views, `t` and `speed`, and writes through `t`
directly. Since a view's elements belong to the caller's array (Chapter
6), both loops change `main`'s arrays.

```
$ coax run pulses.coax
pulse 0: 0.25 0.25
pulse 1: 0.5 0.5
pulse 2: 0.75 0.75
pulse 3: 1.0 1.0
pulse 4: 1.0 1.0
pulse 5: 1.0 1.0
pulse 6: 1.0 1.0
pulse 7: 1.0 1.0
aos loop reads 96 bytes, soa loop reads 64
```

The two columns agree, as they should: the layouts store the same
information. The last line is the difference. The AoS loop has to bring
in all 96 bytes of the array, while the SoA loop reads two 32-byte
arrays and leaves `edge` alone. With eight pulses that doesn't matter.
With a hundred thousand, it's the difference between 1.2 MB and 800 KB
of memory traffic every frame.

SoA isn't free, though. Code that works on one whole pulse, such as
saving it or sending it, now has to gather it from three arrays. Adding
a pulse means appending to three arrays and keeping them the same
length, and in Listing 11-3 nothing but care keeps them in step. The
choice depends on which loops are hot, and it's a measurement to make,
not a rule to follow.

| Layout | Good for | Costs |
| --- | --- | --- |
| Array of structs (AoS) | Code that touches whole elements | Loops over one field drag the others through the cache |
| Struct of arrays (SoA) | Loops over a few fields of many elements; SIMD | Touching a whole element reads several arrays |
| Blocked SoA | 4-wide SIMD and GPU uploads | Partial blocks at the end; harder to index by hand |

The third row is a middle ground: blocks of four elements stored SoA,
`[t×4 speed×4 edge×4] [t×4 speed×4 edge×4] ...`. Each block fits a 4-wide
vector register, and a pulse's fields are still near each other.

## `SOA` storage

> Note: this section is design (spec section 4.1). `coax0` doesn't know
> the `SOA` keyword yet.

Writing SoA by hand works, but it means keeping several arrays in sync
and rewriting every loop if you change your mind. The plan is to make
storage a choice made where the array is declared, with the struct
staying the same:

```jai
pulses_aos: [..] Pulse;          // t speed edge | t speed edge | ...
pulses_soa: SOA [..] Pulse;      // t t t t ... | speed speed ... | edge edge ...
pulses_blk: SOA(4) [..] Pulse;   // [t×4 speed×4 edge×4] [t×4 speed×4 edge×4] ...
```

Loops would read `it.t` and `it.speed` as they do for an ordinary array,
and the compiler would generate the accessors:

```jai
for pulses_soa { it.t = finish(it.t + it.speed * dt); }
```

If you try an `SOA` declaration today, the parser stops at the word it
doesn't know:

```
$ coax check soa.coax
soa.coax:3:14: error[E0201]: expected ';' after the declaration, found '['
    pulses: SOA [8] Pulse;
               ^
  fix: add ';'
1 error
```

Two rules are meant to keep the feature honest. First, copies keep
their meaning: copying element `i` out of an SoA array gives you an
ordinary `Pulse`, gathered from three arrays. There's no pretend pointer
to an "object" that doesn't exist anywhere in memory. Second, the cost
is measured, not assumed. Gathers and scatters cost something, and
switching a layout is a decision backed by a profile.

> Note: Jai has tried both approaches: an `SOA` keyword in early demos,
> and SoA generated with reflection and `#insert` in current code. Coax
> plans the explicit storage form, with accessors and reflection data
> generated from the struct, so the layout and the metadata can't
> disagree.

## Four layouts, kept apart

> Note: per-target layouts are design (spec section 4.3). `coax0` has one
> target, the host its C is compiled for.

One logical struct can have four physical forms, and each is a separate
question:

1. The native CPU layout on a specific target, such as arm64 iOS or
   wasm32.
2. The C ABI layout, when it crosses into C, C++ or Swift.
3. The GPU resource layout, with the shading language's alignment rules.
4. The protocol encoding, when it's saved or sent (Chapter 15).

The rule is that the compiler never answers a layout question with its
own machine's `sizeof`; every layout question names a target. Today
there's only one target, so `size_of` and `coax layout` answer for the
host, and the generated header checks that answer against the C
compiler. When one declaration has to serve several of these forms, it
either fits one explicit restricted layout that's valid for all of them,
or the compiler generates checked adapters between them. There's no
universal padding assumption.

The planned layout report names its target and adds a *fingerprint*, a
hash of the layout that shaders, headers and saved data record, so a
mismatch is caught before the GPU reads garbage:

```jai
#run print_layout(Pulse, target = .ios_arm64);
```

```
Pulse  (ios_arm64)  size 12  align 4  fingerprint 6f1c…
  0  t      f32
  4  speed  f32
  8  edge   u32
```

That output is illustrative; neither `print_layout` nor per-target
fingerprints exist yet.

## Hot and cold fields

> Note: design. `coax0` rejects field annotations like `@hot`.

Some fields are read every frame and others almost never. An entity's
position and velocity are hot; its name and debug color are cold. The
plan is to mark them:

```jai
Entity :: struct {
  position: v3 @hot;
  velocity: v3 @hot;
  name: string @cold;
  debug_color: v4 @cold;
}
```

Fields marked `@cold` would go to a side array, so passes over hot
fields keep more useful data in each cache line. Like `SOA`, this is a
storage decision with generated accessors, and it would show up in the
layout report.

## SIMD from layout

> Note: design. There is no `for_simd` and no lane types in `coax0`.

```jai
for_simd pulses_blk { it.t = finish(it.t + it.speed * dt); }
```

Over `SOA(4)` data, `for_simd` would process one block per iteration
with 4-wide lanes: NEON on Apple arm64 and SIMD128 in WebAssembly.
Explicit lane types such as `f32x4` and `s32x4` would be there when you
want to write the vector code yourself. The usual rule for performance
work applies: SIMD goes where a profiler says it helps, not everywhere.

## Summary

`coax0` lays out structs in the order you write them, at natural
alignment, and never reorders fields, because the layout is part of a
contract with C, the GPU and saved data. `coax layout` shows every
offset and every byte of padding, `size_of` and `#assert` pin a size so
it can't change unnoticed, and the headers `coax header` generates make
the C compiler check the same layout. Across many values, AoS keeps
each element together and SoA keeps each field together; today you
write SoA by hand with one array per field, and `SOA` storage,
per-target layouts, hot and cold fields and `for_simd` are designed to
make that choice a one-word change.

Layout decides what one loop reads. The next chapter is about running
several loops at once, and how Coax plans to let the compiler see which
ones can safely run side by side.
