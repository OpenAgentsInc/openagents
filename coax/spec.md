# Coax: language specification and implementation plan

<!-- coax-release:current:start -->
Current compiler: **coax0 0.25.0**. Status inventory schema: **1**.
<!-- coax-release:current:end -->
Updated: 2026-10-28. Status: **working bootstrap; broader language design**.
The compiler lives in [`coax/`](README.md) at the root of the openagents repo. Source files
end in `.coax`; the executable is `coax`; a future build entry is `build.coax`.

Coax is a systems language for Verse and OpenAgents' real-time code. Its
purpose is to make data representation, compilation, allocation and engine
metadata visible and programmable without obscuring runtime behavior.
This is a Coax design, not a compatibility specification for another language.

**Implementation status matters.** The current bootstrap implements procedures;
sized integers, `f32`/`f64`, `bool` and counted byte strings; untyped
literals and constants; structs with defaults, enums with a declared width,
fixed arrays and views; `for` over half-open ranges and collections, `break`,
`continue`, `defer` and `ifx`; checked casts; left-to-right operand
evaluation; immutable top-level constants, bounded `#run` (including
aggregate results), `#assert` and `type_name(expression)`; checks (`#test`,
`#forall`, `#authority`, `coax test`, `coax mutate`) and always-on `#assert`
statements; math intrinsics; and the C boundary:
`#export_c` with a generated C header, `#foreign` scalar imports, and a
restricted f32/bool Metal library emitter. It generates C11 and uses a host C compiler to
produce executables and object files. Other
syntax below is a target design unless explicitly marked implemented. The
[bootstrap grammar](grammar.ebnf) describes the accepted subset.
Source composition (`#load`, `coax deps`) shipped in 0.11.0. Schema-1 CPU
layout queries (`type_info`, `field_info`, `enum_info`) and compile-time
metadata tables shipped in 0.12.0. Neither completes general modules,
first-class types or arbitrary code generation. The [roadmap](ROADMAP.md)
orders the remaining work and its evidence gates.
The current version and accepted-feature tables are generated from
[`release.coax`](release.coax). `coax status --json` exposes inventory schema 1;
bootstrap, app generation and book publication reject source/snapshot/binary
drift. This display catalogue is separate from protocol schemas and semantic
verification. See [release workflow](RELEASE.md).
The reduced native iOS home now runs scene/HUD logic and scalar shader math in
Coax; native adapters own storage and GPU resources. Fixed-scalar C ABI exports,
foreign imports and a restricted f32/bool Metal library emitter are implemented.
Descriptor imports now generate typed borrowed wire views and bounded codecs
for the supported proto3 subset; see [the importer contract](proto.md). This
does not complete product protocol migration, the broader engine or GPU design.

Contents:

1. Requirements
2. Compiler architecture
3. Design rules
4. Representation and metadata
5. Lexical structure
6. Declarations and types
7. Procedures and specialization
8. Control flow
9. Operators
10. Compile-time execution, reflection and macros
11. Memory, context and errors
12. Jobs and concurrency
13. Determinism
14. Protocol boundaries
15. GPU programs
16. Builds, iteration and hot reload
17. Interop, modules and tooling
18. Engine example
19. Costs and risks
20. Implementation order and exit gates
21. Non-goals
22. Open questions

## 1. Requirements

The [performance plan](../docs/verse/engine.md) provides the engine context.
These are requirements and evaluation criteria, not measured compiler claims.

| Requirement | Concrete test |
| --- | --- |
| Short development loop | Measure edit → check → generation → link → launch/reload, with asset and GPU preparation reported separately. |
| Control of representation | Inspect field offsets, padding, ownership, SoA/AoS layout and generated C. |
| Compile-time programming | Run bounded typed computation; eventually inspect declarations and generate checked code with provenance. |
| Native Apple integration | Preserve the Swift host, C ABI, metal-cpp and native Metal path. |
| Browser delivery | Prove wasm32 memory, browser bindings, async loading and shader integration separately. |
| Deterministic simulation | Replay representative workloads across architectures and worker counts; check exact outputs. |
| Explicit lifetime policies | World, level, thread-scratch and pool storage have visible owners and reset boundaries. |
| Safe parallel scheduling | Validate actual regions and aliases before claiming that effect lists prevent races. |
| Stable interchange | Consume versioned public schemas; retain original signed bytes and existing validation. |
| Inspectable tools | Diagnostics refer to original source; generated output and expansion provenance remain available. |
| Incremental adoption | A C-output subsystem can coexist with C++ without replacing the engine or build system. |

No compiler throughput number or schedule estimate is a release gate by
itself. A smaller implementation that improves a real engine workflow is
more useful than a large unsupported language surface.

## 2. Compiler architecture

### 2.1 Current pipeline

`source → lexer → recursive-descent parser → name/type checker → bounded
AST evaluator → C11 emitter → C compiler/linker → executable`.

The evaluator/C emitter remain in the C++20 host. The lexer, parser and type
checker, including numeric constant folding, layout, coercion and diagnostics,
are Coax compiled to C11. Generated C lexer/parser/checker seeds bootstrap the frontend; normal
builds require exact source/header regeneration and check/mutation gates. It needs no LLVM,
VM package, network access or additional language compiler. `coax check`
stops after checking and compile-time evaluation, without invoking the C
compiler or executing `main`. `coax c` prints the generated C. `coax lib`
and `coax header` produce an object file and a C header for `#export_c`
procedures; `coax layout` reports struct layouts.

A bootstrap compiles from a private copy of its declared compiler inputs and
compares that snapshot with the original inputs before installing artifacts.
A completed bootstrap records the compiler input identity and the hashes of its
binary and checked provenance helper. Site release copies those artifacts to
its clean target worktree and verifies the record against that worktree before
staging. Inputs include compiler sources, checked seeds and Coax modules,
bootstrap and release validation scripts, toolchain identifiers and relevant
build configuration. Unrelated source changes do not invalidate the record.
The record is a local consistency check, not a signature, a hermetic toolchain
attestation or proof of correctness. A hostile writer able to replace both
artifacts and record can forge it; operating-system headers and libraries are
identified only through their environment and toolchain, not content-pinned.

The evaluator uses the checked AST. Bytecode is not required to prove
compile-time semantics; adopt an internal bytecode representation only if
profiling justifies its implementation and debugging costs. The interpreter
belongs to the compiler. Target programs contain native code and small C
support helpers for strings, printing, checked arithmetic, bounds and casts, not that
interpreter. Startup and support code still exist; there is no managed heap.

### 2.2 Planned dependency scheduler

The compiler eventually represents work as declarations and explicit
prerequisites rather than repeated whole-program passes:

1. Load and parse a source unit; allocate stable declaration handles.
2. Collect names, then resolve signatures and constant/type dependencies.
3. Check ready bodies and generic instances; record effects and layouts.
4. Evaluate ready compile-time work within its workspace's permissions.
5. Add generated declarations with their source/expansion provenance.
6. Revisit declarations waiting for those names or layouts.
7. Diagnose unresolved cycles or lack of progress; never spin indefinitely.
8. Freeze the checked workspace, lower it, emit code, and link.

A future `typechecked_available` event means all currently ready work has
been checked; it does not mean the program is complete. Plugins may add
bounded work at this point. `complete` is emitted only after generated
work converges and all required declarations succeed. Generated code must
pass the same parser, checker, effect and layout rules as handwritten code.

The bootstrap resolves global scalar constant dependencies and forward
procedure signatures. It does not implement plugins, generated declarations,
parallel checking, a server or a general fixed-point scheduler yet.

### 2.3 Backends and target separation

C output is the first debug and release path. Version 0.25.0 separates runtime
arithmetic, native optimization, build-profile selection and native debug
information. The legacy defaults are retained: without flags, checked runtime
integers, native `-O0`, development `BUILD_DEBUG` selection and native debug
information; `--release` selects wrapping runtime integers, native `-O2`, release
selection and no default native debug information. Explicit `--arithmetic
checked|wrapping`, `--opt 0|1|2|3`, `--build-profile development|release` and
`--debug-info` / `--no-debug-info` override their axes independently of argument
order. Native optimization is accepted only for `build`, `lib` and `run`.

The checked Coax profile policy admits command/option combinations and resolves
the preset and overrides; the native driver transports arguments and performs
tool/file effects. `--profile-report FILE` requests a separate schema-1 JSON
profile record. It does not change the existing timing, dependency, source-map
or release-inventory schemas. Ordinary generated output remains unchanged
without the report option. Profile identity accompanies selected-profile
evidence and mutation shard compatibility; this is not a semantic cache.

