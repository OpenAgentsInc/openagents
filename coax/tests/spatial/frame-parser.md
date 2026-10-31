# Literal frame metadata parser

This profile adds syntax and raw metadata transport for declarations such as:

```jai
World :: struct @frame(handedness=right, convention=cartesian,
                       continuity=continuous, min_precision=f64) {}
```

All four names are required exactly once in any order. Values are literal
words, with no lookup, expression, generic parameter or string evaluation.
The parser appends a separate flag and four integer fields to `PS_Node`:
handedness `1/-1`; convention `1/2/3` for cartesian/body/optical;
continuity `1/2` for continuous/jumping; minimum representation `32/64`.
Unit metadata retains its original independent fields and parser body.
A second physical annotation is refused with E0832; existing syntax errors
retain their first code/location.

Shape and spatial admission belong to the checker. In particular, successful
parsing of a generic/nonempty or left-handed optical declaration does not
admit it as a frame. The syntax fixtures explicitly exercise that separation.
A false parser result can retain private diagnostic nodes; callers must use
`ok`, rather than treating a nonzero internal root as success.

`frame-parser-qualification.json` binds the source, generated seeds and proofs.
It records 202 passing check groups and 102/102 killed changed-procedure
mutants, with every original check retained. The native source fixture uses
the actual compiled lexer and parser for 99 cases. Independent direct-token
checks pass 146 cases and 2,024 assertions in each of debug, release and
ASan/UBSan modes. The source fixture also checks ctypes/native C layouts.
These are parser proofs; the whole compiler's reconstruction and semantic
checks are recorded by the integration owner separately.

Reproduce only the changed profile with a qualified compiler, using fresh
output directories to preserve prior receipts:

```shell
COAX=/path/to/qualified-coax
"$COAX" test coax/lib/compiler/parser.coax
python3 coax/tests/frame_parser_scope.py /tmp/new-frame-parser-scope
"$COAX" mutate /tmp/new-frame-parser-scope/root.coax --root-only
COAX_TEST_COMPILER="$COAX" python3 coax/tests/parser.py Parser.test_frame_source_metadata_and_negative_fixtures
python3 coax/tests/spatial/frame_parser_native.py --compiler "$COAX" --output /tmp/new-frame-parser-native
```

The generated parser seeds use the normal C/header emission recipe from the
`coax/` directory. No compiler is installed or cached by these tools. Do not
repeat a passing unchanged profile merely to confirm it.
