# Appendix A. Keywords, directives, types and current status

<!-- Generated from coax/release.coax; do not edit this snapshot. -->
Current compiler: **coax0 0.17.0**. Status inventory schema: **1**.

`works` means the bounded feature is implemented; `partial` records its limits.
`reserved` is recognized and refused; `designed` is planned syntax. A status claim
is reviewed metadata, not a proof, permission or complete target-support claim.

Introduced is the historical implementation version when recorded; — means
unrecorded or unimplemented. It is independent of this release and inventory
schema. Keep signed protocol/schema identities in their own version domains.

## Keywords

| Name | Meaning | Status | Introduced |
| --- | --- | --- | --- |
| `if` `else` | Conditional | works | — |
| `while` | Loop | works | — |
| `for` `break` `continue` | `for i: lo..<hi`, `for x, i: collection`; leave or skip an iteration | works | — |
| `return` | Return from a procedure | works | — |
| `true` `false` | `bool` literals | works | — |
| `ifx` `then` | Conditional expression: `ifx c then a else b` | works | — |
| `defer` | Run at scope exit, in reverse order, on every exit | works | — |
| `struct` `enum` | Type declarations: `Name :: struct { ... }`, `Name :: enum u8 { ... }` | works | — |
| `cast` | `cast(T) x` (checked), `cast,trunc(T) x` (keeps the low bits) | works | — |
| `size_of` | `size_of(T)`: the C size in bytes, as an untyped integer | works | — |
| `union` `enum_flags` | Tagged unions; flag sets | reserved | — |
| `switch` `case` | Exhaustive selection | reserved | — |
| `using` | Bring an embedded field's members into scope | reserved | — |
| `try` `or_else` | Propagate or handle an error result | reserved | — |
| `null` | The empty optional pointer | reserved | — |
| `push_context` | Change the implicit context for a block | reserved | — |
| `remove` | Remove the current element during iteration | reserved | — |
| `it` `it_index` | Implicit element and index names inside `for` (Coax names them instead: `for x, i: ...`) | reserved | — |
| `xx` | Integer-width autocast | reserved | — |
| `constraint` | Declare a generic constraint | reserved | — |
| `inline` `no_inline` | Inlining control at a definition or call | reserved | — |
| `type_info` | CPU type layout metadata, schema 1 | works | 0.12.0 |
| `type_of` | First-class type reflection | reserved | — |
| `Type` `Code` | Meta types | reserved | — |
| `job` `pure` | Jobs and effect markers | reserved | — |
| `strict` `deterministic` | Floating-point and determinism markers | reserved | — |
| `shader` | GPU procedure | reserved | — |

## Directives

| Name | Meaning | Status | Introduced |
| --- | --- | --- | --- |
| `#import "Basic";` | Import a module; `Basic` is the only one | works | — |
| `#run expr` | Evaluate at compile time | works | — |
| `#assert expr;` | A compile-time `bool` check at the top level; an always-on check as a statement | works | — |
| `#test "name" { }` | A check run by `coax test`, never compiled into the program | works | 0.5.0 |
| `#forall a: T, b: lo..<hi { }` (optionally `#forall "name" ...`) | A check over every combination of its bindings | works | 0.5.0 |
| `#authority;` | Every procedure in the file needs a check; checks run on every build | works | 0.5.0 |
| `#export_c` | After a procedure's signature: give it its own C name and the C calling convention (`coax lib`, `coax header`) | works | — |
| `#foreign` | After a procedure's signature, with no body: import a C function that passes only integers, floats and `bool` | works | — |
| `#import_proto "file.pb" as Prefix;` | Generate types and bounded codecs from a protobuf descriptor set (CPU code only) | works | — |
| `#load "part.coax";` | Add a once-only source unit with relative paths, cycle/budget checks and physical-file authority | works | 0.11.0 |
| `#if` | BUILD_DEBUG field/statement blocks; general conditions and top-level declarations remain planned | partial | 0.17.0 |
| `#insert` `#expand` `#text` `#location` | Typed code and macros | reserved | — |
| `#derive(...)` | Generate operations from fields | reserved | — |
| `#bake_arguments` | Explicit partial specialization | reserved | — |
| `#distinct` | A new type with the same representation | reserved | — |
| `#as` | Declared conversion to an embedded field | reserved | — |
| `#import_c` `#c_call` `#no_context` | C header import and calling conventions | reserved | — |
| `#scope_file` `#scope_module` `#scope_export` | Visibility | reserved | — |

## Intrinsics

