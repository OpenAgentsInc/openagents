# CAST Operator Runbook

Date: 2026-03-03
Status: Draft

## Purpose

Define deterministic operator procedures for CAST operations in OpenAgents:

- create order
- cancel and replace order
- partial fulfill order
- sign, broadcast, and verify transaction outputs

## Contract Lock

Pinned values:

- `CAST_APP_VERSION=v0.2.0`
- `CAST_APP_IDENTITY=b/0000000000000000000000000000000000000000000000000000000000000000/a471d3fcc436ae7cbc0e0c82a68cdc8e003ee21ef819e1acf834e11c43ce47d8`
- `CAST_APP_BIN_NAME=charms-cast-v0.2.0.wasm`
- `CAST_APP_RELEASE_URL=https://github.com/CharmsDev/cast-releases/releases/tag/v0.2.0`
- `CAST_SCROLLS_DEFAULT_BASE_URL` must come from current operator/Scrolls deployment (do not assume legacy `scrolls-v9` hostnames)

Binary integrity policy:

- production/CI must set `CAST_APP_BIN_SHA256` and verify binary hash
- local development may omit hash check for rapid iteration

## Environment Contract

Required:

- `CAST_NETWORK`
- `CAST_SCROLLS_BASE_URL`
- `CAST_APP_BIN`
- `CAST_APP_IDENTITY`
- `CAST_OPERATOR_PARAMS_FILE`
- `CAST_PRIVATE_INPUTS_FILE`
- `CAST_PREV_TXS_FILE`
- `CAST_FUNDING_UTXO` (non-charm BTC input for fees/change in create/cancel-replace flows)
- `CAST_CHANGE_ADDRESS`
- `CAST_FEE_RATE`

Conditionally required:

- `CAST_APP_BIN_SHA256` (required in production/CI)
- `CAST_MEMPOOL_BROADCAST_URL` (required for broadcast lane)
- `CAST_CANCEL_XPRV_FILE` (required for cancellation signing)
- `CAST_CANCEL_DERIVATION_PATH` (required for cancellation signing)
- `BITCOIND_CONTAINER` (optional passthrough for `sign-txs`)

Template rendering keys (for `skills/cast/assets/*.template.yaml`):

- `CAST_APP_INDEX` / `CAST_ASSET_APP_INDEX` (default `0` / `1` when CAST app sorts before asset app)
- `CAST_ORDER_SCROLLS_DEST`, `CAST_REPLACEMENT_SCROLLS_DEST`, `CAST_TAKER_RECEIVE_DEST`, `CAST_ORDER_MAKER_DEST`, `CAST_FEE_DEST`
- `tx.coins[*].dest` values must be hex destination bytes (derive via `charms util dest --addr <address>`)

## Legacy Howto Migration (v9 -> v11)

`/Users/christopherdavid/code/charms/cast-releases/docs/howto/*` still uses legacy spell shape (`version: 9`).
Use the migration helper before running check/prove on Charms v11:

```bash
skills/cast/scripts/cast-migrate-howto-v11.sh \
  --input /Users/christopherdavid/code/charms/cast-releases/docs/howto/03-partial-fulfill.yaml \
  --output-spell ./rendered/03-partial-fulfill.v11.yaml \
  --output-private-inputs ./rendered/03-partial-fulfill.private.v11.yaml
```

Notes:

- CAST runtime params must be in the separate private-inputs file, keyed by full app identity.
- For prove, BTC value must cover outputs and fees. Ensure a funding UTXO is present in `tx.ins` and its parent tx is in `CAST_PREV_TXS_FILE`.

## Artifacts Directory

Per-run shape:

- `run/<timestamp>/inputs/`
- `run/<timestamp>/rendered/`
- `run/<timestamp>/proofs/`
- `run/<timestamp>/signed/`
- `run/<timestamp>/receipts/`

## Receipt Contract

Every CAST helper script writes a JSON receipt under `run/<timestamp>/receipts/` by default unless `--receipt-file` is provided.

Core fields used across receipts:

- `ok`
- `receipt_schema_version` (`cast-receipt/v1`)
- `operation`
- `run_dir`
- `timestamp_utc`
- `network` (where applicable)
- `spell` / `spell_file` (where applicable)
- `prev_txs_file` / `prev_txs_hash` (where applicable)
- `input_pointers` / `input_hashes` (where applicable)
- `output_pointers` / `output_hashes` (where applicable)
- `signed_tx_hex[]` (for sign flow)
- `broadcast_txids[]` (for broadcast flow)
- `spell_decode_summary` (for show-spell verification)

Key script-to-receipt mappings:

- `derive-scrolls-address.sh` -> `derive_scrolls_address.json`
- `cast-spell-check.sh` -> `spell_check.json`
- `cast-spell-prove.sh` -> `spell_prove.json`
- `cast-cancel-signature.sh` -> `cancel_signature.json`
- `cast-sign-and-broadcast.sh` -> `sign_and_broadcast.json`
- `cast-show-spell.sh` -> `show_spell.json`

## Execution Summary

1. Run `skills/cast/scripts/check-cast-prereqs.sh <mode>`.
2. Render spell + private-input templates from `skills/cast/assets/`.
3. Run check + mock prove + real prove.
4. Apply wallet and Scrolls signatures.
5. Broadcast only after explicit confirmation.
6. Verify spell decode and write receipt.

## Safety Constraints

- Never commit secrets or raw xprv material.
- Keep `set -x` disabled in secret-handling scripts.
- Do not broadcast from smoke/test checks by default.

## References

- [CAST failure modes](CAST_FAILURE_MODES.md)
- [CAST test matrix](CAST_TEST_MATRIX.md)
- [CAST oa-bitcoind connection runbook](CAST_OA_BITCOIND_CONNECTION_RUNBOOK.md)
