# Appendix A. Keywords, directives and types

> Status per entry: **works** in `coax0` 0.2.0, or **reserved** (refused
> with "not supported by coax0 yet" until it's built).

## Keywords

| Keyword | Meaning | Status |
| --- | --- | --- |
| `if` `else` | Conditional | works |
| `while` | Loop | works |
| `return` | Return from a procedure | works |
| `true` `false` | `bool` literals | works |
| `for` `break` `continue` `remove` | Loops over ranges and arrays; removing during iteration | reserved |
| `it` `it_index` | The current element and index inside `for` | reserved |
| `ifx` `then` | Conditional expression: `ifx c then a else b` | reserved |
| `switch` `case` | Exhaustive selection | reserved |
| `defer` | Run at scope exit | reserved |
| `try` `or_else` | Propagate or handle an error result | reserved |
| `struct` `union` `enum` `enum_flags` | Type declarations | reserved |
| `using` | Bring an embedded field's members into scope | reserved |
| `cast` `xx` | Explicit conversion; integer-width autocast | reserved |
| `null` | The empty optional pointer | reserved |
| `inline` `no_inline` | Inlining control at definition or call | reserved |
| `push_context` | Change the implicit context for a block | reserved |
| `job` `pure` `strict` `deterministic` | Effects and determinism markers | reserved |
| `shader` | GPU procedure | reserved |
| `constraint` | Declare a generic constraint | reserved |
| `size_of` `type_of` `type_info` | Reflection | reserved |
| `Type` `Code` | Meta types | reserved |

## Directives

| Directive | Meaning | Status |
| --- | --- | --- |
| `#import "Basic"` | Import a module (only `Basic` exists) | works |
| `#run expr` | Evaluate at compile time | works |
| `#assert expr` | Compile-time `bool` check | works |
| `#load` | Add a file to the current module | reserved |
| `#insert` `#expand` `#text` `#location` | Typed code and macros | reserved |
| `#if` | Conditional compilation | reserved |
| `#derive(...)` | Generate operations from fields | reserved |
| `#bake_arguments` | Explicit partial specialization | reserved |
| `#distinct` | A new type with the same representation | reserved |
| `#import_c` `#export_c` `#c_call` `#no_context` | C interop and calling conventions | reserved |
| `#import_proto` | Generate types and codecs from `.proto` | reserved |
| `#scope_file` `#scope_module` `#scope_export` | Visibility | reserved |
| `#as` | Declared conversion to an embedded field | reserved |

## Intrinsics

| Name | Meaning | Status |
| --- | --- | --- |
| `print(format, ...)` | Print with checked `%` slots (needs `#import "Basic"`) | works |
| `type_name(expr)` | The type's name as a string; the operand isn't evaluated | works |

## Types

| Type | Status |
| --- | --- |
| `int` (= `s64`), `bool`, `string` | works |
| `s8 s16 s32 u8 u16 u32 u64 uint` | reserved |
| `f32 f64`, `fixed32 fixed64` | reserved |
| `v2 v3 v4 quat m3 m4`, lane types | reserved |
| `*T`, `*T?`, `[N] T`, `[] T`, `[..] T`, `T?`, `Handle(T)` | designed |
| `Any` | reserved (tools and logging only) |
