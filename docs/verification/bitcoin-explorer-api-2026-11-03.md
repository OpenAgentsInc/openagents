# Bitcoin explorer API — 2026-11-03

Epic #12704, chapter 90's partial read-only profile. The typed projection
starts at `proto/openagents/bitcoin/explorer/v1alpha1/explorer.proto` and
`docs/protocol/bitcoin-explorer-v1.md`. New namespace; no frozen contract or
compatibility baseline is edited. Product parsing, lookup, formatting and
HTTP service orchestration are Coax; the host supplies TCP and file effects.

Root-only mutation results, with all imported baselines retained:

| Module | Killed | Survivors |
| --- | ---: | ---: |
| `explorer-request.coax` | 212 | 0 |
| `explorer-rpc.coax` | 155 | 0 |
| `explorer-store.coax` | 128 | 0 |
| `explorer-body.coax` | 54 | 0 |
| Total | 549 | 0 |

The ordinary native service passed 73 independently computed HTTP/RPC
responses. Uppercase-txid search and notification coverage bring the final
sanitized API and sanitized index writer journey to 75 responses. Valid
notifications return HTTP 204 with no body or Content-Length. The oracle
covers the whole 100-block mature prefix, both branches and the return to
the original branch, shared txids, stale transaction lookup, same-block
spends, zero-valued outputs, P2WPKH/P2TR/P2PKH addresses, genesis, JSON-RPC
ID/error behavior, torn state and malformed/missing storage.

The worked program matches its three-line golden transcript in debug and
release. Pinned protoc 29.3/protobuf 5.29.3 generation and protocol checks
pass, including 30 contract tests and unchanged published baselines. The
Bitcoin chapter registry passes. The full book checker remains blocked by
an unrelated existing `coax.wgsl.current` evidence mismatch for
`verse/everglade/gpu.bindings`. Tools inventory ran over 1,750 tracked tools;
nine untouched existing files lack their reason header. The new independent
HTTP oracle declares reason 2.

The body gate also uses original mainnet block 514,999 to check activated
witness handling and a witness-only disk edit. Transaction lookup checks
actual transaction boundaries and exact lengths before returning raw data.
Every response is bound to the one synchronized light/address/tx-index
snapshot, labeled pow-only; full script validation is not implemented.

Replay commands:

```sh
coax/bin/coax mutate coax/lib/bitcoin/explorer-request.coax --root-only --jobs 6
coax/bin/coax mutate coax/lib/bitcoin/explorer-rpc.coax --root-only --jobs 6
coax/bin/coax mutate coax/lib/bitcoin/explorer-store.coax --root-only --jobs 6
coax/bin/coax mutate coax/lib/bitcoin/explorer-body.coax --root-only --jobs 6
python3 coax/tests/bitcoin/explorer.py --api-tool API_BINARY --index-tool INDEX_BINARY
python3 coax/tests/bitcoin/explorer.py --sanitize-with coax/bin/coax
```

Compiler: `coax0 0.48.0`, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
Address-index base: `0d0242e10b`. Signet service, the front end and the final
Core/address-indexing-reference comparison remain the next integration
steps. The service's finite table/log/response limits are documented in
chapter 90; resource failure is explicit, never an incomplete success.
