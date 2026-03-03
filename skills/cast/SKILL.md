---
name: cast
description: Charms CAST DEX workflows for order creation, cancellation/replacement, partial fulfillment, signing, and Bitcoin transaction verification.
metadata:
  oa:
    project: cast
    identifier: cast
    version: "0.1.0"
    expires_at_unix: 1798761600
    capabilities:
      - http:outbound
      - filesystem:read
      - process:spawn
---

# Cast

## Overview

Use this skill when a task requires executable CAST DEX operations on Bitcoin with Charms, including order lifecycle management, Scrolls nonce/address derivation, cancellation signatures, partial fills, signing, and transaction verification.

## Environment

Required commands:

- `bash`, `curl`, `jq`, `envsubst`
- `charms`, `bitcoin-cli`
- `scrolls-nonce`, `sign-txs`, `cancel-msg`

Required artifacts/services:

- CAST app binary (`charms-cast-v0.2.0.wasm` or latest v11-compatible CAST build)
- Operator-signed `fulfill` params payload
- Scrolls API base URL
- `prev_txs` ancestry data for all spell inputs
- fee funding input UTXO for maker flows (`CAST_FUNDING_UTXO`)
- v11 spell file (`version: 11`, `tx.*`, `app_public_inputs`)
- separate private-inputs file passed via `--private-inputs`

## Workflow

1. Run preflight checks for your path:
- `scripts/check-cast-prereqs.sh maker`
- `scripts/check-cast-prereqs.sh taker`
- `scripts/check-cast-prereqs.sh cancel`
- `scripts/check-cast-prereqs.sh server`

2. Follow [order-lifecycle](references/order-lifecycle.md) for create/check/prove flow.

3. For maker edits, follow [cancel-and-replace](references/cancel-and-replace.md).

4. For taker fills, follow [partial-fulfillment](references/partial-fulfillment.md).

5. For signing and broadcast controls, follow [signing-and-broadcast](references/signing-and-broadcast.md).
6. For repeated autonomous execution, follow [autotrade-loop](references/autotrade-loop.md).

7. Keep operations deterministic:
- prefer file-backed inputs over inline shell literals
- use dry-run first for mutation steps
- persist artifacts and receipts for every run
- encode `tx.coins[*].dest` as hex destination bytes (derive via `charms util dest --addr ...`)

## Quick Commands

```bash
# Preflight
skills/cast/scripts/check-cast-prereqs.sh maker

# Derive Scrolls nonce + address
skills/cast/scripts/derive-scrolls-address.sh \
  --funding-utxo "<txid:vout>" \
  --output-index 0 \
  --scrolls-base-url "${CAST_SCROLLS_BASE_URL}"

# Migrate legacy CAST howto spell to v11 (split private inputs + convert coin dests)
skills/cast/scripts/cast-migrate-howto-v11.sh \
  --input /Users/christopherdavid/code/charms/cast-releases/docs/howto/03-partial-fulfill.yaml \
  --output-spell ./rendered/03-partial-fulfill.v11.yaml \
  --output-private-inputs ./rendered/03-partial-fulfill.private.v11.yaml

# Check + prove
skills/cast/scripts/cast-spell-check.sh \
  --spell ./rendered/create-order.yaml \
  --private-inputs-file ./rendered/create-order.private-inputs.yaml \
  --app-bin "${CAST_APP_BIN}" \
  --prev-txs-file "${CAST_PREV_TXS_FILE}"
skills/cast/scripts/cast-spell-prove.sh \
  --spell ./rendered/create-order.yaml \
  --private-inputs-file ./rendered/create-order.private-inputs.yaml \
  --app-bin "${CAST_APP_BIN}" \
  --prev-txs-file "${CAST_PREV_TXS_FILE}" \
  --change-address "bc1q..." \
  --mock
skills/cast/scripts/cast-spell-prove.sh \
  --spell ./rendered/create-order.yaml \
  --private-inputs-file ./rendered/create-order.private-inputs.yaml \
  --app-bin "${CAST_APP_BIN}" \
  --prev-txs-file "${CAST_PREV_TXS_FILE}" \
  --change-address "bc1q..."

# Sign + inspect
skills/cast/scripts/cast-sign-and-broadcast.sh --tx-json ./proofs/tx_to_sign.json --dry-run
skills/cast/scripts/cast-show-spell.sh --tx "<spell_tx_hex>"

# Run one automated iteration (safe defaults: mock prove + dry-run sign)
skills/cast/scripts/cast-autotrade-loop.sh \
  --config skills/cast/assets/autotrade-loop.config.example \
  --once

# Run continuous loop (explicitly controlled)
skills/cast/scripts/cast-autotrade-loop.sh \
  --config /absolute/path/to/autotrade.env \
  --interval-seconds 45 \
  --max-iterations 0 \
  --continue-on-error
```

## References

- [order-lifecycle](references/order-lifecycle.md)
- [cancel-and-replace](references/cancel-and-replace.md)
- [partial-fulfillment](references/partial-fulfillment.md)
- [signing-and-broadcast](references/signing-and-broadcast.md)
- [autotrade-loop](references/autotrade-loop.md)
