# Partial Fulfillment

Use this guide for taker-side partial fills against maker orders with `exec_type.partial`.

## Flow

1. Preflight:

```bash
skills/cast/scripts/check-cast-prereqs.sh taker
```

2. Render partial-fill spell and run check/prove.

```bash
skills/cast/scripts/cast-spell-check.sh \
  --spell ./rendered/partial-fulfill-order.yaml \
  --private-inputs-file ./rendered/partial-fulfill-order.private-inputs.yaml
skills/cast/scripts/cast-spell-prove.sh \
  --spell ./rendered/partial-fulfill-order.yaml \
  --private-inputs-file ./rendered/partial-fulfill-order.private-inputs.yaml \
  --change-address "bc1q..." \
  --mock
```

3. Sign and submit with both wallet and Scrolls signatures.

```bash
skills/cast/scripts/cast-sign-and-broadcast.sh --tx-json ./run/latest/proofs/prove.json --dry-run
```

4. Decode spell tx to verify remainder output.

```bash
skills/cast/scripts/cast-show-spell.sh --tx "<spell_tx_hex>"
```

## Remainder Rules

For partial fills, the remainder output must:

- reference original order via `exec_type.partial.from`
- keep same maker, side, asset, and price
- stay at the same Scrolls address as the original order
- reduce `quantity` and `amount` proportionally to filled size

## Fee Rule

Taker fee formula:

```text
filled_amount * taker_fee / 10000
```

Round/ceiling behavior should follow CAST operator policy and spell proof constraints.

## Failure Hints

- Prove fails on remainder linkage: ensure `exec_type.partial.from` points to original order UTXO.
- Scrolls signing rejects request: re-derive nonce/address and verify same original Scrolls address is used for remainder.
- Fee mismatch: recalculate taker fee (`filled_amount * taker_fee / 10000`) and verify maker payout + fee outputs.
