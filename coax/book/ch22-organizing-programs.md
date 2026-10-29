# 22. Organizing a larger program

<!-- coax-release:current:start -->
Current compiler: **coax0 0.19.0**. Status inventory schema: **1**.
<!-- coax-release:current:end -->
The small programs in the first half of this book fit in one file. A
world, a compiler and an app do not. They need boundaries: who owns a
type, what crosses a boundary, and which parts a build is allowed to
replace together.

The current compiler provides source-unit composition with `#load`. You can share
actual declarations and keep diagnostics and checks tied to their physical
files. General directory modules, visibility and package management remain
planned. A source graph today has one declaration namespace.

## Load another source unit

Run this example from the repository root:

```shell
coax/bin/coax run coax/book/examples/ch22_sources.coax
```

It prints `42`. The entry file imports printing and loads a source unit:

<!-- checked-source: examples/ch22_sources.coax -->
```jai
#import "Basic";
#load "lib/ch22_numbers.coax";
#test "a loaded type has one identity" { #assert cells_total(Cell.{value=20},Cell.{value=22})==42; }
main :: () { print("%\n",cells_total(Cell.{value=20},Cell.{value=22})); }
```

That unit loads a shared type next to itself:

<!-- checked-source: examples/lib/ch22_numbers.coax -->
```jai
#load "ch22_types.coax";
#test "negative and positive cells" { #assert cells_total(Cell.{value= -2},Cell.{value=3})==1; }
cells_total :: (a: Cell, b: Cell) -> int { return a.value+b.value; }
```

The final file declares `Cell`. It is the same declaration in both files,
not two structurally similar types that happen to have the same spelling.
Procedure and type names can refer forward across the graph. Duplicate
names still fail; loading a second unrelated `Cell` is not namespacing.

Paths are literal, nonempty, NUL-free strings, resolved beside the file
containing the directive. Absolute paths are also explicit inputs. There
is no search path, environment expansion, network download or package
version selection. A source load is a file-level declaration, not an
interpreted operation that `#run` can perform.

## One graph, with explicit limits

The compiler reads the entry unit first, then visits its loads depth first
in written order. It parses the composed program after loading the graph.
Declarations resolve across units regardless of their textual position.
Checks retain that deterministic source-unit order too.

Canonical file paths establish source identity. A completed dependency
loads once when another load resolves to that same canonical path, including
dot paths and symlinks. Distinct hard-link names remain distinct units. Loading an active ancestor is a cycle and reports E0602 at
the directive that closes it. This is source identity, not a guarantee
that a filesystem cannot change during a build.

Each compilation admits at most 64 distinct units, 32 active units, 16 MiB
of source bytes and 512 load directives per unit. Budget exhaustion is
E0603. Reads are checked against the byte budget before allocation and as
the file is read. These bounds do not make the whole compiler a hostile
process sandbox: token, syntax-tree and native toolchain work have their
own costs.

Every physical unit is checked with the Coax grammar before composition.
It must contain complete declarations and balance its own delimiters within
depth 128. A brace or semicolon in another file cannot complete its procedure
or constant. Comments and string
contents never become directives. Unreadable or nonregular source inputs
report E0601; invalid load syntax is E0600; unbalanced composed units are
E0604.

## Authority belongs to the authored unit

An `#authority` directive applies to procedures authored in that physical
file. Loading an unmarked utility does not silently mark the utility, and
loading an authority file does not discard its requirements. Checks from
another unit can exercise its procedures.

If any unit is authoritative, ordinary compilation runs the graph's checks
and verifies actual execution of its authority procedures. Dead references
and a folded `#run` still do not satisfy execution coverage. Tests remain
pure; moving a test to another file does not permit native effects in it.

`coax test entry.coax` runs checks throughout the graph. Add `--library`
when a source unit has no program `main` or native exports. `coax mutate`
mutates reachable code, including dependency helpers reached by the checks;
its unchecked-procedure gate covers the entry file and authority units.
An unused non-authority dependency need not be covered by its importer.
For a reusable dependency, run its own checks and mutation gate separately.

This makes Chapter 26's composition possible: a protocol adapter can load
a pure authority policy without requiring checks to execute every unused
accessor in a generated descriptor closure. Importing a descriptor directly
inside the authority unit still places its generated declarations under
that unit's obligations. Do not remove the directive to avoid those checks.

These are source-level verification rules, not runtime permissions. An
author can still edit the files and directives. Chapter 25 explains the
review and trust boundary.

## Inspect the inputs and failures

```shell
coax/bin/coax deps coax/book/examples/ch22_sources.coax
```

`deps` prints JSON with `schema_version: 1`, an ordered `sources` array of
canonical source paths and a `descriptors` array of resolved descriptor
inputs. It checks the program first and exits unsuccessfully for missing
inputs, cycles or failed authority checks. Descriptor paths are explicit
inputs; no guessed placeholders are emitted.

This is an input inventory, not a lockfile or cache key. It does not hash
contents or enumerate native compiler, SDK, linker or environment inputs.
A reproducible build must record those separately. `#load` and `deps` need
no Python, external preprocessor or `protoc` process.

Parser, checker and evaluator diagnostics point at the original physical
file and line. Mutation reports do too. Native traps in composed C output
map their joined source lines back to the original unit. Debugger stepping
still refers to generated C: this is not general Coax-to-debugger source
mapping.

A protobuf import in a loaded file resolves its descriptor beside that
file. An error in generated declarations points to that import directive.
Prefixes still share the graph's namespace; `#load` does not make duplicate
prefixes into separate protocol modules.

## See it in actual builds

The compiler's source graph is now
[compiler.coax](https://github.com/OpenAgentsInc/openagents/blob/main/coax/lib/compiler/compiler.coax).
The bootstrap compiles it, then requires the final compiler to regenerate
the same C. The loader itself is authored in checked Coax, with an explicit
C/header seed and a zero-survivor mutation gate. Native code reads files,
resolves filesystem identities and transports storage; it does not provide
a second load-syntax parser.

The iOS engine's `hud.coax` loads `hud/types.coax`, keeping its native packet
identity in a shared declaration. The capstone loads the same checked
request policy used by Chapter 23. The frontend book example also uses a
source graph instead of shell concatenation.

## Keep source, ABI and protocol identity distinct

A Coax type name identifies a declaration in a compiled source graph. A C
symbol and header describe a native ABI. A protobuf package, message and
field number identify interchange across builds. These are different
contracts.

Independently compiling two `Vec3` declarations still does not make their
native headers compatible. Use descriptive prefixes for exported structs
and procedures, preserve generated layout assertions, and inspect the
actual bridging header when composing independent libraries. Shared source
solves duplicate declarations within a graph; stable cross-library ABI
identity and module visibility are separate work.

Keep pure decisions in Coax, representation adapters at protocol
boundaries, and lifecycle/effects in the host. A useful API documents what
it borrows, copies or allocates, when a result expires, its work and space
bounds, failure values and deterministic guarantees.

`Basic` remains the only named built-in module. Math intrinsics, arrays,
views and generated codecs are available; general collection, allocator,
file, network and concurrency standard-library modules are not. Internal
`coax/lib/` folders are authored implementation sources, not promised public
package APIs. Build the next library from a real consumer and a checked
contract rather than a name in the specification.
