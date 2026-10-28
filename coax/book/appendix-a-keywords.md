# Appendix A. Keywords, directives and types

> Status per entry: **works** in `coax0` 0.5.0, or **reserved** (refused
> with "not supported by coax0 yet" until it's built).

## Keywords

| Keyword | Meaning | Status |
| --- | --- | --- |
| `if` `else` | Conditional | works |
| `while` | Loop | works |
| `return` | Return from a procedure | works |
| `true` `false` | `bool` literals | works |
| `for` `break` `continue` | `for i: lo..<hi`, `for x, i: collection`; leave or skip an iteration | works |
| `remove` | Remove the current element during iteration | reserved |
| `it` `it_index` | Implicit element and index names inside `for` (Coax names them instead: `for x, i: ...`) | reserved |
| `ifx` `then` | Conditional expression: `ifx c then a else b` | works |
| `switch` `case` | Exhaustive selection | reserved |
| `defer` | Run at scope exit, in reverse order, on every exit | works |
| `try` `or_else` | Propagate or handle an error result | reserved |
| `struct` `enum` | Type declarations: `Name :: struct { ... }`, `Name :: enum u8 { ... }` | works |
| `union` `enum_flags` | Tagged unions; flag sets | reserved |
| `using` | Bring an embedded field's members into scope | reserved |
| `cast` | `cast(T) x` (checked), `cast,trunc(T) x` (low bits) | works |
| `xx` | Integer-width autocast | reserved |
| `null` | The empty optional pointer | reserved |
| `inline` `no_inline` | Inlining control at definition or call | reserved |
| `push_context` | Change the implicit context for a block | reserved |
| `job` `pure` `strict` `deterministic` | Effects and determinism markers | reserved |
| `shader` | GPU procedure | reserved |
| `constraint` | Declare a generic constraint | reserved |
| `size_of` | `size_of(T)`: the C size in bytes, an untyped integer | works |
| `type_of` `type_info` | Reflection | reserved |
| `Type` `Code` | Meta types | reserved |

## Directives

| Directive | Meaning | Status |
| --- | --- | --- |
| `#import "Basic"` | Import a module (only `Basic` exists) | works |
| `#run expr` | Evaluate at compile time | works |
| `#assert expr` | Compile-time `bool` check at the top level; an always-on check as a statement | works |
| `#test "name" { }` | A check run by `coax test`, never compiled into the program | works |
| `#forall a: T, b: lo..<hi { }` | A check over every combination of its bindings | works |
| `#authority` | Every procedure in the file needs a check; checks run on every build | works |
| `#load` | Add a file to the current module | reserved |
| `#insert` `#expand` `#text` `#location` | Typed code and macros | reserved |
| `#if` | Conditional compilation | reserved |
| `#derive(...)` | Generate operations from fields | reserved |
| `#bake_arguments` | Explicit partial specialization | reserved |
| `#distinct` | A new type with the same representation | reserved |
| `#export_c` | Give a procedure its own C name and the C calling convention (`coax lib`, `coax header`) | works |
| `#import_c` `#c_call` `#no_context` | C header import and calling conventions | reserved |
| `#import_proto` | Generate types and codecs from `.proto` | reserved |
| `#scope_file` `#scope_module` `#scope_export` | Visibility | reserved |
| `#as` | Declared conversion to an embedded field | reserved |

## Intrinsics

| Name | Meaning | Status |
| --- | --- | --- |
| `print(format, ...)` | Print with checked `%` slots (needs `#import "Basic"`) | works |
| `type_name(expr)` | The type's name as a string; the operand isn't evaluated | works |
| `size_of(T)` | The type's size in bytes (the host C layout) | works |

## Literals

| Form | Example | Status |
| --- | --- | --- |
| Integer | `42`, `0xFF`, `0b1010`, `1_000_000` (up to the u64 maximum) | works |
| Float | `1.5`, `2e-3`, `6.02e23` | works |
| Character | `'a'`, `'\n'` (one ASCII byte, an integer literal) | works |
| String | `"text\n"` | works |
| Struct | `Vec2.{x = 1, y = 2}` | works |
| Array | `[3] int.[1, 2, 3]` | works |
| Enum member | `Outcome.completed` | works |

## Types

| Type | Status |
| --- | --- |
| `int` (= `s64`), `s8 s16 s32 u8 u16 u32 u64`, `uint` (= `u64`) | works |
| `f32 f64` | works |
| `bool`, `string` | works |
| structs and enums | works |
| `[N] T` fixed arrays, `[] T` views | works |
| `fixed32 fixed64` | reserved |
| `v2 v3 v4 quat m3 m4`, lane types | reserved |
| `*T`, `*T?`, `[..] T`, `T?`, `Handle(T)` | designed |
| `Any` | reserved (tools and logging only) |
