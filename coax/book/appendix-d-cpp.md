# Appendix D. Coax for C++ programmers

The engine Coax lives beside is C++, and the Coax code in OpenAgents is
called from C++ and Swift hosts, so this is the comparison most readers will
use day to day. We'll go through the habits that carry over, the ones that
need adjusting, and the C++ features whose Coax counterparts are still
planned. Every Coax example was checked with `coax0` 0.10.0; the outputs and
errors shown are real.

## No headers, any order

In C++ you declare before you use, and you share declarations through
headers. In Coax there are no headers and no forward declarations. A
procedure can be called above the line where it's defined:

```cpp
// C++
#include <cstdio>
#include <cstdint>

int64_t twice(int64_t x);   // needed: twice is defined below main

int main() {
    std::printf("%lld\n", (long long)twice(21));
}

int64_t twice(int64_t x) { return x * 2; }
```

```jai
// Coax
#import "Basic";

main :: () {
    print("%\n", twice(21));
}

twice :: (x: int) -> int {
    return x * 2;
}
```

The checker reads every procedure's signature before it checks any body
(Chapter 27 shows the passes), so order never matters. The same holds in
the generated C, which gets a prototype for every procedure.

## Integers and initialization

Two small differences cause most surprises. First, Coax's `int` is 64 bits,
the same type as `s64`, while C++'s `int` is usually 32. Second, a Coax
variable declared without a value is zero, not garbage:

```jai
n: int;
print("% %\n", n, size_of(int));
```

```jai
0 8
```

If you really want uninitialized memory, say so with `= ---`, as in
`buf: [4096] u8 = ---;`. Struct fields follow the same rule: fields
without a default are zero, and fields with one get it, much like C++
default member initializers:

```cpp
// C++
struct Pulse {
    uint32_t amount;     // indeterminate in `Pulse p;`
    float speed = 1.5f;
};
```

```jai
// Coax
Pulse :: struct {
    amount: u32;
    speed: f32 = 1.5;
}

main :: () {
    p: Pulse;
    q := Pulse.{amount = 9};
    print("% %\n", p, q);
}
```

```jai
Pulse.{amount = 0, speed = 1.5} Pulse.{amount = 9, speed = 1.5}
```

There are no constructors. A struct literal names the fields it sets, and
everything else takes its default or zero. Notice too that `print` can
show a whole struct with one `%`; the format is checked at compile time.

## Compile-time code instead of `constexpr`

In C++, code that runs at compile time has to be marked `constexpr` or
`consteval`, and it's limited to what those allow. In Coax, any ordinary
procedure can run at compile time with `#run`, and `#assert` plays the part
of `static_assert`:

```cpp
// C++
constexpr std::array<uint16_t, 8> squares() {
    std::array<uint16_t, 8> out{};
    for (int i = 0; i < 8; i++) out[i] = uint16_t(i * i);
    return out;
}
constexpr auto SQUARES = squares();
static_assert(SQUARES[7] == 49);
```

```jai
// Coax
squares :: () -> [8] u16 {
    out: [8] u16;
    for i: 0..<8 { out[i] = cast(u16) (i * i); }
    return out;
}

SQUARES :: #run squares();
#assert SQUARES[7] == 49;
```

The generated C contains only the result, a `static const` array of the
eight values; `squares` isn't called at run time. A failed `#assert` stops
the build:

```jai
cpp2b.coax:10:20: error[E0406]: compile-time assertion failed
  #assert SQUARES[7] == 48;
                     ^
1 error
```

Compile-time code can't do I/O and runs within a step budget (Chapter 9).

## `defer` instead of destructors

C++ ties cleanup to a type's destructor. Coax puts it at the place where
you acquire the resource, with `defer`:

```cpp
// C++
struct Log {
    int fd;
    Log() : fd(open_log()) {}
    ~Log() { close_log(fd); }
};

void work() {
    Log log;
    std::puts("working");
}   // ~Log runs here
```

```jai
// Coax
open_log :: () -> int { print("open\n"); return 3; }
close_log :: (fd: int) { print("close %\n", fd); }

main :: () {
    fd := open_log();
    defer close_log(fd);
    print("working\n");
}
```

```jai
open
working
close 3
```

The deferred call runs on every exit from the block, including `return`,
`break` and `continue`, in reverse order. You can see the cleanup without
looking up a type's definition. Chapter 7 covers the details.

## Views instead of `std::span`

A Coax view, `[] T`, is a pointer and a count, like `std::span<T>`, but
built into the language. Slicing an array gives you one, and writes through
it land in the array:

```cpp
// C++
void scale(std::span<float> values, float by) {
    for (float &v : values) v *= by;
}
// ...
float speeds[4] = {1, 2, 3, 4};
scale(std::span(speeds).subspan(1, 2), 10);
```

