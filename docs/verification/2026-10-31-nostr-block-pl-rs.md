# NIP-PL and NIP-RS envelope projection verification

`nostr.block.pl.v1alpha1.PushLease` (kind 30350, for Coder host push leases)
and `nostr.block.rs.v1alpha1.ReadStateEvent` (kind 30078, for the CTRL and WS
read-state consumers) are untrusted projections of the public envelopes. The
NIP-44 contents stay ciphertext; nothing here verifies a signature, decrypts,
checks a clock or TTL window, detects a `client_id` conflict or accepts a
lease. No consumer uses them yet.

Descriptor profiles (protoc 29.3): `block-push-lease.pb`
`2180d92f549da8eb30e53b51c6e1e2dc2673a80862cba255eddc24f9639ae9ca` and
`block-read-state.pb`
`f3687846df1b3f899df6aaf6dc430914a190b448008f1d0722d9faec5e22106f`, each with
`options.proto`, `nostr/event/v1alpha1/event.proto` and its own schema. The
`nostr` module excludes both, so every published descriptor is unchanged
(`scripts/proto.sh check`). NIP-PL (`1e793eb8…bf72`) and NIP-RS
(`d7279c8b…ed2d`) are pinned to Block commit
`24a370606044c0c7be787cc2fd98ec0bfe1d606b`.

`pl-projection.coax` and `rs-projection.coax` (`#authority`) compare each
projection with its original through the shared event projection. Fixtures
(`generate_pl_rs_fixtures.py`, 32 PL and 19 RS) cover accepted envelopes, every
field diverging, missing, duplicate, extra and empty tags, extra values,
non-canonical expirations (leading zero, sign, overflow, empty), and the RS
slot's prefix, case, length and hex.

- `coax test` (each at `--run-steps 250000000`): 202 checks each pass.
- `coax mutate --root-only`: PL 95/95 and RS 69/69 killed, none marked equivalent.
- `block_pl_rs.py`: 35 PL and 22 RS cases through the generated readers,
  checked and release, native ASan/UBSan and WASI.

Full-reference mutation (generated readers included) was not run for PL or
RS; the lease and blob plaintexts are the next slices.