One loaded source graph has one resolved profile. A separately compiled C or
foreign callee executes its own arithmetic policy; the caller does not change
or inherit that policy. Equal C layouts do not imply equal arithmetic semantics.
Native integrations must retain the participating profile records; there is
no claim of general enforcement across independently linked C objects. Existing
GPU source lowerings may select arithmetic without qualifying new devices or
establishing cross-backend numerical equivalence.

A later native debug backend must earn its cost through measured iteration
improvements. An optimizing backend can follow independently; no target is
supported merely because a backend recognizes its CPU name.

Host compile-time values, target layout, browser bindings and GPU code are
separate domains. Layout queries must use an explicit target description,
never the compiler machine's `sizeof`. No compile-time host address, file
handle, allocator state or context pointer may become target initialization.

## 3. Design rules

- **Representation is visible.** No hidden object allocation, mandatory GC,
  implicit deep copy or virtual object hierarchy.
- **Initialization has meaning.** Scalars start at zero; structs use declared
  field defaults. `---` explicitly requests uninitialized storage. Diagnostic
  builds track it where possible; deterministic and compile-time code reject
  an uninitialized read.
- **Errors are values.** Fallible libraries return typed results. Fatal runtime
  arithmetic/bounds errors are traps, not exceptions or recoverable results.
- **Metadata removes duplication.** Registries, inspectors and codecs should
  derive from checked declarations instead of separate handwritten copies.
- **Compilation is programmable and bounded.** Generated code is inspectable,
  attributable to source, and checked before it enters a target program.
- **Authority is external.** A schema, effect annotation or generated API grants
  no network, filesystem, host or payment authority by itself.
- **One form for ordinary work.** A future formatter chooses a canonical form;
  it is not implemented yet. Prefer named fields and explicit conversions.
- **Do not promise general memory safety.** Pointers, shallow views, foreign
  interfaces and mutable shared storage require ownership and alias discipline.

## 4. Representation and metadata

### 4.1 Storage models

AoS, SoA and blocked SoA are explicit storage choices. A future `SOA(4)`
struct transformation produces accessors, layouts and reflection metadata;
it cannot change the logical meaning of copies or expose a fictitious pointer
to an interleaved object. Measure conversion and gather/scatter costs.

Copies of structs copy their fields. Copies of views and strings copy a
pointer/count descriptor, not backing storage. Dynamic-array growth can
invalidate borrowed views. Resetting an arena invalidates its borrows even
when the bytes still appear readable. Use generation-checked handles for
persistent references into pools and worlds.

### 4.2 Derived operations

`#derive(eq, hash, order, print, proto)` is planned library code over typed
metadata, not a bypass around checking. A custom equality must have a
compatible hash before the type can be a table key. Ordering and iteration
policies must be explicit; an insertion-ordered table differs from a hash-order
table. Neither order should silently determine a persistent identity.

### 4.3 Four different layouts

Keep native CPU layout, C ABI layout, GPU resource layout and protocol encoding
separate. A future layout report includes target, field offsets, alignment,
padding and layout fingerprint. Cross-target GPU declarations must either
satisfy one explicit restricted layout or emit separate checked adapters;
there is no universal padding assumption.

Reflection can describe that a field is a pointer. It cannot infer whether to
persist an address, a handle, an artifact reference or nothing. Those meanings
need explicit schema annotations and reviewed adapters.

## 5. Lexical structure

- Source is UTF-8; identifiers use ASCII letters, digits and `_`, beginning
  with a letter or `_`. Keywords cannot be identifiers.
- `//` comments, nested `/* */` comments and whitespace separate tokens.
  `///` documentation attachment is planned.
- Directives start with `#`; annotations start with `@`.
- Integer literals support decimal, hexadecimal, binary and `_` separators,
  up to the u64 maximum. Decimal floating literals have a fraction (`1.5`,
  never `1.`, so `0..<n` stays a range) and/or an exponent (`2e-3`).
  Character literals `'a'` and `'\n'` are integer literals holding one ASCII
  byte. Integer, float and character literals are untyped until used: a use
  gives them its type if the value fits (E0350 otherwise), and with no type
  to take, integers become `int` and floats `f64`.
- Strings support `\n`, `\t`, `\r`, `\0`, `\\` and `\"`. They count bytes,
  may contain NUL, and need not be zero-terminated. UTF-8 is the text
  convention, not a promise that arbitrary byte strings are valid text.
- Semicolons terminate ordinary declarations and statements.
- Unicode escapes, raw strings and non-ASCII character literals are
  planned. Reserved words for later features are diagnosed rather than
  accepted with guessed semantics.

The grammar distinguishes a procedure's parameter list followed by a body
from a parenthesized constant expression. `COUNT :: (40 + 2);` is a constant.

## 6. Declarations and types

### 6.1 Implemented declarations

```jai
COUNT :: 64;
LABEL :: "visible";
square :: (n: int) -> int { return n * n; }

main :: () {
  count := COUNT;
  ready: bool = false;
  scratch: int = ---;
  scratch = square(count);
}
```

Top-level scalar constants use `::`. They may reference later constants and
procedure signatures. Cycles are errors. Constants cannot be assigned or
shadowed. Ordinary calls in constant initializers require `#run`; arithmetic
on constants does not. Local constants and procedure values are not supported.

`int` and `s64` denote the same signed 64-bit type and `uint` is `u64`.
Implemented scalars are `s8 s16 s32 s64 u8 u16 u32 u64`, `f32 f64`, `bool` and
`string`; a procedure without a return annotation returns no value. There are
no implicit numeric or boolean conversions. A constant made only of literals
stays untyped, like a literal, and must fit `int` (or `f64`); `cast(T)` makes
a typed constant.

Implemented aggregates are structs with field defaults (`v: Vec2;` takes
them; `= ---` skips them), enums with a declared width, fixed arrays `[N] T`
(N known while checking) and views `[] T`. Arrays and structs are values:
assignment and calls copy them. A view borrows an array variable's storage
(`a[..]`, `a[lo..<hi]`); writes through it change the array, including through
a view parameter. A view does not extend the array's lifetime. Strings index
to `u8` and slice to strings, and are immutable; `cast(string) bytes[..]`
borrows a `[] u8` as text. Indices and slices are checked in every build.

The checker rejects a returned view or string when local provenance identifies
backing storage in a local fixed array (E0383). This includes inline array
fields/elements, direct local aliases and assignments, subranges, casts and
either `ifx` branch. A local alias keeps its may-borrow flag after reassignment,
so even an overwritten or unreachable local borrow can be refused. Returning
a borrowed parameter, or copying a fixed array value out, remains valid.

This bounded diagnostic is not a general lifetime analysis. It does not track
borrows stored inside aggregates, returned through calls, retained by callees,
or escaping into caller storage, and does not compute loop fixed points or
enforce shorter nested-block lifetimes. Those cases still require explicit
owner/lifetime discipline. Assignment, passing and returning a view copy only
its descriptor; the caller must keep the backing storage valid for every use.
This slice adds no owning value transfer or destruction semantics.

