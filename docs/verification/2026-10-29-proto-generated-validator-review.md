# Generated protobuf validator evidence

The candidate consolidates per-message scalar getters through a bounded wire
search, tracks singular presence and real oneof membership with booleans, and
counts repeated values only when a descriptor requires presence or a count
limit. Strict decoding and partial construction use explicit shared policy
functions; canonical-varint policy remains separate. Descriptor admission
still rejects `max_count` on a singular field.

A getter first performs the existing full message validation. The shared
search alone is not a complete validator: it can stop at a matching field
before malformed trailing bytes. A missing or failed search returns an empty
field with no presence; an error remains an error. Input and returned views
remain borrowed and require live, stable storage. Length checks cannot prove
native pointer provenance. These changes confer no signature or identity
authority.

The original `import-baselines.json` and its historical hashes remain frozen.
`presence-baselines.json` characterizes the new output. The separate
`presence_compare.py` decodes the pinned descriptors using protoc, validates
the old declaration fingerprints, and applies a field-by-field transform to
the old output; every byte of the resulting declarations matched the new
output in 12 profiles. Four negative controls cover a wrong field, a missing
packed increment, duplicate-check ordering and a changed wire constraint.
This is bounded evidence for these profiles, not a proof for arbitrary schemas.
The test fixtures cover required and bounded repeated fields, empty packed
fields, real and synthetic oneofs, nested required fields, malformed duplicates
and partial construction; protoc 36.2 reproduced both descriptors and an
encoded case. The 18 policy cases passed checked/release native and Wasm.

Independent read-only source review covered importer SHA-256
`4f7627cf70d5c8c4471ade4358510932e0d087f7da82c9cf9a061895a8622b37`,
wire SHA-256 `b558aaef83f10e0984d94fa7f57811ca2e63edd98147e77080360410e1e59b3b`,
comparison script SHA-256 `9fdde39b3f0b6d4b5a8d3017129094acf634cb377e82eaace59947566b53acc7`
and policy fixture runner SHA-256 `74c90f96d7c039322e1e1a8e1c23f7203b09a6b51ff5a462c8195a79235a8b91`.
The initial fixture runner was reviewed at its prior hash, and a separate review covered the subsequently added generated-name collision case. The reviewer identified no concrete semantic or boundary blocker under the
stable borrowed-view premise, did not rerun tests and did not grant a security
or target approval.

The wire core killed 401/401 mutations and the importer killed 1,720/1,720;
the complete native importer suite passed, with its optional protoc oracle
covered separately above. The checked/release event and identity experiments
passed protoc, native, Wasm, ASan/UBSan and iOS Simulator fixtures. Their full
reachable mutation gates and the Python-free bootstrap remain separate gates;
this record does not claim either has completed.
The actual OpenAgents iOS Debug simulator app also built successfully in the
isolated worktree with this candidate. This was a local build, not a TestFlight
upload.
The full consumer mutation gates remain open.

The first full FI consumer mutation run completed with 141 survivors out of
3,151 mutations, so it did not pass and the generated-consumer task remains
open. Additional authored checks now exercise each claims field's duplicate,
wire-type, budget, length and text boundaries, nested depth and error
propagation; these checks pass, but a later full mutation run is still needed.

The complete Python-free compiler bootstrap subsequently passed with this
importer and wire source. The additional FI assertions also pass, but that does
not replace the pending full consumer mutation rerun.

After adding direct required-field, nested-depth, noncanonical and forged-error
witnesses, the complete identity projection mutation run killed 3,159/3,159
mutants with no equivalence waivers. The root projection source for that run
was SHA-256 `fa887723059edbb2d4632f0b14aecbefab83a5a276b3638ba04d7fee71bce0fd`;
this is an uncommitted experimental unverified consumer and not a signature decision.
A full reference-mode identity run also killed 3,159/3,159 mutants; its
retained log SHA-256 is
`e1ac49c6b6e9992dd69cd79c793a2e209de3c3609e1de77048342c802435bb8f`.
The event projection gate remains open, so this evidence does not close the
task.

Independent read-only review of the experimental event and FI projections
found no concrete authority bypass under live, stable, physically valid borrowed
input. It did identify that `Event.kind` is a `uint32` in the descriptor even
though the Nostr range is documented as 0..65535: generated validation alone
does not enforce that comment. The event projection separately checks the
original parser's bound and equality. The event parser also imposes its own
JSON and tag limits, including 2,000 tags and 16 values per tag. The FI
projection uses 4,096-byte decoded headers, 65,536-byte decoded payloads, and a
1..16-entry audience profile; the generic generated audience-list validator
does not itself impose that audience count. These are local profile limits,
not protocol-wide guarantees. Tests use a 2 MiB Wasm stack and optional simulator
CLI fixtures; they do not establish iOS app-thread stack limits or pointer
provenance. The reviewed FI projection was
`fa887723059edbb2d4632f0b14aecbefab83a5a276b3638ba04d7fee71bce0fd` and
the event projection was
`46f0fe0fe568431914f280e2d705e0ef5e4132bd61791e7dadd7932910219d70`.

The shared message admission now rejects negative depth as well as depths above
64. The generated public decoders already start at zero; the change also makes
direct internal checker calls fail closed. The changed wire core passed all
14 Coax checks and 404/404 full-reference mutations, with no skipped checks.
A fresh Python-free bootstrap, event and FI generated-check tests, 21 native
importer tests (one optional oracle skipped there), 18 independent protoc policy
cases in checked/release native and Wasm, and the iOS Debug simulator build
passed. Bounds are still count checks on borrowed spans, not proof of native
pointer provenance. The complete event projection mutation gate remains open.
The reviewed changed wire source SHA-256 is
`bf7f55c25616bcabe0417b0ed2cfa56272e75545b2dc8e13c961d88bb458b87a`.
An independent source review also checked the generated-prefix boundary tests;
that review did not rerun tests or approve arbitrary schemas.

The generator now omits a message-specific size branch when that option is no
stricter than the shared 1 MiB admission cap; tighter message limits still
produce their own guard. This removes redundant unreachable mutations without
raising either bound. The importer and wire copies of the cap are checked for
exact equality by a Python-free gate in the private bootstrap; the gate itself
is fingerprinted by provenance. Matching, mismatching, duplicate, missing and
malformed definitions have direct negative controls. The gate applies to the
owned compiler runtime, not arbitrary host-supplied runtime strings.

The changed importer passed 125 checks and 1,723/1,723 full-reference mutations
with zero skipped checks. Independent read-only review covered importer
`66cca8e68a0417a7f92b3bb5cec20eaceb73f8fc5f801692b49edd0fbc8b61f8`,
the cap guard `465f082009c64f3735347687b9ceebbc57fd91469e7b4d37d80d56f5cf08b705`,
and boundary harness `2704d600fc29f2b7a7b43b31b8b0e997f61963c48a010aa388e55318acf3f5b4`;
it did not rerun tests or approve arbitrary runtimes or schemas. A fresh
Python-free bootstrap, 11 provenance tests, 22 importer tests (one optional
oracle skipped), and a Debug iOS simulator build passed. Independent protoc
regeneration and 20 cap cases passed in checked/release native, ASan/UBSan and
Wasm; 18 presence cases passed in checked/release native and Wasm. All 12
frozen descriptor profiles matched the explicitly derived transformation, and
four structural negative controls were rejected. The historical baseline
files remain unchanged. The experimental event projection passed 195 Coax
checks with the new generator; its full mutation gate remains open.
