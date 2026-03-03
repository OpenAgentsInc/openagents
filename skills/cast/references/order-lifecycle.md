# Order Lifecycle

Use this guide for maker order creation and the standard check/prove loop.

## Inputs Required

- CAST app binary (`CAST_APP_BIN`)
- operator params payload (`CAST_OPERATOR_PARAMS_FILE`)
- previous tx ancestry (`CAST_PREV_TXS_FILE`)
- funding UTXO/value and change address
- spell template rendered to final YAML

## Core Flow

1. Run preflight:

```bash
skills/cast/scripts/check-cast-prereqs.sh maker
```

2. Validate spell structure before proof generation:

```bash
skills/cast/scripts/cast-spell-check.sh --spell ./rendered/create-ask-order.yaml
```

3. Run mock prove first:

```bash
skills/cast/scripts/cast-spell-prove.sh --spell ./rendered/create-ask-order.yaml --mock
```

4. Generate real proof artifacts:

```bash
skills/cast/scripts/cast-spell-prove.sh --spell ./rendered/create-ask-order.yaml
```

5. Optional sign/dry-run broadcast lane:

```bash
skills/cast/scripts/cast-sign-and-broadcast.sh --tx-json ./run/latest/proofs/prove.json --dry-run
```

## Invariants To Enforce

- `version` must match the runtime-supported Charms spell version.
- `apps.$CAST` must match pinned CAST contract identity.
- `private_inputs.$CAST.params` must contain signed operator params.
- Each input UTXO in `ins` must be satisfiable from supplied `prev_txs`.
- Order outputs must satisfy CAST value/quantity/price invariants.

## Failure Hints

- `App bin hash mismatch`: set `CAST_APP_BIN_SHA256` to the expected SHA-256 of `CAST_APP_BIN`.
- `prev_txs file is empty` or missing ancestry errors: rebuild `CAST_PREV_TXS_FILE` to include all parent tx hex.
- `Real prove requires funding_utxo`: provide `CAST_FUNDING_UTXO`, `CAST_FUNDING_UTXO_VALUE`, `CAST_CHANGE_ADDRESS`.
