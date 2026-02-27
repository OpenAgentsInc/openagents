---
name: charms
description: Charms workflows for Bitcoin app contracts, spell proving, and UTXO asset operations.
metadata:
  oa:
    project: charms
    identifier: charms
    version: "0.1.0"
    expires_at_unix: 1798761600
    capabilities:
      - http:outbound
      - filesystem:read
      - process:spawn
---

# Charms

## Overview

Integrate and operate Charms for programmable Bitcoin assets. Use this skill when a task requires Charms app development, spell creation and proof generation, commit and spell transaction submission, spell inspection on existing transactions, wallet charm inventory, or API and wallet integration.

## Environment

- Requires `bash`, `curl`, and `jq`.
- Requires `charms` CLI.
- For app development, requires Rust and `wasm32-wasip1` target.
- For Bitcoin casting and wallet flows, requires `bitcoin-cli` connected to a node (testnet4 for quick iteration).

Use this skill for concrete implementation and operations, not generic protocol theory.

## Workflow

1. Pick the execution path first:
- App and spell lifecycle (new app, build, verify key, check, prove, submit).
- Spell schema and validation debugging.
- Wallet and API integration (`wallet list`, `tx show-spell`, `server` endpoint).

2. Run preflight checks:
- `scripts/check-charms-prereqs.sh app`
- `scripts/check-charms-prereqs.sh spell`
- `scripts/check-charms-prereqs.sh wallet`
- `scripts/check-charms-prereqs.sh server`

3. For app and spell operations, follow [app-and-spell-workflow](references/app-and-spell-workflow.md):
- Scaffold app with `charms app new`.
- Build and derive app verification key.
- Validate spells with `charms spell check`.
- Produce package-ready transactions with `charms spell prove`.

4. For schema and validation issues, use [spell-format-and-validation](references/spell-format-and-validation.md):
- Confirm app identifiers and VKs.
- Confirm `ins` and `outs` line up with transaction reality.
- Keep private inputs off-chain and pass them through the private input file path.

5. For wallet and API surfaces, use [wallet-and-server-integration](references/wallet-and-server-integration.md):
- Inspect wallets for charm-bearing outputs.
- Decode spell content from known transactions.
- Run `charms server` and call `/spells/prove` in JSON or CBOR mode.

6. Apply execution safety constraints:
- Use low-value UTXOs and testnet4 while iterating.
- Never submit package transactions before validating both commit and spell tx hex.
- Keep prover and wallet secrets out of logs and source control.

## Quick Commands

```bash
# App scaffold and build
charms app new my-token
cd my-token
app_bin="$(charms app build)"
charms app vk "$app_bin"

# Spell validation and proving
cat ./spells/mint-nft.yaml | envsubst | charms spell check --app-bins="$app_bin" --prev-txs="$prev_txs"
cat ./spells/mint-nft.yaml | envsubst | charms spell prove --app-bins="$app_bin" --prev-txs="$prev_txs" --funding-utxo="$funding_utxo" --funding-utxo-value="$funding_utxo_value" --change-address="$change_address"

# Wallet and tx inspection
charms wallet list --json
charms tx show-spell --chain bitcoin --tx "$tx_hex" --json

# API server
charms server --ip 0.0.0.0 --port 17784
```

## Reference Files

- [app-and-spell-workflow](references/app-and-spell-workflow.md): end-to-end app scaffold, check, prove, sign, and package submission.
- [spell-format-and-validation](references/spell-format-and-validation.md): spell fields, app mapping, proof and version checks, and frequent failure modes.
- [wallet-and-server-integration](references/wallet-and-server-integration.md): wallet parsing, tx inspection, API server usage, and wallet UI integration path.
