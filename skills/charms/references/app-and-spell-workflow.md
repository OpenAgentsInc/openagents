# App And Spell Workflow

Use this reference for the standard Charms build and transaction flow.

## Source Of Truth

- `~/code/charms/README.md`
- `~/code/charms/charms/README.md`
- `~/code/charms/charms/src/cli/mod.rs`
- `~/code/charms/charms-docs/src/content/docs/guides/charms-apps/pre-reqs.md`
- `~/code/charms/charms-docs/src/content/docs/guides/charms-apps/get-started.md`
- `~/code/charms/charms-docs/src/content/docs/guides/charms-apps/cast-spell.md`

## Prerequisites

1. Install and run Bitcoin Core (`testnet4` recommended for development):
- `server=1`
- `testnet4=1`
- `txindex=1`
- `addresstype=bech32m`
- `changetype=bech32m`

2. Ensure wallet is loaded:

```bash
bitcoin-cli createwallet testwallet
bitcoin-cli loadwallet testwallet
```

3. Install dependencies:

```bash
brew install jq
rustup target add wasm32-wasip1
export CARGO_TARGET_DIR="$(mktemp -d)/target"
cargo install --locked charms
unset CARGO_TARGET_DIR
```

4. Fund test wallet with enough UTXOs for fee + data outputs.

## App Lifecycle

Create and build a new Charms app:

```bash
charms app new my-token
cd my-token
cargo update
app_bin="$(charms app build)"
app_vk="$(charms app vk "$app_bin")"
```

`charms app build` compiles to `wasm32-wasip1` and prints the Wasm path.

## Spell Check (Fast Iteration)

Use `charms spell check` before proving:

```bash
cat ./spells/mint-nft.yaml | envsubst | charms spell check \
  --app-bins="$app_bin" \
  --prev-txs="$prev_txs"
```

Check mode validates spell structure, app binary mapping, previous transactions, and proof constraints without producing package tx hex.

## Spell Prove (Package Generation)

Generate commit + spell transactions:

```bash
cat ./spells/mint-nft.yaml | envsubst | charms spell prove \
  --app-bins="$app_bin" \
  --prev-txs="$prev_txs" \
  --funding-utxo="$funding_utxo" \
  --funding-utxo-value="$funding_utxo_value" \
  --change-address="$change_address"
```

Output is a JSON array of tx hex values that must be signed and submitted together.

Expected behavior:
- Bitcoin: array of two tx hex entries (commit tx then spell tx).

## Submission And Verification

1. Sign txs with your wallet flow.
2. Submit both txs as a package:

```bash
bitcoin-cli submitpackage '["<commit_hex>", "<spell_hex>"]'
```

3. Inspect result:

```bash
charms tx show-spell --chain bitcoin --tx "<spell_tx_hex>" --json
```

## Agent Automation Pattern

For deterministic automation loops:
1. Load app binaries and compute/verify VK upfront.
2. Resolve and lock exact funding UTXO + change address.
3. Render spell template with explicit env vars.
4. Run `spell check`; fail fast on mismatch.
5. Run `spell prove`; parse JSON output into commit/spell artifacts.
6. Gate signing and broadcasting behind explicit confirmation policy.
