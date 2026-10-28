# JSON decoder checks: 2026-10-28

The shipped decoder/writer's full reached mutation closure now kills **971/971
mutants**, with no equivalence annotations or source-selection filter. Its19
checks include256 byte-classification cases, reused-state reset, inclusive1MiB
input/64KiB string limits, exact token/error ranges, independently encoded UTF-8
boundaries, surrogate failures and bounded writer failure prefixes.

Checks came first. Remaining redundant branches were then simplified:

- UTF-8 validation checks the standard lead/continuation-byte windows directly;
  it no longer reconstructs code points solely to test the same constraints.
- One raw-string limit check covers the scan instead of repeating it after it.
- Punctuation dispatch uses a bounded self-call after consuming a comma/colon,
  eliminating an unreachable loop tail. Each call can recurse once: it changes
  the state to a value/key state, which returns a token or refusal directly.
- `json_valid` returns from the actual terminal token; generic integer parsing
  uses a bounded accumulation guard and still admits inclusive±2^53.
- Error comparisons use the actual -1 sentinel and failed control-byte appends
  return before an invalid cursor could be used. Defined partial prefixes match
  the reference; callers must still discard a writer's failed result.

The decoder remains separate from the domain authority policy. It establishes
syntax/decoding observations, never signatures, JWT claims, permissions or grants.
`json-policy.coax` owns the selected admission decisions. There are no compiler
or source-scope mutations in this work.

## Independent reference and sanitizer evidence

`coax/tests/nostr/json_parser.py` freezes the exact runtime prefix at
`a165c9bc26a5aeb95a371a137583425ec6948398`, with its SHA-256 recorded in the
script. It emits and runs both actual implementations. **42328 cases per mode**
agree under native ASan/UBSan on token kinds, cursor/depth/state/ranges/error
positions, decoded string bytes, generic integer results and bounded partial
writer output. Modes are emitted checked and wrapping; both native compilations
use-O1. This is a compatibility comparison, supplemented by independent Python
UTF-8 fixtures, JSON/pair/number oracle cases and the pinned JSONTestSuite.

The separate admission oracle passes3363 cases per mode, the real relay corpus
and trapping-allocator check pass, and the affected WebAssembly site build is
checked. Native results are not physical-phone performance or a crypto audit.
Python is optional testing tooling, with no ordinary build/runtime dependency.

```sh
coax/bin/coax test coax/lib/nostr/json.coax --library
coax/bin/coax mutate coax/lib/nostr/json.coax --library
python3 coax/tests/nostr/json_parser.py
python3 coax/tests/nostr/json_admission.py
coax/bin/coax test coax/lib/nostr/adapters/json-admission.coax --library
coax/bin/coax test coax/tests/nostr/json_corpus.coax --steps 200000000
python3 coax/tests/nostr.py CoaxJsonTests
COAXIAL_MODE=release apps/openagents-web/build.sh
```

Ordinary product consumer builds, their own heavy integration checks and target
stack budgets remain distinct gates. Adding an authority marker to a widely
imported decoder would currently trigger every consumer check at the default
interpreter budget; it is not a substitute for domain policy or explicit
consumer tests. This change retains the decoder's existing classification and
runs its full mutation closure explicitly.
