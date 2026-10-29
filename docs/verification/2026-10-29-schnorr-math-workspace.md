# Experimental BIP-340 scalar equation

This isolated Coax slice reduces supplied nonce and challenge hashes modulo the
secp256k1 order, applies supplied key/nonce parity bits, and computes
`s = k + e*d mod n`. Invalid secret keys and zero reduced nonces select a
nonzero fallback for the arithmetic while clearing `work.valid`. A true return
means metadata was admitted, not that a signature is eligible: even invalid
inputs leave a computed fallback result. A complete signer must retain the
validity bit in protected state, bind both parities to the actual points,
verify the signature and gate publication. This slice alone produces no
verified signature and provides no public signing API.

Callers supply stable, valid, disjoint views, truthful alias facts and accurate
point-derived parity bits, and must erase the entire workspace after any
admitted result. Prior workspace bytes are preserved by early metadata refusal.
Explicit workspace wiping does not cover by-value `ScLoad` or scalar copies,
`sc_add`/`sc_montgomery` intermediate arrays, compiler/ABI/stack/register
copies, original inputs, caller output, snapshots, crash cleanup, target timing
or physical devices. No production-secret approval follows.

All 29 loaded Coax checks and 489/489 complete mutations passed without
waivers. Checked/release native and actual Wasm each passed 507 independent
integer-model cases across all four parity combinations, including invalid
secret keys and zero/reduced-zero nonces, with test-host workspace wipes. Native
ASan/UBSan covered valid/refusal cases; each iOS 26.5 Simulator profile executed
small equation and refusal cases. A Coax check includes the first official
BIP-340 vector's scalar equation. The external oracle run preceded a test-only
addition asserting the zero-nonce fallback; implementation was unchanged.
Independent review of source SHA-256
`97a253083cceac10ed99c32eba15cab894534c2e4e08604cea6d3305c5aee787`
found no concrete arithmetic or admission blocker under those premises; it did
not rerun the tests or approve a complete signer or target behavior.

Checked generated C SHA-256
`20aedaec7171f219976a658a9163385597ad8e995ad2e5a69cca4e297d6437a3`,
Wasm `3da61954f44cac0ee1c5bf9313bf4d08ecd5a210dffe66f8e587082294fcf325`;
release C `098cf6c6c65dec99e213e118be2fb0e9b92b0f03acf372a6205635ecbb62257b`,
Wasm `78f5588f519a4727b82689c603c1b6af9d8483aee5705140326c087914af3c42`.
The checked/release simulator executables and compiler versions are recorded
in the run output; compiler SHA-256 is
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
