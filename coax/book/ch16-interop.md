# 16. Living beside C, C++ and Swift

> **Partly runs today.** On `coax0` 0.3.0, `#export_c`, `coax lib` and
> `coax header` run (16.4 and 16.5), and the example is checked;
> `coax/examples/pulse` links a Coax kernel into a C host; `#foreign`
> (16.2) and `coax metal` (16.6) run too. Importing C
> headers (16.2), ownership and nullability annotations in headers, and
> anything that carries the context are *Design* (spec §17.2).

Coax will live beside 40,000 lines of C++ for years. It doesn't replace
the engine. It takes over one subsystem at a time behind a C boundary,
the same boundary Swift already uses to drive the engine
(`src/metal/home_app.h` in the Metareal source archive).

## 16.1 The C boundary is the contract

Every crossing uses plain C:

- **Fixed-width data.** Coax `int` is 64 bits. C `int` usually isn't. At
  the boundary, write `s32` or `s64` explicitly; never assume a host
  integer width. The generated header spells every type with `<stdint.h>`
  names, so `int` arrives as `int64_t`.
- **Explicit lengths.** A Coax `string` is a pointer and a count, not a
  `char *`, and a view `[] T` is the same: `{data, count}`. Passing one
  to C means passing both, or producing a zero-terminated copy on
  purpose.
- **Opaque handles** for objects one side owns and the other only refers
  to.
- **Named release functions.** Whoever allocates says how to free, and
  each allocation has exactly one way out.
- **No context across the line.** The Coax context never leaks into a C
  signature.

A generated header records, for every function: sizes, alignment,
calling convention, ownership, nullability, callback lifetimes and
allocator pairing. Today's header records sizes and offsets, as checked
assertions (16.4); the rest is *Design*.

## 16.2 Calling C

One C function at a time works today. `#foreign` declares a symbol the host
links in, by its exact name:

```coax
host_load :: (h: int, slot: int) -> f64 #foreign;
host_store :: (h: int, slot: int, value: f64) #foreign;
```

Foreign procedures pass integers, floats and `bool`, nothing else
(`E0376`): there is no pointer or ownership ABI yet, so a host that owns
memory hands Coax an integer handle. Compile-time code can't call them
(`E0404`). The generated header declares them, so the C compiler checks
the host's definitions against Coax's signatures. The native iOS home
(`metareal/home.coax`) is built this way.

Importing whole headers is design:

```coax
#import_c "stb_image.h";
```

Header import reads C declarations through libclang at compile time. The
rules must skip
what isn't callable (static inline functions, `va_list` variants, macros
that aren't constants) and say so, rather than generating calls to
symbols that don't exist.

## 16.3 C++ through shims (design)

Coax doesn't parse C++. Classes, templates and metal-cpp stay behind
`extern "C"` shims, which is the shape the engine's Swift boundary
already has. That's a few lines per entry point, and it keeps two
language-specific object models from ever touching.

## 16.4 Being called

`#export_c` makes a procedure callable from C, C++ and Swift. It keeps
its name, uses the C calling convention, and goes in the generated
header. This is how the first Coax subsystem ships: the C++ frame loop
calls a Coax function exactly as it would call a C one.

This is the kernel from `coax/examples/pulse`, the shape of the
per-frame pulse update in the engine's `src/paynet`. The book's copy
adds a `main` so it also runs on its own:

```coax
// The kernel from examples/pulse/pulse.coax, with a main so it also runs alone.
#import "Basic";

Pulse :: struct {
  position: f32;  // 0 <= position < 1 along the current channel
  speed: f32;     // channel lengths per second
  channel: s32;
  hops: s32;      // channels left to cross; 0 means it has arrived
}

CHANNELS :: 64;

route :: (channel: s32) -> s32 {
  return (channel * 17 + 5) % CHANNELS;
}

advance_pulses :: (pulses: [] Pulse, dt: f32) -> s64 #export_c {
  arrived: s64 = 0;
  for i: 0..<pulses.count {
    p := pulses[i];
    if p.hops == 0 { continue; }
    p.position += p.speed * dt;
    while p.position >= 1 {
      p.position -= 1;
      p.channel = route(p.channel);
      p.hops -= 1;
      if p.hops == 0 {
        arrived += 1;
        break;
      }
    }
    pulses[i] = p;
  }
  return arrived;
}

main :: () {
  pulses: [3] Pulse;
  pulses[0] = Pulse.{position = 0.5, speed = 2, channel = 1, hops = 1};
  pulses[1] = Pulse.{position = 0.25, speed = 1, channel = 7, hops = 3};
  pulses[2] = Pulse.{position = 0.75, speed = 0.5, channel = 9, hops = 0};
  arrived := advance_pulses(pulses[..], 0.5);
  print("arrived %\n", arrived);
  for p: pulses { print("%\n", p); }
}
```

