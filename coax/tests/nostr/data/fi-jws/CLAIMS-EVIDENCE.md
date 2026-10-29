# Untrusted FI claims projection evidence

The new `fi-claims-parse.coax` and `fi-claims-select.coax` are
candidate-only work. They do not verify a signature, authenticate a key source,
prove NIP-42 possession or grant an identity/session. The two-pass selector
first resolves the Host/community without inspecting issuer configuration,
then projects only the issuer and checks the exact issuer allowlist before
parsing the selected class with that issuer's configured marker. Dedicated
assertions ignore unverified `client_id`; access tokens validate it. The caller must keep the
original input immutable through both passes and original-byte verification.
The configured marker may coincide with `iss`, `sub`, `client_id` or scalar
`aud`: the same decoded member supplies both roles. Numeric or array marker
overlap refuses. Marker absence remains observable and later policy must deny
where it is required; a selection status is not an authorization.

The payload is capped at 65536 bytes, with at most 64 top-level members and
128 decoded bytes per member name. Top-level names are compared after JSON
unescaping and any duplicate refuses. Fixed fields use explicit string, array
and numeric types and the selected FI bounds. `aud` accepts one string or
1–16 unique strings; dates accept integer seconds in 0…2^53−1. Unknown values
are ignored after syntax skipping: nested duplicate names and escaped Unicode surrogate-pair validity are not
verified there, so later consumers must never
promote those unknown bytes into claims. These parser limits and the required
original-byte signature behavior must be included in a future full policy ID.

`coax test fi-claims-parse.coax` passed 173 check groups and
`coax test fi-claims-select.coax` passed 176. An independent Python oracle
covers 131 projection cases per checked/release native and wasm32-wasip1 mode;
those same cases passed native ASan/UBSan. The Wasm harness provisions a
1 MiB stack for projection and 2 MiB for selection/candidate; this is not a
claim about an unconfigured browser host. Fourteen issuer/marker selection cases
also passed each checked/release native, Wasm and native sanitizer mode.
The first projection mutation run killed 1604/1656; after additional checks,
the second killed 1648/1655, and the third killed 1653/1655. No survivors
were waived. After simplifying the text-decoding helper and adding boundary
checks, the previous full default-budget projection run killed 1655/1655 with zero
survivors. The class-aware parser subsequently changed, so that result does
not cover its current source. The first current class-aware run killed 1678/1683; five mode/required-bit
survivors were not waived. Boundary assertions were added. The current class-aware parser gate killed
1683/1683, zero survivors and no waivers; the selector gate also passed 1698/1698; semantic candidate gate also passed 1729/1729; the local-JWS
closure gate subsequently passed 3397/3397 on the new source.

A separate read-only source review (`fixedbase_rescue`) found no concrete
blocker in the projection scope and recorded the ignored-nested-value caveat
above. Reviewed SHA-256: `fi-claims-parse.coax`
`25250f59b5cf4230bc98ca2acaaf5a216305c1f5bd8be7a2f8c00a1a5a9358e7`,
`fi-policy.coax` `ce60dca52da63c28b0e8b06799129cb6f651acfe766355933394d3ea56397652`,
and `json.coax` `87d55aa85f17d824338046ec27903684e51bd7a70b11a30f9c3da65255280f5f`.
The parser subsequently changed to SHA-256
`fa619d1bc4ea2c387694af9d40feaf48a5372376932f3697e11b32da00b8b6b2`;
independent delta review found no new projection-scope blocker: both string
paths check a positive decode length before the ASCII helper. The same
unknown-value and no-authorization limits apply. The first review did not review the separate selector or establish production assurance.

Other source/test SHA-256:

