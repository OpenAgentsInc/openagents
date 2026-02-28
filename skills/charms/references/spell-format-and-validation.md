# Spell Format And Validation

Use this reference when authoring spells or debugging validation and proving failures.

## Source Of Truth

- `~/code/charms/charms-docs/src/content/docs/references/spell-json.md`
- `~/code/charms/charms-docs/src/content/docs/concepts/spells.md`
- `~/code/charms/charms/src/cli/spell.rs`
- `~/code/charms/charms/src/cli/mod.rs`

## Core Spell Fields

Top-level spell sections:
- `version`
- `apps`
- `public_inputs` (optional)
- `private_inputs` (optional and not written on-chain)
- `ins`
- `outs`

Use `charms spell vk` to read the currently supported spell version and spell verification key:

```bash
charms spell vk
```

Treat this output as authoritative for the prover/runtime you are using.

## App Mapping Rules

`apps` entries map short app aliases (`$00`, `$01`, ...) to `tag/identity/vk`:
- tag `n` for NFT-style app data
- tag `t` for fungible token amount data

Every app reference used in `ins`, `outs`, `public_inputs`, or `private_inputs` must exist in `apps`.

## Validation Checklist

Before proving:
1. Confirm every `ins[].utxo_id` appears in `--prev-txs` ancestry.
2. Confirm every app that needs a contract proof has its binary in `--app-bins`.
3. Confirm private inputs are passed through `--private-inputs` and not embedded into public spell payload.
4. Confirm `outs` count and charm placement are coherent with the target Bitcoin transaction outputs.
5. Confirm `fee_rate >= 1.0` for proving paths.

## Useful Commands

```bash
# Validate without producing tx artifacts
charms spell check \
  --spell ./spells/send.yaml \
  --app-bins "$app_bin" \
  --prev-txs "$prev_txs"

# Generate commit + spell tx artifacts
charms spell prove \
  --spell ./spells/send.yaml \
  --app-bins "$app_bin" \
  --prev-txs "$prev_txs" \
  --funding-utxo "$funding_utxo" \
  --funding-utxo-value "$funding_utxo_value" \
  --change-address "$change_address"
```

## Common Failure Modes

- Missing or wrong app binary for an app used by the spell.
- `--prev-txs` missing one of the transactions that created referenced inputs.
- Spell content shape mismatch (`apps` map does not match `ins` or `outs` references).
- Invalid fee rate (`< 1.0`) during proving.
- Address or UTXO data rendered from stale environment variables.

## Transfer Optimization Note

Charms docs note optimized proof paths for simple transfers where tags `n` and `t` can avoid full app proofs. Do not assume this applies to minting or app-state transitions; keep explicit app binaries for contract logic changes.
