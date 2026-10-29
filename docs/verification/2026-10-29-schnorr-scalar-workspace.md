# Experimental BIP-340 scalar preparation

This Coax slice decodes a 32-byte secret scalar, derives a validity bit for
`1 <= d < n` and selects the scalar or a safe nonzero value for subsequent
fixed-work computation. It also provides parity selection and fixed-width
big-endian encoding for internal canonical values. It is not a signer: the
caller must bind the validity bit through publication, supply a parity bit of
zero or one derived from the correct point, and prevent any signature on
failure. Views must be valid, stable and disjoint with truthful alias facts;
the host must erase the entire workspace on admitted success or failure.
Pre-admission refusal preserves previous workspace bytes.

The explicit workspace is not complete secret erasure: scalar helpers pass
secret-derived arrays or scalars by value and produce temporaries. Compiler,
ABI, stack and register copies, source key/output, crash cleanup, target
machine-code timing and physical devices remain outside the test wipe. There is
no production-secret approval or claim that an invalid key follows a proven
constant-time complete signing path.

All 26 loaded Coax checks and 438/438 full mutations passed with no waivers.
The checked/release native and actual Wasm profiles each passed 1,007
independent integer-model scalar cases with both parities and test-host wipes,
including zero, one, n-1, n, n+1 and 2^256-1. Native ASan/UBSan covered two
parities and refusal. Each iOS 26.5 Simulator profile executed representative
valid/invalid and parity cases. A test-only Coax adapter exposes parity for the
oracle; the large corpus is not physical-device coverage. The oracle run
preceded a test-only addition checking stale high-limb reset, with unchanged
implementation. Independent review of source SHA-256
`6a5b214e6a9ac87bcd0afa34b2bf0668126fa7ce4b8fe7f69bff6e50b4ee52f9`
found no concrete admission, selection or endianness blocker under the stated
premises; it did not rerun tests or approve signing/target behavior.

Checked generated C SHA-256
`966e4163973b118371360d1e3c908c5befba4283178814bdaf4a91ced9715a50`,
Wasm `5798cb605896052efc28a70e543d5271903bb38a37079492ba5382b634bef8cd`;
release C `3fc223a892cb3aa06f0f2602cef7ab1471c5310baaed28eef75493a3d360c050`,
Wasm `9251c6495a2eafdaa4592c2f45ab3cbcf2eda0c7a1df3505a89d71d077eb0aab`.
Checked/release simulator executable hashes:
`42f9dff893f2dbc2314ba66a08cf9e197484afc98e9a7fc6b65fc5c69f3976a4` and
`36729d3f4181f59eef3b9da48680d01338f842475d55e4f15aecb79f2ff72935`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
