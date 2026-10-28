# Appendix A. Keywords, directives and types

This appendix lists every keyword, directive, intrinsic, literal form and
type the Coax design names, and says whether `coax0` 0.11.0 accepts it.

- **works**: implemented and tested in `coax0` 0.11.0.
- **reserved**: the word is reserved, and `coax0` refuses it with "not
  supported by coax0 yet" (E0204 for keywords, E0210 for directives, E0301
  for types) rather than guessing what it means.
- **designed**: part of the specification, but the syntax isn't recognized
  yet, so `coax0` reports an ordinary syntax error.

## Keywords

The lexer reserves exactly these 41 words (`LX_keyword` in
`coax/lib/compiler/lexer.coax`). Type names such as `int` and `f32` are not
keywords; they're ordinary identifiers that the checker resolves.

| Keyword | Meaning | Status |
| --- | --- | --- |
| `if` `else` | Conditional | works |
| `while` | Loop | works |
| `for` `break` `continue` | `for i: lo..<hi`, `for x, i: collection`; leave or skip an iteration | works |
| `return` | Return from a procedure | works |
| `true` `false` | `bool` literals | works |
| `ifx` `then` | Conditional expression: `ifx c then a else b` | works |
| `defer` | Run at scope exit, in reverse order, on every exit | works |
| `struct` `enum` | Type declarations: `Name :: struct { ... }`, `Name :: enum u8 { ... }` | works |
| `cast` | `cast(T) x` (checked), `cast,trunc(T) x` (keeps the low bits) | works |
| `size_of` | `size_of(T)`: the C size in bytes, as an untyped integer | works |
| `union` `enum_flags` | Tagged unions; flag sets | reserved |
| `switch` `case` | Exhaustive selection | reserved |
| `using` | Bring an embedded field's members into scope | reserved |
| `try` `or_else` | Propagate or handle an error result | reserved |
| `null` | The empty optional pointer | reserved |
| `push_context` | Change the implicit context for a block | reserved |
| `remove` | Remove the current element during iteration | reserved |
| `it` `it_index` | Implicit element and index names inside `for` (Coax names them instead: `for x, i: ...`) | reserved |
| `xx` | Integer-width autocast | reserved |
| `constraint` | Declare a generic constraint | reserved |
| `inline` `no_inline` | Inlining control at a definition or call | reserved |
| `type_of` `type_info` | Reflection | reserved |
| `Type` `Code` | Meta types | reserved |
| `job` `pure` | Jobs and effect markers | reserved |
| `strict` `deterministic` | Floating-point and determinism markers | reserved |
| `shader` | GPU procedure | reserved |

## Directives

| Directive | Meaning | Status |
| --- | --- | --- |
| `#import "Basic";` | Import a module; `Basic` is the only one | works |
| `#run expr` | Evaluate at compile time | works |
| `#assert expr;` | A compile-time `bool` check at the top level; an always-on check as a statement | works |
| `#test "name" { }` | A check run by `coax test`, never compiled into the program | works |
| `#forall a: T, b: lo..<hi { }` (optionally `#forall "name" ...`) | A check over every combination of its bindings | works |
| `#authority;` | Every procedure in the file needs a check; checks run on every build | works |
| `#export_c` | After a procedure's signature: give it its own C name and the C calling convention (`coax lib`, `coax header`) | works |
| `#foreign` | After a procedure's signature, with no body: import a C function that passes only integers, floats and `bool` | works |
| `#import_proto "file.pb" as Prefix;` | Generate types and bounded codecs from a protobuf descriptor set (CPU code only) | works |
| `#load "part.coax";` | Add a once-only source unit with relative paths, cycle/budget checks and physical-file authority | works |
| `#if` | Conditional compilation | reserved |
| `#insert` `#expand` `#text` `#location` | Typed code and macros | reserved |
| `#derive(...)` | Generate operations from fields | reserved |
| `#bake_arguments` | Explicit partial specialization | reserved |
| `#distinct` | A new type with the same representation | reserved |
| `#as` | Declared conversion to an embedded field | reserved |
| `#import_c` `#c_call` `#no_context` | C header import and calling conventions | reserved |
| `#scope_file` `#scope_module` `#scope_export` | Visibility | reserved |

Annotations such as `@gpu` are recognized by the lexer as tokens, but no
annotation is accepted anywhere in a `coax0` program yet.

## Intrinsics

These look like procedure calls but are built into the compiler. They work
the same in generated C, at compile time and, for the math functions, in
`coax metal` output.

| Name | Meaning | Status |
| --- | --- | --- |
| `print(format, ...)` | Print with `%` slots checked at compile time; needs `#import "Basic"` | works |
| `type_name(expr)` | The expression's type name as a string; the operand isn't evaluated | works |
| `size_of(T)` | The type's size in bytes (the host C layout) | works |
| `len(x)` | The length of a string, array or view (the same as `.count`) | works |
| `byte_at(s, i)` | The byte at index `i` of a string | works |
| `sin` `cos` `tan` `exp` `log` `sqrt` `floor` `abs` | One float argument, same type back | works |
| `pow(a, b)` | Two floats of one type | works |
| `f32_bits(x)` `f64_bits(x)` | A float's bit pattern as `u32` or `u64` | works |
| `f32_from_bits(b)` `f64_from_bits(b)` | The reverse | works |
| `bytes_overlap(a, b)` | Whether two byte views or strings share memory | works |

## Conversions

| Form | Meaning | Status |
| --- | --- | --- |
| `cast(T) x` | Convert between numbers, or between an enum and an integer; traps if the value doesn't fit | works |
| `cast,trunc(T) x` | Convert between integer types, keeping the low bits | works |
| `T(x)` for a number type | The same as `cast(T) x`: `int(x)`, `f64(x)`, `u8(x)` | works |
| `cast(string) v` | Borrow a `[] u8` view as a string | works |
| `xx x` | Automatic integer-width conversion | reserved |

## Literals

| Form | Example | Status |
| --- | --- | --- |
| Integer | `42`, `0xFF`, `0b1010`, `1_000_000` (up to the `u64` maximum) | works |
| Float | `1.5`, `2e-3`, `6.02e23`; with a suffix, `1.5f32`, `1.5f64` | works |
| Character | `'a'`, `'\n'` (one ASCII byte, an integer literal) | works |
| String | `"text\n"`, with the escapes `\n \t \r \0 \\ \"` | works |
| Struct | `Vec2.{x = 1, y = 2}` | works |
| Array | `[3] int.[1, 2, 3]` | works |
| Enum member | `Outcome.completed` | works |
| Uninitialized | `buf: [128] u8 = ---;` | works |
| Inferred struct literal | `.{x = 1}` without the name | designed |

## Types

| Type | Status |
| --- | --- |
| `int` (the same type as `s64`), `s8 s16 s32 u8 u16 u32 u64`, `uint` (the same as `u64`) | works |
| `f32 f64` | works |
| `bool`, `string` | works |
| structs and enums | works |
| `[N] T` fixed arrays, `[] T` views | works |
| `[..] T` dynamic arrays | reserved (E0301) |
| `fixed32 fixed64` | reserved (E0301) |
| `v2 v3 v4 quat m3 m4` | reserved (E0301) |
| `Any` | reserved (E0301); planned for tools and logging only |
| `*T`, `*T?`, `T?`, `Handle(T)`, lane types | designed |
