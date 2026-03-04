# CAST to `oa-bitcoind` Connection Runbook

Date: 2026-03-04  
Status: tested against live infra

## Purpose

Connect local CAST tooling (`charms`, `sign-txs`, `bitcoin-cli`) to the existing OpenAgents Bitcoin Core node (`oa-bitcoind`) and run the CAST loop as far as currently possible.

## Authoritative Infra References

Primary repo sources:

- `docs/deploy/SYMPHONY_GCP_RUNBOOK.md`
- `scripts/deploy/symphony/common.sh`
- `docs/MAESTRO_SYMPHONY_INFRA_AND_DEPLOYMENT.md` (section 15.11)

Backroom confirmation:

- `/Users/christopherdavid/code/backroom/openagents-doc-archive/2026-02-21-stale-doc-pass-2/docs/lightning/status/20260215-current-status.md`

## Known Node Contract

- Project: `openagentsgemini`
- Zone: `us-central1-a`
- VM: `oa-bitcoind`
- Internal RPC: `10.42.0.2:8332`
- RPC creds secret: `oa-bitcoind-rpc-creds` (`rpcuser=...`, `rpcpassword=...`)

## 1) Authenticate `gcloud`

```bash
gcloud auth login --brief
gcloud config set project openagentsgemini
gcloud config set compute/zone us-central1-a
```

## 2) Verify node health over IAP SSH

```bash
gcloud compute ssh oa-bitcoind \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  --command "sudo -u bitcoin bitcoin-cli -conf=/etc/bitcoin/bitcoin.conf -datadir=/var/lib/bitcoin getblockchaininfo"
```

Expected: `chain=main`, `blocks == headers`, `initialblockdownload=false`.

## 3) Create local RPC tunnel

Use a local port that does not conflict with local bitcoind (`18443` used here):

```bash
nohup gcloud compute ssh oa-bitcoind \
  --project openagentsgemini \
  --zone us-central1-a \
  --tunnel-through-iap \
  -- -N -L 127.0.0.1:18443:127.0.0.1:8332 \
  >/tmp/oa-bitcoind-iap.log 2>&1 &
```

Check listener:

```bash
lsof -nP -iTCP:18443 -sTCP:LISTEN
```

## 4) Fetch RPC creds and test local RPC path

```bash
creds="$(gcloud secrets versions access latest \
  --secret oa-bitcoind-rpc-creds \
  --project openagentsgemini)"
rpc_user="$(printf '%s\n' "$creds" | awk -F= '/^rpcuser=/{print $2}' | tail -n1)"
rpc_pass="$(printf '%s\n' "$creds" | awk -F= '/^rpcpassword=/{print $2}' | tail -n1)"

bitcoin-cli \
  -rpcconnect=127.0.0.1 \
  -rpcport=18443 \
  -rpcuser="$rpc_user" \
  -stdinrpcpass \
  getblockchaininfo <<<"$rpc_pass"
```

## 5) Make CAST tools use the tunnel automatically

`sign-txs` shells out to `bitcoin-cli` without custom RPC flags.  
Use a temporary wrapper at the front of `PATH`:

```bash
mkdir -p /tmp/cast-bin
cat >/tmp/cast-bin/bitcoin-cli <<'EOF'
#!/usr/bin/env bash
set -euo pipefail
: "${OA_BITCOIND_RPC_USER:?required}"
: "${OA_BITCOIND_RPC_PASS:?required}"
exec /opt/homebrew/bin/bitcoin-cli \
  -rpcconnect=127.0.0.1 \
  -rpcport=18443 \
  -rpcuser="$OA_BITCOIND_RPC_USER" \
  -stdinrpcpass "$@" <<<"$OA_BITCOIND_RPC_PASS"
EOF
chmod +x /tmp/cast-bin/bitcoin-cli

export OA_BITCOIND_RPC_USER="$rpc_user"
export OA_BITCOIND_RPC_PASS="$rpc_pass"
export PATH="/tmp/cast-bin:$PATH"
```

Sanity check:

```bash
skills/cast/scripts/check-cast-prereqs.sh maker
```

## 6) CAST v11 migration + one loop iteration

Download official CAST app binary:

```bash
gh release download v0.2.0 \
  --repo CharmsDev/cast-releases \
  --pattern 'charms-cast-v0.2.0.wasm' \
  --dir /tmp/cast-oa-bitcoind-test
```

Migrate legacy howto spell to v11:

```bash
skills/cast/scripts/cast-migrate-howto-v11.sh \
  --input /Users/christopherdavid/code/charms/cast-releases/docs/howto/02-cancel-and-replace-order.yaml \
  --output-spell /tmp/cast-oa-bitcoind-test/rendered/02-cancel-and-replace-order.v11.yaml \
  --output-private-inputs /tmp/cast-oa-bitcoind-test/rendered/02-cancel-and-replace-order.private.v11.yaml
```

Run one dry-run loop (`check,prove,sign`):

```bash
skills/cast/scripts/cast-autotrade-loop.sh \
  --config /tmp/cast-oa-bitcoind-test/autotrade.env \
  --stages check,prove,sign \
  --once
```

## Observed Results (2026-03-04)

- `oa-bitcoind` reachable and synced via IAP tunnel.
- `getindexinfo.txindex.synced=true` on remote node.
- CAST `check`, `prove --mock`, and `prove` (real) succeeded using migrated v11 spell.
- `cast-show-spell.sh` successfully decoded the real-prove transaction spell payload.
- `sign` stage failed on live node path:
  - `sign-txs` fails on RPC `signrawtransactionwithwallet` with `-32601 Method not found`.
  - Direct RPC confirms wallet methods unavailable on this node:
    - `listwallets` -> `Method not found`
    - `signrawtransactionwithwallet` -> `Method not found`

Current maximum validated path with `oa-bitcoind`:

- `check -> prove(real) -> inspect` works.
- `sign` and therefore `broadcast` do not work against this node as configured.

## What Is Needed For Full Trading

The current `oa-bitcoind` is suitable for chain/index reads, but not wallet signing.

To complete full loop (`check -> prove(real) -> sign -> scrolls sign -> broadcast`) one of these is required:

1. Provide a signer-capable Bitcoin RPC endpoint (wallet RPC enabled) and point `sign-txs`/`bitcoin-cli` to it.
2. Extend signer flow to use `signrawtransactionwithkey` and provide key material securely.

Also required for live execution:

- Live, unspent funding/order UTXOs.
- Matching `prev_txs` ancestry for those UTXOs.
- Valid CAST private inputs/operator params for current market policy.
- Scrolls sign request/nonce inputs for Scrolls-controlled order inputs.
- Explicit broadcast endpoint approval (`CAST_MEMPOOL_BROADCAST_URL` + `--yes-broadcast`).

## Cleanup

Stop the tunnel if needed:

```bash
pkill -f "gcloud compute ssh oa-bitcoind.*127.0.0.1:18443:127.0.0.1:8332" || true
```
