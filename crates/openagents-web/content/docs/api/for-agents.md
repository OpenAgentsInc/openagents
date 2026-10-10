# For agents

How an agent finds OpenAgents, says who it is, and pays. Every step is
one request or one command.

**Beta.**

## Find us

Start from any of these. None needs a key.

| What | Where |
| --- | --- |
| A short index of the site and its docs | `https://openagents.com/llms.txt` |
| Every doc in one file | `https://openagents.com/llms-full.txt` |
| The API, described in OpenAPI 3.1 | `https://api.openagents.com/v1/openapi.json` |
| The API catalog | `https://openagents.com/.well-known/api-catalog` |
| The AI catalog (MCP, agent card, skills, API) | `https://openagents.com/.well-known/ai-catalog.json` |
| The docs as an MCP server | `https://openagents.com/mcp/docs` |
| How keys work | `https://openagents.com/auth.md` |
| Models and prices as JSON | `https://api.openagents.com/v1/rates` |

Any page is also Markdown: add `.md` to its address, or send
`Accept: text/markdown`.

```sh
curl -s https://openagents.com/llms.txt
```

## Say who you are

You don't have to. A call paid per request needs no account. When you do
want one:

| How | What to send | Status |
| --- | --- | --- |
| An API key | `Authorization: Bearer oak_...`. A person makes it at [Settings, API keys](https://openagents.com/settings/api-keys). | Works now |
| Sign in for a person (OAuth) | For MCP clients and agents acting for someone: [Sign in with OAuth](https://openagents.com/auth.md#sign-in-with-oauth) | Works now: the token opens [openagents.com/mcp](https://openagents.com/mcp) |
| Your Nostr key | A signed `Authorization: Nostr ...` header (NIP-98) | Coming |

```sh
curl https://api.openagents.com/v1/responses \
  -H "Authorization: Bearer $OPENAGENTS_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model": "openagents/chat", "input": "Say hello."}'
```

## Pay

### With a key

Calls on a key are paid from its credit, at each answer's actual cost.
When the credit runs out the answer is `402` with the type
`insufficient_balance`. Add credit at [Settings](https://openagents.com/settings), then retry.

### Per request, with no key

Send the call with no `Authorization` header. The answer is
`402 Payment Required` with the price and one Lightning invoice for
exactly that request, written once for each way to pay listed below. Pay
it once, then send the same request again with the proof in the header
your method uses. Each step is in [Pay per request](/docs/api/pay-per-request).

{{ways to pay}}

The `openagents` command does all of it from your wallet:

```sh
openagents inference google/gemini-3.8-flash "Say hello." --pay x402 --max-msat 10000
```

### Ways to pay

{{payment methods}}

This table is read from the API itself, so it lists a way to pay only
while the API takes it. The `402` answer lists the same ways (its
`methods` field), and so do the OpenAPI description
(`x-payment-info` on each paid route), the
[API catalog](/.well-known/api-catalog), and the docs MCP server's
`list_payment_methods` tool. Follow the plan in
[#11085](https://github.com/OpenAgentsInc/openagents/issues/11085).

## What you get back

A paid answer carries a receipt header from the method you used
(`PAYMENT-RESPONSE` for x402, `Payment-Receipt` for the `Payment`
scheme), our own receipt id in `x-openagents-receipt`, and the answer's
actual cost in `x-openagents-cost-usd`. Errors are listed in [Errors](/docs/api/errors).
