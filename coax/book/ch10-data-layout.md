# 10. Data-oriented layout

> **Design.** Spec §4. Nothing in this chapter is built yet. The first
> real use is the `src/paynet` pulse update (Chapter 20).

A frame at 120 Hz is 8.3 milliseconds. In most engine loops, the time
goes to waiting for memory, not to arithmetic. Data-oriented design means
laying data out for the loops that read it, and Coax makes the layout a
visible, checkable choice.

## 10.1 AoS, SoA and blocks

```coax
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

```coax
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

## 10.2 Four layouts, kept apart

One logical struct can have four physical forms, and each one is a
separate question:

1. **Native CPU layout** on a specific target (arm64 iOS, wasm32...).
2. **C ABI layout**, when it crosses into C, C++ or Swift.
3. **GPU resource layout**, with the shading language's alignment rules.
4. **Protocol encoding**, when it's saved or sent.

The compiler never answers a layout question with its own machine's
`sizeof`. Every layout query names a target. When one declaration must
serve several, it either fits one explicit restricted layout that's valid
for all of them, or the compiler generates checked adapters between them.
There is no universal padding assumption.

## 10.3 Seeing the layout

```coax
#run print_layout(Pulse, target = .ios_arm64);
```

```
Pulse  (ios_arm64)  size 12  align 4  fingerprint 6f1c…
  0  t      f32
  4  speed  f32
  8  edge   u32
```

A layout report gives the target, field offsets, alignment, padding, and
a **fingerprint**: a hash of the layout. Shaders, C headers and saved
data can record the fingerprint they were built against, and a mismatch
is caught before the GPU reads garbage.

## 10.4 Hot and cold fields

```coax
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

## 10.5 SIMD from layout

```coax
for_simd pulses_blk { it.t = min(it.t + it.speed * dt, 1.0); }
```

Over `SOA(4)` data, `for_simd` processes one block per iteration with
4-wide lanes: NEON on Apple arm64 and SIMD128 in WebAssembly. Explicit
lane types (`f32x4`, `s32x4`) are there when you need to write the
vector code yourself. The performance stack's rule still applies: SIMD
goes where a profiler says, not everywhere.
