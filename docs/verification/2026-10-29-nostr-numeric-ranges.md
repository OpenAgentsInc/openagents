# Nostr numeric range profile (#ef04)

The new `nostr.event.v1alpha2` descriptor opts its four kind fields into
`openagents.options.v1alpha2.max_uint32 = 65535`. This is a DTO constraint;
Nostr event identity, signatures and authorization require their existing
separate validation. Existing `v1alpha1` descriptors and their semantics remain
unchanged, and legacy aggregates explicitly exclude the new option and profile.
The old importer refuses the unknown policy. A combined version graph changes
ambiguous generated names to their package-qualified forms.

The maximum is inclusive and its presence is explicit: zero is a real bound,
while absence leaves a uint32 unconstrained. It applies only to uint32 fields,
including packed and unpacked repeated elements. Malformed, duplicate,
misplaced, incompatible and unknown options fail closed. Checks reject values
before a successful view is returned; setters can modify caller-provided
scratch output before reporting a refusal, as documented in `coax/proto.md`.

## Independent review

A separate reviewer examined importer SHA-256
`34aac6d7db8ee24297a8b828360669f264bcd505e32b90e8824462ada3a9d84b`,
event schema `7afc66668a7a670ae1aafa5e0e403712ec5b4a7aa159825022a726eb38253e9b`,
and option schema `27a33a292e3154523f1a2ae45a456f6b8a6c8515e2f2c649aa32ebc6dde0ae13`.
The review found no concrete defect in option presence, field/type admission,
packed/unpacked constraints or frozen aggregate exclusions, assuming the
existing descriptor and valid borrowed-view premises. This is source review,
not proof of event authorization or a substitute for runtime tests.

After that review, mutation testing found a missing check of the exact
`UINT32_MAX` option boundary. A direct Coax assertion was added; the importer
production code was unchanged. The final source SHA-256 is
`49f0f70af48e9037aa0929c36748294a1d25a1e68d8ba74a90adb9d678fc16da`.

## Focused validation

- Coax importer: 128 checks passed. The full reference mutation run used 16
  shards and killed all 1,751 mutants, with no waivers; the earlier missing
  boundary was the only survivor before the added test.
- The pinned protoc 29.3 interoperability and existing importer regression
  suite passed 22 tests, including unchanged descriptor and generated
  declaration hashes for the 12 frozen profiles. A legacy importer rejects the
  new option. A combined v1alpha1/v1alpha2 graph generates distinct
  package-qualified names.
- Focused schema and generic policy fixtures exercised explicit zero,
  65535/65536, maximum uint32, missing and required fields, duplicate,
  malformed, noncanonical, packed, unpacked, mixed and nested inputs. Their
  generated C passed checked and release native ASan/UBSan, Wasm and arm64 iOS
  Simulator probes. The Home protobuf runtime passed eight tests.

The target and regression probes above used candidate compiler SHA-256
`1839d832bcecff5be6663e7c264a03a82322bff9df4c51ccbf3959fd9eb5eef0`.
The subsequent mutation test added only an importer check; final compiler and
app-build provenance must be recorded separately.

The corrected Python-free bootstrap completed on base
`97938eb3b9f7be11b9d44670814f4152ba98c0d6` with four mutation workers and
no surviving mutants. Its verified compiler input fingerprint was
`730b57dd00b0a88c963ef9102005fdc81433f7306bf254a6408ca292110a5058` and
binary SHA-256 was
`59d439322ef2b4fe88d8a7340082dfa3c7ed006baeaf4b01048a1152e61e92dc`.
The certified compiler's checked/release C and headers for the two target
fixtures, Home, light and voice-wire match the tested candidate except for the
expected importer source hash in generated comments; no executable or ABI
text differs. Thus the unchanged target runs apply without repeating them after
the test-only importer edit.

The full OpenAgents iOS Debug simulator build also succeeded with the certified
compiler. Main subsequently contained the unrelated Metal emitter change
`5695c01464`. A comparison-only compiler built with that emitter produced the
same checked/release target and Home/light/voice-wire C and headers, except for
the same importer-source hash comment; it was not presented as a separately
certified build. The shipped app and target checks above therefore identify the
base graph actually tested rather than implying a new full bootstrap on an
unrelated remote commit.

The merged full-reference mutation report has SHA-256
`ac0a7101620aead65b974eb14eb38382b3557432a4b8e2412df85f24135e552c`;
the constrained descriptor has SHA-256
`f8ae8c1e72338d8ff15a9b5a2edbadadfa72db3b52d08425fc03ef61a3bd9aba`.
After integrating the independent MP profile and exclusion, the targeted
`scripts/proto.sh check` passed again with protoc 29.3 and all 18 Nostr schema
pins, keeping the legacy aggregate bytes intact.