| Name | Meaning | Status | Introduced |
| --- | --- | --- | --- |
| `print(format, ...)` | Print with `%` slots checked at compile time; needs `#import "Basic"` | works | — |
| `type_name(expr)` | The expression's type name as a string; the operand isn't evaluated | works | — |
| `size_of(T)` | The type's size in bytes (bootstrap LP64 CPU layout) | works | — |
| `type_info(T)` | Type name, kind, layout and counts as `CoaxTypeInfo` | works | 0.12.0 |
| `field_info(T, i)` | Struct field name, type and layout as `CoaxFieldInfo` | works | 0.12.0 |
| `enum_info(T, i)` | Enum member name and underlying bits as `CoaxEnumInfo` | works | 0.12.0 |
| `len(x)` | The length of a string, array or view (the same as `.count`) | works | — |
| `byte_at(s, i)` | The byte at index `i` of a string | works | — |
| `sin` `cos` `tan` `exp` `log` `sqrt` `floor` `abs` | One float argument, same type back | works | — |
| `pow(a, b)` | Two floats of one type | works | — |
| `f32_bits(x)` `f64_bits(x)` | A float's bit pattern as `u32` or `u64` | works | — |
| `f32_from_bits(b)` `f64_from_bits(b)` | The reverse | works | — |
| `bytes_overlap(a, b)` | Whether two byte views or strings share memory | works | — |
| `add_wrap(a, b)` `sub_wrap(a, b)` `mul_wrap(a, b)` | Matching u32/u64 arithmetic modulo the word width, including checked and compile-time execution | works | 0.15.0 |
| `rotl(a, count)` `rotr(a, count)` | Matching u32/u64 rotation; count reduced modulo the word width | works | 0.15.0 |
| `mul_wide(a, b)` | Two u64 operands produce CoaxU64Product with hi/lo halves; native128-bit and portable wasm32 lowering | works | 0.16.0 |
| `value_barrier(x)` | u32/u64 identity opaque to native optimization; compiler identity; backend-specific evidence is required for constant-time claims | works | — |

## Conversions

| Name | Meaning | Status | Introduced |
| --- | --- | --- | --- |
| `cast(T) x` | Convert between numbers, or between an enum and an integer; traps if the value doesn't fit | works | — |
| `cast,trunc(T) x` | Convert between integer types, keeping the low bits | works | — |
| `T(x)` for a number type | The same as `cast(T) x`: `int(x)`, `f64(x)`, `u8(x)` | works | — |
| `cast(string) v` | Borrow a `[] u8` view as a string | works | — |
| `xx x` | Automatic integer-width conversion | reserved | — |

## Literals

| Name | Meaning | Status | Introduced |
| --- | --- | --- | --- |
| Integer | `42`, `0xFF`, `0b1010`, `1_000_000` (up to the `u64` maximum) | works | — |
| Float | `1.5`, `2e-3`, `6.02e23`; with a suffix, `1.5f32`, `1.5f64` | works | — |
| Character | `'a'`, `'\n'` (one ASCII byte, an integer literal) | works | — |
| String | `"text\n"`, with the escapes `\n \t \r \0 \\ \"` | works | — |
| Struct | `Vec2.{x = 1, y = 2}` | works | — |
| Array | `[3] int.[1, 2, 3]` | works | — |
| Enum member | `Outcome.completed` | works | — |
| Uninitialized | `buf: [128] u8 = ---;` | works | — |
| Inferred struct literal | `.{x = 1}` without the name | designed | — |

## Types

| Name | Meaning | Status | Introduced |
| --- | --- | --- | --- |
| `int` (the same type as `s64`), `s8 s16 s32 u8 u16 u32 u64`, `uint` (the same as `u64`) |  | works | — |
| `f32 f64` |  | works | — |
| `bool`, `string` |  | works | — |
| structs and enums |  | works | — |
| `[N] T` fixed arrays, `[] T` views |  | works | — |
| `[..] T` dynamic arrays | (E0301) | reserved | — |
| `fixed32 fixed64` | (E0301) | reserved | — |
| `v2 v3 v4 quat m3 m4` | (E0301) | reserved | — |
| `Any` | (E0301); planned for tools and logging only | reserved | — |
| `*T`, `*T?`, `T?`, `Handle(T)`, lane types |  | designed | — |

## Capabilities

| Name | Meaning | Status | Introduced |
| --- | --- | --- | --- |
| `#load` / `coax deps` | Bounded source composition and input inventory | works | 0.11.0 |
| `#import_proto` | Selected proto3 descriptor generation in Coax; domain admission remains separate | works | 0.10.0 |
| Native source debugging | Original-file line maps and optional LLDB names; optimized values and generated-code limits remain | partial | 0.13.0 |
| E0383 local-borrow diagnostics | Simple local-array returns only; interprocedural and aggregate-contained lifetimes remain unchecked | works | 0.13.0 |
| Release/status generation | Canonical Coax catalogue, CLI status and build/publication drift gate | works | 0.14.0 |
| Differential conformance tooling | Optional bounded evaluator/native tests with independent oracle and reduced witnesses | partial | 0.14.0 |
| General ownership | Transfer, destruction and complete region/lifetime rules remain planned | designed | — |
| Apple C11 host | Tested Apple LP64 host; target support requires runtime/ABI/debugging evidence | partial | — |
| Coaxial browser path | Current Wasm/WebGPU consumers; general target layout/runtime/debugging remain incomplete | partial | — |

Annotations such as `@range` and `@unit` are lexed but not accepted yet.
The accepted grammar and focused implementation tests remain separate evidence
for these reviewed status claims.
