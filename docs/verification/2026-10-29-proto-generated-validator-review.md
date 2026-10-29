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
The event projection and an additional full reference-mode identity run remain
open; this evidence does not close the task.
