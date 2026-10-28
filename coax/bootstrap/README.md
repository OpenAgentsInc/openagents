# Generated lexer seed

`lexer.c` and `lexer.h` are generated from `../lib/compiler/lexer.coax`.
They are the only lexer seed. There is no authored C++ scanner fallback and no
binary download, Python generator or dependency on an existing coax executable
in a normal build.

The native C compiler builds this seed, then the first C++ frontend links it
with proto imports disabled. That compiler runs the Coax lexer's checks and
zero-survivor mutation gate, emits fresh lexer C/header, and compares both with
the seed. A mismatch fails the build; it never updates the checkpoint silently.
The final compiler links regenerated Coax code and must reproduce both its
lexer C and its combined compiler-module C before publication.

After an intentional lexer change, refresh with an already trusted compiler:

```sh
coax/refresh-lexer-seed.sh /absolute/path/to/trusted/coax
coax/build.sh test
```

Commit source and regenerated seed together. The refresh command runs checks
and mutation testing before replacing the seed. A previous compiler must be
able to read the new source; grammar changes need a staged migration. The seed
is a trust input: byte identity does not establish its provenance or prevent a
malicious bootstrap compiler. The C++ frontend, C toolchain and ABI are still
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
