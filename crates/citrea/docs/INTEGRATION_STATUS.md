# Citrea CLI + crate status (OpenAgents2)

## What shipped

### New crate: `openagents-citrea`
Location: `crates/citrea/`

Core utilities implemented:
- BIP340 Schnorr sign/verify helpers (reuses `bitcoin::secp256k1`).
- NIP-06 compatible key derivation via `nostr::derive_keypair_full`.
- EOA address derivation (keccak over uncompressed pubkey).
- CREATE2 address derivation.
- ERC20 `balanceOf`/`transfer` calldata helpers.
- JSON-RPC client with Citrea/EVM methods:
  - `eth_chainId`, `eth_blockNumber`
  - `eth_getBalance`, `eth_getTransactionCount`
  - `eth_call`, `eth_sendRawTransaction`
  - `eth_getTransactionReceipt`
  - `citrea_sendRawDepositTransaction`
  - `txpool_content`

Relevant files:
- `crates/citrea/src/lib.rs`
- `crates/citrea/src/keys.rs`
- `crates/citrea/src/address.rs`
- `crates/citrea/src/rpc.rs`
- `crates/citrea/src/util.rs`

### CLI: `oa citrea ...`
Location: `crates/openagents-cli/src/citrea_cli.rs`

New subcommand wired into `openagents-cli` (`oa` / `openagents`).

Key commands (offline):
- `oa citrea new`
- `oa citrea derive`
- `oa citrea seed`
- `oa citrea pubkey`
- `oa citrea sign`
- `oa citrea verify`
- `oa citrea address eoa`
- `oa citrea address create2`
- `oa citrea address pubkey`

RPC commands (no network tests here):
- `oa citrea chain info`
- `oa citrea balance --address ... [--token ...]`
- `oa citrea nonce --address ...`
- `oa citrea call --to ... --data ...`
- `oa citrea send --raw ...`
- `oa citrea receipt --tx-hash ...`
- `oa citrea deposit submit --raw ...` (Citrea RPC)
- `oa citrea txpool`

Env fallbacks:
- `CITREA_RPC` / `CITREA_RPC_URL` for RPC URL
- `CITREA_MNEMONIC` / `OPENAGENTS_MNEMONIC` for key derivation

## Tests added

### New crate tests
- `crates/citrea/tests/citrea_tests.rs`
  - Schnorr sign/verify roundtrip
  - EOA address derivation (privkey = 1)
  - ERC20 balanceOf calldata encoding

### CLI tests
- `crates/openagents-cli/src/citrea_cli.rs` basic clap parse test

## Tests run locally

- `cargo test -p openagents-citrea`
- `cargo test -p openagents-cli`

## What’s next (recommended)

1) **RPC integration tests (local-only)**
   - Add a local JSON-RPC stub for `eth_*` and `citrea_*` methods.
   - Validate request payloads and response parsing for `balance`, `call`, `deposit submit`.

2) **Smart-account address derivation**
   - Once the OpenAgents Citrea smart-account factory + init code are defined,
     add a deterministic `oa citrea address smart` command (CREATE2 wrapper with
     factory + init-code hash).

3) **TreasuryRouter + receipts**
   - Wire Citrea actions into receipts (REPLAY/ARTIFACTS) once TreasuryRouter
     is available in OpenAgents2.

4) **Account abstraction flow**
   - Add commands for:
     - constructing meta-tx payloads
     - Schnorr signature for smart-account auth
     - optional passkey / secp256r1 co-sign

5) **Token helpers**
   - Add ERC20 `decimals`, `symbol`, `name` helpers in CLI.

