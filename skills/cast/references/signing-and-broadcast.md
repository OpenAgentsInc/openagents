# Signing And Broadcast

Use this guide for nonce/address derivation, signing, and safe broadcast controls.

Spell reminder for Charms v11:

- `tx.coins[*].dest` must be hex-encoded destination bytes.
- derive destination bytes with `charms util dest --addr <bitcoin_address>`.

## Scrolls Address Derivation

```bash
skills/cast/scripts/derive-scrolls-address.sh \
  --funding-utxo "<txid:vout>" \
  --output-index 0 \
  --scrolls-base-url "${CAST_SCROLLS_BASE_URL}"
```

Expected JSON fields:

- `funding_utxo_id`
- `output_index`
- `nonce`
- `scrolls_address`

## Signing Flow

1. Sign wallet-owned inputs with `sign-txs`.
2. Request Scrolls signatures for Scrolls-controlled inputs.
3. Produce final signed tx hex.

Use `--dry-run` before any broadcast.

## Broadcast Flow

1. Confirm tx hex and expected txid(s).
2. Broadcast through configured endpoint.
3. Persist txid receipt and decode spell for verification.

## Verification

```bash
skills/cast/scripts/cast-show-spell.sh --tx "<spell_tx_hex>"
```

The sign wrapper captures signed tx hex and broadcast txids in its JSON receipt.

## Safety Rules

- No private keys in shell history.
- Prefer file-based secret inputs.
- Persist artifacts before broadcast.
- Require explicit confirmation for live broadcast.

## Failure Hints

- `Broadcast requires explicit confirmation`: pass `--yes-broadcast` only after running `--dry-run`.
- `No signed bitcoin tx hex values found`: inspect `sign-txs` output structure and confirm `.bitcoin` fields are present.
- Scrolls `/sign` HTTP errors: validate `CAST_SCROLLS_BASE_URL` and sign request payload.