- `coax/lib/nostr/block/fi-claims-select.coax`: `fe1f918d146009b794a6f2e2d1abaead359a179c363f56d21a00649c2240f8cd`
- `coax/tests/nostr/fi_claims_parse.py`: `3e864173ec58d48c13b6a28dacaebd1e5cd6ae12a52a750b7886c05cef0b0e46`
- `coax/tests/nostr/fi_claims_fixture.c`: `583ca95379de751eac667f31d72ed80a1ee55967011d5afe4fe9925298927a8b`
- `coax/tests/nostr/fi_claims_wasm.cjs`: `acbc795256b29b546331050dbb0c76b34f118a7dbf84f20b16200e55cc58c11f`
- `coax/tests/nostr/fi_claims_select.py`: `a5af5d34e00e2ddd2054f680b0f176a674ceb0b0ebe8612faec78f57819fde05`
- `coax/tests/nostr/fi_claims_select_fixture.c`: `a7041dc629dd7de0ffc6f1c26a8007e65e1d09c34b7603195ac470a88a8c1fea`
- `coax/tests/nostr/fi_claims_select_wasm.cjs`: `48a0cbb872c4a8e12fb0a451cded738095ec579fe9e5db72cb5e4238483e6a1d`

A separate read-only review (`fixedbase_rescue`) of selector SHA-256
`f99e69f0622ef1ebc879233c8ef2936fa20fef0e8289145e4a71820eaca25a82`
found no concrete blocker within untrusted selection scope: the community
allowlist is checked before matching issuer configuration; the second pass
uses that issuer's marker and rechecks the issuer. In addition to immutable
payload text, caller-owned community/issuer arrays and their strings/slices
must remain valid and stable through selection and later use of returned
indices. Marker absence remains possible and requires later policy refusal.
This is not provider or authorization approval.

The separate `fi-claims-candidate.coax` bridges the selected owned projection
to temporary borrowed `FiClaims` views and calls the existing typed token-class,
claim, audience and time predicates. Its header and payload inputs are
untrusted; `FI_CANDIDATE` is semantic filtering only and is not signature,
provider, possession or session authority. The local views never escape. The
source passed 180 Coax check groups and 20 checked/release native, Wasm and
native ASan/UBSan cases. Its full default-budget mutation gate killed 1729/1729 on the class-aware
parser with zero survivors and no waivers.
Source/test SHA-256:

- `coax/lib/nostr/block/fi-claims-candidate.coax`: `98938e095d60d02dedaa948d7225e0c2af21fb7889e01dacc034f98b21b9bf42`
- `coax/tests/nostr/fi_claims_candidate.py`: `2fe16e83424c8816e6c94983840c5c6e802f1905738d76c3cfe5222eb70224d8`
- `coax/tests/nostr/fi_claims_candidate_fixture.c`: `ba89fe6dc7b38fb1075438f8483ab535d68f88faa3f3dcf3ff9f3d3459bdb156`
- `coax/tests/nostr/fi_claims_candidate_wasm.cjs`: `03bbe94fff9ebb2331f1c8d231c1ec5093e5f66444c0d562070bfcd3dfc3143f`

Independent read-only review (`fixedbase_rescue`) of semantic candidate SHA
`986a3a1e40fbc60ee98d5042114b8aa3d663e76c8d4b2c592225a8da7d58dfb1`
found no concrete blocker within the untrusted candidate scope. Temporary
claim/audience views refer to local owned storage and are consumed before
return; presence/time/owner fields map through, missing access-token marker
or client is rejected by existing policy, and selection status precedes index
use. All input views/configuration must remain valid and stable during the
call, and header/payload must be independently bound to verified compact bytes.
No signature, provider, possession or production approval is implied.

The reviewer confirmed that parser `fa619...` → current
`9baa770e74035c9670a0a727416f3afcf5911b33484394be124c8cff483abbf8`
adds only two boundary assertions in a new `#test`; production code is
unchanged. The same projection and no-authorization review caveats apply.

The selector now first probes the Host/community using an empty issuer; valid
allowlists reject empty issuers, so that probe cannot touch the issuer registry.
Unknown Host or invalid community therefore returns unavailable even when
claims are malformed. Guard pages confirmed unknown Host, invalid community configuration, malformed
claims and community-unauthorized issuer do not read an inaccessible issuer registry.
The reviewer compared the prior `f99...` version to the Host-first revision
`b895aaec797a48d3aaeb0a3b5b73588b925d3f39351a6cdace561ceceb514e94`
and found no concrete ordering issue under stable valid input-view assumptions.

