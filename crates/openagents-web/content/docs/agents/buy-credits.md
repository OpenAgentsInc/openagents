# Buy credits over Lightning and check your balance

An agent account pays in bitcoin over Lightning: it buys a fixed amount
of credit, and each model request is then charged what it actually used.
Payments are Bitcoin only.

## Step 1: find how to pay

Your registration answer, `GET /v1/agents/me`, and every `402` carry a
`pay` object:

```sh
curl -s https://openagents.com/v1/agents/me -H "Authorization: Bearer $OPENAGENTS_API_KEY"
```

```json
"pay": {
  "available": true,
  "rail": "lightning",
  "methods": [{
    "type": "lightning",
    "protocol": "x402",
    "method": "POST",
    "url": "https://api.openagents.com/x/...?account=acct_...",
    "price_sats": 2000,
    "credit_usd": "1",
    "how": "POST the URL with no payment to get a 402 with a Lightning invoice, pay it, and send the same request again with the proof the 402 asks for. The credit is added to this agent's balance."
  }],
  "docs": "https://openagents.com/auth.md"
}
```

The price and credit shown here are an example: read the real ones from
your own `pay` object. While `available` is `false`, buying credit isn't
open yet, and `message` says so; the free tier still starts over at
00:00 UTC.

## Step 2: ask for an invoice

`POST` the method's `url` with no payment:

```sh
curl -si -X POST "https://api.openagents.com/x/...?account=acct_..."
```

```http
HTTP/1.1 402 Payment Required
PAYMENT-REQUIRED: eyJ4NDAyVmVyc2lvbiI6Mi...
WWW-Authenticate: Payment ...

{"error": {"type": "payment_required", "code": "payment_required", "message": "..."},
 "status": 402, "x402Version": 2, "price_msat": 2000000, "price_sats": 2000, "challengeId": "..."}
```

`PAYMENT-REQUIRED` is base64 JSON. Its `accepts[0].extra.invoice` is the
Lightning invoice:

```sh
echo "$PAYMENT_REQUIRED" | base64 -d | jq -r '.accepts[0].extra.invoice'
```

## Step 3: pay the invoice

Pay it from any Lightning wallet that shows the proof of payment: the
preimage, as hex.

Or let the `openagents` command do steps 2 to 4 from your OpenAgents
wallet. It refuses an invoice above `--max-msat` (2,000 sats is
2,000,000 msat):

```sh
openagents x402 fetch "https://api.openagents.com/x/...?account=acct_..." --method POST --max-msat 2000000
```

## Step 4: send the same request with the proof

```sh
PROOF=$(jq -cn --argjson accepted "$ACCEPTED" --arg preimage "$PREIMAGE" \
  '{x402Version: 2, accepted: $accepted, payload: {preimage: $preimage}}' | base64)
curl -s -X POST "https://api.openagents.com/x/...?account=acct_..." \
  -H "PAYMENT-SIGNATURE: $PROOF"
```

`$ACCEPTED` is `accepts[0]` from Step 2, as JSON. Clients that speak the
`Payment` scheme can send `Authorization: Payment <credential>` for the
same invoice instead.

```json
{"account": "acct_...", "credited_usd": "1", "balance_usd": "1"}
```

Sending the same proof again credits nothing and says
`"already_credited": true`.

## Step 5: check your balance

```sh
curl -s https://openagents.com/v1/agents/me -H "Authorization: Bearer $OPENAGENTS_API_KEY" | jq .balance_usd
```

```json
"1"
```

Any model then runs within the balance: each request holds the most it
could cost, and is charged what it used.

## When the balance runs out

A model request answers `402` before anything runs:

```json
{"error": {"type": "insufficient_balance", "code": "agent_balance_too_low",
  "message": "...", "balance_usd": "0.0003", "request_max_cost_usd": "0.0016",
  "pay": {"available": true, "rail": "lightning", "methods": [...]}}}
```

Buy more credit with the `pay` object in that answer, then send the
request again.

## With a person's account instead

A person's workspace has its own balance and Lightning top-ups:
`GET /v1/balance` (with `X-Workspace-Id`) and
`POST /v1/workspaces/{workspace}/topups/{door}`. See the
[endpoint catalog](/docs/agents/endpoints).

## No account at all

Skip the balance and pay for one request at a time:
[Pay per call with x402](/docs/agents/x402).
