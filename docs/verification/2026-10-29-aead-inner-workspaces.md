# Experimental AEAD with owned inner workspaces

The staged AEAD operation now embeds the independently tested `XcWorkspace`
and `PolyWorkspace` in `AeadScratch`, and uses them for the one-time MAC key,
message cipher, tag and authenticated opening. The existing host wipes the full
scratch allocation, including both embedded workspaces, before publishing. On
failure and cooperative cancellation it also wipes staged output; a failed
wipe remains pending or results in an explicit output quarantine. Admission
refusals before registration do not wipe or alter pre-existing caller storage.
Successful output, original inputs, nonce uniqueness and allocation lifetime
remain the caller's responsibility.

This changes the experimental scratch ABI: consumers must rebuild with the
new generated header and allocate the complete new type. Host address checks
still rely on truthful extents and on stable valid allocations, and target
linear-memory bounds must be established by the caller. Source-level scalars,
helper returns, compiler copies, stack spills, registers, OS snapshots, crash
cleanup, machine-code timing and physical-device behavior are not proven by
workspace erasure. This is not production-secret approval.

All 45 loaded Coax checks and all 1,445 mutations passed without waivers. The
checked/release native and actual Wasm paths passed 132 independent HChaCha plus
OpenSSL AEAD vectors with actual test-host workspace wipes. They check all tag
byte changes and the first ciphertext, nonce and AAD byte where present. The
checked/release iOS 26.5 Simulator executed the draft vector and all tag-byte
changes, plus a real host fixture covering the first ciphertext, nonce and AAD
byte, cancellation, wipes and injected effect failures. Native ASan/UBSan runs
cover the workspace maximum-size seal and the host fixture, rather than the
entire independent vector corpus. Independent review of source SHA-256
`5ca874864f2b4adfdf3ec1c48fff4756a029800c2e40cb4f3159b7e33e3c93d4`
found no concrete auth, alias or lifecycle blocker under the documented host
premises; it did not establish target behavior or production suitability.

Checked generated C SHA-256
`68e7390df146760a54f4ab4914af06a9b93176f1eb12ff619544b4b36a2c8277`,
Wasm `4aba95485e24673feaec4f341a01b669b7283473779ac67b49f23e197681f7fd`;
release C `caa83be716b722deefb9ca130d850a9a326e581825200ef340bb4fbe5f2c4d0d`,
Wasm `f65c9f34de4d6e9e4751955979d7607b2518d9396965d577727b7481ebcd1770`.
The checked/release simulator vector executables hash to
`84ebb1151bce39387751b8b9964d84a0fee03965d106cac925b782b137f33f48` and
`5e1172bf76206d12b05f35db64fc3e8fbb71c2c1e3c739426a7bbfe6239f379a`.
Compiler: coax0 0.22.0, SHA-256
`9857663bf2c87462d9acdd09b118c9a3e298a3a9caa43ab842dfeabedcce0cd7`.
