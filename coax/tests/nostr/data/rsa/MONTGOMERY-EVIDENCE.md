# Montgomery RSA prototype evidence (2026-10-28)

This is an intermediate unintegrated public-arithmetic prototype for #bd06. The intermediate source bytes were not retained separately; the recorded source hash identifies the observed run, not the current source. It does not
authorize FI claims. The original bit-serial prototype and its evidence are
preserved separately in `prototype-v1.coax.txt`, `prototype-v1-test.py.txt`,
`prototype-v1-positive.coax.txt` and `PROTOTYPE-EVIDENCE.md`.

The checked source at this measurement had SHA-256
`9110e4d882ec5f498afc765fb944b98411f5dea57764ec384798680df4a00556`.
The test harness `coax/tests/nostr/rsa_verify.py` had SHA-256
`77ce9afd0f2b6225aa7dc8321a04869c7e337696af894fb6bdec1c14ef68992e`;
the compiler binary had SHA-256
`56f87661b9f1f105598132bf5c45f13e07fea46f4f23249921a22111d58f3ee4`.
The original evidence gives the observed compiler worktree revision and pins
all three independently sourced Wycheproof files. No build provenance is
inferred from those hashes.

`coax test coax/lib/crypto/rsa_verify.coax` passed under its unchanged default
budget. Its real 2048-bit positive signature check used 14,792,257 steps. The
independent Python integer oracle matched 773 pinned Wycheproof cases and 300
seeded Montgomery products per native mode, with limb counts 1, 2, 3, 63, 64,
65, 95, 96, 127 and 128. Actual Wasm matched the 773 cases in each mode.
The harness's observed artifact hashes were:

| Mode | Generated C | Native library | Wasm |
| --- | --- | --- | --- |
| checked | `f44a4dd8c03ebb981b1e562cce79953123de524a3cc3aa793be3886221854c42` | `0bda0865404648fb720f39f7e7c730ed023d8cebb5a2584166f12418d35edcd5` | `9d93a0d404a18e1ffe8a302f6cf394123263c39c3934f61b1d06674da2c92767` |
| release | `c26873999c3610f3b6d221875c6142e0344640f985b50466034bd728c95c05ed` | `4162f42caf33f2db7670e511a9dc9188d95d3ee06568b21f7ba37dc69dfeff13` | `98d02180412bf8dec541573823efb0398cba424d3e9917dc03430817c3f0de52` |

The authority mutation gate is running and no success is implied here. All
other limitations in `PROTOTYPE-EVIDENCE.md` continue to apply. The profile's
JWK parser must enforce e=65537 before calling this fixed-exponent primitive.
