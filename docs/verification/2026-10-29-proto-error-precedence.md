# Generated protobuf error precedence (#5869)

Generated DTO errors are ordinal values, not bit flags. Previously a carried
LIMIT (2) combined with a fresh WIRE (1) became DUPLICATE (3). The generator
now returns a carried nonzero error before accessing that DTO's wire; otherwise
it returns the first decoder or later operation error unchanged. Unknown nonzero
u32 values are preserved. Nested set/append checks parent carried error, parent
partial validation, child carried error, child strict decoding, rewrite, then
final parent validation. It does not promise transactional output scratch after
final-validation failure.

This intentionally changes error results when a DTO already carries an error
and its bytes independently fail. Successful encodings, protobuf wire, DTO
layouts, and method names stay unchanged. A failed unknown-field read keeps
`done=false`: `verse/home.coax` treats `!done` as rejection. A failed iterator
initialization keeps `done=true`; `next` preserves its preexisting shape. These
are unverified views, not evidence of signature, identity or authorization.
Ordinary callers must still lend live, stable, physically valid storage.

The input importer SHA-256 is
`7a76ca6bfc29e93add3f83dcea17d3aedd80e9673ffc734eb66b153104aea987`;
the normal Python-free bootstrap completed and its provenance-verified compiler
SHA-256 is `5386852c6fe6d39d34f2fa278abcede4440aeda261b9a123bfc5199be4ace981`.
The changed importer passed 125/125 checks and default bootstrap mutation killed
1,723/1,723. The original generated-code check matrices were transformed from
their authored prior expectations by the specified rule; all historical
`import-baselines.json`, `presence-baselines.json` and
`message-limit-baselines.json` remain frozen. The separately named
`error-precedence-baselines.json` is a current characterization, not an
independent correctness oracle.

The actual generated API tests cover seven ordinal errors plus an unknown
sentinel, malformed and valid input, absent and required values, scalar/string/
bytes/child getters, repeated initialization and next state, oneof selection,
unknown result shapes and parent/child nested write precedence and canaries.
Home, Nostr event and Block FI descriptors have independent direct checks.
The original compiler fails the carried-error matrix and the native guard-page
control; it passes fresh-error controls. The fixed candidate passes them.

Checked/release generated C was tested with an inaccessible native guard page
under ASan/UBSan, a Wasm out-of-bounds address, and an actual iOS Simulator
process. Valid malformed-wire controls still return WIRE when no error is
carried. These tests establish early return for the exercised calls, not general
native pointer validation or lifetime safety. The tested checked/release emitted
C hashes were `9db174a2269108d5ae883903a95ac6eb7f211f5797a7c83e37bbd0fbe7e70577`
and `d717aedc3e40bc59541d3f9e54e5a6ca4c4a2c688e44013a7d9f302a0dabee53`;
the certified compiler emits byte-identical code.

`error_precedence_compare.py` used independently decoded protoc descriptors to
count every field, nested writer and actual oneof, checked the frozen old output
hash, transformed only the five reviewed error branches, then compared every
byte against the new generated declarations across 12 profiles. Corrupted-stage
negative controls fail. The earlier `presence_compare.py` still verifies its
historical transformation and four structural negative controls, then applies
this separately specified transformation. The certified compiler matches all
12 new characterization fingerprints. Pinned protoc 29.3 successfully
reproduced and independently round-tripped `types.pb`.

The generated-code importer suite passed 20 of 22 tests on its first candidate
run; the two remaining cases failed because the final bootstrap artifact and
pinned protoc were not yet supplied. Each passed separately once supplied,
using the same candidate compiler and its fixed-point generated importer C. The
Home consumer passed 8/8 runtime/protoc checks and ASan/UBSan; the Voice wire
consumer passed 7/7 checks. The actual OpenAgents iOS Debug simulator app build
passed for arm64 and x86_64. Its fixed-point candidate and the provenance-
verified compiler emit identical Home and Voice C for the same source paths.
A separately scoped independent review found no concrete defect in the five
templates or the check matrix under the stated live/stable borrowed-view
premises; it does not replace target or mutation gates.

The changed importer also passed the stricter full-reference mutation run:
16 shards merged into 1,723/1,723 killed with no survivors or exemptions.
Each shard used stage0 SHA-256
`bbd59eadc6c7429a40df69a91927e1501f7bb80c38f24454873fefebbdfb7765`
and full scheduling, with no checks skipped by selective scheduling. The
merged summary SHA-256 is
`7f69794764b80394f873589db72e19217f1158cc5cf3a474ff587d1fa05a5797`.
The direct core matrix source SHA-256 is
`c0ac75bc7e7a588c1d6f9cf5455b5db0e695df52fc4cff8547a9d002bbb0cf72`.

The complete bootstrap and target tests above used the compiler inputs based
on `5d8c693b98`. While this work was in progress, `c98e206bb6` added a Metal
compute backend and changed compiler source files. No passing gate was rerun
for that unrelated commit. After rebasing, a comparison compiler built from the
new C++ source and the same modified importer regenerated its importer C at a
fixed point (comparison binary SHA-256
`c09cc0e807f2d6ef3b550586faa8d5cb59979d47a9f0c521118ac5a211673566`). It
emitted byte-identical checked/release precedence fixture C and ABI headers,
Home, light and Voice wire C, and all 12 tested descriptor-declaration hashes.
That comparison binary is not a full bootstrap or a substitute for the
provenance-qualified tested compiler identified above.
