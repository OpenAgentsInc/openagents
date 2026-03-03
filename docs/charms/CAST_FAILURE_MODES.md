# CAST Failure Modes

Date: 2026-03-03

## Scope

Deterministic failure classes and remediation for CAST operations run through `skills/cast/scripts`.

## RPC / Node Failures

Symptoms:

- prereq check reports `bitcoin-cli cannot reach bitcoind`
- sign/decode commands fail with RPC errors

Remediation:

1. Verify bitcoind is running and wallet is loaded.
2. Confirm RPC auth and network mode (`mainnet` or `testnet4`) are correct.
3. Re-run `skills/cast/scripts/check-cast-prereqs.sh maker`.

## Prover / Validation Failures

Symptoms:

- `cast-spell-check.sh` or `cast-spell-prove.sh` exits non-zero
- errors mention app mapping, missing UTXO ancestry, or invalid spell fields

Remediation:

1. Confirm `CAST_APP_IDENTITY` and `CAST_APP_BIN` match pinned contract version.
2. Confirm `CAST_PREV_TXS_FILE` includes all ancestry for `ins[].utxo_id`.
3. Confirm `private_inputs.$CAST.params` includes valid operator-signed params.
4. Re-run check before prove.

## Scrolls Signing Failures

Symptoms:

- `cast-sign-and-broadcast.sh` fails in Scrolls signing branch
- HTTP non-success or malformed response from `/sign`

Remediation:

1. Confirm `CAST_SCROLLS_BASE_URL` and request payload are correct.
2. Confirm nonce and input index mapping are correct.
3. Retry with `--dry-run` to preserve artifacts without broadcast.

## Cancellation Signature Failures

Symptoms:

- cancel signature rejected by proving step
- `cancel-msg` output does not match expected cancel input index

Remediation:

1. Regenerate message with exact target `{utxo_id}`.
2. Re-sign with expected derivation path.
3. Ensure signature placed at `private_inputs.$CAST.edit_orders.cancel.<index>` for the canceled input.

## Broadcast Rejections

Symptoms:

- mempool endpoint rejects tx
- returned txid missing from node/mempool lookup

Remediation:

1. Confirm package/signature correctness and fee policy.
2. Decode tx hex and verify inputs are still unspent.
3. Retry broadcast only after confirming replacement/non-conflicting UTXOs.

## Stale / Incomplete `prev_txs`

Symptoms:

- prove path fails with missing previous transaction references

Remediation:

1. Rebuild `CAST_PREV_TXS_FILE` from authoritative source for every referenced input.
2. Hash and compare against previous run receipts to detect drift.
3. Re-run check/prove after refresh.

## Safe Retry Policy

- Always keep first failure artifacts in `run/<timestamp>/`.
- Retry with a new run directory to preserve forensic history.
- Do not broadcast after any failing check/prove/sign stage.
