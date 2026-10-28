# 10. Data-oriented layout

> **Partly runs today.** `coax layout` and the layout assertions in
> generated headers (10.3) run on `coax0` 0.3.0, and the example is
> checked. Per-target layouts, `SOA`, hot and cold fields, `for_simd` and
> layout fingerprints (10.1, 10.2, 10.4, 10.5) are *Design* (spec §4).
> The first real use is the `src/paynet` pulse update (Chapter 20).

A frame at 120 Hz is 8.3 milliseconds. In most engine loops, the time
goes to waiting for memory, not to arithmetic. Data-oriented design means
laying data out for the loops that read it, and Coax makes the layout a
visible, checkable choice.

## 10.1 AoS, SoA and blocks (design)

```jai
Pulse :: struct { t: f32; speed: f32; edge: u32; }

pulses_aos: [..] Pulse;          // t speed edge | t speed edge | ...
pulses_soa: SOA [..] Pulse;      // t t t t ... | speed speed ... | edge edge ...
pulses_blk: SOA(4) [..] Pulse;   // [t×4 speed×4 edge×4] [t×4 speed×4 edge×4] ...
```

| Layout | Good for | Costs |
| --- | --- | --- |
| Array of structs (AoS) | Code that touches whole elements | Loops over one field drag the others through the cache |
| Struct of arrays (SoA) | Loops over a few fields of many elements; SIMD | Touching a whole element reads several arrays |
| Blocked SoA, `SOA(4)` | 4-wide SIMD and GPU uploads | Partial blocks at the end; harder to index by hand |

`SOA` is a storage choice made where the array is declared. Loops still
read `it.t` and `it.speed`, and the compiler generates the accessors:

```jai
for pulses_soa { it.t = min(it.t + it.speed * dt, 1.0); }
```

Two rules keep this honest:

- **Copies keep their meaning.** Copying element `i` out of an SoA array
  gives you an ordinary `Pulse`, gathered from three arrays. There's no
  fake pointer to an "object" that doesn't exist in memory.
- **The cost is measured, not assumed.** Gathers and scatters aren't free,
  and switching a layout is a measurement to make.

Jai has shown both approaches: an `SOA` keyword in early demos, and SoA
generated with reflection and `#insert` in current code. Coax starts with
the explicit storage form and generates its accessors and reflection data
from the struct, so the layout and the metadata can't disagree.

## 10.2 Four layouts, kept apart (design)

One logical struct can have four physical forms, and each one is a
separate question:

1. **Native CPU layout** on a specific target (arm64 iOS, wasm32...).
2. **C ABI layout**, when it crosses into C, C++ or Swift.
3. **GPU resource layout**, with the shading language's alignment rules.
4. **Protocol encoding**, when it's saved or sent.

The compiler never answers a layout question with its own machine's
`sizeof`. Every layout query names a target. `coax0` has only one
target, the host its C is compiled for, so `size_of` and `coax layout`
answer for that host, and the generated header checks the answer against
the C compiler (10.3). When one declaration must
serve several, it either fits one explicit restricted layout that's valid
for all of them, or the compiler generates checked adapters between them.
There is no universal padding assumption.

## 10.3 Seeing the layout

Fields are laid out in the order you wrote them, each at its natural
alignment. Order decides padding:

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

```
24 16
96 64
```

`coax layout` prints every struct's size, alignment, field offsets and
padding, and every enum's width and values, without building anything:

```
$ coax layout examples/padding.coax
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

The columns are offset, size and field. More than half of `Loose` is
padding: an array of a million of them is 8 MB bigger than the same
array of `Tight`, and every byte of that is cache traffic that carries
nothing. The compiler won't reorder the fields for you (Chapter 5): the
layout is part of the contract with C, the GPU and saved data. It shows
you instead, and you decide.

The report is also the agent's view. An agent changing a struct can run
`coax layout` before and after and see exactly what moved, without
reading generated C.

### Layout the C compiler must agree with

When a struct crosses into C (Chapter 16), the header `coax header`
generates asserts Coax's layout, so a C compiler that lays it out
differently refuses to build. From `coax header examples/pulse/pulse.coax`
in the compiler's folder:

```c
COAX_LAYOUT(sizeof(coax_t_Pulse) == 16, "layout of Pulse");
COAX_LAYOUT(offsetof(coax_t_Pulse, position) == 0, "offset of Pulse.position");
COAX_LAYOUT(offsetof(coax_t_Pulse, speed) == 4, "offset of Pulse.speed");
COAX_LAYOUT(offsetof(coax_t_Pulse, channel) == 8, "offset of Pulse.channel");
COAX_LAYOUT(offsetof(coax_t_Pulse, hops) == 12, "offset of Pulse.hops");
```

`COAX_LAYOUT` is `_Static_assert` in C and `static_assert` in C++. A
packing pragma, a different compiler or a hand-edited struct can't
silently disagree with the Coax side: the build stops at the assertion
that names the field.

*Design:* layout reports per named target, and a **fingerprint**, a hash
of the layout that shaders, headers and saved data record, so a mismatch
is caught before the GPU reads garbage:

```jai
#run print_layout(Pulse, target = .ios_arm64);
```

```
Pulse  (ios_arm64)  size 12  align 4  fingerprint 6f1c…
  0  t      f32
  4  speed  f32
  8  edge   u32
```

## 10.4 Hot and cold fields (design)

```jai
Entity :: struct {
  position: v3 @hot;
  velocity: v3 @hot;
  name: string @cold;
  debug_color: v4 @cold;
}
```

Fields marked `@cold` go to a side array, so passes over hot fields keep
more useful data per cache line. Like `SOA`, this is a storage decision
with generated accessors, and it shows up in the layout report.

## 10.5 SIMD from layout (design)

```jai
for_simd pulses_blk { it.t = min(it.t + it.speed * dt, 1.0); }
```

Over `SOA(4)` data, `for_simd` processes one block per iteration with
4-wide lanes: NEON on Apple arm64 and SIMD128 in WebAssembly. Explicit
lane types (`f32x4`, `s32x4`) are there when you need to write the
vector code yourself. The performance stack's rule still applies: SIMD
goes where a profiler says, not everywhere.
