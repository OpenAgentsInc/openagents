# Experimental scrypt host bridge

This internal platform bridge connects the Coax job, numeric address-range
admission and the existing observable erasers. The Coax job decides whether a
run is active, which cleanup region is due and whether publication is allowed;
the C host observes pointers and carries out synchronous effects. The caller
provides a zero-initialized host and stable, live, accessible, disjoint scratch,
password and salt. It serializes calls and prevents other writers. Password is
already NFKC UTF-8; the input and published output have caller-owned lifetimes.

The bridge validates the full host object and original scratch extents against
32- or 64-bit address limits and overlap policy. This is arithmetic validation,
not a proof that memory is allocated, accessible, correctly sized or remains
alive; Wasm linear-memory extent and actual host ownership remain separate
obligations. A failed start can still leave cleanup pending. Inspect the status
and finish cleanup rather than assuming failure touched nothing.

Scratch cleanup is chunked to at most 1 MiB per call; state and any abandoned
staged key are acknowledged only after the host eraser reports success. Closing
an unpublished successful job cancels publication and requires the staged-key
wipe. After publication the caller owns the copied key. A post-publication wipe
failure returns distinct status 2: output may contain key material, even if an
attempted output wipe also fails, and must be quarantined and erased by the
caller. Closing still attempts to wipe the host. No effect result authenticates
itself against a dishonest host. Stack/register/compiler copies, crashes, OS
snapshots, asynchronous callers and physical-device behavior remain unproved.
This is not integrated into the app or a production backup/key-custody path.

The fixture checks independent scrypt output, guard bytes, start and output
alias refusal, address wrap, successful/cancelled/failed paths, abandon-before-
publish and a populated 2 MiB chunked wipe. A test-only effect hook injects
state/key/output erase failures: it verifies cleanup remains pending on failure,
retry, the distinct publication error and both successful and failed attempted
output wiping. These injected failures do not assert that the real eraser
naturally fails in the tested profile. Checked/release native ASan/UBSan, actual
Wasm and iOS 26.5 Simulator runs passed both the normal and fault fixtures.
No physical phone, concurrency, inaccessible memory or hidden-copy guarantee
is inferred. Python/Node are test tools, not build/runtime dependencies.

Independent read-only review covered host C SHA-256
`17091bec2f1fa788988c6b7148a3502e8021adf17c8280aa10847aa076ebb38f`,
header `5d492ddd04c5f71f0216f63be9636ff028fc736eaf31097ce501c90f99e0033a`
and fixture `4f087b3fa2e3bc73221914844b419141a0f28c6e6d7f83814596c5ef24e51f70`.
It found the publication-failure ambiguity; the distinct status and injected
failure cases address it under the stated trusted, serialized-host contract.
No production-secret or target-security approval follows.

Run with an explicitly booted simulator to include iOS:

```sh
COAX_IOS_SIMULATOR=<booted-simulator-udid> python3 coax/tests/nostr/scrypt_host.py
```

The checked normal/fault Wasm artifact hashes were
`5d5ab2967b0764447cb556b855260646bd2bb9bd8d9d93aa78db5e135c7ebbd4` and
`7be25b1a6f256c410f005d4e7c2b99091f4bac605484d93ec319e7d036763945`;
release normal/fault were `8a527fed2e66265965171a81532c0cb6f953807dccd39098b512d45f77e73a05`
and `78701e394381a2680233aeb60b2c5525234151b2b4732ed970ce02084cb7eea9`.
Simulator executables in the same order had SHA-256
`66497ba2b65c29b3969a7c1ab08d6c15147f28cedde79803affbf02f57cd09c5`,
`1a238a883a1ae31801bf039e93589c036e815c2d47a4cf38e37d1d6e685e6127`,
`cae64b2ce7743e02186b22625e2562cacbba57da459c842e78896b9cfd611867`, and
`9357250592296043930431526c4998e956e1041b073915f400c399c692ef7c54`.
These are simulator results, not a physical-device result.

A separate simulator fixture (`coax/tests/nostr/scrypt_host_large_ios.c`,
SHA-256 `95da4be5673cec5d54bb8966039ad895e44ba861d7b47560011b1d0119227ebf`)
ran the full host bridge at N=65536, r=8, p=1 against the independent vector,
checked every byte of the 64 MiB scratch after 64 wipe chunks and kept its
canaries intact. It passed checked and release in iOS 26.5 Simulator; executable
SHA-256 values were `481f1d1cae6be37dbb543fd91b9080ae616c52340fc59562a0c9d8d57ad6e302`
and `8b22569994d1cba1a77d8d9f446d13e17cebfc8275dc1d42f9a15019df9f6811`.
Set `COAX_IOS_LARGE=1` with the simulator variable to reproduce this longer
case. It is a public-test success path, not a device or crash-cleanup guarantee.