Implemented scalar extensions in 0.3.0 are specified in the
[compiler reference](README.md#native-libraries-and-metal): explicit casts,
finite compile-time math, native runtime floating arithmetic, `#foreign`
imports and the Metal scalar profile.

### 6.2 Planned scalar and aggregate types

| Group | Target vocabulary and rule |
| --- | --- |
| Integers | **Implemented.** `s8 s16 s32 s64 u8 u16 u32 u64`; `uint` is u64, not a C integer alias. |
| Floating point | **Implemented** as IEEE with no fused operations; explicit strict-mode library behavior is planned. |
| Fixed point | `fixed32` and `fixed64`, after scale, overflow and rounding rules have vectors. |
| Small math/SIMD | `v2 v3 v4 quat m3 m4`, integer vectors and explicit lane types. |
| Pointers | `*T` non-null, `*T?` nullable; address-of `*x`, dereference `p.*`. |
| Inline arrays | **Implemented.** `[N] T`: storage belongs to the containing value. |
| Views | **Implemented.** `[] T`: pointer/count borrow; no ownership transfer or lifetime extension. |
| Dynamic arrays | `[..] T`: data/count/capacity plus allocator policy; explicit growth/free. |
| Optional | `T?`: test or unwrap before access. |
| Distinct | `RoomIndex :: #distinct int;`: no implicit mixing with int. |
| Handles | `Handle(T)`: explicit index/generation with stale-reference checks. |
| Meta types | `Type`, `Code`, constrained polymorphic parameters; not implemented. |
| Dynamic values | `Any`: borrowed type/value descriptor for tools and logging; no implicit owned box. |

Array bounds checks must have an explicit build-mode policy. Releasing a
container does not free separately owned allocations unless its explicit
`destroy` operation says so.

### 6.3 Structs, enums and unions

```jai
Room :: struct #derive(eq, hash) {
  name: string;
  tunnels: [..] RoomIndex;
  selected: RoomIndex = -1;
}
Outcome :: enum u8 { completed; refused; failed; cancelled; unknown; }
Answer :: union { text: string; count: int; }
```

This is target syntax; `#derive`, `[..]`, `#distinct` and unions are not
implemented. Implemented: `Name :: struct { field: T; field: T = default; }`,
`Name.{field = value}` literals that name each field they set, and
`Name :: enum u8 { a; b = 4; }` with `Name.a`, `==`/`!=`, printing by name,
and `cast` to and from integers that traps on a value that names no member.
Enum values default to the previous value plus one and must be distinct.
Struct fields follow declaration order and natural
alignment unless an explicit layout mode is chosen. `using` exposes an
embedded field's members; it does not establish virtual inheritance. `#as`
conversion relationships must be separately declared and checked. Copies
remain shallow.

Unions are tagged by default and require exhaustive handling. Raw overlapping
storage, if introduced, needs an explicit unsafe form and manual discriminator
rules. Enums specify an underlying width; flags use a separate `enum_flags`
form. Persistent enum values and field identities must be assigned explicitly,
not inferred from declaration order.

## 7. Procedures and specialization

### 7.1 Current rules

Procedures have named, typed scalar parameters and zero or one return value.
Parameters are immutable; copy one into a local to update it. Procedures can
refer to later procedures. Calls check arity, types and whether results are
used. A program has `main :: ()` with no parameters or return value.

Struct, array and view parameters are passed by value; a view parameter's
elements may be written, since they belong to the caller's array.
`name :: (...) -> T #export_c { }` gives a procedure its own C name and the C
calling convention for `coax lib` and `coax header`.

Multiple returns, named/default arguments, overloads, variadics, callbacks and
explicit pointer mutation are planned. Passing a large value by an optimized
physical convention must not silently make it a mutable reference.

### 7.2 Planned generics

`$T` binds a type; `$N` binds a compile-time value. A constraint such as
`Hashable` describes the operations a specialization needs. Constraints are
checked at instantiation with a diagnostic that names the call and missing
operation. They do not imply a runtime vtable or heap object.

Specialization keys include resolved types, constant arguments, target and
relevant effects/options. Cache repeated instances and report instance counts,
code size and generation time. Put limits on recursive specialization and
produce a source-level instantiation trace on failure.

`#bake_arguments draw(limit = 64)` is a proposed explicit partial
specialization: the baked arguments become constants, and the remaining
parameters stay ordinary parameters. It must use the same specialization
cache and budget as inferred generics. No silent constant specialization of
every call is permitted; that would hide code-size and compilation costs.

A future semantic customization hook may calculate a result type from
resolved argument types, for example combining physical dimensions. It must
be a pure, bounded type computation with an explicit input/output contract,
not permission to mutate arbitrary compiler state. Establish ordinary generic
constraints and diagnostics before deciding whether a separate `#modify`
surface is necessary. None of these facilities exists in the bootstrap.

### 7.3 Evaluation order

Operands and call arguments are evaluated left to right, with
short-circuit boolean operators and `ifx` evaluating only what they select.
This is implemented at compile time and at run time. The C emitter stores
operands in temporaries, in source order, whenever an operand list contains a
call and another operand that could call or trap, so C's unspecified argument
order never shows. An assignment evaluates its place, then its value; a
compound assignment evaluates its place once.

## 8. Control flow

Implemented: blocks, `if`/`else if`/`else`, `while`, `return`, assignments
to variables, fields and elements, compound assignments, expression
statements for void calls, `for i: lo..<hi`, `for x: collection` and
`for x, i: collection` over arrays, views and strings, `break`, `continue`,
`defer`, and `ifx condition then a else b`. Non-void procedures must return
on all statically covered paths. Loop variables are immutable copies; an
array is iterated in place, so writes by the body are seen by later
iterations.

Planned: exhaustive `switch`, `try` and `or_else`.

Use half-open `0..<n` for the first range implementation. An empty range has
zero iterations. Reverse iteration and removal need explicit invalidation
rules before acceptance. Do not add an implicit iterator allocation.

`defer` executes cleanup at lexical-scope exit in reverse registration
order, including `return`, `break` and `continue` (implemented) and propagated
errors (with `try`, planned). A returned value is computed before deferred
code runs. Deferred code cannot return, break or continue out of its block
(E0361). It is not an automatic type destructor.

## 9. Operators

The bootstrap implements `+ - * /` on integers and floats, `%` on integers,
unary `-` on signed integers and floats, bitwise `& | ^` and shifts `<< >>`
on integers, comparisons, boolean `&& || !`, and equality on scalars, enums
and strings. Bitwise operators share the precedence of `+` and `*`, as in Go.
Comparisons cannot chain. Checked runtime arithmetic traps on overflow (signed
and unsigned); wrapping arithmetic wraps `+`, `-`, `*` and signed negation.
The defaults are checked without flags and wrapping under the legacy
`--release` preset. `--arithmetic checked|wrapping` overrides that choice without
changing native optimization or `BUILD_DEBUG`. Division by zero always traps.
The minimum integer divided by minus one traps; its remainder is zero.
Shifts by a negative amount or by the type's width or more trap in every
build; `<<` discards high bits. Float arithmetic is IEEE binary32/binary64
with one rounding per operation; the generated C disables contraction into
fused multiply-add.

`cast(T) x` converts numbers and enums and traps in every build if the value
does not fit; float to integer truncates toward zero and traps on NaN or out
of range. `cast,trunc(T) x` converts between integer types keeping the low
bits.

Ordinary compile-time arithmetic is checked in every build mode (§10). This avoids
release settings silently changing generated constants.

`add_wrap(a, b)`, `sub_wrap(a, b)` and `mul_wrap(a, b)` accept two
matching `u32` or `u64` operands and return their sum, difference or product
modulo 2^32 or 2^64. They wrap in checked builds, `--release` and compile-time
execution; ordinary operators retain their existing overflow rules.
`rotl(a, count)` and `rotr(a, count)` use the same matching unsigned operand
width and reduce the count modulo that width. A zero or full-width count
returns `a`; there is no shift-by-width undefined behavior. Operands execute
once, left to right. These CPU intrinsics lower to C11 and wasm32; the Metal
scalar profile continues to reject integer/storage intrinsics.

`mul_wide(a, b)` takes two `u64` operands and returns the compiler-owned
`CoaxU64Product` record (`hi:u64`, then `lo:u64`). It represents the exact
128-bit unsigned product as two64-bit halves, in both checked and wrapping
profiles and under `#run`. This does not add general128-bit values or tuple
return syntax. Literals are admitted with the usual u64 range checks;
other operand widths are refused. Arguments execute once, left to right.
The named result schema is available when the type or intrinsic is used;
conflicting source definitions are refused. Its C ABI is a16-byte record of
two u64 fields. Native targets with128-bit C arithmetic use that operation;
wasm32 uses four32-bit partial products and explicit carries. Neither path
allocates, and unused programs gain no helper or result declaration. Metal
continues to refuse integer/storage intrinsics. This local record layout is
not a wire format; interchange still uses explicitly versioned schemas.


`value_barrier(x)` takes a `u32` or a `u64` and returns it unchanged, but the
optimizer cannot see through it: it cannot learn that the result is 0 or all
ones, so it cannot turn a mask built from a secret back into a branch.
Constant-time code builds its masks through it, for example
`mask := value_barrier(sub_wrap(cast(u64) 0, bit)); r := b ^ ((a ^ b) & mask);`
for a `u64` bit. Native C emits
an empty `__asm__("" : "+r"(x))`; wasm32 has no such asm and calls an
identity that is never inlined, through a volatile. At compile time it is the
identity. This obstructs specific optimizer transformations; it does not prove
whole-program constant time or secure erasure. `tests/constant_time.py` records
masked-select native/Wasm evidence. `tests/wrapping.py` checks defined wrap/rotate
semantics against an independent integer oracle, including actual Wasm execution
and native sanitizers.

The optional [scalar secret-path audit](../tools/coax-nostr/secret-check/README.md)
uses the actual Coax frontend and typed source-map closure. Its Coax authority
policy classifies all entry parameters as secret, tracks scalar origins through
direct callees and refuses secret control/index operations, public-only
`_vartime` calls and unsupported syntax. It admits a bounded unsigned scalar
subset, including selected masked operations; arrays, aggregate access and all
loops remain unsupported. Source/compiler/tool hashes bind each result to the
analyzed snapshot. This is source policy only: checked arithmetic guards,
optimizer/object/JIT behavior, erasure and production cryptographic admission
remain separate gates. There is no `#secret` language annotation.

Planned overloaded operators must be pure, nonallocating and nonfallible.
Assignment, field lookup, short-circuit booleans and procedure calls are not
overloadable. Indexing, arithmetic and comparison require specific constraints.
An overload cannot hide an I/O operation or an allocation behind math syntax.

## 10. Compile-time execution, reflection and macros

### 10.1 Implemented `#run` and `#assert`

```jai
square :: (n: int) -> int { return n * n; }
TABLE_SIZE :: #run square(16);
#assert TABLE_SIZE == 256;
#assert type_name(TABLE_SIZE) == "int";
main :: () { count := TABLE_SIZE; }
```

`#run` evaluates its following unary expression; parenthesize a larger
expression, for example `#run (square(16) + 2)`. Results are scalars, strings,
enums, structs or arrays: scalars become literals in the generated C and
aggregates become static constants. A result cannot contain a view (E0409),
since it would point into memory that exists only while compiling. `#run` requires a value;
top-level void execution is not supported. `#assert` is a top-level bool
expression that must evaluate true before C emission. Use `#run` for calls
inside an assertion as well.

Evaluation uses ordinary checked procedures, branches, loops, recursive calls,
mutable locals and immutable parameters. An executed attempt to print is a
compile error. There is no compile-time file, process, network, clock,
randomness, foreign call or allocator API in this subset. The compiler driver
still reads input and invokes the selected C toolchain; this restriction is
about interpreted program code, not process sandboxing.

A `#run` expression has an isolated frame. It can read global constants and
call procedures with compile-time arguments. It cannot capture a surrounding
runtime local or parameter, even when its containing procedure is itself
called by another compile-time expression. All `#run` sites in checked
procedure bodies must succeed, including sites in runtime branches.

Limits for the bootstrap:

| Resource | Limit and diagnostic |
| --- | --- |
| Evaluation work | 100,000 steps by default across one compilation; `--run-steps N` selects 1 through 250,000,000. Each evaluated expression or statement and each zero-initialized array element counts; E0400. |
| Aggregate storage | At most 1,572,864 live aggregate-vector capacity slots and 131,072 live aggregate containers, independently of the work limit and in every profile; E0400. |
| Aggregate traversal | At most 134,217,728 visited values/fields per frontend evaluation or individual check, independent of the step budget; E0400. |
| Cast-string snapshots | At most 67,108,864 live owned bytes per compilation and 268,435,456 materialized bytes per frontend evaluation or individual check; E0400. Each cast charges its output length plus one terminator. |
| Active interpreted calls | 64, E0403; call sites are printed as bounded source notes. |
| Active constant dependencies | 64, E0403; type-resolution dependencies also stop at 64, E0347. |
| Constant cycles | E0340 for type dependencies; E0402 for a value cycle reached through a procedure. |
| Runtime capture | E0401. |
| Executed output | E0404. |
| Uninitialized read | E0405. |
| False assertion | E0406. |
| Overflow or division by zero | E0407, in debug and release. |
| Call missing `#run` | E0408. |
| View in a result | E0409. |
| Index or slice out of bounds | E0410. |
| Cast, enum value or shift out of range | E0411. |

The optional decimal `--run-steps N` applies to all compile commands, including
`test` and `mutate`; it changes frontend evaluation only. It does not change
`coax test --steps`, mutation budgets, native optimization or arithmetic.
Zero, signed, malformed, repeated or above-maximum inputs are refused before
source evaluation. `coax deps` records the selected limit and fixed storage
limits, including traversal and string limits; `--timings` records frontend
evaluation steps, aggregate traversal work and tracked storage peaks before
authority checks begin.

The independent storage limit counts actual allocated vector capacity, including
nested copies and retained constants, and releases it with the last owner or
view. The evaluator also clears view cycles when the compilation ends; an
unreachable cycle may continue consuming budget until then. Interpreted
byte-view-to-string casts own immutable snapshots. Copies, slices, aggregate
fields and returned values share that owner; folded strings remain owned until
compilation ends. Live byte accounting includes the terminator once per owner,
and the work limit charges each successful materialization even after its owner
is released. Source literals and other strings, metadata, syntax trees, host
I/O, and generated output are not covered. An aggregate cycle holding a string
may retain it until compilation ends; these limits do not bound total RSS.

Aggregate cloning, assignment, view detection, field default and struct or
array literal initialization charge a separate traversal meter before each
visited value or field. This includes recursive copies and work done before a
failed operation; the original expression/statement step counts are unchanged.
The meter resets per frontend evaluation and per individual check, including a
check's quantified cases. Cleanup does not spend traversal work: aggregate
live-slot/container limits and the bounded weak-storage registry bound a single
cleanup's retained graph, but do not bound all host cleanup or total RSS.
String comparisons and AST processing are also outside this meter.
The work budget counts interpreted work, not parsing, every compiler allocation
or total wall time. It is not a complete hostile-source resource sandbox.
Memoized constants and folded `#run` sites are not re-executed. Host pointers
and runtime mutable state cannot be retained because this subset cannot
construct them. Only checked values without views cross into target
initialization.

### 10.1.1 Implemented checks

Checks are written before the code they check, and run in the same evaluator
as `#run`. They are never part of the generated program.

```jai
#authority;
refund_ok :: (paid: int, refund: int) -> bool {
  return refund >= 0 && refund <= paid;
}
#forall paid: 0..<40, refund: -2..<45 {
  #assert refund_ok(paid, refund) == (refund >= 0 && refund <= paid);
}
#test "refund of everything" { #assert refund_ok(7, 7); }
```

- `#assert condition;` is also a statement. In a check it fails the check; in
  generated code it traps with `assertion failed` in debug **and** release.
  No build mode removes it.
- `#test "name" { ... }` is a block of ordinary statements. `#forall [name]
  a: T, b: lo..<hi { ... }` runs its block once for every combination of its
  bindings, the last binding changing fastest. A binding is `bool`, an enum,
  an integer of at most 16 bits, or a half-open `int` range with constant
  bounds (E0379); at most 2^24 cases. Domains must be nonempty and ranges
  increasing. Range spans use widened arithmetic, and domains are represented
  lazily so a large range cannot allocate millions of values before its budget. A failure reports the first
  counterexample as values.
- `coax test FILE [--steps N]` runs every check, each with its own budget of
  20,000,000 steps unless `--steps` says otherwise. The same rules hold as in
  `#run`: no output, no host calls, overflow and bounds checked.
- `#authority;` marks a file whose rules guard money, signatures or grants.
  Every procedure authored in that physical source unit except `main` and `#foreign` imports must actually execute
  in a check, directly or through calls (E0380). Dead references, short-circuited
  calls and pre-folded `#run` results do not count. Every compile of the
  file runs its checks: a failing check is a failed build.
- `coax mutate FILE` changes the code reached from checks one site at a time:
  comparison boundaries (`<` and `<=`), equality, arithmetic, logical and
  bitwise operators, `+=`/`-=`, integer literals (n to n+1), bool literals,
  negated `if`/`while` conditions, and removed assignments and calls. Each
  mutant must fail some check, under a budget of ten times the steps the
  unmutated check used. Survivors are printed with their source line; it
  exits 1 if any survive, a procedure is unchecked, or there are no reachable
  mutation sites. A reviewer may mark a line `// mutant: equivalent`
  when no input can observe the change; surviving mutants there are counted
  apart. Exemptions require human justification; they are not solver results.

Mutation starts by running every check and recording which procedures it actually
entered. For a graph whose checks can be isolated, a mutant runs the checks
that entered its owning procedure; all statically reachable mutation sites
are still generated, including those no baseline check entered. Such a mutant
survives unless another rule rejects the run. The per-check mutant budget is
the greater of 10,000 steps and ten times that check's baseline steps.

The compiler falls back to running every check per mutant if aggregate types
can contain views, a string cast or unknown graph element may allocate
long-lived storage, or baseline tracked storage changes. This relies on current
constants, defaults and `#run` expressions being folded before mutation; new
lazy evaluation or shared mutable state requires revisiting isolation. Host
allocation failures and external resource pressure are not modeled as a
deterministic equivalence guarantee. `--mutation-reference` requests the full
runner; `--mutation-stats` and `--mutation-trace` expose scheduling counts
and each mutant's first killing check for comparison.

These gates do not prove a program correct. Execution coverage is not branch
coverage or a connection between an output and its assertion. Quantification
covers only the stated finite instance, and mutation covers the implemented
operators, not every possible defect. `coax test` rejects files with no checks;
`--steps` requires a positive uint64 value. JSON failure messages retain the
check name and concrete counterexample. The compiler requires checks to exist,
not a particular textual or historical order of writing them.

### 10.2 Reflection now and later

`type_name(expression)` is implemented for every type. Its operand
is typechecked but unevaluated. It returns a string constant without evaluating
side effects or arithmetic in the operand; `type_name(1 / 0)` is `"int"`.
Unknown names and ill-typed operands remain errors. It needs no module import.

`size_of(T)` and, since 0.12.0, `type_info(T)`, `field_info(T, index)` and
`enum_info(T, index)` describe the bootstrap's **LP64 CPU target layout**.
They take type syntax, not a first-class type value. They require no import.
The parser supplies ordinary record definitions when a query is present:

| Record (schema 1) | Fields |
| --- | --- |
| `CoaxTypeInfo` | `schema_version:u32`, `name:string`, `kind:string`, `size:int`, `alignment:int`, `bits:int`, `is_signed:bool`, `field_count:int`, `member_count:int`, `element_count:int`, `element_name:string` |
| `CoaxFieldInfo` | `schema_version:u32`, `index:int`, `name:string`, `type_name:string`, `offset:int`, `size:int`, `alignment:int` |
| `CoaxEnumInfo` | `schema_version:u32`, `index:int`, `name:string`, `value_bits:u64`, `bits:int`, `is_signed:bool` |

Names use resolved checker spellings (`s64` is `int`, `uint` is `u64`). Kinds
for value types are `integer`, `float`, `bool`, `string`, `struct`, `enum`,
`array` and `view`. Fields/members follow declaration order, with zero-based
indices. Counts are zero outside their category; `element_name` is empty
except on arrays/views. A view's `element_count` is zero: its runtime length
is not part of its type. `bits` and `is_signed` are the checker's scalar or
enum attributes (including the sign bit on floats); aggregates use 0/false.
Empty structs retain the bootstrap's one-byte representation.

`field_info` requires a nonempty struct and `enum_info` a nonempty enum
(E0382). Indices are integers. Constant out-of-range indices fail with E0374;
dynamic indices retain bounds traps in debug **and release**. `value_bits`
is the underlying-width enum bit pattern, zero-extended to u64: s8 -2 is
254, not 18446744073709551614. Use `bits`/`is_signed` to interpret it.

Queries lower in the Coax checker to typed struct/array literals and ordinary
indexing; the C++ bridge only transports them. Direct scalar projections such
as `type_info(T).size` and `field_info(T, 1).offset` are known while checking,
so they can size arrays and participate in constants. Metadata is also an
ordinary value: pass it, retain it or return tables from `#run`. A dynamic
selector materializes a finite metadata array; generate a table once with
`#run` instead of rebuilding it per frame. A query-bearing source graph
requests 65,536 extra checker nodes. Expansion shares the bounded checker
pool and refuses exhaustion with E0381; it does not allocate without limit.

The record names and query names are compiler-owned when reflection is used;
user definitions cannot replace them. `type_info` is a keyword; `field_info`
and `enum_info` are intrinsic names. The metadata describes the current CPU
layout only, not wire serialization, a Metal vector layout or a cross-target
ABI. Generated C/header layout assertions still validate the actual native
compiler. No query silently substitutes the compiler host's layout for a
future GPU or WebAssembly target.

Still planned: first-class `type_of`/`Type`, explicit alternate-target
queries, stable declaration IDs, defaults, annotations, procedure/effect
metadata, `#derive`, hygienic typed insertion and plugins. These APIs supply
layout data, not arbitrary syntax generation or compiler callbacks.

A compiler's temporary type ID is not a persistent component ID. Registered
engine component identities and protocol names remain explicit and stable
through a rebuild. A future `Any` borrows its value; neither reflection nor a
boxed descriptor extends storage lifetime.

### 10.3 Planned typed code and macros

`Code` represents syntax with a phase and expected category: expression,
statement, declaration or type. Quoted code records its original source span
and lexical bindings. `#insert` splices the expected category and checks the
result. `#expand` defines a call-site macro. Ordinary execution, quotation,
expansion and later target execution are distinct operations.

Expansion-local names are hygienic. Access to caller bindings requires named
captures with read/write permission. A macro cannot acquire a caller's locals
through text matching or mutate unrelated declarations. Arguments are not
silently evaluated twice; the expansion must state duplication explicitly or
bind a temporary.

Maintain an expansion tree: source invocation → generator → generated node →
checked declaration → emitted range. Expose expanded text/typed nodes without
executing target code. Primary diagnostics point at the invocation or original
declaration, with notes into generated code.

Budgets use deterministic work and node counts. Initial proposed defaults are
64 nested expansions and 100,000 new nodes per workspace; validate these with
real generators before freezing them. Wall-time profiling can warn, but a
machine-dependent one-millisecond threshold must not change program validity.

### 10.4 Planned compiler plugins

A plugin gets typed events and scoped declaration handles, not mutable raw
compiler pointers. Events include file-loaded, parsed, signature-ready,
typechecked, typechecked-available, before-lowering and complete. Workspaces
isolate build declarations from target declarations.

Plugins can propose source, dependencies and diagnostics through checked APIs.
They cannot mutate a frozen target or mark an unchecked declaration valid.
Adding a declaration schedules its prerequisites; unresolved dependency cycles
produce a finite explanation. Plugin identity, version, options and recorded
inputs enter cache keys. A failed plugin invalidates the workspace output.

This API, `Code`, macros and generated declaration scheduling are not
implemented by the scalar evaluator.

### 10.5 Build-profile conditionals; general conditions planned

Version 0.17.0 supports the fixed workspace selector `#if BUILD_DEBUG { ... }`
with an optional `else { ... }` inside struct field lists and procedure/check
statement blocks. `BUILD_DEBUG` selects development by default and release under
the legacy `--release` preset. Version 0.25.0 adds the independent explicit
`--build-profile development|release` override; arithmetic and optimization
flags do not change this selector. It is context-specific, not ordinary name resolution;
user constants cannot configure it. Selected fields join the enclosing struct
in source order. Selected statement blocks keep ordinary lexical scope.

Both branches parse under the same syntax/node/nesting budgets. Only the selected
syntax-tree nodes participate in name resolution, field layout/defaults,
reflection, #run, authority reachability and emission. Inactive nodes are cleared
before helper discovery, so inactive reflection or wide arithmetic cannot inject
helper declarations. The native host passes the explicit build-mode input; the
selection policy lives in the checked Coax parser, not a source-text preprocessor.

This changes struct layout. Every producer/consumer of such a native record must
use the same profile and generated header. Keep profile-dependent fields out of
published wire/native contracts unless the contract explicitly versions them.
The legacy `--release` preset retains its integer-wrapping default unless an
explicit arithmetic override is supplied. Build-profile selection itself does
not change overflow policy. Native optimization, NDEBUG, Xcode's configuration
and Coax build profile are separate inputs. Current app generation keeps checked
Coax arithmetic; it does not silently adopt release arithmetic.

Arbitrary `#if CONSTANT`, negated/compound selectors, conditional imports and
top-level declarations remain unsupported. General conditions will need the
declaration dependency scheduler: a condition cannot depend on a declaration
inside its own selected branch, and cycles must be diagnosed. Target/module
configuration must enter future cache keys. The existing runtime `if` still
checks both bodies and its #run sites; ordinary #assert still runs in release.

## 11. Memory, context and errors

### 11.1 Planned explicit context

Ordinary procedures can receive an implicit context holding allocator,
scratch arena and logging services. `push_context` derives a block-local
context without mutating an unrelated caller's policy. `#no_context` leaf
procedures and C-callable procedures omit it. Dependencies such as allocation
and host I/O appear in transitive effect summaries even when context passing
is implicit.

A foreign allocator is not redirected by changing Coax context. Context layout
is private to its versioned runtime; independently built modules use explicit
C ABI entry points and opaque handles. The current bootstrap has no
implicit context convention yet.

### 11.2 Lifetimes and allocation

| Storage | Owner and invalidation |
| --- | --- |
| Persistent world | Resident host; versioned handles survive code replacement. |
| Level arena | Level owner; every borrow ends before arena reset. |
| Thread scratch | Worker/thread; cannot escape into another worker or persistent state. |
| Reusable pool | Pool owner; generation changes on recycling. |
| Dynamic array | Container owner; growth can move storage and invalidate views. |
| Foreign buffer | Explicit acquire/release contract with the foreign owner. |

Allocation operations, deep copies and releases are explicit. `New`,
`NewArray`, `array_add`, `free` and `destroy` are planned library operations.
`destroy` only releases what its contract explicitly owns. Debug poisoning
and allocation provenance help find stale views; they cannot prove that every
raw pointer is safe. No automatic recursive destruction is implied.

An allocation effect is checked transitively before a job, shader or strict
no-allocation section accepts a call. Low-level FFI needs a declared effect
summary and manual review. Scratch lifetimes cannot be enforced solely by an
allocator's existence.

### 11.3 Errors and resource cleanup

Fallible APIs return a value plus a typed optional error or a tagged result.
Ignored errors are compile errors. `try` propagates an error and runs active
`defer` cleanup; `or_else` handles it explicitly. This syntax is planned.

Foreign errors map through explicit adapters. An HTTP status or protocol
refusal keeps its original domain and code; it is not collapsed to an
unqualified integer. Fatal arithmetic, invalid handles and failed invariants
must stop the affected operation; an assertion handler cannot safely continue
past arbitrary memory corruption.

## 12. Jobs and concurrency

A planned job declares reads/writes of memory regions and subfields. The
scheduler allows overlap only when writes do not conflict with other reads
or writes. SoA field separation must correspond to actual disjoint storage.
A pointer hidden in a view can alias another region; field names alone are
not a race-freedom proof.

Require an alias/region discipline, disjoint range validation, and effect
summaries for callees before enabling parallel execution. Foreign calls,
atomics and shared allocators need declared behavior. Ordinary raw pointers
remain manually managed; there is no general borrow checker.

Thread contexts and scratch arenas are separate. Shared general-purpose
allocator state is still synchronized. A future atomic API must specify
ordering for each operation and target; using native threads does not define
that memory model automatically.

Deterministic reductions combine in a specified index/tree order. Test the
same replay with 1, 2, 4 and 8 workers. No job syntax, scheduler integration,
atomic API or static race checker exists in the bootstrap yet.

## 13. Determinism

A future `strict` floating mode forbids contraction, reassociation and
approximate reciprocal/square-root substitutions. It pins rounding and math
implementations with exact vectors. The engine's existing fdlibm behavior and
`-ffp-contract=off` invariants remain authoritative. Do not substitute a claim
of correctly rounded transcendental math for those tested algorithms.

A deterministic function cannot read clocks, uninitialized memory, ambient
randomness or hash-order iteration, nor call a function with incompatible
effects. Random generators are explicit values. Fixed-point arithmetic still
needs explicit scale, rounding and overflow rules; it does not automatically
reproduce a different algorithm's floating-point outputs.

Cross-platform proof includes serializations, replay order, compiler flags,
SIMD lowering and scheduling. The bootstrap evaluator computes integers, floats,
bools, strings and aggregates; its float arithmetic matches generated C on the
same host (one rounding per operation, no fused operations), and math
intrinsics use the host libm. There is no cross-platform floating-point claim
yet.

## 14. Protocol boundaries

The [OpenAgents Protocol](../docs/protocol/openagents-protocol-spec.md) is the
defining interchange specification for every OpenAgents surface, including
Verse. Root [proto/](../proto/README.md) is its schema authoring source.
**Coax is the implementation target for all those surfaces. Verse is the
engine and the world/experience it runs; its worlds are realms.**

Shared engine contracts use `openagents.engine.*`; Verse world/product types
use `openagents.verse.*`; other domains use their owning `openagents.*`
packages. Vendor schemas retain upstream namespaces and compile in separate
modules. Consumer profiles select roots plus actual imports rather than
forking definitions per language or device.

The recovered nine published schemas, Nanopb limits and nine immutable
snapshots preserve their pre-reset identities. New alpha options and Verse
world/presence projections are experimental inputs. Current descriptors are
in `proto/descriptors/`, generated and tested by `scripts/proto.sh`; the
Coax compiler now ingests selected proto3 closures through `#import_proto`.
Version 0.10.0 implements descriptor parsing, validation and source generation in
`lib/proto/importer.coax`. Its `#authority` checks and mutation gate execute on
the actual implementation. A proto-disabled C++ stage0 compiles it to C11,
then the final compiler links it and must regenerate identical C. Coax defines
its bounded scratch capacities. `lib/proto/fingerprint.coax` also implements
SHA-256, runtime extraction and the provenance/source envelope, with authority
checks and mutation gates. Native code owns allocation and I/O only.
The runtime remains in `lib/proto/wire.coax` with separate authority/mutation
gates and two native-address wrappers. The frontend, check evaluator, C emitter,
host effects and toolchain remain trusted; finite checks are not a proof of
the bootstrap chain. See the evidence scopes in
[proto.md](proto.md#native-importer-audit-boundary). Normal builds and imports
need no Python, protoc, subprocess-based importer or installed helpers.
`coax proto` exposes generated Coax source; independent schema/oracle tests
retain their own tooling.

`#import_proto` generates typed views, repeated iterators, explicit presence,
oneof cases and caller-buffer codecs using the implemented data types.
Unknown fields survive edits; the first reader rejects duplicate known singular
fields/oneof members rather than merging them. Maps, proto2, editions, groups
and extension fields are rejected. Domain adapters and product migration are
separate gates; [proto.md](proto.md) specifies the exact supported subset.
Coax declarations may propose schema changes,
but they return through public schema review and compatibility checks.
There is no second Coax-only wire schema.

The [engine rename migration](../proto/migrations/README.md) records legacy reader
aliases, new canonical IDs and versioned writer transitions. Published layout,
asset profile identity, protocol name and brand are separate axes. A new
signature domain needs a new signature; old signed bytes are never rewritten.
The typed asset catalog migration is implemented; other consumer migrations
remain gated on real restored implementations.

Bounded codecs validate sizes, nesting, missing-versus-zero/null, unknown-field
policy, units and revisions before admitting domain values. Decoding grants
no authority. Original Nostr JSON, signed statements and content-addressed
bytes retain their own validation; protobuf serialization is not canonical
hashing. Native/GPU layouts, saves and reload migration remain separate.

The post-reset repository has the Coax bootstrap and recovered contract
foundation. Historical native code and old product crates are migration
sources, not a competing architecture or already restored services.

## 15. GPU programs

A future shader subset may share typed declarations with CPU code while
lowering separately to MSL and WGSL. It has no general pointers, allocator,
implicit context or recursion. Resource access modes and target capabilities
are explicit. Native Metal features require target-specific validation and
fallback decisions.

A CPU procedure compiled to Wasm is not a GPU shader. Browser WebGPU consumes
WGSL through browser bindings; native Metal consumes its own compiled pipeline.
The first shader experiment must compare shared-schema generation against a
new language backend before choosing either approach.

Validate offsets, strides, alignment and upload lengths for each pipeline.
GPU resources remain resident host objects with explicit queue/fence ownership.
Replacing a shader creates a new pipeline between frames, retires old resources
after completion, and cannot reinterpret a live buffer under an incompatible
layout. The initial Metal emitter only handles scalar f32/bool visual functions;
native stage wrappers supply vector packing and resource binding. The general
GPU design in this section remains planned.

`coax cuda` is a second, separate profile: the CUDA compute subset. Its
admission is the Coax policy `lib/compiler/compute.coax` over the checked
tree. Each `#export_c` procedure is a kernel that returns nothing and takes
its `int` work item first; the emitted entry takes an item count and runs
the procedure once for each item below it, in no defined order. A kernel and
everything it reaches use only bool, f32, f64, sized integers, fixed arrays of
at most 4096 scalars and views of numbers (kernel parameters are scalars or
views of numbers), `if`/`else`, `for` loops and acyclic calls; everything else
is E0700, and a file without a kernel is E0701. Integer, cast and bounds
checks keep their meaning and trap the launch. Float operations round once
each when compiled with `--fmad=false`, so +, -, *, / and sqrt agree with the
C build bit for bit; other math functions use the device library. A caller
must give each output element to exactly one work item. Shared memory,
barriers, atomics and target capability queries are not part of the subset.

A kernel that contains `for lane: 0..<32` loops (exactly those constant
bounds; a loop named `lane` with other bounds is an ordinary loop) is a warp kernel: each work
item is one 32-lane warp, launched as 32 threads per item. Each lane runs a
lane loop's body once with its own `lane`; code outside lane loops is the
warp's shared control flow and must give every lane the same values. A local
array of 32*k values indexed `lane*k + c` inside lane loops, where `c` is a
constant or the counter of a constant loop in `0..<k`, is per-lane: lane l
keeps elements `l*k ..< l*k+k` in registers. Per-lane arrays are used only that
way or passed whole (`a[..]`) to a device primitive outside lane loops. A lane
loop may not assign a value declared outside it, except per-lane elements and
stores through views, and may not `break`, `continue` or `return` out of it.
`cu_mma_bf16_16x8x16(a, b, c)` in `lib/compute/mma.coax` is the first device
primitive: a 16x16 by 16x8 BF16 product added into 16x8 f32, with the PTX
m16n8k16 fragment layout. Its Coax body is its meaning on the CPU; the device
runs one `mma.sync` instruction, whose f32 partial sums may round differently,
so its results are compared within a tolerance.

## 16. Builds, iteration and hot reload

### 16.1 A capability-aware build program

`build.coax` is a proposed typed build API. A workspace selects explicit
sources, targets, defines, libraries, outputs and plugins. It distinguishes
build-host execution from target compilation. File/process/network access
requires declared permission. Pure `#run` expressions do not inherit it.

A reproducible build records compiler hash/version, target description,
toolchain versions, source/module/plugin hashes, options and every admitted
input. File reads record paths and content; directory enumeration is sorted.
Environment reads use an allowlist and enter the cache key. Clock and randomness
are absent by default. Network access needs a pinned fetched artifact and
explicit opt-in; results are not a silently trusted cache dependency.

C compiler and linker invocation remain explicit toolchain steps. The current
driver uses `COAX_CC` or `cc`; it does not provision or pin that compiler.
Dependency resolution, package locks, remote caching, hermetic execution and
artifact signing are additional systems, not automatic consequences of a
programmable build language.

`build.coax` may generate CMake and Xcode source manifests first. Keep existing
CMake, `project.yml`, Xcode and Emscripten paths until generated parity and
real native/browser builds pass. Do not delete working build inputs merely
because a language build API has been designed.

### 16.2 Measure the whole loop

Record clean and warm parse/check/evaluate/emit/link times separately, then
asset processing, launch, GPU preparation and reload. For web delivery add
Wasm processing, compression, transfer, instantiation and browser setup.
Measure on named hardware, source revision and toolchain with multiple runs.
Report workload size and specialization/expansion counts alongside timings.

Version 0.13.0's `--timings FILE` records monotonic driver phase wall times,
status and generated byte count, with separate native compile/link phases.
These are host observations, never clocks or I/O available to `#run`. The real
engine harness also records process wall time, compiler peak RSS and repeated
single-shot invocations. OS caches remain uncontrolled; this is not an
incremental/cold-cache or complete install/launch profile.

An eventual resident compiler can invalidate declarations and cached
specializations precisely. That requires explicit dependency tracking and
bounded cache lifetime. The single-shot bootstrap is not incremental and has
not demonstrated any whole-engine compilation-speed target.

### 16.3 Transactional host reload

The resident host owns persistent storage, platform services, queues and
resource lifetimes. Reloadable code is behind a versioned C interface. Stop
workers at a safe point, verify no old stack frame/callback can execute, and
replace procedure-table entries only after compatibility checks.

Stored procedure references use stable IDs, not stale code addresses. Layout
changes need an explicit migration plan: allocate new storage, migrate checked
fields, validate handles/resources, and atomically publish or roll back. Do
not infer compatibility from matching field names. Preserve original state
until the migration succeeds.

Mac-host development reload is distinct from a signed iOS device image.
Device builds keep platform code-signing and deployment rules. A faster
compiler alone supplies no stack migration, safe pointer replacement, worker
quiescence or GPU resource preservation.

## 17. Interop, modules and tooling

### 17.1 Modules

Version 0.11.0 implements `#load "part.coax";` at file scope. Literal paths
resolve relative to the declaring unit (absolute paths are explicit too).
Canonical source paths provide once-only identity; active cycles are E0602.
The entry source followed by depth-first loads in written order forms one
flat declaration namespace. Each physical unit is validated by the Coax grammar before composition and
must balance its own delimiters within depth 128; no declaration may cross
a unit boundary. Comments and string contents never become directives. Names resolve across
units, and duplicates remain errors. Loads are not interpreted `#run` effects.

The checked Coax kernel in `lib/compiler/sources.coax` owns load syntax,
identity/cycle decisions, verification-scope policy and resource admission.
Native code owns filesystem resolution, reads and storage transport. A separate
C/header seed reproduces exactly in the Python-free bootstrap, with authority
checks and a zero-survivor mutation gate. `lib/compiler/compiler.coax` is the
actual bootstrap source graph, not a concatenated copy.

One compilation admits 128 canonical units, 32 active units, 16 MiB of source
bytes and 512 load directives per physical unit (E0603). Invalid loads are
E0600, unreadable/nonregular files E0601, and unbalanced composed units E0604.
Reads check the source-byte budget before allocation and while reading. These
bounds are not a whole-process sandbox or a bound on every compiler allocation.

`#authority` belongs to a physical source unit. Loading an unmarked unit does
not mark its procedures, and loading a marked unit retains its obligations.
Any authority unit causes graph checks to run during ordinary compilation;
actual execution must cover every authority procedure except `main` and
foreign imports. Tests may cover procedures from another unit. `coax test`
runs graph checks; `coax mutate` mutates reachable procedures and requires
coverage of the entry unit and authority units. Both accept `--library` for
units without a program main or native exports. Tests/directives remain
editable source and are not a runtime authorization mechanism.

Diagnostics, mutation reports and composed native traps retain original file
and line. Descriptor paths resolve beside their own import unit; generated
failures point at that directive. Since 0.13.0, optional native line maps retain
physical Coax locations through generated C; default debug native builds enable
them. Schema-1 maps also record source hashes and procedure/variable names.
`coax deps FILE` emits schema-version-1 JSON with canonical sources in visitation
order, resolved descriptor inputs and the selected work/storage limits. It checks the program first. This is a
path inventory, not a content-hashed lockfile, native-toolchain manifest or
cache key. Normal compilation and dependency listing invoke no Python,
preprocessor or protoc helper.

`#import "Basic";` remains the only named built-in module, providing checked
`print`. General directory modules with `module.coax`, explicit visibility,
separate namespaces, locked packages and conditional module parameters remain
planned. Vendored/locked sources are explicit inputs; import does not download
code. Future conditional compilation and module parameters must be represented
in cache keys and editor analysis.

### 17.2 C, C++, Swift and platform support

Implemented: `#export_c` procedures, `coax lib` (an object file without a C
`main`) and `coax header`. The header declares every type the exported
procedures use (`coax_t_Name` structs, `coax_v_T` views as `{data, count}`,
enum constants as `COAX_Name_member`) and asserts each struct's size and
field offsets with `_Static_assert` (or `static_assert` in C++), so a C
compiler that disagrees with Coax's layout refuses to build. Struct field
names cross unchanged, so C and C++ keywords are refused as field names
(E0371). A failed check inside an exported call aborts the host process.
`examples/pulse` links a Coax kernel into a C host and checks it against a C
reference every frame.

Use fixed-width C ABI data, explicit lengths, opaque handles and named release
functions. `int` is s64, not C `int`. A counted Coax string is not `char *`.
Generated headers must record size, alignment, calling convention, ownership,
nullability, callback lifetime and allocator pairing. Test both directions.

C++ classes and metal-cpp stay behind `extern "C"` shims. Swift uses the same
boundary. A future context-carrying internal convention must not leak across
these interfaces. Header import through libclang is planned and needs rules
for macros, inline-only symbols, variadics and unsupported declarations.

C11 output runs on the tested host. iOS, Android, Linux, Windows and wasm32
need distinct runtime, linker, ABI and debugging evidence before becoming
supported compiler targets. No C output claim establishes that an arbitrary
platform integration is complete. The current Coaxial splash uses
`coax/web/wasm.sh` (upstream LLVM/WASI plus a C face) and a WebGPU host. This
narrow browser consumer is useful feasibility evidence, not a complete engine
runtime, target-aware layout implementation or browser source debugger.

For Wasm, test pointer width, linear-memory growth and invalidated typed-array
views, explicit host imports, async loads, main-loop ownership and resource
release. Include browser input, geometry upload and rendering in the vertical
slice, not just a successful Wasm link.

### 17.3 Tooling

Implemented commands: `coax version`, `check`, `c`, `build`, `run`, `lib`,
`header`, `layout` (struct sizes, alignment, offsets and padding), `deps`
(source/descriptor input inventory) and
`explain` (every error code, also as the Markdown table the book uses).
`--keep-c` retains generated source for inspection; `--json` prints
diagnostics as JSON lines for tools and agents. Errors have stable codes,
source locations and fix text where available; compile-time failures add call
site notes. Version 0.13.0 adds `--debug-info` line directives and
`--source-map FILE` for original locations, source hashes and procedure/local
name/type labels. Default debug native builds enable line mapping; plain C
output opts in. Mac LLDB tests cover stepping, values and stacks across loaded
units, original-file native diagnostics and sanitizer traces. An optional LLDB
presentation script checks source hashes. This is not a Coax expression
evaluator, complete native type encoding, shader map or device-support claim.
See [DEBUGGING.md](DEBUGGING.md).

Planned tools include a formatter, typed symbol/effect queries,
expansion traces and a compiler server. The editor needs a safe analysis mode
that does not automatically execute privileged build plugins. Missing generated
inputs must be reported honestly rather than replaced with guessed symbols.

The standard library will grow by consumer: strings/views, containers, arenas,
reflection, math, jobs, files/processes, protocol adapters and platform bridges.
The existence of a module name in this plan does not mean it is importable.

## 18. Engine example

The complete runnable bootstrap example is
[budget.coax](examples/budget.coax): derive a static visibility
budget with ordinary checked computation, assert it, and print the result.
The emitted C contains the computed literal. It does not invoke an evaluator
on startup or alter the shipping engine.

The first representative engine slice is implemented under
[roadmap R0](ROADMAP.md): real cast poses against a pinned historical native
evaluator, plus robot/body and player/collision timings and actual iOS Metal
frame capture. [The evidence](../docs/verification/coax-r0-2026-10-28.md) records
output tolerance and zero observed kernel allocations. The
[pose follow-up](../docs/verification/coax-r0-pose-2026-10-28.md) passes the same
proposed 5% median gate in all three profiles; raw tail/outlier and device
limits remain explicit. The reference reader adaptation still requires independent
review, and physical-device/full-loop evidence remains open. The
historical `src/paynet` pulse update remains a candidate only after its
reference source/data/host path is recovered; that directory is not present
in the current tree. Required evidence is:

- Same input and output as the existing C++ update, including edge cases.
- No hidden frame allocations; explicit scratch and ownership boundaries.
- Exact deterministic results where the existing subsystem requires them.
- Native Metal pipeline receives verified layout and bounds.
- Frame time within an agreed budget, compared on the same hardware.
- The real iOS path remains functional; browser support requires its own
  later vertical-slice proof rather than being inferred from native parity.

This example determines which structs, arrays, loops and FFI features to add
next. Do not implement the whole target language before proving a subsystem.

## 19. Costs and risks

A production language includes diagnostics, debuggers, ABIs, standard
libraries, platforms, documentation, build reproducibility and maintenance.
Parsing a few examples is not the expensive part.

| Risk | Mitigation |
| --- | --- |
| Metaprogramming hides behavior | Inspect generated code, dependencies and expansion provenance. |
| Manual lifetime bugs | Stable handles, explicit owners, scratch boundaries, poisoning and runtime checks. |
| Specialization/expansion explosion | Deterministic budgets, caching, per-declaration time/code-size reports. |
| Compiler work delays engine work | Every stage proves a usable workflow and has a stop decision. |
| Premature new backend | Keep C output until measured evidence justifies replacement. |
| Build side effects | Separate pure evaluation from explicitly permitted build work. |
| Claimed cross-platform support | Publish a target matrix backed by ABI, runtime and end-to-end tests. |
| Metadata becomes a second schema source | Preserve public contracts, stable IDs and reviewed adapters. |
| Unsafe reload | Safe points, versioned C ABI, transactional migration and rollback. |

## 20. Implementation order and exit gates

The detailed [implementation roadmap](ROADMAP.md) is the execution plan for
the full vision. It starts with measured native adoption and source debugging,
then expands the language and tools through dependency-gated milestones.
The stage numbers below remain coarse topic identifiers used by existing
plans; they do not require an unrelated subsystem to wait for every lower
number. In particular, target/browser evidence must precede new backend work,
and the first GPU/browser slice need not wait for complete parallel jobs.

The protocol plan's P0 recovery runs now. Its P1 ingestion follows stage 2's
actual type/ABI prerequisites. Version 0.4.0 ships selected descriptor imports
and bounded codecs with Nostr/external, scalar and protoc interoperability tests;
product/domain parity remains open. The initial 0.3.0 home port used a
hand-written resource codec and opaque bounded host storage. The current iOS
home now uses generated `ios-home` codecs for configuration and invocation,
with bounded scratch buffers and the same closed resource admission policy.
Its scene storage and scalar host ABI remain in place. Native/protoc and
simulator tests cover this first product consumer; broader product adoption
remains separate. Stage 6 expands that foundation rather than deferring the
first bounded codec until every compiler feature is complete.

Source composition in 0.11.0 supplies the explicit source graph and input
inventory needed by later build workspaces. It does not implement package
resolution, visibility, reflection, plugins or the stage-4 build program, and
does not complete the remaining stage-2 engine/ABI evidence.

Work follows explicit dependencies and evidence gates, not calendar promises.
Each stage produces something independently testable. Platform/engine rollout
happens only after its own gate; a bootstrap compiler does not mean the app
already uses it. The reduced home now uses the tested scalar subset; it does
not satisfy all of stage 2 or stage 6. Frame-time parity against the prior
engine remains unmeasured.

| Stage | Work and prerequisites | Exit evidence |
| --- | --- | --- |
| 0 — Bootstrap semantics | Current lexer/parser/checker/C emitter; documented scalar grammar and behavior. | Existing pass/fail fixtures, checked formats and overflow behavior preserved. |
| 1 — Compile-time scalar slice | Implemented: global constants, `#run`, `#assert`, `type_name`, `check`; bounded typed-AST evaluation. In 0.5.0, checks: `#test`, `#forall`, `#authority`, `coax test` and `coax mutate`. | Forward dependencies, cycles, limits, capture/I/O rejection, debug/release errors, inspectable literal output and standalone native execution. |
| 2 — Data and C boundary | Implemented in 0.3.0: structs/defaults, enums, fixed arrays/views, half-open `for`, `defer`, `ifx`, checked casts, ordered runtime operands, `#export_c` with generated headers and layout assertions. `#foreign` imports scalar C symbols. Not yet: importing C headers. | Field/layout vectors and a pulse kernel matching a C reference byte for byte (`examples/pulse`) exist. The reduced native iOS home runs its scene, HUD and resource admission in Coax over that scalar ABI. R0 now compares the actual pose against a pinned native evaluator and records kernel/Metal-frame timings. The follow-up pose rewrite passes the proposed median budget; device/tail, full-loop and independent review gates remain open. |
| 3 — Metadata and generation | Started in 0.12.0: schema-1 CPU type/field/enum layout queries and ordinary metadata values, with bounded lowering. Stable declaration IDs, derived operations and hygienic typed code remain next. | Runnable compile-time inspector table; Verse derives its shipped Haunted House vertex stride, layer offset and scalar admission table; independent C layout and invalid-query tests. General registries, provenance-aware derived code and alternate targets remain gated. |
| 4 — Build workspaces and tools | Dependency scheduler, capability model, input manifests and staged compiler events; then plugins and `build.coax`. | One real asset/registry workflow; reproducible inputs and cache invalidation; generated CMake/Xcode manifests match before replacement. |
| 5 — Effects and deterministic jobs | Region/alias model, allocator/effect summaries, strict math and scheduler integration. | Real subsystem replay at 1/2/4/8 workers; incompatible effects and aliases rejected. |
| 6 — Protocol expansion and GPU experiments | Stable metadata/layouts and previous native ABI proof. Existing protocol adapters remain until parity. | Extend the first selected-profile codec into more domains; prove a Metal pipeline separately and compare generated shaders before choosing a compiler backend. |
| 7 — Browser vertical slice | wasm32 runtime/host ABI and separate shader bridge. | Geometry, memory growth, async resources, input, rendering and debug path in an actual browser. |
| 8 — Resident compiler and reload | Dependency cache, stable code IDs, safe points and transactional state migration. | Mac-host edit/reload retains state and resources, rolls back failures and has measured latency. No device code-loading bypass. |
| 9 — Additional backends | Earlier workflows demonstrate value; C backend remains a reference. | Native debug/optimized backends pass the same conformance and target suites and improve a measured workload. |

Stop after stage 2 if the subsystem is harder to maintain or less reliable
than its C++ equivalent. Pause after stage 4 if ordinary C++ code generation
provides the same workflow with less infrastructure. At every stage, keep
working engine paths until replacement parity is demonstrated.

## 21. Non-goals

No mandatory GC, runtime VM, implicit recursive destructors, exceptions,
virtual inheritance, header preprocessor, automatic package downloading or
platform code-signing bypass. There is no claim of general memory safety,
whole-engine migration, automatic GPU compilation or fully hermetic builds.

The bootstrap intentionally rejects unsupported reserved syntax. It does not
pretend to implement pointer ownership, macros, effects, context, general GPU
programs or compiler plugins.

## 22. Open questions

1. Which device/tail budgets and independent evidence close the adoption gate after the pose median fix?
2. Which region/alias model is sufficient for field/range effects without
   overpromising safety for arbitrary pointer graphs?
3. What stable IDs distinguish compiler declarations, engine registrations,
   GPU layouts and protocol names across independent builds?
4. Which generated registry or asset workflow should establish plugin value
   before a programmable build replaces existing source manifests?
5. Which host capability interface can support useful file/process work while
   preserving input provenance and safe editor analysis?
6. When does measured AST evaluation cost justify an internal bytecode VM?
7. Should shader layout sharing remain a generator feeding existing shader
   tools, or justify a restricted Coax backend after the pipeline experiment?
8. Decided 2026-10-27: Coax lives in `coax/` at the root of the openagents
   repo, with this spec, the grammar, the compiler and the book
   (`coax/book/`). The earlier engine repository is a historical migration source.
