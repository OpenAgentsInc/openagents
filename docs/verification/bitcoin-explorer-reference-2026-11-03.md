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

The full-chain native import is still in progress. Core and Electrs have
been stopped; final acceptance must compare the completed own-node API to
this fixed record and run `bitcoin/explorer/signet-check.coax` on desktop
and mobile. The earlier 329,328 snapshot in the native integration record
was superseded by a signet reorg and is not this fixture's chain identity.
