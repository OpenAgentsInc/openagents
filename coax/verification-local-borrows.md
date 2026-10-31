# Local-array return diagnostics

This first R3 slice adds E0383 in the Coax checker. A returned view/string that
directly borrows a local fixed array is rejected, including simple local
alias declarations/assignments, further slicing, casts, either `ifx` arm and
inline fixed-array subobjects of local arrays/structs. A local alias retains
its may-borrow flag after reassignment. Unreachable assignments and later safe
overwrites can therefore produce conservative rejections.

Borrowed parameters and their aliases/subranges remain returnable; returning
a fixed array value copies it out. No owning-value syntax, allocation,
implicit retain, deep copy or destruction rule is introduced. View/string
descriptors still borrow storage whose owner must outlive all uses.

This is not general lifetime safety. The checker does not summarize calls,
track borrows stored inside arrays/structs, diagnose stores into caller memory
or retention by a callee, compute loop dataflow fixed points, or enforce
shorter nested-block storage lifetimes. For example, returning `identity(a[..])`
and returning a struct containing `a[..]` still require manual review.
Alias propagation around a loop back edge can also evade this local pass.
Returning a borrowed parameter does not certify that its caller supplied
valid storage or will honor a foreign API's lifetime contract.

Verification uses check-first Coax typed-graph properties, the checker authority
mutation gate, parser-to-emitted-checker fixtures, native borrowed-return ABI
execution under ASan/UBSan, focused pass/fail compiler fixtures and the normal
Python-free seed regeneration/bootstrap gates. The initial new property failed
on the existing checker for a direct local-array return before implementation.
The generated seed is refreshed with the existing validated 0.13 compiler;
the native frontend remains a graph/storage transport with no second lifetime
policy. Finite checks and reproducible seeds do not prove semantic soundness.

The trusted refresh ran on arm64 macOS with Apple clang 21.0.0 and passed
553 checks and 3,762/3,762 killed mutants, with no survivors. Its existing
0.13 compiler SHA-256 was
`2d0d51d2681cd60d508d754130ab24f905c51fcc95789ec93a571945bcded685`.
The checker source Git blob is `20051f4ff25a1e3e45348d313beb717528dcdcdf`;
the generated C/header blobs are `1c4274e5db12b393dfbc5a5a140d668c756b13a0`
and `c9e32c6bd3cfbe5d9d13ea9a16ff4ddcc46d7473`, respectively.

Generated LP64 layout assertions show checker-local scratch records growing
from 64 to 72 bytes per slot. The target view/string ABI is unchanged; this
diagnostic adds no target allocation or retain operation.

The normal `coax/build.sh` bootstrap passed seed comparisons, module gates,
combined compiler regeneration and final frontend seed regeneration. The final
compiler SHA-256 is
`3746fef915ddd452bab14758bd8eead5230f2efe586bf669565bfc6cb7028edd`.
Focused final checks passed: `tests/checker.py` (six tests, including 22 new
borrow source vectors and the emitted checker under ASan/UBSan),
`tests/native.py NativeTests.test_borrowed_return_storage_under_sanitizers`,
the two error-table checks in `tests/tools.py`, the new pass fixture in debug
and release, all six negative fixture modes, and JSON E0383 diagnostics.
Book navigation/link checks passed. No unrelated product suite was rerun.

The valid `tests/pass/borrow_returns.coax` fixture emits byte-identical checked
C with the trusted and final compilers: 11,288 bytes, SHA-256
`63d6a58f99986b344c98f03c0dc7db7ba5dc40db1529641410d6b14d0246bc01`.
