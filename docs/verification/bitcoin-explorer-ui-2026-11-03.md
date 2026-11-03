# Bitcoin explorer interface — 2026-11-03

Epic #12704, after address index `0d0242e10b` and API `a0b8cfcbe5`.
The interface is `bitcoin/explorer/explorer.coax`, with a thin C Wasm ABI,
Coax-generated WGSL and the existing generic browser host. No JavaScript
product logic or visible DOM controls are introduced. The host adds an
optional focus-event bridge so Coax can draw keyboard focus for mirrored
controls. State initialization uses the compiler's in-place defaults.

The client response/snapshot gate kills all 179 root mutants, retaining
imported baselines. Its checks cover malformed snapshots, network changes,
old responses, canceled/consumed slots, failures, recovery and same-height
hash changes. The exact static asset allowlist kills all six root mutants.
No mutants are marked equivalent.

The Coax build tool emits the browser module and shader. The UI bounds a
reply at 16 MiB and 65,536 JSON nodes; exhaustion clears visible data. Lists
paginate twenty rows without discarding the accepted response. Search is
same-origin. HTTP responses remain read-only, no-store and connection-close.
The original API regression passes all 75 independent responses after the
static-service addition; earlier API mutation and sanitizer results are
recorded in `bitcoin-explorer-api-2026-11-03.md`.

`coax/tests/bitcoin/explorer-browser.py` supplies independently replayed
raw-block fixtures to `bitcoin/explorer/check.coax`. The journey uses only
headless muted Chrome with hardware Apple Metal WebGPU. It submits typed
height search, hash/txid/address searches, follows a transaction's block link,
checks one spent and three unspent transaction outputs, verifies mirrored
keyboard focus, and captures desktop/mobile screenshots. In the same open
page, competing-branch and return reorgs must update the P2TR address's
balance from 1,999,998,000 sat to zero and back. Removing the state file must
clear data and balance; restoring it must recover. Invalid and absent
queries must refuse. The final journey also runs with the API and writer
under ASan/UBSan.

Compiler: coax0 0.48.0, SHA-256
`cdaceb89e69c8d523e41242a940af34774e8b6b4f945998c2d3bf38486c357fb`.
Generated WGSL passes its drift check. Tools inventory ran over 1,753 tracked
files and reports eleven untouched existing files without reason headers.
The new Python fixture oracle declares independent-oracle reason 2.

This is frontend qualification against complete regtest fixture history.
Serving the full signet chain from our own writer and comparing its answers
with Core and an address-indexing reference remain explicit epic acceptance
work. No signet completion is claimed by these checks.
