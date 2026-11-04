# Block validation reference fixtures

Captured 2026-11-04. These are public chain records, with no wallet or key material.

Mainnet records come from the read-only Bitcoin Core JSON-RPC interface at
`https://bitcoin-rpc.publicnode.com`: `getblockhash`, `getblock` (0 and 3),
and `getblockheader`. The recorder independently checks every txid, the
transaction Merkle root, header hashes and complete serialization. External
spent outputs retain Core's amount, creation height, coinbase bit and script.
The two BIP-30 records also retain the original identical coinbase and its
documented overwritten outputs.

Testnet4 and signet block 1 come from `https://mempool.space/NETWORK/api/`.
Their txids, Merkle roots, parent links and genesis headers are checked;
the own header loader also admits both headers before full validation.

The mainnet context is an independently supplied admitted-chain test boundary.
It includes all header times/hashes that influence these decisions; it is
not an own-node sync from genesis or a complete dump of the historical UTXO set.

| Fixture | Height | Transactions | Predecessor coins | SHA256 |
| --- | ---: | ---: | ---: | --- |
| [mainnet-170060.json](mainnet-170060.json) | 170060 | 23 | 61 | `140afcfba6c64e4772bf9d67a60e9a0560f7a2b646f431e4b6c6cb8dfdfa5574` |
| [mainnet-173805.json](mainnet-173805.json) | 173805 | 106 | 234 | `d3fabaa8f7114892febfa332e78d1f8021b0ff32ea21e5a46cd153311fde1063` |
| [mainnet-227931.json](mainnet-227931.json) | 227931 | 364 | 599 | `00b7d0d22cc002f3c0cbbee95afa92ff213d29970bc5a6320ca7d428c394eb17` |
| [mainnet-363725.json](mainnet-363725.json) | 363725 | 1 | 0 | `e17a16992b9b51eb1a301a98c510254ed86b411e62c5538910f25e348363bb4c` |
| [mainnet-388381.json](mainnet-388381.json) | 388381 | 784 | 1576 | `dcfea06d4d1707b7639ac3a5235cb259cc293d307034e6a85703e43cbd503240` |
| [mainnet-419328.json](mainnet-419328.json) | 419328 | 1667 | 4398 | `57ab03a751a590b497ee123211a9c1c4fc49fab3ba67a9ee51d85d861564f240` |
| [mainnet-481824.json](mainnet-481824.json) | 481824 | 1866 | 4897 | `724da3c7a56d4c1f485837fe427a3448cf0ed4d514edc875ea17c02f607495c5` |
| [mainnet-692261.json](mainnet-692261.json) | 692261 | 2637 | 6157 | `ca7be2450c01f2d9c39f2c54aaf12498d0ed952a752ea4cf4e3ca3dcf355ae32` |
| [mainnet-709632.json](mainnet-709632.json) | 709632 | 2043 | 6303 | `907f6288eb6563fe654a6b83b12c8f4ec4bb76b722b38866dcb0f786a734364d` |
| [mainnet-91842.json](mainnet-91842.json) | 91842 | 1 | 1 | `ed8181443da98a975a7630f45ca08cede30a94e73a35741a4f35d220ec36cbc3` |
| [mainnet-91880.json](mainnet-91880.json) | 91880 | 2 | 2 | `0787d1c299238fc8d2927e8ce6a541b4bb02e81c8850e7b050c2c36e745b04f1` |
| [signet-1.json](signet-1.json) | 1 | 1 | 0 | `8a8bd6a4fbb96ece64feb60e9c51a9eebc487198e021e559b8381ddde117dc33` |
| [testnet4-1.json](testnet4-1.json) | 1 | 1 | 0 | `2f46da29676c0518d96cbdc6abda8406741b9ae9bc3e4221be09a9c8324f0c60` |

`core-regtest.json` records 35 deterministic crafted blocks against an isolated
Bitcoin Core 32.1.0 daemon, with peers and wallet disabled. Its SHA256 is
`6dbf5a01671a1af0427860ff60553e9f48ea480728a7363078be80dee48e6c59`. The source generator independently
builds each block and its expected reject family; the native oracle compares
Core's actual result and every accepted post-block coin record.

The official arm64 Darwin archive used for the running reference is
`bitcoin-32.1-arm64-apple-darwin.tar.gz`, fetched from
`https://bitcoincore.org/bin/bitcoin-core-32.1/` and checked against its
`SHA256SUMS`: `cd3dcdacb4ff861e862b2d7df4786ecef66472e2468dca1673a70dd761e064e7`.
No upstream scripts are executed.

`networks-first-core.json` records four actual Core comparisons on isolated
testnet4 and signet nodes: a changed body refuses, then the original valid
body for the same header is accepted. SHA256:
`7f88ab7a080b554fcfdc274cfdbdd5deebb87c8eb61a1a31e9a32aa0ae96e88d`.
