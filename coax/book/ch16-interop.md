# 16. Living beside C, C++ and Swift

> **Design, on top of a working base.** `coax0` already emits C11 that
> links with anything. Exported entry points, imported headers and
> generated bindings are next (spec §17.2).

Coax will live beside 40,000 lines of C++ for years. It doesn't replace
the engine. It takes over one subsystem at a time behind a C boundary,
the same boundary Swift already uses to drive the engine
(`src/metal/home_app.h` in the Metareal source archive).

## 16.1 The C boundary is the contract

Every crossing uses plain C:

- **Fixed-width data.** Coax `int` is 64 bits. C `int` usually isn't. At
  the boundary, write `s32` or `s64` explicitly; never assume a host integer width.
- **Explicit lengths.** A Coax `string` is a pointer and a count, not a
  `char *`. Passing one to C means passing both, or producing a
  zero-terminated copy on purpose.
- **Opaque handles** for objects one side owns and the other only refers
  to.
- **Named release functions.** Whoever allocates says how to free, and
  each allocation has exactly one way out.
- **No context across the line.** The Coax context never leaks into a C
  signature.

A generated header records, for every function: sizes, alignment,
calling convention, ownership, nullability, callback lifetimes and
allocator pairing.

## 16.2 Calling C

```coax
#import_c "stb_image.h";
```

Header import reads C declarations through libclang at compile time. The
rules must skip
what isn't callable (static inline functions, `va_list` variants, macros
that aren't constants) and say so, rather than generating calls to
symbols that don't exist.

## 16.3 C++ through shims

Coax doesn't parse C++. Classes, templates and metal-cpp stay behind
`extern "C"` shims, which is the shape the engine's Swift boundary
already has. That's a few lines per entry point, and it keeps two
language-specific object models from ever touching.

## 16.4 Being called

```coax
advance_pulses :: (pulses: *Pulse_Soa, count: s64, dt: f32) #export_c { ... }
```

`#export_c` makes a procedure callable from C, C++ and Swift with the C
calling convention, and adds it to the generated header. This is how the
first Coax subsystem ships: the existing C++ frame loop calls a Coax
function exactly as it would call a C one.

## 16.5 What `coax0` does today

`coax0` already proves the base of this. Its output is one C11 file with
`static` functions, a small embedded runtime and no dependencies, so it
builds with any C compiler, including Xcode's clang and Emscripten. The
next step is exported functions and a generated header, so the generated
file can link into the engine instead of being its own program.
