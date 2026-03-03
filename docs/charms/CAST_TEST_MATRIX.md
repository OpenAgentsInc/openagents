# CAST Test Matrix

Date: 2026-03-03

## Goal

Validate CAST scripts and workflows with deterministic positive and negative scenarios.

## Positive Scenarios

1. Create ask order
- Render `create-ask-order.template.yaml`
- Render `create-ask-order.private-inputs.template.yaml`
- Run `cast-spell-check.sh`
- Run `cast-spell-prove.sh --mock`
- Expect success receipt files.

2. Cancel and replace order
- Migrate `/Users/christopherdavid/code/charms/cast-releases/docs/howto/02-cancel-and-replace-order.yaml` with `cast-migrate-howto-v11.sh`
- Generate cancellation signature with `cast-cancel-signature.sh`
- Run check + mock prove
- Expect signature accepted at configured cancel input index.

3. Partial fulfillment
- Migrate `/Users/christopherdavid/code/charms/cast-releases/docs/howto/03-partial-fulfill.yaml` with `cast-migrate-howto-v11.sh`
- Run check + mock prove
- Verify remainder invariants in rendered spell and decode output.

## Negative Scenarios

1. Bad cancel signature
- Inject an invalid signature hex
- Expect `cast-spell-check.sh` or prove path failure.

2. Wrong nonce/address mapping
- Use incorrect nonce for Scrolls sign request
- Expect Scrolls sign failure.

3. Stale or incomplete `prev_txs`
- Remove one required ancestry transaction
- Expect prove failure mentioning missing ancestry.

4. Invalid fee or output invariants
- Set impossible fee/output values
- Expect check/prove rejection.

5. Missing CAST private inputs
- Omit `--private-inputs-file` for CAST spell
- Expect failure with explicit hint about required private inputs keyed by CAST app identity.

## Smoke Harness

Use the non-broadcast smoke harness:

```bash
skills/cast/scripts/smoke-cast.sh
```

Default behavior:

- no network broadcast
- validates script syntax and template rendering only
- writes a smoke receipt under `run/<timestamp>/receipts/smoke.json` (or `CAST_SMOKE_RECEIPT_FILE` override)

Optional full mode (operator-provided inputs/dependencies required):

- run `cast-spell-check.sh` and `cast-spell-prove.sh --mock` with explicit spell/app/prev-txs inputs
- run `cast-sign-and-broadcast.sh --dry-run` only unless explicit live-broadcast approval is provided

## Pass Criteria

- All positive scenarios return successful JSON receipts.
- All negative scenarios fail with non-zero exit and clear error messages.
- No smoke check performs live broadcast unless explicitly overridden.
