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
- `CAST_SCROLLS_DEFAULT_BASE_URL=https://scrolls-v9.charms.dev/main`

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
- `CAST_PREV_TXS_FILE`
- `CAST_FUNDING_UTXO`
- `CAST_FUNDING_UTXO_VALUE`
- `CAST_CHANGE_ADDRESS`
- `CAST_FEE_RATE`

Conditionally required:

- `CAST_APP_BIN_SHA256` (required in production/CI)
- `CAST_MEMPOOL_BROADCAST_URL` (required for broadcast lane)
- `CAST_CANCEL_XPRV_FILE` (required for cancellation signing)
- `CAST_CANCEL_DERIVATION_PATH` (required for cancellation signing)
- `BITCOIND_CONTAINER` (optional passthrough for `sign-txs`)

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
2. Render spell template from `skills/cast/assets/`.
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