```jai
// Coax
scale :: (values: [] f32, by: f32) {
    for i: 0..<values.count {
        values[i] = values[i] * by;
    }
}

main :: () {
    speeds := [4] f32.[1, 2, 3, 4];
    scale(speeds[1..<3], 10);
    print("%\n", speeds);
}
```

```jai
[1.0, 20.0, 30.0, 4.0]
```

Unlike `std::span`'s `operator[]`, every index is bounds-checked, in debug
and release builds. Fixed arrays like `speeds` are values: assigning one
copies it, as `std::array` does. `string` is a view too, a counted run of
bytes, so copying a string never copies its characters.

## Undefined behavior, made defined

Several things that are undefined or unspecified in C++ have one meaning
in Coax:

- Signed overflow is undefined in C++. In Coax it traps in development
  builds and wraps in release builds; it's never undefined.
- C++ leaves the order of function arguments unspecified. Coax evaluates
  operands and arguments left to right, and the generated C enforces it
  (Chapter 27 shows how).
- An out-of-range index is undefined in C++. In Coax it traps.
- A `printf` whose format doesn't match its arguments compiles in C++, at
  most with a warning. In Coax it doesn't compile. Here's the error from
  Chapter 27's program with an argument missing:

```jai
tiny.coax:17:11: error[E0320]: format has 4 '%' slots but print was given 3 values
      print("area % pulse % % %\n", AREA, p.amount, p.hops);
            ^
  fix: write '%%' for a literal percent sign
```

Floating-point code is compiled with contraction off, so the C compiler
can't fuse a multiply and an add and change your results, the effect C++
programmers usually get with `-ffp-contract=off` per file.

## Current counterparts and remaining design

Some C++ features have a bounded working counterpart; others remain
planned. This table keeps those statuses separate. The linked chapters
explain each current limit and the remaining design.

| C++ | Coax design | Status in `coax0` |
| --- | --- | --- |
| Templates | `$T` procedure and fixed-storage struct instances with `$N` lengths; view-containing struct bindings are refused (Chapter 8) | bounded instances work; constraints planned |
| Classes, inheritance, `virtual` | Structs, `using` for composition, explicit function tables where needed; no hidden vtables | structs work; the rest planned |
| Exceptions | Error values with `try` and `or_else`; traps for bugs (Chapter 7) | traps work; error values planned |
| `std::vector<T>` | `[..] T` with an allocator from the context (Chapter 6) | planned |
| `std::unordered_map` | `Table(K, V)`, iterating in insertion order | planned |
| `union` | A tagged `union` with an exhaustive `switch`; raw unions only in an explicit unsafe form (Chapter 5) | planned |
| `enum class E : uint8_t` | `E :: enum u8 { ... }` | works |
| Pointers | `*T`, and `*T?` for one that may be null | planned |
| Preprocessor `#if` | Parsed conditional fields/statements with `BUILD_DEBUG`; general conditions and top-level declaration selection follow | `BUILD_DEBUG` fields/statements work; general forms planned |
| Per-file `-ffp-contract=off` and similar | `strict` on the procedure, so the rule travels with the code (Chapter 16) | contraction is always off; `strict` planned |
| CMake | `build.coax`, generating CMake and Xcode lists before replacing them (Chapter 19) | planned |

## Calling between them

C++ calls Coax through procedures marked `#export_c`. `coax lib` builds
them into an object file, and `coax header` writes a C header that works
from C and C++, with the Coax struct layouts checked by `static_assert`:

```jai
Pulse :: struct {
    amount: u32;
    hops: u8;
    speed: f32 = 1.5;
}

advance :: (p: Pulse) -> Pulse #export_c {
    q := p;
    q.hops += 1;
    return q;
}
```

The header's core is:

```c
typedef struct coax_t_Pulse coax_t_Pulse;
struct coax_t_Pulse { uint32_t amount; uint8_t hops; float speed; };
COAX_LAYOUT(sizeof(coax_t_Pulse) == 12 && COAX_ALIGNOF(coax_t_Pulse) == 4, "layout of Pulse");
COAX_LAYOUT(offsetof(coax_t_Pulse, amount) == 0, "offset of Pulse.amount");
COAX_LAYOUT(offsetof(coax_t_Pulse, hops) == 4, "offset of Pulse.hops");
COAX_LAYOUT(offsetof(coax_t_Pulse, speed) == 8, "offset of Pulse.speed");
coax_t_Pulse advance(coax_t_Pulse p);
```

In C++, `COAX_LAYOUT` is `static_assert` and the declarations sit inside
`extern "C"`. Going the other way, Coax calls C functions declared with
`#foreign`, which in `coax0` pass only integers, floats and `bool`; C++
code is reached through `extern "C"` shims. Data crosses as fixed-width
fields, explicit lengths and opaque handles. Chapter 20 covers the whole
boundary.
