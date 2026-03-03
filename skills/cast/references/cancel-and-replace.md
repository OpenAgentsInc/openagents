# Cancel And Replace

Use this guide when a maker wants to replace an existing order in one transaction.

## Cancellation Signature Contract

Message format:

```text
{utxo_id} {outputs_hash}
```

Where `outputs_hash` is derived from the spell outputs payload expected by CAST.

## Flow

1. Build cancellation message + signature:

```bash
skills/cast/scripts/cast-cancel-signature.sh \
  --spell ./rendered/cancel-replace-order.yaml \
  --cancel-utxo "<txid:vout>" \
  --xprv-file ./secrets/maker.xprv \
  --path "0/0"
```

2. Inject signature into `private_inputs.$CAST.edit_orders.cancel.<input_index>`.

3. Run check/prove via `cast-spell-check.sh` and `cast-spell-prove.sh`.

## Invariants To Enforce

- Cancel signature index must match the canceled order input index.
- New replacement order output must satisfy CAST contract fields.
- If replacement supports partial fills, `exec_type.partial` must be explicit.

## Failure Hints

- `Cancellation message must match "{utxo_id} {outputs_hash}"`: validate `--cancel-utxo` and spell output hash source.
- `xprv file is empty`: pass a file-backed xprv with no extra whitespace/newlines.
- Signature rejected in prove: confirm `private_inputs.$CAST.edit_orders.cancel.<index>` matches canceled input index.
