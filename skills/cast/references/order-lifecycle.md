# Order Lifecycle

Use this guide for maker order creation and the standard check/prove loop.

## Inputs Required

- CAST app binary (`CAST_APP_BIN`)
- operator params payload (`CAST_OPERATOR_PARAMS_FILE`)
- previous tx ancestry (`CAST_PREV_TXS_FILE`)
- fee funding UTXO included in spell inputs (`CAST_FUNDING_UTXO`)
- change address (`CAST_CHANGE_ADDRESS`)
- v11 spell template rendered to final YAML
- private-inputs YAML rendered separately (`CAST_PRIVATE_INPUTS_FILE`)

## Core Flow

1. Run preflight:

```bash
skills/cast/scripts/check-cast-prereqs.sh maker
```

2. If starting from legacy CAST howto spells (`version: 9`), migrate first:

```bash
skills/cast/scripts/cast-migrate-howto-v11.sh \
  --input /Users/christopherdavid/code/charms/cast-releases/docs/howto/03-partial-fulfill.yaml \
  --output-spell ./rendered/03-partial-fulfill.v11.yaml \
  --output-private-inputs ./rendered/03-partial-fulfill.private.v11.yaml
```

3. Validate spell structure before proof generation:

```bash
skills/cast/scripts/cast-spell-check.sh \
  --spell ./rendered/create-ask-order.yaml \
  --private-inputs-file ./rendered/create-ask-order.private-inputs.yaml \
  --app-bin "${CAST_APP_BIN}" \
  --prev-txs-file "${CAST_PREV_TXS_FILE}"
```

4. Run mock prove first:

```bash
skills/cast/scripts/cast-spell-prove.sh \
  --spell ./rendered/create-ask-order.yaml \
  --private-inputs-file ./rendered/create-ask-order.private-inputs.yaml \
  --app-bin "${CAST_APP_BIN}" \
  --prev-txs-file "${CAST_PREV_TXS_FILE}" \
  --change-address "bc1q..." \
  --mock
```

5. Generate real proof artifacts:

```bash
skills/cast/scripts/cast-spell-prove.sh \
  --spell ./rendered/create-ask-order.yaml \
  --private-inputs-file ./rendered/create-ask-order.private-inputs.yaml \
  --app-bin "${CAST_APP_BIN}" \
  --prev-txs-file "${CAST_PREV_TXS_FILE}" \
  --change-address "bc1q..."
```

6. Optional sign/dry-run broadcast lane:

```bash
skills/cast/scripts/cast-sign-and-broadcast.sh --tx-json ./run/latest/proofs/prove.json --dry-run
```

## Invariants To Enforce

- `version` must be `11` for Charms v11.
- `app_public_inputs` keys must be sorted and include pinned CAST app identity.
- private inputs must be passed via a separate `--private-inputs` file.
- `tx.ins` must be satisfiable from supplied `prev_txs`.
- `tx.coins[*].dest` must be hex destination bytes (not addresses).
- Order outputs must satisfy CAST value/quantity/price invariants.

## Failure Hints

- `App bin hash mismatch`: set `CAST_APP_BIN_SHA256` to the expected SHA-256 of `CAST_APP_BIN`.
- `prev_txs file is empty` or missing ancestry errors: rebuild `CAST_PREV_TXS_FILE` to include all parent tx hex.
- `missing field tx`: spell is still in pre-v11 shape; migrate to `version: 11` + `tx` + `app_public_inputs`.
- `--change-address` required: set `CAST_CHANGE_ADDRESS` even in mock mode.