```
arrived 1
Pulse.{position = 0.5, speed = 2.0, channel = 22, hops = 0}
Pulse.{position = 0.75, speed = 1.0, channel = 7, hops = 3}
Pulse.{position = 0.75, speed = 0.5, channel = 9, hops = 0}
```

Two commands turn a file like this into something C can link:

```sh
coax lib pulse.coax --release -o pulse.o   # the #export_c procedures, no C main
coax header pulse.coax -o pulse.h          # their declarations, with layout checks
```

`coax lib` leaves out `main`, and the procedures that aren't exported
stay `static`, so `route` can't collide with anything in the host. The
header for `examples/pulse/pulse.coax`, from `coax header`, is short
enough to read whole. Its core:

```c
typedef struct coax_t_Pulse coax_t_Pulse;
struct coax_t_Pulse { float position; float speed; int32_t channel; int32_t hops; };
typedef struct { coax_t_Pulse *data; int64_t count; } coax_v_t_Pulse;
typedef struct { int32_t *data; int64_t count; } coax_v_s32;
COAX_LAYOUT(sizeof(coax_t_Pulse) == 16, "layout of Pulse");
COAX_LAYOUT(offsetof(coax_t_Pulse, position) == 0, "offset of Pulse.position");
COAX_LAYOUT(offsetof(coax_t_Pulse, speed) == 4, "offset of Pulse.speed");
COAX_LAYOUT(offsetof(coax_t_Pulse, channel) == 8, "offset of Pulse.channel");
COAX_LAYOUT(offsetof(coax_t_Pulse, hops) == 12, "offset of Pulse.hops");

int64_t advance_pulses(coax_v_t_Pulse pulses, float dt);
void channel_load(coax_v_t_Pulse pulses, coax_v_s32 counts);
```

- **Types are prefixed.** A Coax struct `Pulse` is `coax_t_Pulse` in C,
  and a view `[] Pulse` is `coax_v_t_Pulse`, so Coax names can't clash
  with the host's. Fields keep their names, which is why a field can't
  be a C or C++ keyword (`E0371`).
- **Enums become integers and constants.** An `enum u8` is a `uint8_t`
  typedef with one `#define` per member (`COAX_Channel_busy`).
- **Layout is asserted, not assumed** (Chapter 10). `COAX_LAYOUT` is
  `_Static_assert` in C and `static_assert` in C++.
- **The header is plain C11**, wrapped in `extern "C"` when a C++
  compiler includes it. Swift reaches it the way it reaches the engine
  today, through C.

What the header can't carry is said in its opening comment, which `coax
header` writes every time:

```c
/* Generated by coax0 from examples/pulse/pulse.coax. Do not edit: change the .coax file.
   Coax int is int64_t; a Coax string or view is a pointer and a count, not
   NUL-terminated; a failed check inside a call aborts the process. */
```

That last clause matters. A bounds or cast check that fails inside
`advance_pulses` stops the host process. Struct defaults stay on the
Coax side too: a `coax_t_Pulse` a C caller makes starts as whatever C
put in it.

## 16.5 Checking it against C

`coax/examples/pulse/host.c` is a C host that runs 100,000 pulses for
600 frames. Every frame it calls the Coax kernel on one copy and a plain
C version of the same loop on another, and compares the two copies with
`memcmp`. `coax/examples/pulse/run.sh` builds the header and the
release object, compiles the host at `-O2`, and runs it. On the Mac this
book was written on:

```
100000 pulses x 600 frames: Coax matches the C reference byte for byte
arrived 61834, still moving 38166
time per frame: Coax 0.203 ms, C 0.203 ms
```

Byte for byte is a strong claim for float code, and it holds for the
reasons in Chapter 12: Coax's generated C turns off contraction, and the
host does the same (`#pragma STDC FP_CONTRACT OFF`), so both round once
per operation. The kernel keeps its bounds checks in the release build,
and the cost doesn't show at this size: both sides take about 0.2 ms a
frame. The times are one machine's measurement, not a promise.

This is the gate the engine work waits on (spec §20, stage 2): the real
`src/paynet` kernel called from the engine's frame loop, its outputs
compared and its frame cost measured, before anything in the app or its
CMake and Xcode builds changes.

`coax0`'s output is one C11 file with a small embedded runtime and no
dependencies beyond the C library, so it builds with any C compiler,
including Xcode's clang. Other targets (iOS, Android, Windows, wasm32)
need their own runtime, linker and ABI evidence before they count as
supported (Chapter 17).

## 16.6 Metal functions

The same checked procedures can become Metal Shading Language, for the
scalar math a shader stage calls:

```sh
coax metal metareal/light.coax > Light.metal
```

The Metal profile accepts `f32` and `bool` values, `if`/`else`, locals and
calls that don't recurse. Loops, strings, integers, `f64`, `print` and
`#foreign` calls are `E0377`. The native stage still binds textures and
attributes and packs vectors; Coax supplies the functions it calls. The C
build of the same file is what the tests run, so one source is checked on
the CPU and drawn on the GPU.
