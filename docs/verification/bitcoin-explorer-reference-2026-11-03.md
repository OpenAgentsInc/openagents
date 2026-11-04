# Explorer Core/Electrs references — 2026-11-03

Epic #12704. These public fixtures separate independent reference collection
from the native explorer's query path. No authentication cookies or wallet
secrets are retained. Reference processes use isolated data directories.

Bitcoin Core is 32.1.0, from the official archive with SHA-256
`61545c17c3120fb7412f896d248b11993af04ec8817aad3649c55b2afb044963`.
The Electrum server reports `electrs/0.11.0`, protocol 1.4.
`explorer-reference.py` requires both references to identify the same chain.
It independently parses raw transactions and checks block Merkle roots,
transaction identities, Core output states, and Electrs exact-script
histories, balances and unspent sets. The own service reads raw block files
and its own admitted P2P headers; neither RPC service supplies its answers.

## Controlled reorgs

`explorer-core-reorg.py` mines 100 mature-prefix blocks on isolated regtest,
then Core validates secretless P2SH spends on two competing branches.
Electrs follows each tip before the oracle records its reference. The
retained fixture has 108 distinct raw blocks and three snapshots:

| Height | Address balance, sat | HTTP comparisons |
| ---: | ---: | ---: |
| 102 | 2,999,998,000 | 47 |
| 103 | 1,000,000,000 | 39 |
| 104 | 2,999,998,000 | 47 |

`explorer-core-browser.py` passes all 133 comparisons in both release and
checked ASan/UBSan builds of the native node from `42ef855bad`. One open
headless, muted browser searches by height, hash, txid and address, then
follows both reorgs through the same writer process. The final screenshot
was inspected: the pow-only label, height, complete identifiers, history,
outputs and balance are legible. This comparison reuses the already passing
node binaries; it does not repeat their unchanged mutation gates.

The fixtures under `bitcoin/fixtures/explorer-core-reorg/` contain only raw
blocks, exported headers, public RPC results and the journey parameters.

## Frozen signet

`bitcoin/fixtures/explorer-signet-329338.json` records height 329,338, hash
`00000002c7899f4927ee1bd6cac42d972ebacd4ea1294f21b0140edd882d9f81`.
It contains six blocks, fifteen transactions, and four complete address
histories with balances 4,384,562,810, 88,000, 22,000 and 0 sat. Sample
transaction outputs include sixteen spent, seventeen unspent and seventeen
unspendable outputs. Address output sets include a spent and three unspent
outputs. Reference collection made 69 Core and 36 Electrs calls.

Seven busy candidate addresses were skipped after Electrs' explicit lookup
limit refusal; their public identifiers are recorded. Other errors refuse
collection. The selected address histories are complete, not truncated.
The reference server used a 1,000-entry lookup limit after an overly broad
query exceeded Core's 50,000-item P2P getdata limit. This is reference setup
evidence, not a passing explorer result.

The first complete native import reached this tip at 23:35 CST on November
3. With Core and Electrs stopped, all 63 HTTP comparisons passed: tip,
height/hash block lookups and searches, transactions, and complete address
histories, balances and spend sets. The comparison finished within fifteen
minutes. This is sampled reference coverage, not an exhaustive comparison
of every transaction or address.

The full-chain browser journey then exposed a height-search timeout: the
lookup iterated the entire 100 GB arena before reaching block receipts.
It now seeks directly to the `J` range and ends at that range's boundary.
All 128 explorer-store mutants are killed, with zero survivors. Release
and checked ASan/UBSan pass 108 native HTTP comparisons each, including
complete restart with a missing raw body and recovery after its return.
Both profiles also pass the 133 real Core/Electrs reorg comparisons and
the same open browser through both branches. The teaching API and writer
pass all 75 original HTTP/RPC responses under ASan/UBSan.

The updated Linux release process from `f28e24aa616deee4b5f7cb0ebc87e26c6c5ace24`
finished replay at 00:59 CST on November 4, still at durable sequence 23,932
in segment 69. Core and Electrs remained stopped. Before running the new
reference comparison, one fresh headless, muted Metal WebGPU browser passed
typed height, hash, txid and address searches against this full index.
The exact 4,384,562,810-sat balance passed. Desktop (1440×1000) and mobile
(390×844 CSS pixels, device scale 2) captures were inspected: identifiers
wrap completely, the balance fits, and history and output states are legible.
The updated service then passed all 63 independent reference responses.

The optional mempool page also passed on the full index. Its retained public
P2P feed contains 315 observations; the own-node projection prices 31 and
omits 284 against this frozen chain. It reports `unvalidated` and
`complete: false`. Public fee parity with Core is not claimed; the controlled
three-transaction Core fee comparison is the separate mempool evidence.

Retained evidence: [result and producer identities](bitcoin-explorer-signet-329338/result.json),
[desktop](bitcoin-explorer-signet-329338/desktop.png),
[mobile](bitcoin-explorer-signet-329338/mobile.png),
[mempool](bitcoin-explorer-signet-329338/mempool.png), and the passing logs,
public input feed and output JSON in that directory. `SHA256SUMS` binds those
artifacts. The full-size run is release; sanitizer qualification uses the
bounded independent fixtures described above. The service's cgroup peaked
at 107,002,216,448 bytes including cache, with zero high/max/OOM events.

This completes #12704's proof-of-work-only explorer acceptance. Full script
validation and chapter 82's full acceptance/relay policy remain later work.
The earlier 329,328 snapshot in the native integration record was superseded
by a signet reorg and is not this fixture's chain identity.
