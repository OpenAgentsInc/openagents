# Symphony Query Recipes

## Base checks

```bash
curl -fsS "${SYMPHONY_BASE_URL}/" 
curl -fsS "${SYMPHONY_BASE_URL}/tip" | jq .
```

## Address queries

```bash
ADDR="bc1qxy2kgdygjrsqtzq2n0yrf2493p83kkfjhx0wlh"
curl -fsS "${SYMPHONY_BASE_URL}/addresses/${ADDR}/tx_count" | jq .
curl -fsS "${SYMPHONY_BASE_URL}/addresses/${ADDR}/utxos" | jq .
```

## Runes queries

```bash
curl -fsS "${SYMPHONY_BASE_URL}/addresses/${ADDR}/runes/balances" | jq .
curl -fsS "${SYMPHONY_BASE_URL}/addresses/${ADDR}/runes/utxos" | jq .

curl -fsS -X POST "${SYMPHONY_BASE_URL}/runes/info" \
  -H 'Content-Type: application/json' \
  -d '["840000:1","UNCOMMON•GOODS"]' | jq .
```

## Freshness check

```bash
RPC_PAYLOAD='{"jsonrpc":"1.0","id":"maestro","method":"getblockchaininfo","params":[]}'
BITCOIND_HEIGHT="$(curl -fsS --user "${BITCOIND_RPC_USER}:${BITCOIND_RPC_PASS}" \
  --data-binary "$RPC_PAYLOAD" \
  -H 'content-type: text/plain;' \
  "${BITCOIND_RPC_URL}" | jq -r '.result.blocks')"

SYMPHONY_HEIGHT="$(curl -fsS "${SYMPHONY_BASE_URL}/tip" | jq -r '.height // .block_height // .data.block_height')"
LAG=$((BITCOIND_HEIGHT - SYMPHONY_HEIGHT))
if (( LAG < 0 )); then LAG=0; fi
printf 'lag_blocks=%s\n' "$LAG"
```
