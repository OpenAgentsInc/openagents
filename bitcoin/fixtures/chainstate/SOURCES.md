# Chapter 58 independent fixtures

Mainnet records were captured read-only from `https://bitcoin-rpc.publicnode.com`
with the chapter-57 independent recorder. Each pins raw transactions, their
Merkle root, spent coin metadata and the sparse archival parent context.
`../validation/mainnet-170060.json` starts the contiguous range through 170,063.
The boundary is the complete set of touched coins, not a global UTXO snapshot.

`core-regtest.json` comes from the live isolated Core 32.1 journey in
`coax/tests/bitcoin/chainstate.py`. Its node starts at genesis, has no wallet
or peers, compares full UTXO MuHash at heights 100–102 and after both
disconnects, and reads the actual XOR-masked `rev00000.dat` to compare all
102 serialized undo bodies. Core binary provenance is retained in
`../validation/SOURCES.md`.

Undo serialization follows pinned Core's `src/undo.h` at revision
`ddea9196493d3b8fd841a36a6c53efd85dac7a05`, together with the already pinned
`coins.h`, `compressor.h`, `compressor.cpp`, `serialize.h` and `validation.cpp`.
The formatter was read, not executed. Python's independent serializer reuses
chapter 56's independent compressed coin model and adds the legacy dummy.

| File | SHA256 |
| --- | --- |
| `core-regtest.json` | `5040b24668b7b40fef242f029c40ed4bd99fbc0a9ad7016472dd35c2836ad035` |
| `mainnet-170061.json` | `cd8aec0275d768fee8907a962b7c01b8e00a51382b3e3bf4988b5910d4a39539` |
| `mainnet-170062.json` | `ed16508f7ecb603cc95141fe075aadf00b23d285cda0080b101b96b0b72da368` |
| `mainnet-170063.json` | `dfcb1a4c90b06f04ee9269efe0c9be31077c4251e4996d6195c9890e2a4238d3` |