The first full Host-first selector mutation run killed 1668/1669. Its single
survivor was the combined second-parse status/issuer check; no waiver was
used. That check was factored into a directly tested helper, and selector and
dependent closure gates were restarted on SHA-256
`3ef3ccac013c306fecb876dc99f28c1878e34ff1b2d542994ca5fd5e849295b3`.
Independent delta review found no new selector-scope blocker: the helper
preserves the prior conjunction of second-parse success and exact issuer
equality. Input stability and no-authorization caveats still apply.

The independent reviewer compared the prior parser `9baa...` to current
`984216504d401655967ff2d5243a4d5f5a072567638e9670fffb86734c8bf842`
and prior selector `3ef3...` to current
`1d0a4539a67a9ea26e52f0effb3336b7b3f5c7ba6ee73ef7bbf797472bcf145e`.
No concrete blocker was found within untrusted selection scope: the issuer-only
pass retains top-level duplicate and complete-JSON checks, selected dedicated
class skips client_id, selected access class validates it, and the second pass
rechecks issuer equality. Stable inputs, ignored-nested-value limits and no
authorization still apply. `fi_claims_profile.py` separately passed 198
class-mode cases in checked/release native, Wasm and ASan/UBSan.

The semantic candidate's prior reviewed `986...` source was reconstructed and
compared with current `39139fefe6584c56d1c3624fb42c28c8ffc7dd4580a531e2b33be54a4c47437b`.
The independent reviewer confirmed that only the dedicated/access class
`#test` was added; production code is unchanged. Prior candidate limitations
and no-authorization scope remain.

Class-mode independent test SHA-256:

- `coax/tests/nostr/fi_claims_profile.py`: `19cdade845968dcd107bb683ef4f896ba7cd4f1854879195ae9b6f00db6631e1`
- `coax/tests/nostr/fi_claims_profile_fixture.c`: `3c0f073537ce86cc87fde7d5e4c787c6016fcf62dc3da59657afe05c8d65cd11`
- `coax/tests/nostr/fi_claims_profile_wasm.cjs`: `1bfd76ca58a3d413c47107b94dde8146f69e31af10926091ea2fcb5896431b3f`

The class-aware parser reviewed at `984216...` subsequently gained only a
boundary `#test`; current parser SHA-256 is
`b3947fecaec8811b38a1094af1fdca7b0708f46068a22be370811df6fe6441dc`.
Independent delta review confirmed that only the boundary `#test` changed;
production code and prior projection limitations are unchanged.

The first class-aware selector mutation killed 1697/1698. The survivor
changed the first pass from issuer-only to full parsing and hid an unavailable
issuer configuration behind malformed claims. No waiver was used; an explicit
ordering assertion and independent native/Wasm/sanitizer case were added, and
the dependent gates restarted. The current full selector gate killed
1698/1698 with zero survivors and no waivers. Current selector SHA-256 is
`fe1f918d146009b794a6f2e2d1abaead359a179c363f56d21a00649c2240f8cd`.
Independent delta review confirmed selector `1d0a...` → `fe1f...` adds only
that ordering assertion; production code and prior review limits are unchanged.

The mutation results in this note use the FI worktree compiler,
which reports coax0 0.18.2 and has SHA-256
`56f87661b9f1f105598132bf5c45f13e07fea46f4f23249921a22111d58f3ee4`.

The first semantic candidate mutation killed 1723/1729. Six projection
mapping survivors (subject, iat, nbf and agent-owner paths) were not waived;
an additional direct test was added; the candidate gate subsequently killed
1729/1729; the dependent local gate subsequently passed 3397/3397.
The reviewer confirmed candidate `39139...` → current
`98938e095d60d02dedaa948d7225e0c2af21fb7889e01dacc034f98b21b9bf42`
changes only that `#test`, not the production procedure.

The current 131-case strict projection and 198-case class-aware suites also
passed checked/release native, Wasm and ASan/UBSan when compiled with the
verified current-main coax0 0.21.0 binary SHA-256
`73d2aeb0e1e57c355d87242946f89e1462f8b765f57087f4307d471a0277e1d4`.
The mutation evidence remains tied to the separately identified 0.18.2 binary.
