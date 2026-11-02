# Sixteen-step borrow-origin foundation

This qualifies the supplied-fact `borrow-summary.coax` kernel and its native
ABI after extending complete logical paths from eight to sixteen steps.
It does not qualify the compiler's resolver, retained-effect policy, whole
desktop application or lifetime safety by itself.

Only `BS_MAX_PATH` and its inline storage capacity change in production.
Every step remains represented; there is no truncation, local-store skip or
manual fallback. Set and binding capacities remain 64, the pool stays 256 and
candidate expansion work stays 128. The compiler's separate per-procedure
effect capacity is outside this foundation's scope.

- **29 Coax checks pass**: depth 16 admitted, 17 refused; all newly admitted
  slots 8–15 retain kind/index identity through canonical union and exact
  substitution; inline rebasing preserves the final slot; a shorter formal
  prefix does not match; nonzero inactive suffixes refuse before unknown facts.
- **Native O0, O2 and fatal ASan/UBSan pass**: 166 API cases/calls with 59
  refusals in each profile. Independent literal C records compare all sixteen
  logical slots. Short/refused output and untouched tails remain byte-exact;
  unspecified padding inside successful records is not compared.
- The LP64 ABI is `BS_Path` 264 bytes, `BS_Origin` 296 bytes and `BS_Binding`
  336 bytes, each aligned to eight. View and result layouts are unchanged.
- The existing C oracle's stale expectation that variant step kind 4/index 1
  was invalid is corrected to match already-supported `BS_VARIANT` behavior,
  with explicit variant 63/64 and invalid-kind controls. This changes no
  production variant semantics.

No full mutation, whole-model or whole-compiler gate was run for this record.
LeakSanitizer is disabled on this macOS host; leak checking is not claimed.
The separate checker and desktop qualification belongs to the parent record.

[record.json](record.json) identifies actual source/compiler/native producers.
Source and generated C/header snapshots use deterministic gzip, with both
uncompressed and encoded identities recorded. [manifest.json](manifest.json)
pins every committed artifact. No executable or model weights are included.

```shell
coax/bin/coax test coax/lib/compiler/borrow-summary.coax --library
COAX_TEST_COMPILER=/path/to/coax python3 coax/tests/borrow_summary_native.py
```
