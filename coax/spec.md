# Coax: a language for Aethyr

Date: 2026-10-27. Status: **draft specification, heading toward a start.**
Nothing here is built yet. The owner wants to start soon. §20 is the start
plan, and the rest of this document is what the first version has to
satisfy.

Coax is a programming language for the Verse and the rest of OpenAgents'
real-time code. It starts from Jonathan Blow's Jai and changes what our
constraints call for. Source files end in `.coax`. The compiler is `coax`.

The draft draws on two sources about Jai:

- [The Jai Primer](https://github.com/BSVino/JaiPrimer/blob/master/JaiPrimer.md),
  an early outside summary of Blow's talks. Blow has said it does not
  describe the language well.
- **Real Jai code**: Forrest Smith's
  [Advent of Code 2022 solutions](https://github.com/forrestthewoods/aoc2022/tree/master/jai).
  These are a 4,821-line `main.jai` and a 53-line `modules/autorun.jai`
  metaprogram plugin, written against the closed beta compiler. Where this
  document says "Jai does X", it is citing that code (as `aoc2022
  main.jai:LINE`) or the primer. It is never citing the compiler itself,
  which we have not used.

Contents:

- Part A, the case (§1–§4): what we need, why Jai is the dream, why we
  can't depend on it, and what Coax does differently.
- Part B, the language (§5–§17).
- Part C, the plan (§18–§22).

---

# Part A. The case

## 1. What we are asking for

Every requirement comes from [aethyr-cpp](https://github.com/OpenAgentsInc/aethyr-cpp)
(the C++ Verse engine, "this repository" below), the OpenAgents monorepo, or
the owner's direction on 2026-10-27 to make the Verse as fast as possible.

| # | Requirement | Where it comes from |
| --- | --- | --- |
| R1 | **Compile in well under a second** for a one-file change, and keep it there as the code grows | The owner: C++ compiles faster than Rust, and that matters. The 11.9 s `ecs.h` rebuild ([performance stack](https://github.com/OpenAgentsInc/aethyr-cpp/blob/main/docs/performance-stack.md)) |
| R2 | **Hot reload that keeps state**, including when a struct's layout changes | Performance stack item 1 |
| R3 | **Data-oriented by default**: SoA and AoS from one declaration, arenas, no hidden allocation | Item 3; the ECS's dense sparse sets |
| R4 | **Deterministic simulation**: bit-identical results across machines and worker counts | `-ffp-contract=off` on Ice Cream Man and the text reading; replays and saves |
| R5 | **Safe parallelism without a borrow checker** | Item 3's job tiers |
| R6 | **One source for CPU and GPU data layouts**, and shaders that share it | Item 4; `home.metal` and the instance buffers in `src/metal` |
| R7 | **SIMD over SoA** without hand intrinsics for each target | Item 5 |
| R8 | **Every target we ship**: native Metal on iPhone, iPad and Mac first, WebAssembly for the web worlds, later Android, Linux and Windows | `apps/openagents-ios` (metal-cpp), the Emscripten build |
| R9 | **The protocol is native**: OpenAgents Protocol schemas, data classes, limits and versions are types | openagents `docs/protocol/openagents-protocol-spec.md`; `third_party/openagents-engine-proto` |
| R10 | **Reflection** for saves, replication and the editor, with versions | The ECS's registered fields; world saves |
| R11 | **C interop, and a path from C++**, so it can live beside 40,000 lines of C++ for years | Every rewrite risk |
| R12 | **Agents write most of the code**: one canonical form, errors with fixes, a compiler you can query | How this repository is built |
| R13 | **The build lives in the source**: no CMake and `project.yml` both listing files | `CMakeLists.txt`, `apps/openagents-ios/project.yml` |
| R14 | **Bounded data at trust boundaries**: decoders refuse oversize input before allocating | Protocol principle 6 |

## 2. Why Jai is the dream

In the performance discussion I said that the best possible stack, with
unlimited time, would be "Jai, if we had access to it". Here is what that
means, feature by feature. These are properties no shipping language we
can use today has all at once.

### 2.1 Compile speed is a design goal, not an afterthought

Blow's stated target is compiling a large game in about a second, and
public demos have shown builds of large programs in that range. Three
design choices make it possible, and C++ and Rust lack all three:

- **No headers and no textual includes.** Each file is parsed once per
  build. C++ reparses `ecs.h` in every translation unit that includes it,
  which is why touching it costs 80 s of CPU here.
- **No template instantiation model that multiplies work.** Polymorphic
  procedures (`$T`) are instantiated once per type set, without
  two-phase lookup, SFINAE or header-only libraries.
- **A fast debug backend** that does not go through LLVM for the inner
  loop.

R1 is the requirement we care about most. Jai is the only language
designed from the start around it at game scale.

### 2.2 The build is a program, and it can watch the compiler

`aoc2022 modules/autorun.jai` is the clearest demonstration in the
sources. It is a **metaprogram plugin**: ordinary Jai code that the
compiler loads, and that receives a message for each compiler event:

```jai
message :: (_p: *Plugin, message: *Compiler.Message) {
    p := cast(*AutorunPlugin) _p;
    if message.kind == .COMPLETE {
        complete := cast(*Compiler.Message_Complete) message;
        if complete.error_code != .NONE { p.should_run = false; }
    }
}

shutdown :: (_p: *Plugin) {
    ...
    using options := Compiler.get_build_options(p.workspace);
    if output_type != .EXECUTABLE { log(...); }
    else { ...; Process.run_command(run_string); }
}
```

The plugin reads the workspace's build options, sees whether the
compile succeeded, and runs the program. That alone replaces a shell
script. The same message stream also delivers every typechecked
declaration, so a plugin can:

- generate code from types;
- reject code that breaks a project rule;
- emit side files such as bindings or schemas;
- drive the whole build: which files, which targets, which flags.

For us this is R13 (no CMake) and much of R9 and R10 (codecs and
reflection generated from types) with no separate generator.

### 2.3 Code is a value at compile time

`aoc2022 main.jai:86-105` builds assertion macros that print the source
text of their arguments:

```jai
code_to_string :: (code: Code) -> string #expand {
    PP       :: #import "Program_Print";
    Compiler :: #import "Compiler";
    code_node := Compiler.compiler_get_nodes(code);
    builder: String_Builder;
    PP.print_expression(*builder, code_node);
    return builder_to_string(*builder, allocator=temp);
}

assert_eq :: (a: Code, b: Code) #expand {
    sa := #run code_to_string(a);   // the expression's text, computed at compile time
    va := #insert a;                // the expression's value, at run time
    ...
    assert(va == vb, "...left: %  expr: %...", va, sa, vb, sb, #location(a));
}
```

Here is what that code shows:

- A parameter of type `Code` receives an unevaluated expression.
- `#expand` makes the procedure a hygienic macro, inlined into the
  caller's scope.
- `compiler_get_nodes` turns the code into an AST the program can walk.
- `#insert` splices code back in.
- `#location` gives the call site.

Rust needs procedural macros in a separate crate to do this, and C++
needs the preprocessor. Jai does it with ordinary procedures in the same
file.

### 2.4 `#run` runs anything

Any procedure can run at compile time, including file IO, through a
bytecode interpreter. Lookup tables, baked assets, generated SIMD
kernels, protocol codecs and test vectors become ordinary code. They are
not build steps in another language.

### 2.5 Memory is explicit but not painful

- **The implicit `context`** carries the allocator. Libraries allocate
  through it, and the caller chooses where memory comes from.
- **The temporary allocator** is built in. `aoc2022 main.jai:79` sets
  `result.allocator = temp;`, and `main.jai:92` passes `allocator=temp`.
  Scratch memory is a pointer bump, freed in bulk, with no lifetimes to
  annotate.
- **`[..]` dynamic arrays** carry their allocator. `array_add(*result, x)`
  is the whole API.
- **`---`** opts out of zero initialization where it matters.

That is the arena-and-frame-allocator discipline game engines use, as the
language default instead of a convention.

### 2.6 Data-oriented layout in the type system

The primer's `SOA` keyword switches a struct's array layout without
changing the code that uses it. Every engine that wants cache-friendly
data hand-writes this in C++, as our ECS does.

### 2.7 Small things that add up

All of these appear in aoc2022:

- **`ifx a then b else c`** expressions (`main.jai:167`, `:1623`).
- **Multiple return values**: `file, success := read_entire_file(...)`.
- **Distinct types**: `RoomIndex :: #type,distinct int;` (`main.jai:2747`)
  makes an index type that cannot be mixed with a plain `int` or another
  index type. That is a typed handle for free.
- **Struct literals** `Vec2.{a, b}` and defaults in declarations
  (`cur_room : RoomIndex = -1;`).
- **`for` with `it` and `it_index`**, plus ranges such as `for 0..n-1`.
- **`defer` blocks** (`main.jai:3-6` times the whole program with one).
- **`using` and `#as using base:`** (`autorun.jai`) for composition, with
  implicit conversion to the base. This is inheritance's useful part
  without vtables.
- **`#scope_module`** to make the rest of a file private.
- **Module aliases**: `Compiler :: #import "Compiler";`.
- **A real standard library** for this kind of code: `Basic`, `String`,
  `Hash_Table`, `Sort`, `File` and `Bit_Operations`.

### 2.8 Made by game developers for games

Every feature is chosen for one audience:

- programmers who ship 60 Hz software;
- who know where their memory is;
- who would rather have power than guard rails.

That is our audience too, except that most of our code is written by
agents (§4.1).

## 3. Why we can't depend on Jai

- **No access.** Jai is a closed beta by invitation. There is no public
  download, no public license and no published release date.
- **No license we can plan around.** We would not know the license terms
  for shipping a commercial product, the compiler's redistribution terms
  for CI and agent hosts, or whether we could patch it.
- **One maintainer team, one roadmap.** Blow's studio builds Jai for its
  own games. If our needs diverge, we cannot change the language, fix a
  compiler bug, or add a backend.
- **No platform story we can depend on.** We need:
  - first-class Metal on iOS, under Apple's code-signing rules;
  - WebAssembly for the web worlds;
  - Xcode and App Store integration.
  
  None of these is a published commitment.
- **No GPU language.** Jai does not compile shaders. Our CPU/GPU layout
  sharing (R6) would remain a separate system.
- **No determinism model.** There is no stated floating-point
  contraction policy, and `Table` iterates in hash order (§4.3).
- **Agents cannot learn it.** There is very little public Jai code. aoc2022
  is one of the few large public samples. Models write languages well in
  proportion to how much of them they have seen. For a language of our
  own, we would write the corpus: spec, examples, tests and a translated
  engine.

So "the dream" is the design. It is not a dependency. Coax takes the
design and adds what we need, in a compiler we own.

## 4. What Coax does differently, and why

### 4.1 Agents write most of the code

Jai trusts the programmer ("design for good programmers"). Coax keeps the
power but adds checks on by default:

| Jai | Coax | Why |
| --- | --- | --- |
| Bounds and null checks are the programmer's job | On by default in debug and dev builds; `#no_check` on a block turns them off, and the compiler lists every unchecked block | Agent mistakes surface in the first run |
| `cast(*AutorunPlugin) _p` is unchecked (`autorun.jai`) | A downcast through `#as` base is checked in debug (type ID in the base) | Plugins and event systems are full of these |
| `xx` autocasts to whatever the context needs | `xx` only between integer widths, and checked in debug; other casts are written out | An autocast hides a lossy conversion where an agent cannot see it |
| Format strings are `print("%", x)` over `..Any` | Same at the call site, but the format is checked against the arguments at compile time | Wrong argument counts are a run-time surprise in Jai |
| Optional semicolons and several equivalent spellings in places | One canonical form, printed by the compiler (`coax fmt`) | Agents and diffs both want one form |

### 4.2 Derive what Jai makes you hand-write

aoc2022 repeats the same three blocks for every key type (`main.jai:2766-2800`):

```jai
operator == :: (a: d16_search_state, b: d16_search_state) -> bool {
    return a.opened_valves == b.opened_valves && a.released_pressure == ... ;
}
get_hash_d16_search_state :: (v: d16_search_state) -> u32 {
    result = hash_combine(result, Hash.get_hash(v.opened_valves)); ...
}
full_list : Table(d16_full_key, [..]d16_search_state, get_hash_d16_full_key);
```

Hash tables then find `operator ==` and `get_hash` by name. The author
needed `#poke_name Hash_Table operator==;` (end of `main.jai`) to put
those names where the hash-table module could see them. Coax replaces
that with:

- **`#derive(eq, hash, order, print, proto)`** on a struct. The
  compiler writes field-by-field implementations from reflection, in
  declaration order.
- **Explicit constraints** on polymorphic parameters:
  `Table :: struct ($K: Type/Hashable, $V: Type)`. A missing `hash` is an
  error at the `Table` use site with a fix ("add `#derive(hash)` to
  `d16_full_key`"). It is never a name lookup inside another module.
- No `#poke_name`.

### 4.3 Determinism is a language property

Jai has no float model, and hash-table iteration is in hash order. That
is fine for puzzles, but it breaks replays and lockstep simulation. Coax
adds:

- `strict` blocks, `deterministic` procedures and `fixed32` (§13);
- `Table` iteration in insertion order by default, with `Hash_Order_Table`
  as an explicit opt-in.

### 4.4 Parallelism has a contract

Jai's primer mentions that captures could restrict what threads touch.
Coax makes capture lists into **effects** that the job scheduler and the
compiler both check (§12). This is the biggest addition.

### 4.5 The protocol and the GPU are in the language

- `@proto` types with field numbers, limits, data classes and versions,
  checked against the frozen descriptors (§14).
- `shader` procedures that compile to MSL first, sharing `@gpu` struct
  layouts with CPU code (§15).

### 4.6 Hot reload is designed in

Jai can hot-reload a DLL like any native language. Coax adds:

- `#persistent` state;
- procedure IDs instead of raw function pointers;
- compiler-generated layout migration (§16).

### 4.7 The compiler is a service

The compiler is long-running, with a query API: types, uses, layouts,
effects and "what changed". Agents call it instead of grepping (§17).

### 4.8 Targets we ship

Metal-first native on Apple, WebAssembly, and C output for the
transition (§16, §20). Jai's announced targets do not include iOS or wasm
as commitments.

### 4.9 Summary table

| Area | Jai | Coax |
| --- | --- | --- |
| Availability | Closed beta | Ours, in this repo |
| Compile speed | Core goal | Core goal (same design, §16) |
| Metaprogram | Plugins with compiler messages | Same, plus typed build API and the query server |
| Macros | `Code`, `#expand`, `#insert` | Same, plus hygiene rules written down and a size limit on expansions |
| Equality and hashing | Hand-written, found by name, `#poke_name` | `#derive`, explicit constraints |
| Operator overloading | Any type | Math, comparison and indexing only, and pure (§9) |
| Casts | `cast`, `xx` | `cast`, `xx` for integers only, checked in debug |
| Checks | Programmer's job | On in debug and dev, listed when off |
| Floats | Platform default | `strict`, `fixed32` |
| Hash maps | Hash-order iteration | Insertion order by default |
| Threads | Library | Effects in capture lists, checked |
| Serialization | Reflection and libraries | `@proto` types, versions, limits |
| GPU | None | `shader` procedures, `@gpu` layouts, MSL first |
| Hot reload | DLL reload | `#persistent`, procedure IDs, migrations |
| `Any` | Everywhere | Tools and logging only; refused in `@proto` and `#persistent` types |
| Tooling | Compiler | Compiler server, query API, `coax fmt`, diagnostics with fixes |
| Targets | Desktop | Apple Metal first, wasm, C output |

---

# Part B. The language

## 5. Lexical structure

- **Source.** UTF-8. One file is one source unit. Line comments `//`,
  nestable block comments `/* */`, and doc comments `///` on the next
  declaration.
- **Identifiers.** ASCII letters, digits and `_`. Unicode identifiers are
  not allowed, so the same name never has two spellings.
- **Keywords**, about 40, reserved:

  `struct union enum enum_flags if else ifx then while for break continue
  return defer switch case using cast xx inline no_inline job pure strict
  deterministic shader try or_else push_context null true false it it_index
  size_of type_of type_info Type Code`

- **Directives** begin with `#` (`#run`, `#import`, `#insert`...).
  Annotations begin with `@` (`@proto`, `@gpu`, `@1`...). Directives
  change compilation. Annotations attach data to declarations, and
  reflection can read them.
- **Literals.**
  - Integers: `123`, `0x7f`, `0b1010` and `1_000_000`.
  - Floats: `1.5` and `1e-3`.
  - Strings: `"..."` with `\n \t \\ \" \u{...}`.
  - Raw strings: `#string END ... END`, as in Jai.
  - Characters: `#char "a"` (`aoc2022 main.jai:719`).
- **Semicolons** end statements. `coax fmt` inserts nothing and accepts
  nothing else.

## 6. Declarations and types

### 6.1 Declarations

```coax
counter: s32 = 0;                 // variable with type and value
name := "Kiki";                   // inferred
speed: f32;                       // zero
scratch: [4096] u8 = ---;         // uninitialized, explicit
GRAVITY :: 9.81;                  // constant, compile-time
Vec3 :: struct { x, y, z: f32; }  // type
lerp :: (a: f32, b: f32, t: f32) -> f32 { return a + (b - a) * t; }
```

`::` is a constant binding. Types and procedures are values bound with
`::`. Order of declarations in a file does not matter. There are no
forward declarations.

### 6.2 Built-in types

| Group | Types |
| --- | --- |
| Integers | `s8 s16 s32 s64 u8 u16 u32 u64`, `int` (= s64), `uint` (= u64) |
| Floats | `f32 f64` |
| Fixed point | `fixed32` (Q16.16), `fixed64` (Q32.32) |
| Small math | `v2 v3 v4 quat m3 m4`, and integer `v2i v3i v4i` |
| SIMD lanes | `f32x4 s32x4 u32x4 f32x8` |
| Other | `bool`, `string` (view: pointer + byte count, UTF-8), `Type`, `Code`, `Any` (§4.9 limits) |

Integer overflow traps in debug and dev and wraps in release, unless a
block says `#wrap`, which makes wrapping the defined result in every
build. There is no implicit conversion between integer types, between
integers and floats, or from `bool`.

### 6.3 Composite types

- **Pointers.**
  - `*T` is never null.
  - `*T?` may be null.
  - `p.x` reads through a pointer. `p.*` dereferences it (Jai writes
    `<<p`, as in `aoc2022 <<most_time`; we use a postfix form so chains
    read left to right).
  - `*x` takes the address of `x`, as in Jai.
- **Arrays.**
  - `[N] T` has a fixed size.
  - `[] T` is a view.
  - `[..] T` is dynamic and carries its allocator.
  - Views never decay to pointers.
  - Indexing is bounds-checked in debug and dev.
- **Optionals.** `T?`. The compiler requires a test (`if x {...}`,
  `x or_else default`, `try x`) before the value is used.
- **Distinct types.** `RoomIndex :: #distinct int;` is Jai's
  `#type,distinct`. Arithmetic within the type is allowed, and mixing it
  with `int` needs a cast.
- **Handles.** `Handle(T)` is a 32-bit index plus a generation (like
  `ecs::Entity`). A stale handle fails its generation check in debug.

### 6.4 Structs

```coax
Room :: struct #derive(eq, hash, print) {
  name:    string;
  mask:    u64;
  rate:    int;
  tunnels: [..] RoomIndex;
  cur:     RoomIndex = -1;      // default
}

Player :: struct {
  #as using body: Body;         // composition; *Player converts to *Body
  score: int;
}
```

- Fields are laid out in declaration order with natural alignment, unless
  the struct says `#packed` or `#c_layout` (C ABI, §17).
- `using` brings a field's members into scope. `#as` also allows implicit
  conversion to that field's type. This follows Jai (`autorun.jai`).
- Literals: `Room.{name = "AA", rate = 0}` names its fields. A positional
  `Vec2.{1, 2}` is allowed only for the built-in math types and for
  structs marked `#positional`. Agents get field names wrong less often
  than they get positions wrong.

### 6.5 Enums and unions

```coax
Outcome :: enum u8 {
  completed @wire "completed";
  refused   @wire "refused";
  failed    @wire "failed";
  cancelled @wire "cancelled";
  unknown   @wire "unknown";
}

Answer :: union {
  noul:   struct { noul: f64; };
  choice: struct { choice: u32; confidence: f64; probabilities: [] f64; };
}
```

- Every enum has an explicit underlying type. `enum_flags` makes bit
  sets.
- `@wire` gives the protocol spelling (§14).
- Unions are tagged. `switch` on a union or enum must be exhaustive, or
  have `case else`.

## 7. Procedures

### 7.1 One form everywhere

```coax
                                  { ... }   // block
                       [read m]   { ... }   // captured block
    (i: s32) -> f32    [read m]   { ... }   // anonymous procedure
f :: (i: s32) -> f32   [read m]   { ... }   // named: same text, local or global
```

This is the primer's code maturation path. Capture lists are optional on
ordinary code and required on jobs (§12).

### 7.2 Parameters and returns

- Parameters are immutable. To change something, pass `*T`.
- Defaults: `(n: int = 4)`. Named arguments at the call site:
  `split(s, sep = ",")`.
- **Multiple returns**, as in Jai: `read_file :: (path: string) -> (data:
  string, ok: bool)`, and the call site writes `data, ok := read_file(p);`.
- Varargs: `(args: ..T)`. `..Any` is allowed only in procedures marked
  `#format` (§4.1), which the compiler checks.
- Calling conventions: the Coax convention by default (it passes
  `context`), `#c_call` for C, and `#no_context` for leaf procedures and
  callbacks.

### 7.3 Inlining and specialization

- `inline` and `no_inline` work at the definition and at the call site, as
  in the primer.
- `#bake_arguments f(n = 4)` creates a specialized copy at compile time.
  This is the primer's `#bake`.

### 7.4 Polymorphism

```coax
max :: (a: $T/Ordered, b: T) -> T { return ifx a > b then a else b; }

Pool :: struct ($T: Type, $N: int) {
  items: [N] T;
  free:  [N] u32;
  count: u32;
}
```

- `$T` binds a type at the call site, and `$N` binds a compile-time
  value.
- `/Ordered` is a **constraint**: a named set of procedures that must
  exist for `T`. The constraints are `Ordered`, `Hashable`, `Equatable`,
  `Numeric`, `Printable`, `Proto` and `Gpu`. A user constraint is a
  struct of procedure types:

  ```coax
  Hashable :: constraint ($T) { hash: (T) -> u64; eq: (T, T) -> bool; }
  ```

- Instantiations are cached by type signature across builds (§16).
- There are no type-level metaprograms. Anything fancier is `#run` code
  that generates declarations (§10).

## 8. Expressions and control flow

- `ifx c then a else b` is the conditional expression (Jai's spelling).
- `if`, `while`, `for`, `switch`, `defer`, `break` and `continue`.
  Labeled loops: `for :outer rows { for cols { break outer; } }`.
- **`for`** forms:

  ```coax
  for 0..n-1 { ... }              // range, inclusive like Jai: it, it_index
  for i: 0..<n { ... }            // half-open, named
  for items { it.x += 1; }        // array, view or SOA: it is a pointer-like accessor
  for < items { ... }             // reverse
  for k, v: table { ... }         // insertion order (§4.3)
  ```

  Removing the current element during a `for` over a dynamic array uses
  `remove it;`, as in Jai. The compiler swaps and keeps the loop correct.
- **`defer`** runs at scope exit, last in first out. `defer` blocks may
  not `return`.
- **Errors are values** (§11). `try` unwraps or returns the error, and
  `or_else` supplies a default.

## 9. Operators

Operators are allowed on user types, unlike our first draft, because
aoc2022 shows how much they help with small math types (`Vec2`, `Vec3`
at `main.jai:247-300`). They are limited so they cannot hide work:

- Overloadable operators:
  - arithmetic `+ - * / %` and unary `-`;
  - comparison `== != < <= > >=`;
  - indexing `[]`;
  - bitwise `& | ^ ~ << >>`.
  
  Assignment, `.`, `&&`, `||` and calls cannot be overloaded.
- **An operator procedure must be `pure`**: it touches only its
  parameters, cannot allocate, and cannot fail. It reads like math, so it
  must behave like math.
- `==` and `hash` come from `#derive` unless a type needs something else.
  If `==` is written by hand, `hash` must be too, or the type cannot be a
  `Table` key.
- The built-in math types already have their operators. `v3 * f32`,
  `m4 * v4` and `quat * v3` need no code.

## 10. Compile time: `#run`, `Code` and macros

### 10.1 `#run`

```coax
SINE_TABLE :: #run make_sine_table(1024);     // array computed at compile time, stored in the binary
#run check_layouts();                         // a compile-time assertion pass
```

- `#run expr` evaluates any expression at compile time in the bytecode
  interpreter, and the result becomes a constant. The interpreter can do
  file IO, network IO only with `#allow_network`, call C through
  `#import_c`, and allocate.
- A `#run` that reads files records them as build inputs, so changing
  the file rebuilds what depends on it.
- `#run` output is deterministic. The interpreter offers no clock and no
  randomness unless the build options allow them.

### 10.2 `Code` and macros

Taken from the aoc2022 assert macros, with rules added:

```coax
assert_eq :: (a: Code, b: Code) #expand {
  va := #insert a;
  vb := #insert b;
  if va != vb {
    fail("left: % (%)\nright: % (%)", va, #text a, vb, #text b, at = #location(a));
  }
}
```

- A `Code` parameter receives an unevaluated, typechecked expression or
  block.
- `#insert c` splices it, and `#insert #run gen()` splices code that a
  compile-time procedure built.
- `#text c` is the expression's source text, normalized by `coax fmt`.
  It replaces the `code_to_string` helper that aoc2022 writes by hand.
- `#location(c)` gives the file, line and column.
- **Hygiene.** Names declared inside an `#expand` body are private to it,
  unless they are declared `#export_to_caller`. Jai's `#expand` lets a
  macro read and write the caller's locals through backticks. Coax allows
  that only for names the macro lists in its signature:
  `(a: Code) #expand [write caller.count]`.
- **Budgets.** The compiler reports any macro whose expansion is over
  4 KB of AST nodes, or over 1 ms of interpreter time per use. A macro
  nobody notices is a compile-time cost nobody budgets.

### 10.3 Reflection

```coax
info := type_info(Room);           // *Type_Info_Struct
for info.members {
  print("% % @%\n", it.name, it.type.name, it.offset);
  for it.notes { ... }             // annotations: @proto, @1, @limit(...)
}
```

- Every type has a `Type_Info` at compile time and at run time. Struct
  members carry names, types, offsets and annotations.
- `#derive`, codecs, the editor's inspector and save files are all
  library code over reflection.
- Run-time type info for a type can be dropped with `#no_type_info`, so
  shipped binaries carry only what they need.

### 10.4 Metaprogram plugins

Coax keeps Jai's plugin model (`autorun.jai`) and types it:

```coax
Autorun :: struct { #as using base: Build.Plugin; should_run := true; }

on_message :: (p: *Autorun, m: Build.Message) {
  switch m {
    case .typechecked: (d: Build.Declaration) { lint_layout(d); }
    case .complete:    (c) { p.should_run = c.ok; }
  }
}

on_finish :: (p: *Autorun, w: *Build.Workspace) {
  if p.should_run && w.options.output == .executable { Process.run(w.output_path); }
}
```

- `Build.Message` is a tagged union, so a plugin cannot mistake one
  message kind for another. aoc2022 casts `*Compiler.Message` to
  `*Compiler.Message_Complete` by hand.
- Message kinds:
  - `file_loaded`;
  - `parsed`;
  - `typechecked` (per declaration);
  - `phase` (before and after code generation);
  - `complete`.
- A plugin may:
  - add declarations (`Build.add_source(w, text)`);
  - add files;
  - set build options;
  - report diagnostics with fixes;
  - write side outputs (bindings, `.proto` files, shader sources).
- Plugins run in the interpreter. They are part of the build's input
  hash, so a plugin change rebuilds what it affects.

## 11. Memory and errors

### 11.1 The context

```coax
Context :: struct {
  allocator:      Allocator;
  temp:           *Temp_Allocator;      // reset by the caller, usually once a frame
  logger:         Logger;
  jobs:           *Job_System;
  assert_handler: Assert_Handler;
}
```

- `context` is passed implicitly to every procedure, except `#no_context`
  and `#c_call` procedures.
- `push_context .{allocator = frame_arena} { library_call(); }` changes
  it for a block.
- **`temp`** is a pointer-bump arena, as in Jai. `reset_temp()` frees it
  all, and a frame loop calls it once a frame. In debug, memory from
  `temp` is poisoned after a reset, and a pointer to it held in
  `#persistent` data is reported at the reset that invalidates it.

### 11.2 Allocation rules

- Allocation is explicit: `New(T)`, `NewArray(n, T)`, `array_add` and
  `free`. A procedure that allocates says so with `#allocates`, and the
  compiler checks that transitively. A `job` or `shader` cannot allocate
  except from an arena it was handed.
- Owned pointers `!*T` free what they own when their owner is freed (the
  primer). They are meant for the few tree-shaped structures. Most data
  lives in arenas and pools.
- `@joint` fields share one allocation (the primer's mesh example).

### 11.3 Errors

```coax
Read_Error :: enum u8 { not_found; too_large; denied; }

read_file :: (path: string, max: int) -> (string, Read_Error?) #allocates { ... }

load :: (p: string) -> (Level, Read_Error?) {
  text := try read_file(p, 1 << 20);      // returns the error to our caller
  level := parse(text) or_else default_level();
  return level, null;
}
```

- A procedure that can fail returns an error as its last result, typed
  as an optional enum or union. `try` returns it upward, and `or_else`
  handles it in place. Ignoring an error result is a compile error.
- There are no exceptions and no panics in library code. `assert` calls
  `context.assert_handler`, which in a shipped build records and
  continues, or stops the frame, as the app decides.
- Protocol refusals are ordinary enum values (`Refusal.limit_exceeded`).

## 12. Jobs: captures as effects

The most important addition to Jai.

```coax
move :: (dt: f32) job [write transforms.position, read velocities] {
  for_parallel transforms grain 1024 {
    it.position += velocities[it_index] * dt;
  }
}

animate :: () job [write poses, read transforms.orientation, read clips] { ... }
```

1. A `job` declares every piece of shared data it touches, as `read` or
   `write`. The compiler refuses a job body that touches anything else,
   including globals.
2. **Effects are per field.** `write transforms.position` and
   `read transforms.orientation` do not conflict, and SOA makes that true
   at the memory level.
3. **The scheduler builds the frame graph from capture lists.** Two jobs
   run in parallel only if each one's writes are disjoint from the
   other's reads and writes. This is checked at compile time when the
   graph is static, and at schedule time otherwise.
4. **`for_parallel` splits a range.** A split writes only its own index
   range of a `write` array, and the compiler checks that writes are
   indexed by `it_index`.
5. **Deterministic merge.** Reductions (`sum`, `min`, appends) combine in
   index order, so results do not depend on the worker count.
6. A `job` can call only procedures that are `job`s with a subset of its
   effects, or `pure`.

This is the protocol's `Effects` object (reads, writes, attenuation)
applied to memory. It is not a borrow checker. Inside a job, pointers are
ordinary pointers. The guarantee is about which data two jobs may touch
at once, which is the guarantee a frame graph needs.

## 13. Determinism

```coax
step_ice_cream :: (s: *Sim, input: Input) deterministic strict { ... }
```

- **`strict`** means:
  - no FMA contraction;
  - no reassociation;
  - IEEE rounding;
  - no approximate reciprocal or rsqrt;
  - math from `Strict_Math` (correctly rounded `sin`, `cos`, `pow`,
    `sqrt`) instead of the platform libm.
  
  Today that takes per-file `-ffp-contract=off` in CMake, which is easy
  to lose. Here it travels with the code.
- **`deterministic`** is checked transitively. A deterministic procedure
  cannot:
  - read clocks;
  - read uninitialized memory;
  - iterate a `Hash_Order_Table`;
  - call non-`strict` float code;
  - start jobs other than `for_parallel` with ordered merges.
- **`fixed32` and `fixed64`** for results that must match the Rust build
  or `web/semantic.js` exactly.
- **Randomness is a value.** `Rng` is passed in, never global.
- **Replay tests:** `#test replay "fixtures/icecream/*.trace"`.

## 14. The protocol is native

The OpenAgents Protocol keeps `.proto` files as the public source of
truth. Coax imports them, or emits them, and generates codecs at compile
time:

```coax
#import_proto "openagents/engine/v1/state.proto";

EntityState :: struct @proto("openagents.verse.presence.v1.EntityState")
                      @class(presence) @version 3 @max_bytes 4096 {
  entity:   string      @1 @limit(max_bytes = 64) @grammar(entity_id);
  position: v3          @2 @unit("m");
  stamp:    Stamp       @3;
  look:     AvatarLook? @4 @since 2;
  shade:    u32         @5 @[1, 2];   // versions 1–2 only; number and name stay reserved
}
```

- Field numbers, limits, data classes, sensitivity and units are the
  protocol's `openagents.options.v1` options, written as annotations.
- `@version`, `@since` and version ranges are the primer's automatic
  versioning. Reading an old record runs the generated migration.
- **Breaking changes fail the build.** The compiler checks each `@proto`
  type against the frozen descriptors in `proto/compatibility/`, as
  `buf breaking` does.
- **Bounded decoders** check every limit before allocating (R14), on
  every target.
- **NIP JSON.** `@wire`, `@json_key` and `@nullable` produce exact NIP
  JSON. Signed bytes are never re-serialized for verification.
- `Any`, raw pointers and `Hash_Order_Table` are refused inside `@proto`
  types.
- The same annotations drive saves, replication and hot-reload
  migration: one set of versions and one migration path.

## 15. GPU: shaders in the same language

```coax
Instance :: struct @gpu { model: m4; color: v4; }

node_vs :: (v: Vertex_In, @instance inst: Instance, @uniform frame: Frame_Uniforms) -> Vertex_Out
           shader vertex {
  out: Vertex_Out;
  out.position = frame.view_proj * inst.model * v4.{v.position.x, v.position.y, v.position.z, 1};
  out.color = inst.color;
  return out;
}

cull :: (@storage instances: [] Instance, @storage draws: *Indirect_Args) shader compute(64) { ... }
```

- **`shader` procedures** are a restricted subset:
  - no pointers except bound resources;
  - no recursion;
  - no allocation;
  - no `context`.
  
  They compile to **MSL** first for the native Metal path, then WGSL for
  WebGPU, then SPIR-V.
- **`@gpu` structs** get one layout valid on every target. The compiler
  reports the padding, and the CPU upload uses the same type, so instance
  buffers like `src/metal`'s cannot disagree with their shaders.
- **Metal is first class.** Argument buffers, indirect command buffers,
  memoryless targets, tile shaders and imageblocks are attributes. The
  compiler says which features a shader uses that a target lacks.
- **Shaders hot-reload** like code, and the pipeline is rebuilt between
  frames.

## 16. The compiler and hot reload

### 16.1 Speed targets

| Measure | Target |
| --- | --- |
| Front end (parse and typecheck) | 1,000,000 lines per second per core |
| Debug build, whole engine (~40k lines) | under 0.5 s |
| One-procedure change, debug, to running code | under 50 ms |
| Optimized release | minutes are fine |

How:

- no headers, no preprocessor, one parse per file;
- **the compiler is a server**: it holds the typed program in memory and
  invalidates per declaration, not per file;
- **own debug backends** for arm64, x86-64 and wasm, emitting machine code
  from a simple IR. Release builds go through LLVM, or through C output
  during the transition (§20);
- polymorphic instantiations cached across builds;
- `#run` in a bytecode interpreter.

### 16.2 Hot reload

- **`#persistent` globals and what they own** live in host memory, and
  survive a reload. Everything else is code.
- **Procedure IDs.** In debug builds, calls go through a per-procedure
  table, so a changed procedure replaces its entry between frames.
  Procedure values stored in persistent data are IDs, never raw
  addresses.
- **Layout migration.** If a `#persistent` struct changes, the compiler
  generates the migration from `@version` (or field names), applies it to
  live data, then resumes.
- **iOS devices** cannot load code at runtime. Device builds are fixed
  images, and the reload loop runs on a Mac host (performance stack item
  1). Coax does not try to get around code signing.

### 16.3 The build API

The build is a program, so a project root has `build.coax`:

```coax
#import "Build";

build :: () {
  w := Build.workspace("verse");
  w.options.output = .executable;
  w.options.targets = .[.macos_arm64, .ios_arm64, .wasm32];
  w.options.optimize = .debug;
  Build.add_files(w, "src/**/*.coax");
  Build.add_c_library(w, "third_party/stb", headers = "stb_image.h");
  Build.add_plugin(w, Autorun);
  Build.emit_xcode_target(w, "apps/openagents-ios");   // replaces project.yml source lists
}
#run build();
```

This replaces `CMakeLists.txt` and the source lists in
`apps/openagents-ios/project.yml` with one program (R13). Xcode and
Emscripten remain the linkers and packagers, at first.

## 17. Interop, modules, tooling and the standard library

### 17.1 Modules

- `#import "Basic";` brings a module's public names into scope.
- `Hash :: #import "Hash";` binds it to a name (Jai, `aoc2022 main.jai`
  end).
- `#load "file.coax";` adds a file to the current module.
- A module is a directory with `module.coax`. Everything is public unless
  it is under `#scope_file` or `#scope_module` (Jai), and `#scope_export`
  ends a private section.
- There are no cyclic imports between modules. Files within a module may
  refer to each other freely.
- Versions and fetching are not the language's job. Modules are vendored
  directories, as `third_party/` is today.

### 17.2 C, C++ and Swift

- **C both ways.** Coax calls C directly and exports `#export_c` entry
  points. `#import_c "stb_image.h"` reads a C header through libclang at
  compile time.
- **C++ through C shims.** Coax does not parse C++. metal-cpp, Jolt and our
  own C++ engine get an `extern "C"` layer, which is the shape
  `src/metal/home_app.h` already has.
- **Swift** calls Coax through the same C boundary a bridging header uses
  today.
- `#c_layout` structs pass between Coax and C++ with no copying.

### 17.3 Tooling

- `coax fmt`: the parser printing its tree. One form.
- **Diagnostics** carry a stable code, a primary span, an explanation and
  a machine-applicable fix where possible. A capture error says exactly
  which `read` or `write` to add.
- **The query server** answers over a local socket: a symbol's type, its
  uses, a job's effects, a struct's layout (`#layout`), the macros
  expanded at a site, and what changed since the last build.
- **Tests in the language**: `#test` blocks, replay tests and shader
  golden-image tests.
- **Debugging**: DWARF from the debug backends, so LLDB and Xcode work.

### 17.4 Standard library, first cut

| Module | Contents |
| --- | --- |
| `Basic` | `print`, `log`, `assert`, `New`, `free`, array procedures, `temp`, `get_time` |
| `String` | views, `split`, `trim`, `starts_with`, `String_Builder`, number parsing |
| `Table` | `Table($K/Hashable, $V)` in insertion order; `Hash_Order_Table` |
| `Sort` | stable and unstable sorts, `Heap` |
| `Math` | the built-in math types, `Strict_Math`, `Fixed` |
| `Simd` | lane types, `for_simd` support |
| `Jobs` | the scheduler, `for_parallel`, frame graph |
| `File`, `Process` | IO for tools and `#run` |
| `Proto` | codecs, NIP JSON, JCS |
| `Metal` | device, queues, argument buffers, through C shims over metal-cpp |
| `Build` | workspaces, options, plugins, messages |
| `Reflect` | `Type_Info`, `#derive` implementations |

---

# Part C. The plan

## 18. Worked example: the payment network's pulses

`src/paynet` moves up to 100,000 payment pulses a second along channels
on Metal. In Coax:

```coax
Pulse :: struct SOA(4) { t: f32 @hot; speed: f32 @hot; edge: u32; }
Edge  :: struct #derive(eq) { from, to: u32; length: f32; }

#persistent pulses: [..] Pulse;
#persistent edges:  [..] Edge;
#persistent nodes:  [..] v3;

advance_pulses :: (dt: f32) job [write pulses.t, read pulses.speed] deterministic strict {
  for_simd pulses { it.t = min(it.t + it.speed * dt, 1.0); }
}

Pulse_Instance :: struct @gpu { position: v3; glow: f32; }

collect_pulses :: (out: [] Pulse_Instance) job [read pulses, read edges, read nodes, write out] {
  for_parallel pulses grain 4096 {
    e := edges[it.edge];
    out[it_index] = .{position = lerp(nodes[e.from], nodes[e.to], it.t), glow = 1 - it.t};
  }
}

pulse_vs :: (@instance p: Pulse_Instance, @uniform f: Frame_Uniforms) -> Vertex_Out shader vertex { ... }

frame :: (dt: f32) {
  advance_pulses(dt);                  // writes pulses.t
  collect_pulses(frame_instances());   // reads pulses.t: scheduled after
  draw(pulse_pipeline, frame_instances());
  reset_temp();
}
```

The source shows every property we care about:

- the layout (`SOA(4)`, `@hot`);
- what each job touches;
- what must be deterministic;
- the layout the GPU shares;
- what survives a reload (`#persistent`).

## 19. What it costs

A production language like this is years of work for a small, strong
team. Jai has been in development since 2014. The parser is not the
expensive part. The expensive parts are:

- the backends;
- debug info and LLDB on Apple;
- Xcode, Metal and wasm integration;
- the standard library and tooling;
- keeping all of it working through every OS and Xcode release.

The start plan below spends that cost in stages. Each stage ships
something the engine uses, and each has a point where we can stop.

## 20. Start plan

The rule: **every phase produces code that runs in the shipping app**, and
nothing waits on the next phase. C output is the bridge. It goes through
Xcode and Emscripten exactly as today's C++ does, so there is no new
toolchain risk until we choose to take it.

### Phase 0. Freeze the core (about 2 weeks)

- Turn Part B into a grammar file (`coax/grammar.ebnf`) and a list
  of the 50 programs it must accept or reject (`coax/conformance/`).
- Translate three real subsystems to Coax on paper, without a compiler:
  - `src/paynet` pulses;
  - the ECS move pass in `src/ecs`;
  - one shader from `home.metal`.
  
  Every feature they don't use is cut from version 0.
- Decide the open questions in §22 that block the grammar.

Exit: the grammar parses the three translations with a throwaway
parser.

### Phase 1. `coax0`: a transpiler to C (about 6 weeks)

**Started 2026-10-27:** `coax/` in the openagents repo compiles and runs hello world, with
procedures, `int`/`bool`/`string`, control flow, checked `print` formats
and overflow traps ([coax/README.md](README.md)).

- Written in C-style C++ in `coax/` at the root of the openagents repo,
  with no new dependencies. Nothing of Coax lives in aethyr-cpp (owner,
  2026-10-27); the engine consumes the generated C.
- Input: declarations, structs, `SOA` and `SOA(4)`, procedures, multiple
  returns, `ifx`, `defer`, `for`, `#distinct`, optionals, `try`, unions,
  `#derive(eq, hash)`, `pure` operators, and `job` capture lists checked
  for effects.
- Output: one C file per module plus a header for the C boundary, which
  CMake and `project.yml` pick up like any other source.
- No `#run` yet. Generated tables come from a small `coax0` built-in list.

Exit: `src/paynet`'s pulse update is written in Coax, ships in the
OpenAgents iOS app, and its frame time matches the C++ version within 5%.

### Phase 2. Effects and determinism in production (about 4 weeks)

- The job scheduler (performance stack item 3) reads Coax effect lists.
- `strict` and `deterministic` are checked and emitted as per-function
  attributes plus `-ffp-contract=off` on the generated file.
- Replay tests run in `ctest` over the Ice Cream Man traces.

Exit: one deterministic simulation system runs in Coax and passes replay
tests at 1, 2, 4 and 8 workers.

### Phase 3. `#run`, reflection and the build program (about 8 weeks)

- A bytecode interpreter in `coax`, running `#run`, `Code`, `#insert` and
  `#expand`, and metaprogram plugins.
- `type_info` and `#derive` as library code.
- `build.coax` generates the CMake source list and `project.yml` sources.
  The old lists are deleted when it does.

Exit: the asset baker or the protocol codecs are generated by `#run`
instead of a separate tool.

### Phase 4. Protocol and GPU (about 8 weeks)

- `@proto` types, `#import_proto`, bounded decoders and the breaking-change
  check against `proto/compatibility/`.
- `shader` procedures to MSL, and `@gpu` layouts shared with the C output.

Exit: one presence type and one Metal pipeline are defined only in Coax.

### Phase 5. The compiler server and hot reload (about 8 weeks)

- `coax` becomes a long-running server with the query API.
- Hot reload on the Mac Metal host, with `#persistent` and migrations.

Exit: a one-procedure change reaches the running Mac host in under 200
ms, with state kept.

### Phase 6. Own backends (open-ended)

- A debug backend for arm64 Apple targets first, then wasm, then x86-64.
- C output stays the release path until a backend beats it.

Exit: the R1 targets in §16.1.

### Kill and pause criteria

- **After Phase 1:** stop if writing the pulse system in Coax was not
  faster for agents and people than C-style C++, or if agents produced
  more defects in it.
- **After Phase 3:** pause the language work if the build-program and
  `#run` gains could be had as a C++ code generator for less.
- **At any phase:** if Jai ships publicly under a license we can use,
  compare it to Coax on R1–R14, and move the Coax-only features (§4) into
  libraries and plugins on Jai if it covers enough.

### What changes in the C++ now

These apply from today, whether or not Phase 1 starts this month:

- Plain structs, C boundaries between subsystems, systems registered with
  their reads and writes, arenas instead of ad hoc allocation, and
  `strict` code isolated in its own files. Code written that way
  translates to Coax almost line for line.

## 21. Not planned

- As in Jai: no garbage collection, RAII, exceptions, inheritance,
  references, header files, preprocessor, or virtual machine in shipped
  code.
- Beyond Jai: no borrow checker, because capture-list effects cover the
  case we have. No runtime code loading on iOS. No package manager. No
  Unicode identifiers.

## 22. Open questions

1. **Inclusive ranges.** Jai's `0..n-1` is inclusive. Should Coax keep that
   for familiarity, or use only `0..<n` to remove off-by-one bugs?
   Decide in Phase 0.
2. **Shader language.** Is a Coax shader subset better than Slang (the
   performance stack's choice) with layouts shared through generated
   code? Phase 4 decides it with one pipeline written both ways.
3. **Effect granularity.** Field-level effects fit SOA data. Do
   pointer-heavy structures such as navmeshes and BVHs need explicit
   partitions?
4. **Where the compiler lives.** Decided 2026-10-27: `coax/` at the root
   of the openagents repo, with the spec (`coax/spec.md`) and the book
   (`coax/book/`). Does it move to its own repo later?
5. **Macro budgets.** Are 4 KB of AST nodes and 1 ms per use the right
   limits, or should they be per-module settings?
6. **Odin and Zig as hosts.** Either could host part of this as libraries
   plus code generation. Phase 1's kill criteria are the test of whether
   a language of our own earns its cost.
