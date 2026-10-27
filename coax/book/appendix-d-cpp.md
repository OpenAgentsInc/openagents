# Appendix D. Coax for C++ programmers

The engine Coax lives beside is C++, so this table is the practical one.

| C++ | Coax | Notes |
| --- | --- | --- |
| `#include` headers, declarations before use | `#import`; any declaration order | No headers, no forward declarations |
| `int x = 0;` | `x := 0;` or `x: int = 0;` | Coax `int` is 64-bit; C `int` usually isn't |
| `int x;` (garbage) | `x: int;` (zero) | Uninitialized only with `= ---` |
| `constexpr` / `consteval` | `NAME :: expr;` and `#run f()` | Any ordinary procedure can run at compile time |
| `static_assert` | `#assert` | Can call procedures through `#run` |
| Templates | `$T` generics with constraints | Errors at the use site; instance counts reported |
| Classes, inheritance, virtual | Structs, `using` for composition, explicit function tables if needed | No hidden vtables |
| Constructors and destructors (RAII) | Field defaults; `defer` at the call site | Cleanup is visible where the resource is acquired |
| Exceptions | Error values with `try` / `or_else`; traps for bugs | Every early exit is visible |
| `std::string` | `string` (counted view) plus explicit builders | Copies don't copy characters |
| `std::vector<T>` | `[..] T` with an allocator | Growth can move storage, as in C++ |
| `std::span<T>` | `[] T` | Same idea, built in |
| `std::unordered_map` | `Table(K, V)` with insertion-order iteration | Deterministic iteration by default |
| `union` | `union` (tagged) | Raw unions only in an explicit unsafe form |
| `enum class E : uint8_t` | `E :: enum u8 { ... }` | Explicit values where they persist |
| Preprocessor `#if` | `#if` on compile-time constants | Both branches parse |
| `-ffp-contract=off` per file | `strict` on the procedure | The rule travels with the code |
| Signed overflow is undefined behavior | Traps in development, wraps in release | Never undefined |
| `printf` format mismatches | `print` checked at compile time | Wrong counts don't compile |
| CMake | `build.coax` (planned) | Generates CMake and Xcode lists before replacing them |

## Calling between them

C++ calls Coax through `#export_c` functions and a generated C header.
Coax calls C++ through `extern "C"` shims. Data crosses as fixed-width
fields, explicit lengths and opaque handles (Chapter 16).
