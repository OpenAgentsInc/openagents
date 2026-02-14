# EP212 Liquidity Bootstrap Log (GCP Self-Hosted LND)

Date: 2026-02-14

This is an operator log of actions taken to (1) fund our self-hosted LND wallet, (2) open an initial Lightning channel for minimal demo liquidity, and (3) validate a basic L402 pay flow once liquidity exists.

## 1) Wallet Funding

- Verified `oa-lnd` wallet received two on-chain deposits of 20,000 sats each.
- Current total (at time of writing): 40,000 sats confirmed.

Command:

```bash
gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo lncli --lnddir=/var/lib/lnd --network=mainnet walletbalance"
```

## 2) UTXOs (For Channel Funding)

Confirmed two UTXOs of 20,000 sats each:

```bash
gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo lncli --lnddir=/var/lib/lnd --network=mainnet listunspent"
```

## 3) Channel Open (Pending)

Next step is to open a first channel (private) with a non-zero `push_amt` to create inbound liquidity immediately.

Notes:

- LND enforces a **minimum channel size of 20,000 sats**.
- Opening a 20,000 sat channel required extra sats for the funding tx fee; funding 40,000 sats total makes this possible.

### 3.1) Attempt: nicehash-ln1 (Rejected)

Target:

- Node: `nicehash-ln1`
- Pubkey: `037659a0ac8eb3b8d0a720114efc861d3a940382dcfa1403746b4f8f6b2e8810ba`

Result:

- Rejected by remote: min chan size required was **0.005 BTC (500,000 sats)**.

### 3.2) Opened: HeldenLight (Funding Broadcast)

Target:

- Node: `HeldenLight`
- Pubkey: `0290cc884704073b2b633f69f852e8ca2a37660bb359a1e861f2b48760c298ac53`

Channel open parameters:

- `local_amt=30000`
- `push_amt=10000` (creates inbound liquidity immediately)
- `private=true`
- `sat_per_vbyte=3`
- `memo=ep212-bootstrap`

Funding transaction:

- `funding_txid=b81d47bbdbda44f9033a9381973d3bfddbdb50063164261c254c349d6129bed5`
- `channel_point=b81d47bbdbda44f9033a9381973d3bfddbdb50063164261c254c349d6129bed5:1`

Pending channel status (immediately after broadcast):

- `capacity=30000`
- `local_balance=19056`
- `remote_balance=10000`
- `confirmations_until_active=3`

## 4) Channel Activated

Polled `pendingchannels` until `confirmations_until_active` reached `0` and the channel moved to `listchannels`.

Verification commands:

```bash
gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo lncli --lnddir=/var/lib/lnd --network=mainnet pendingchannels"

gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo lncli --lnddir=/var/lib/lnd --network=mainnet listchannels"

gcloud compute ssh oa-lnd --zone us-central1-a --tunnel-through-iap --command \
  "sudo lncli --lnddir=/var/lib/lnd --network=mainnet channelbalance"
```

Active channel snapshot:

- `active=true`
- `peer_alias=HeldenLight`
- `channel_point=b81d47bbdbda44f9033a9381973d3bfddbdb50063164261c254c349d6129bed5:1`
- `capacity=30000`
- `local_balance=19056`
- `remote_balance=10000`

## 5) E2E Lightning Payment Check (L402 Buyer Flow)

Goal: verify the node can pay a real Lightning invoice end-to-end (routing + settlement) using the new channel.

Target:

- External L402 endpoint: `https://sats4ai.com/api/l402/text-generation`

Flow:

1. First request returns `402 Payment Required` with an `L402` `www-authenticate` challenge that includes a macaroon and BOLT11 invoice.
2. Decode invoice with `lncli decodepayreq` to verify amount.
3. Pay invoice with `lncli payinvoice` (fee-limited).
4. Re-issue the same HTTP request with `Authorization: L402 <macaroon>:<preimage>` and confirm a `200 OK` response.

Observed:

- Invoice amount: `21 sats`
- Payment succeeded with a sub-1-sat routing fee.
- After payment, channel balances shifted as expected (local decreased by ~22 sats, remote increased by ~22 sats).

Commands (examples):

```bash
# Step 1: get challenge
curl -i -X POST https://sats4ai.com/api/l402/text-generation \
  -H "Content-Type: application/json" \
  -d '{"input":[{"role":"User","content":"Tell me a 1-sentence fact about Bitcoin."}],"model":"Standard"}'

# Step 2: decode invoice
sudo lncli --lnddir=/var/lib/lnd --network=mainnet decodepayreq <BOLT11>

# Step 3: pay (cap routing fees)
sudo lncli --lnddir=/var/lib/lnd --network=mainnet payinvoice \
  --fee_limit=10 --timeout=2m --force <BOLT11>

# Step 4: retry with macaroon + preimage (DO NOT log these long-term)
curl -i -X POST https://sats4ai.com/api/l402/text-generation \
  -H "Content-Type: application/json" \
  -H "Authorization: L402 <MACAROON>:<PREIMAGE>" \
  -d '{"input":[{"role":"User","content":"Tell me a 1-sentence fact about Bitcoin."}],"model":"Standard"}'
```
