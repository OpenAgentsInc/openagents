# Khala Inference — Quickstart (OpenAI-compatible)

Run AI inference on **OpenAgents Khala** in two minutes. It's an **OpenAI-compatible**
API, so any OpenAI client/SDK works by pointing `base_url` at us. There's a **free tier**
— no signup, no payment — you just mint a free key from one endpoint.

> Verified live 2026-06-24 against production. One public model: **`openagents/khala`**.

## Handout card

```
Base URL:   https://openagents.com/api/v1      (the bare /v1 also works as an alias)
Model:      openagents/khala
Get a key:  curl -X POST https://openagents.com/api/keys/free
Auth:       Authorization: Bearer <your oa_agent_... token>
Free tier:  200 requests/day · 200,000 tokens/day (resets at UTC midnight)
```

## 1. Get a free key

No account, no payment. One call:

```sh
curl -X POST https://openagents.com/api/keys/free
```

Response (real shape):

```json
{
  "tier": "free",
  "model": "openagents/khala",
  "credential": {
    "token": "oa_agent_XXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXXX",
    "tokenPrefix": "oa_agent_XXXX",
    "createdAt": "2026-06-24T22:00:00.000Z"
  },
  "quota": { "maxRequestsPerDay": 200, "maxTokensPerDay": 200000, "window": "utc_day" },
  "usage": { "requestsToday": 0, "tokensToday": 0 }
}
```

Your key is **`credential.token`** (starts with `oa_agent_`). Save it:

```sh
export OA_API_KEY="oa_agent_..."   # the credential.token from above
```

Minting is rate-limited per IP (a handful of mints per UTC day) — mint once and reuse it.

## 2. Make a request (curl)

```sh
curl https://openagents.com/api/v1/chat/completions \
  -H "Authorization: Bearer $OA_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{
    "model": "openagents/khala",
    "messages": [{"role": "user", "content": "Hello"}]
  }'
```

## 3. Use any OpenAI SDK (zero new code)

It's OpenAI-compatible — just set the base URL and key.

**Python**

```python
from openai import OpenAI

client = OpenAI(
    base_url="https://openagents.com/api/v1",
    api_key="oa_agent_...",            # your credential.token
)
resp = client.chat.completions.create(
    model="openagents/khala",
    messages=[{"role": "user", "content": "Hello"}],
)
print(resp.choices[0].message.content)
```

**JavaScript / TypeScript**

```ts
import OpenAI from "openai";

const client = new OpenAI({
  baseURL: "https://openagents.com/api/v1",
  apiKey: process.env.OA_API_KEY,      // your credential.token
});
const resp = await client.chat.completions.create({
  model: "openagents/khala",
  messages: [{ role: "user", content: "Hello" }],
});
console.log(resp.choices[0].message.content);
```

## 4. Streaming (SSE)

Add `"stream": true` for token-by-token Server-Sent Events (standard OpenAI
`chat.completion.chunk` frames, then `[DONE]`):

```sh
curl -N https://openagents.com/api/v1/chat/completions \
  -H "Authorization: Bearer $OA_API_KEY" \
  -H "Content-Type: application/json" \
  -d '{"model":"openagents/khala","stream":true,"messages":[{"role":"user","content":"Hello"}]}'
```

OpenAI SDKs: pass `stream=True` / `{ stream: true }` as usual.

## 5. Endpoints

| Method & path | What it does | Auth |
|---|---|---|
| `POST /api/keys/free` | Mint a free-tier API key | none |
| `GET  /api/v1/models` | List models (one: `openagents/khala`) + pricing | Bearer |
| `POST /api/v1/chat/completions` | Chat completion (streaming + non-streaming) | Bearer |

(`https://openagents.com/v1/...` is a non-breaking alias for `https://openagents.com/api/v1/...`.)

## 6. Limits & errors

- **Free quota:** 200 requests/day and 200,000 tokens/day, per key, resetting at **UTC
  midnight** (`window: "utc_day"`).
- **`401`** — missing or invalid `Authorization` header / key.
- **`402`** — over the free quota (or calling a paid-only lane without credits). The free
  bypass falls through to the normal balance gate; add credits or wait for the UTC reset.
- **`429`** — too many key mints from one IP in a day. Reuse your existing key.

## 7. Free vs paid

- **Free** within the quota above — real inference, no payment, on `openagents/khala`.
- **Beyond the free quota** (or for higher throughput), it's the same key + credits/budget
  on your account; per-token pricing is published in `GET /api/v1/models`
  (`oa_price`, in both USD and credits per million tokens).
- **One public model** — `openagents/khala`. The orchestrator picks the backing lane; you
  buy the outcome. (There is no `khala-mini`/`khala-pro`/`khala-code` — just `openagents/khala`.)

## Why a key at all? (it's not a paywall)

The free tier needs no payment and no signup — the key is a free, self-serve, throwaway
handle so we can (a) match the OpenAI `Authorization: Bearer` contract every client expects,
(b) enforce the per-caller free quota and rate limits (so one abuser can't drain the free
lane on shared conference WiFi), and (c) meter usage for the public "tokens served" counter
and future contributor revshare. Mint one with `curl -X POST .../api/keys/free` and you're in.

---

*See also: the public agent guide at <https://openagents.com/AGENTS.md>. This quickstart is
documentation, not product-claim copy; the product-promise registry governs claims.*
