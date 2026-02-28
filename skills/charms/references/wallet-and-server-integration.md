# Wallet And Server Integration

Use this reference when integrating Charms into wallet UX or backend services.

## Source Of Truth

- `~/code/charms/charms/src/cli/wallet.rs`
- `~/code/charms/charms/src/cli/tx.rs`
- `~/code/charms/charms/src/cli/server.rs`
- `~/code/charms/charms-docs/src/content/docs/guides/wallet-integration/visualization.md`

## Wallet Inventory

List wallet outputs that contain charms:

```bash
charms wallet list --json
```

The wallet command reads local unspent outputs from `bitcoin-cli listunspent 0`, extracts spells from source transactions, and returns:
- `apps`: index-to-app map
- `outputs`: UTXO map with sats, confirmations, and charm payloads

This is the fastest way to build wallet-side inventory and rendering pipelines.

## Spell Inspection

Decode and verify spell metadata from a known Bitcoin transaction:

```bash
charms tx show-spell --chain bitcoin --tx "$tx_hex" --json
```

Use this for:
- explorer backfills
- debugging a single transaction
- validating that produced transactions embed the expected spell

## API Server (Proving Endpoint)

Run local Charms server:

```bash
charms server --ip 0.0.0.0 --port 17784
```

Server behavior from CLI source:
- `GET /ready` health check
- `POST /spells/prove` proof and tx generation endpoint
- accepts JSON or CBOR request body
- returns JSON or CBOR based on request content type

Health check:

```bash
curl -sS http://127.0.0.1:17784/ready
```

For `POST /spells/prove`, use the same `ProveRequest` structure used by CLI proving flow and keep payload generation centralized in one serializer in your app.

## Wallet UI Integration Pattern

For wallet visualization flows:
1. Use `charms wallet list --json` for local wallet-owned outputs.
2. Use `charms tx show-spell` for transaction drill-down pages.
3. For browser or service-side parsing, use `charms_lib.wasm` bindings where needed.
4. Normalize NFT-like data and fungible token amounts into separate renderer branches.

Do not assume every output has charms data; treat charms as optional metadata attached to UTXOs.
