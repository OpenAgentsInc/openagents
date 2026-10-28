# Appendix C. Coax for Jai programmers

Coax starts from Jai's design. If you know Jai, most of Coax will look
familiar, and the differences are deliberate. Jai syntax here follows public
Jai sources, such as Forrest Smith's 2022 Advent of Code code.

## Same idea, same spelling

| Jai | Coax |
| --- | --- |
| `x := 1;` `x: int = 1;` `X :: 1;` `buf: [128] u8 = ---;` | Same |
| `f :: (a: int) -> int { ... }` | Same |
| `#import "Basic";` | Same (only `Basic` exists in `coax0`) |
| `#run f(16)` | Same, but sealed: no output or host I/O (Chapter 9) |
| `*x` address-of, `p.*` dereference | Same as current Jai (older Jai wrote `<<p`) |
| `[N] T`, `[] T`, `[..] T` | Same three array forms |
| `string` as a counted byte view | Same |
| `defer` | Same, plus it runs on `try` propagation |
| `ifx c then a else b` | Same (works in `coax0`) |
| `$T`, `$N`, `#bake_arguments` | Same ideas, with mandatory instance reporting |
| `using`, `#as` | Same split: `using` is naming, `#as` a declared conversion |
| `#scope_file`, `#scope_module` | Same |
| Implicit `context`, `push_context` | Same idea, plus allocation as a tracked effect |
| `Code`, `#insert`, `#expand` | Same idea, with hygiene, provenance and budgets |
| Metaprogram plugins with compiler messages | Same idea, typed messages and capabilities |

## Different on purpose

| Jai | Coax | Why |
| --- | --- | --- |
| `union` overlays storage | `union` is tagged and `switch` is exhaustive | The safe form should be the default |
| Bounds and conversion checks; overflow not generally checked | Overflow traps in development, wraps in release; compile-time overflow is always an error | Agents and people both write overflow bugs |
| `#specified` enum values are opt-in | Explicit values required where an enum crosses a boundary | Saved data must not change meaning on reorder |
| Equality and hash found by name; `#poke_name` to make them visible | `#derive(eq, hash)` and named constraints | Errors at the use site, with a fix |
| `$T/interface` constraints available | Constraints are the normal way to write generics | A generic's signature should say what it needs |
| Macro backticks reach into the caller | Caller access listed in the macro's signature | Reading a call site should show what it can change |
| `#run` can do anything, including I/O | `#run` is pure; I/O needs declared build capabilities | Same source, same constants; safe editor analysis |
| `xx` autocasts to the context's type | Explicit conversions; `xx` only between integer widths | No hidden lossy conversions |
| No shader language | Shared `@gpu` layouts; shader backend decided by experiment | The CPU/GPU struct mismatch is a real bug class |
| Hash tables iterate in hash order | Insertion order by default; hash order is opt-in | Replays and determinism |
| No published float model | `strict` and `deterministic`, checked transitively | Bit-identical simulation |
| Threads and mutexes | Jobs with read/write effects, scheduled from a frame graph | Parallelism with a contract |
| Shadowing allowed | No shadowing | One name, one meaning per procedure |
| Custom x64 backend and LLVM | C output first, own backends only when measured | Ship through Xcode and Emscripten from day one |

## Not in Coax

`Any` in persistent or protocol data, raw memory images as save files,
runtime code loading on iOS, a package manager inside `#import`, and
`#modify` (unless ordinary constraints prove insufficient).

## What Coax owes Jai

The central idea, that the compiler's knowledge of the program is
infrastructure the program should be able to use, is Jai's. So are the
declaration syntax, compile-time execution, the context, the build as a
program, and the conviction that a large program should rebuild in
seconds. Coax exists because we can't use Jai, not because we think we
can do without its ideas.
