# Experimental variable-base multiplication

Task `secp-mult-const` (#3e29). This candidate implements mathematical `k·P`
for bounded peer coordinates and reduces the 256-bit scalar modulo the group
order. A canonical infinity peer is accepted as a mathematical point; an
invalid bounded peer is refused with canonical infinity. An actual ECDH
consumer must separately reject inappropriate peers and invalid secret keys.
Caller-owned point fields must satisfy the existing group magnitude bound.
The validation kernel performs field arithmetic before raw-limb admission,
and masked selection evaluates peer normalization eagerly. This API therefore
does not promise refusal of arbitrary unbounded limbs; a byte-key admission
boundary belongs in the ECDH consumer. This evidence does not approve production
secrets.

The scalar is recoded into65 signed radix-16 digits. Every online row uses four
unconditional doubles, scans all nine table entries with a complete mask,
conditionally negates through masked selection, and adds. Only public loop
counters index arrays. The table selector and internal 32-bit group functions
are private by convention, not language-enforced; only internally generated,
canonical valid table points may reach them. Signed selection requires its mask
to be exactly zero or all ones. This source schedule is not a machine-code or
JIT timing proof.

The candidate converts the admitted point into canonical eight-limb base-2^32
field elements. Field multiplication uses an 8×8 schoolbook product and folds
high words using `2^256 ≡ 2^32+977 mod p`. For each product accumulation,
`(2^32-1)^2 + 2*(2^32-1) = 2^64-1`, so the unsigned accumulator fits.
The first high fold's carry is at most978; its remaining top is at most
`2^32+977`. Folding that top produces a value below `2^256+(2^32+977)^2`;
if it carries, the low part is below that square, so one more fixed fold cannot
carry again. Converting from10×26 to8×32 relies on the normalized22-bit top
limb to make its wrapped spill zero; conversion back masks the wrapped bits.
For addition, canonical operands satisfy `a+b<2p`; if the sum carries bit256,
the low part is below `2^256-2*(2^32+977)`, so one fold cannot carry again.
The implementation's assertions are authored guard obligations, not discharged
machine-code proofs. Field/group inputs and outputs depend on the canonical
preconditions recorded here; arbitrary external `Sf32` values are not admitted.
An independent read-only arithmetic/schedule review found no mathematical
blocker under those preconditions; it did not approve secret-derived guards,
target timing or production use.

## Current evidence and limitations

The original full schedule using the existing group kernels measured53,073,965
steps in a public cost probe and failed the unchanged20,000,000-step check.
The 8×32 candidate passes complete positive, negative-digit, higher-window,
infinity and invalid-peer checks under the unchanged limit; the largest used
19,921,696 steps. The current complete suite passes81/81 checks. No check budget
was raised and no partial-path substitute was used.

An independent native bigint run checked100,000 random plus1,560 boundary
field pairs per mode for addition, subtraction, multiplication and canonical
outputs ([field report](2026-10-29-secp-field32-preliminary.json)). Field
mutation killed244/244; its edge cases include a constructed product that forces
the second high reduction carry. Full variable-base arithmetic passed100,000
random scalar/peer pairs per checked and release native mode and100,517 pairs
per Wasm mode, plus raw boundary and exceptional-peer checks, guard bytes and
ASan/UBSan ([initial report](2026-10-29-secp-mult-const-preliminary.json)).
Native random streams check serialized canonical output; raw native boundaries
and all raw Wasm results check limb and field bounds. A separate probe passed
1,000 random scaled-projective peers per native mode (test script SHA-256
`015d98513f4c9872681a164cf99630369fac18bf48612b706d5f8de16b06b2e0`). The Wasm test build
reserves1 MiB stack; it does not establish a production host stack budget.

The initial full arithmetic report pins the source before a later test-only
addition for distinct finite group addition. That test was added after initial
group mutation found missing generic-add coverage; the complete100,000-pair
native/Wasm run subsequently passed again on that changed source
([report](2026-10-29-secp-mult-const-correctness.json)). A later test-only
first-entry selector check also changed the source/diagnostic mapping; the
full runner passed again on that source
([report](2026-10-29-secp-mult-const-correctness-v3.json)). A separate
Wasm probe passed12 bounded infinity/invalid-peer and scalar-boundary cases
per mode with canonical outputs and guards
([report](2026-10-29-secp-mult-const-wasm-exceptions.json)). Group mutation
killed all1,024 mutants after the distinct-point check was added. The first
scheduled complete-path mutation killed1,334 of1,335: skipping the first
window survived the small-scalar tests. An independent bit255 full-path test
was added and the current-source mutation killed all1,335 mutants with the
validated0.19.0 scheduler (compiler SHA-256
`5d9239a19f3567bf6691c0db034124ead36952885d2fb08bfb5627425fc190ce`). The
scheduler selected2,821 checks and skipped37,819 based on its validated
dependency analysis; borrowed, string and unstable fallback counts were zero.
No equivalent waiver, exclusion or check budget change was used.
The full100,000-pair runner passed again after that test was added
([report](2026-10-29-secp-mult-const-correctness-v4.json)), as did the separate
Wasm exceptional-peer cases
([report](2026-10-29-secp-mult-const-wasm-exceptions-v2.json)).
An expanded15-case-per-mode Wasm exception probe also checks a negative
generator peer with zero, one and order-minus-one scalars
([report](2026-10-29-secp-mult-const-wasm-exceptions-v3.json)).

For the selector-test source, the tested0.18.2 compiler and the independently
validated0.19.0 mutation scheduler compiler emitted byte-identical C under
`c coax/lib/crypto/secp_mult_const.coax --library`, with and without `--release`
and without `--header`. Checked C SHA-256 was
`5082fdab3748d80d9062a0b9056cbf1384c70395dec4262a121de7599e7b4223`; release
was `8d189f6d6a757efa5892c74519b11acdbf79a6d95291e039b69c15dd70a2fca5`.
On the later high-bit-test source the same comparison also matched: checked
SHA-256 `eaa2e0b77118d0b0ee23b30af8a414a1bb5ff3a2f856e34c9c22b96ba138ec2b`
and release `8731dc412a891270a31b15557a7e83ca90ea806349ca7380a17009eaeff209fb`.
This is emission parity; the reports retain their actual compiler provenance.

A one-million-observation native timing experiment on the earlier source
recorded variable t=-1.8859 and fixed-control t=0.5860
(no detected differences), and planted-branch t=-331.5777 (detected), with
independent recomputation agreeing ([report](2026-10-29-secp-mult-const-timing-preliminary.json)).
Its raw observations and binary remain in the ignored
`coax/bin/nostr-variable-base-timing` directory. A separate timing run on the
subsequent group-test source reported variable t=-3.2681, fixed control
t=0.3223 and planted branch t=-296.7278 with the expected classifications
([report](2026-10-29-secp-mult-const-timing-v2.json)); raw observations and
binary are retained in `coax/bin/nostr-variable-base-timing-v2`. The later
selector test is not part of that source snapshot. The separate selector-source
run reported variable t=-2.6796, fixed control t=-1.4354 and planted branch
t=-285.6754, with expected classifications and independently checked statistics
([report](2026-10-29-secp-mult-const-timing-v3.json)); its raw observations and
binary are retained in `coax/bin/nostr-variable-base-timing-v3`. The later
high-bit-test source reported variable t=-0.4621, fixed control t=-0.5374 and
planted branch t=-354.1962, with expected classifications and independently
checked statistics ([report](2026-10-29-secp-mult-const-timing-v4.json)); raw
data and binary are in `coax/bin/nostr-variable-base-timing-v4`. None of these
experiments establishes constant-time or production-secret approval; all alarms
are retained without filtering.

The earlier aggregate source audit did not complete: it refused at its existing262,144
visit limit after nine reached procedures and identified an existing
secret-derived authored assertion in `sf_normalize`
([refusal report](2026-10-29-secp-mult-const-source-refusal.json)). The
subsequent selector-test snapshot reproduced the same refusal and guard
obligation ([report](2026-10-29-secp-mult-const-source-refusal-v2.json)); the
current high-bit-test snapshot also refuses at the same bound
([report](2026-10-29-secp-mult-const-source-refusal-v3.json)). The
follow-up #f07b added bounded public-loop/callee analysis without a limit waiver.
The repaired auditor subsequently analyzed the complete `smc_multiply` source
closure in the native-C checked-library profile: 97,203 visits, 43 reached
procedures and three secret-derived authored assertions. The result is
`analyzed-with-guard-obligations` with `accepted=false`; the unresolved guards
are `secp_field.coax:239` and `secp_field32.coax:70,92`. The
[source report](2026-10-29-secp-variable-base-source-reaudit.json) records the
exact source, compiler, audit-tool and type identities. These observations are
not source admission or proof that the assertions are unreachable. Guard discharge,
compiler/object/JIT inspection, independent security review, device evidence,
erasure and production-secret admission remain separate gates.

## Reproduce

```sh
coax/bin/coax test coax/lib/crypto/secp_field32.coax
coax/bin/coax mutate coax/lib/crypto/secp_field32.coax
coax/bin/coax test coax/lib/crypto/secp_group32.coax
coax/bin/coax mutate coax/lib/crypto/secp_group32.coax
coax/bin/coax test coax/lib/crypto/secp_mult_const.coax
coax/bin/coax mutate coax/lib/crypto/secp_mult_const.coax
COAX_FIELD32_REPORT=/tmp/field32.json python3 coax/tests/nostr/secp_field32.py
COAX_CONST_REPORT=/tmp/variable.json python3 coax/tests/nostr/secp_mult_const.py
python3 coax/tests/nostr/secp_mult_const_scaled.py
python3 coax/tests/nostr/secp_mult_const_wasm_exceptions.py
python3 coax/tests/nostr/secp_mult_const_timing.py
python3 tools/coax-nostr/secret-check/aggregate.py coax/lib/crypto/secp_mult_const.coax --entry smc_multiply --report /tmp/variable-source.json
```
