# Generated frontend seeds

`lexer.c` and `lexer.h` are generated from `../lib/compiler/lexer.coax`.
`parser.c` and `parser.h` are generated from `../lib/compiler/parser.coax`.
`checker.c` and `checker.h` are generated from `../lib/compiler/checker.coax`.
These are the only lexer/parser/checker seeds. There is no authored C++ grammar/scanner/type-policy fallback and no
binary download, Python generator or dependency on an existing coax executable
in a normal build.

The native C compiler builds these seeds, then the first C++ frontend links it
with proto imports disabled. That compiler runs each module's checks and
zero-survivor mutation gate, emits fresh C/header pairs, and compares both with
their seeds. A mismatch fails the build; it never updates the checkpoint silently.
The final compiler links regenerated Coax code and must reproduce both its
lexer/parser/checker C and its combined compiler-module C before publication.

After an intentional frontend change, refresh with an already trusted compiler:

```sh
coax/refresh-lexer-seed.sh /absolute/path/to/trusted/coax
coax/refresh-parser-seed.sh /absolute/path/to/trusted/coax
coax/refresh-checker-seed.sh /absolute/path/to/trusted/coax
coax/build.sh test
```

Commit source and regenerated seed together. Each refresh command runs checks
and mutation testing before replacing the seed. A previous compiler must be
able to read the new source; grammar changes need a staged migration. The seed
is a trust input: byte identity does not establish its provenance or prevent a
malicious bootstrap compiler. The C++ evaluator/emitter, native tree bridge, C toolchain and ABI are still
trusted. Finite properties and mutation scores are not correctness proofs.

The lexer carries token spans and line/column locations in byte units. Source
is bounded at 2^31 - 2 bytes so one-based EOF locations fit the existing signed
32-bit parser ABI. Keywords and identifiers remain ASCII; strings/comments
retain arbitrary bytes. Existing recovery and literal tokenization are
preserved rather than tightened in this port.

Decimal conversion retains up to 800 significant digits and a sticky remainder
in fixed 128-word unsigned limbs, scales a rational value and rounds to
binary64, nearest ties to even. This exceeds the 768 significant digits needed
to distinguish binary64 decimal rounding boundaries; see the primary
[algorithm discussion](https://github.com/Alexhuszagh/rust-lexical/blob/main/lexical-parse-float/docs/Algorithm.md#other-optimizations).
No implementation was copied from that library. Native tests independently
compare against Python's float conversion and exact Decimal-generated
midpoints, including values longer than the retained significand. Typed f32
conversion continues to use the frontend's binary64 value plus its f32 hint.

Migration characterization also compared the Coax kernel with the previous
C++ lexer on the real compiler/app sources, every two-byte string and 10,000
random strings, including token bits, locations and diagnostic text. That is
compatibility evidence. Permanent checks and native oracle/fuzz tests exercise
the actual Coax implementation; neither form of evidence is a universal proof.

## Language parser

The parser receives lexer-produced tokens (including EOF, decoded strings,
raw spelling lengths and source-byte locations). `PS_caps` supplies bounded
node/message/import-request storage sizes. Node 0 is null. `PS_Tag` is the
fixed-width tree vocabulary; C++ asserts agreement with the existing AST,
allocates objects first, copies references/scalars next and list values last.
All source-backed strings retain their source/compile-arena owner.

`PS_parse` owns grammar, precedence, balanced procedure lookahead, check
bindings, label trimming and diagnostics. Its first failure freezes cursor and
diagnostic; partial storage is unspecified and must never be admitted. Active
recursive grammar entries are bounded at 128, with E0221 on exhaustion. Native
tests exercise actual C output at storage limits and on deep syntax, independently
of the evaluator's own recursion budget. `PS_imports` also owns source proto
import syntax, top-level/Metal admission, NUL/duplicate-prefix rejection and the
eight-import limit. The driver performs path resolution, descriptor reads and
bounded generated-source transfer only.

Normative grammar/tree tests, truncated-prefix and finite operator checks test
the actual implementation. Readable indexed-tree fixtures and first-error
fixtures characterize compatibility with the former parser, not correctness.
Migration evidence separately compared reconstructed ASTs/diagnostics on real
repository files and edited/truncated programs. Native tests add independently
constructed expression trees, ASan/UBSan of the Coax kernel and complete native
compiler bridge, and the real iOS app remains a separate consumer gate.

## Type checker

The checker consumes the trusted parser's indexed graph. `CK_caps` owns scratch
capacities; `CK_run` owns name/scope/type resolution, composite interning, LP64
layout, numeric literal folding/ranges, intrinsic calls, control flow, foreign
and Metal admission, and check/authority reachability. Encoded IEEE values and
four unsigned limbs preserve native nonfinite values and the full literal
arithmetic domain without relying on the evaluator's host numeric behavior.
A failed run never admits a partial typed graph. The native bridge only copies
records, owns storage and reconstructs native pointer/value lists, after all
references are populated. Enum ABI mappings are compile-time assertions.

Contextual type hints use `CK_Hint`; by-value and view layout requests use
`CK_Access`. Constant folding walks explicit bounded frames, with a 64-edge
limit independent of the evaluator's procedure-call stack.

Active semantic nesting is bounded at 256; E0381 reports storage/nesting and
native layout-size exhaustion. The graph is an internal parser contract, not
an untrusted serialized-input API. Numeric vector tests use independent exact
integer/rational expectations; typed-record fixtures characterize migration.
Native IEEE/layout/policy oracles and ASan/UBSan exercise actual emitted C and
the complete bridge. The compiler suite and clean iOS simulator build remain
separate consumer gates; mutation coverage is not a proof of soundness.
