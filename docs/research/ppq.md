# PayPerQ (ppq.ai): API, pricing, and 402 payments

2026-10-02. Research note. Every fact below was fetched on 2026-10-02 from the
URL cited next to it, unless marked otherwise. "Observed" means we sent the
request ourselves that day and quote the live response. "Unverified" means we
could not check it. Speculation is labelled as such. Model names are as PPQ and
the providers list them on that date.

Why we looked: PPQ is the closest existing product to the paid, keyless,
Lightning-first API we are designing in
[the OpenAgents API doc](../api/2026-10-02-openagents-api.md). That doc does
not name PPQ and should not; this note is where the comparison lives.

Raw captures used for the numbers (not committed): `GET https://api.ppq.ai/v1/models`
(391 chat models), `GET https://api.ppq.ai/v1/models?type=all` (531 models),
`GET https://openrouter.ai/api/v1/models`, `GET https://api.ppq.ai/v1/audio/models`,
`GET https://api.ppq.ai/v1/decisions/models`, `GET https://api.ppq.ai/v1/data/endpoints`,
and the server-rendered https://ppq.ai/pricing page.

## Contents

1. [What PPQ is and who it is for](#1-what-ppq-is-and-who-it-is-for)
2. [API surface](#2-api-surface)
3. [Auth and accounts](#3-auth-and-accounts)
4. [402 payments](#4-402-payments)
5. [Pricing](#5-pricing)
6. [Margin analysis](#6-margin-analysis)
7. [What OpenAgents should take, and where we differ](#7-what-openagents-should-take-and-where-we-differ)
8. [Sources](#8-sources)

## 1. What PPQ is and who it is for

- **A pay-per-use AI gateway with a chat web app on top.** "PayPerQ is a
  unified AI API gateway with 500+ models including Chat, Image, Video,
  Embeddings, STT, TTS, private (TEE) inference, and more from all major
  providers. OpenAI-compatible endpoints, pay-per-use pricing (no
  subscriptions), and cryptocurrency payment support"
  ([llms.txt](https://ppq.ai/llms.txt)). The live catalog had 531 models: 391
  chat, 71 image, 33 embedding, 21 video, 10 decision, 5 audio
  (`/v1/models?type=all`, observed).
- **Founder and history.** Created by Matt Ahlborg
  ([FAQ](https://ppq.ai/faq)). Launched as "ChatGPT4 Pay-per-Query Over
  Lightning" on 2024-04-05 ([No BS Bitcoin](https://www.nobsbitcoin.com/payperq-ai/));
  a third-party profile gives February 2024 as the founding date (search
  result summary, unverified). Raised $500k from Initial Capital and Fulgur
  Ventures, reported 2025-08-12
  ([Lightning Labs newsletter](https://lightninglabs.substack.com/p/summer-of-lightning-from-billions)).
- **Audience.** "I want PayPerQ to become the preferred AI interface for all
  Crypto adjacent developers and professionals around the world"
  ([FAQ](https://ppq.ai/faq)). The original pitch was people who cannot afford
  a $20 subscription or lack Visa/Mastercard access
  ([No BS Bitcoin](https://www.nobsbitcoin.com/payperq-ai/)). Today the API docs
  lead with coding-agent integrations: Claude Code, Cline, Aider, OpenClaw,
  Roo, OpenWebUI, Kilo Code, OpenCode, Cursor, Goose and more
  ([API docs](https://ppq.ai/api-docs)).
- **Pricing philosophy.** "Subscriptions charge too much to casual users and
  unfairly limit or diminish the experience of power users. We feel that the
  best and most simple way to consume AI is on a pay per-use-basis!"
  ([FAQ](https://ppq.ai/faq)). "The average query on our website is only 1.5
  cents and the average user spends only $4 per month in the web app"
  ([pricing](https://ppq.ai/pricing)).
- **Privacy positioning.** No-signup Credit IDs, three model privacy tiers
  (Anon, ZDR, E2EE), and since mid-September 2026 every chat request (web and
  API) runs through an AWS Nitro "Enclave Router" that PPQ says it cannot read
  into, with signed routing receipts
  ([Enclave Router post, 2026-09-30](https://ppq.ai/blog/introducing-the-enclave-router);
  [privacy policy](https://ppq.ai/privacy)).
- **Distribution.** Gift cards on Bitrefill, a partner program with revenue
  share for wallets and exchanges (Fedi is the showcased partner), a public
  stats dashboard at stats.ppq.ai, its own Lightning node
  ([FAQ](https://ppq.ai/faq); [integrate](https://ppq.ai/integrate);
  [stats post](https://ppq.ai/blog/introducing-ppq-stats-dashboard)).

## 2. API surface

Base URL `https://api.ppq.ai`; audio is documented at `https://ppq.ai/api/v1`
in [llms.txt](https://ppq.ai/llms.txt) but the [API docs](https://ppq.ai/api-docs)
show `https://api.ppq.ai/v1/audio/*`, and `GET https://api.ppq.ai/v1/audio/models`
answered 200 (observed). Most routes have a `/v1` and a bare alias.

### Inference

| Method and path | Purpose | Shape | OpenAI-compatible |
| --- | --- | --- | --- |
| `POST /v1/chat/completions` (alias `/chat/completions`) | Chat, any of 391 models | OpenAI chat body; SSE with `stream: true` | Yes |
| `POST /v1/responses` (alias `/responses`) | OpenAI Responses API | `model` + `input` required; `instructions`, `tools`, `reasoning`, `previous_response_id`, `store`, `metadata`, `stream`. Final cost in `response.completed` | Yes (`client.responses.create`); not for `private/*` models |
| `POST /v1/embeddings` | Embeddings, 33 models | `model`, `input` (string or array), `encoding_format`, `dimensions` | Yes |
| `POST /v1/images/generations` | Image generation, sync | `model`, `prompt`, `image_url`, `n` (1-4), `quality`, `size`/`aspect_ratio`, `resolution`, `negative_prompt`, `output_format`, `response_format: b64_json`. Response `{created, model, cost, data:[{url, content_type}]}`; `url` is a PPQ-signed link valid 24 h | Mostly; unsupported params silently dropped |
| `POST /v1/images/edits` | OpenAI-spec edit endpoint (multipart `image`, `prompt`, `model`) | Returns `b64_json` by default | Yes, for OpenWebUI/LibreChat |
| `POST /v1/videos` then `GET /v1/videos/:id` | Video generation, async | Submit returns 202 `{id, status:"pending", estimated_cost}`; poll every 5-10 s; 200 with `data.url` and `cost` when done | No (PPQ shape) |
| `POST /v1/audio/transcriptions` | STT (Deepgram Nova-3 default) | multipart `file` (≤25 MB), `model`, `response_format` (json/text/srt/vtt/verbose_json), `language`, `prompt` | Yes |
| `POST /v1/audio/speech` | TTS (Deepgram Aura 2, ElevenLabs) | `input`, `model`, `voice`, `language` (422 on voice/language mismatch) | Yes |
| `GET /v1/audio/models`, `GET /v1/audio/voices?language=` | Catalogs with prices | Each model has `base_price`, `api_price`, `ui_price` | n/a |
| `POST /v1/decisions` (alias `/v1/systemone`) | Typed decision models (TypeSafe Jev and others) | `model`, `state`, `questions` map of `noul` / `choice` / `score`; returns `answers` keyed by question name; billed per input token, output free; not streamed | No; the TypeSafe SDK works unchanged with `baseURL: "https://api.ppq.ai"` |
| `GET /v1/decisions/models` | Decision catalog | 10 models observed, e.g. `typesafe/jev-1.13` at $0.0462/M input | n/a |
| `GET /v1/models` (`?type=all`, `?type=image,video`, `?type=embedding`) | Catalog with prices and `privacyLevel` (`anon`/`zdr`/`e2e`) | Public, no auth | OpenAI list shape plus extras |

All from the [API docs](https://ppq.ai/api-docs) and [llms.txt](https://ppq.ai/llms.txt)
unless marked observed.

Chat request options worth knowing ([API docs](https://ppq.ai/api-docs)):

- **Web search ("Online Mode").** `plugins: [{"id":"web","max_results":5,"engine":"exa"|"native"|"auto"}]`
  always searches; `tools: [{"type":"web_search"}]` lets the model decide. Exa
  is the default engine at "$0.02 per request (in addition to the model's token
  pricing)"; native search is "pricing varies by provider". The legacy alias
  `openrouter:web_search` is still accepted, which shows the OpenRouter lineage.
- **Provider routing.** A `provider` object with `zdr`, `data_collection`,
  `order`/`only`/`ignore`, `sort` (price/throughput/latency), `max_price`,
  `preferred_max_latency`, `preferred_min_throughput`, `quantizations`,
  `require_parameters`, `allow_fallbacks`. These are OpenRouter's provider
  routing fields.
- **Model suffixes.** `:nitro` (fastest provider), `:floor` (cheapest),
  `:exacto` (best tool-calling), `:online` (web search), `:thinking`,
  `:extended`. "Billing is unaffected — you are always charged the real
  generation cost" ([routing post](https://ppq.ai/blog/routing-preferences-on-payperq)).
  Suffixes are no-ops on pinned models.
- **Routers.** `auto` and `autoclaw` are priced "Variable"
  ([pricing](https://ppq.ai/pricing)). AutoClaw scores the prompt "across 15
  weighted dimensions" with keyword detection in five languages, in "under 1
  millisecond", into four tiers mapped to fixed models (e.g. Simple → GLM 5.3
  Flash, Complex → Claude Sonnet 5), claiming "cutting costs by up to 70%"
  ([AutoClaw post](https://ppq.ai/blog/using-autoclaw-with-payperq)).
- **Private (TEE) models.** `private/*` (Kimi K3, GPT-OSS 120B, GLM-5.3, Gemma
  4, DeepSeek V4.1 Flash, Llama 3.3 70B) run in Tinfoil enclaves and require
  PPQ's local HPKE proxy (`npx ppq-private-mode`), which also exposes an
  Anthropic Messages endpoint for Claude Code ([API docs](https://ppq.ai/api-docs)).

### Data enrichment

Base `https://api.ppq.ai/v1/data`. `GET /v1/data/endpoints` lists 48 priced
routes without auth (observed): X/Twitter (12 routes, $0.0115 each), Google
Maps and Solar/Aerial (11), FullEnrich/PDL/CompanyEnrich, Minerva identity
resolution, Exa (search, find-similar, contents, answer), Firecrawl, Clado
LinkedIn contacts, Serper (news, shopping, images, people images, Lens),
Reddit, Whitepages, Hunter email verifier. Prices run from $0.0023 (Exa
contents) to $0.253 (Whitepages). The [API docs](https://ppq.ai/api-docs) say
"39 data enrichment endpoints"; the live list is longer.

### Account and billing endpoints

| Method and path | Auth | Purpose |
| --- | --- | --- |
| `POST /accounts/create` | none | New account; returns `credit_id` and `api_key` |
| `POST /credits/balance` | `credit_id` in body, Bearer key, or `api-key` header | `{"balance": <usd>}` |
| `POST /topup/create/{method}` | key | Deposit invoice; methods `btc-lightning`, `btc`, `ltc`, `lbtc`, `xmr` |
| `GET /topup/status/{invoice_id}` | key | Invoice status |
| `GET /topup/payment-methods` | none | Methods, currencies, limits |
| `POST /nwc-auto-topup/connect`, `GET /nwc-auto-topup`, `DELETE /nwc-auto-topup/connection` | `x-credit-id` | Nostr Wallet Connect auto top-up below a threshold (default $10, min $5) |
| `GET /queries/history` | key, `api-key`, or `credit_id` | Per-query ledger with filters and pagination |
| `GET/POST /keys`, `GET/PATCH/DELETE /keys/{id}` | `x-credit-id` | Key management with spend caps, reset periods, expiry |

([API docs](https://ppq.ai/api-docs).)

### Errors and limits

- Documented error body: `{"status":"error","statusCode":401,"message":"Unauthorized"}`;
  codes 400, 401, 402 ("insufficient credit balance"), 500
  ([llms.txt](https://ppq.ai/llms.txt)). Live bodies are not uniform
  (observed): chat without auth returns
  `401 {"error":{"message":"Missing x-credit-id, Authorization or x-api-key","code":401}}`;
  embeddings return `401 {"error":"Unauthorized","message":"Missing or invalid Authorization header"}`;
  chat on an empty account returns `402 {"error":"Insufficient credit."}`;
  data on an empty account returns
  `402 {"error":{"message":"Insufficient balance","reason":"insufficient_balance","required":"0.0115","type":"insufficient_balance"}}`.
- "There is no explicit rate limiting — usage is controlled by your credit
  balance" ([llms.txt](https://ppq.ai/llms.txt)). The terms still forbid
  bypassing "rate limits" ([terms](https://ppq.ai/terms)).
- Failed queries appear in `/queries/history` with `error_code` from a fixed
  list (`upstream_error`, `timeout`, `insufficient_balance`,
  `free_model_unauthorized`, ...) and `price_in_usd` = "what was actually
  charged (0 when nothing was billed)" ([API docs](https://ppq.ai/api-docs)).

## 3. Auth and accounts

- **Keys.** `Authorization: Bearer sk-...`; also accepted: `api-key` header,
  `x-api-key`, and `x-credit-id` (the account itself) on many routes
  ([API docs](https://ppq.ai/api-docs); observed 401 text).
- **No-signup accounts.** `POST /accounts/create` with no body returned
  `{"success":true,"credit_id":"…","api_key":"sk-…","balance":0}` (observed;
  we created one zero-balance account to see the shapes). The Credit ID "is a
  bearer credential: whoever holds it can access that balance"; you can add an
  email/Google/GitHub login for recovery, or a passkey for encrypted chat sync
  ([account types post](https://ppq.ai/blog/payperq-account-types-explained)).
  The web app stores the Credit ID in the browser and the original payment
  memo ([FAQ](https://ppq.ai/faq)).
- **Two identifiers, two powers.** The `credit_id` (via `x-credit-id`) manages
  keys and NWC; an `sk-` key spends. Keys carry `usage_limit_usd` (min $0.01),
  `reset_period` (`daily`/`weekly`/`monthly`), and `expire_at`; the full key is
  shown once; revocation is a soft delete ([API docs](https://ppq.ai/api-docs)).
  A new account gets one key named "Default" (observed).
- **Top-ups.** `POST /topup/create/btc-lightning` with
  `{"amount":1,"currency":"USD"}` returned (observed):

  ```json
  {"created_at":"1790959684","invoice_id":"EMQAAmVH3GiTJyZYiGgBDc","amount":"1","currency":"USD",
   "expires_at":"1790960584","checkout_url":"https://btc-ltc-doge-ppq-v3.eastus2.clou…",
   "lightning_invoice":"lnbc11730n1…","crypto_amount_due":"1.173e-05"}
  ```

  The invoice was 1,173 sats for $1, i.e. $85,251/BTC, against a Coinbase spot
  of $85,240 and Kraken $85,252 at the same minute (observed): no visible FX
  spread. The checkout host and invoice id format look like a self-hosted
  BTCPay Server (inference from the id format and hostname, unverified).
  Limits ([payment-methods](https://api.ppq.ai/topup/payment-methods), observed):

  | Method | Range (USD) | Invoice expiry |
  | --- | --- | --- |
  | `btc-lightning` | $0.10 - $1,000 (100 - 1,000,000 sats) | 15 min, "5% Lightning fee bonus" |
  | `btc` on-chain | $10 - $10,000 | 60 min |
  | `ltc` | $2 - $1,000 | 60 min |
  | `lbtc` (Liquid) | $2 - $10,000 | 60 min |
  | `xmr` | $5 - $10,000 | 60 min |

  Cards go through Stripe on the website only ([llms.txt](https://ppq.ai/llms.txt);
  [privacy](https://ppq.ai/privacy)). llms.txt also lists USDT and USDC "and
  any other cryptocurrency" in its summary line, but no API method for them
  exists in `payment-methods` (observed). The exact mechanics of the 5%
  Lightning bonus (credited on top of the deposit, presumably) are
  unverified; we did not pay an invoice.
- **Activity.** `/queries/history` returns `timestamp`, `model`,
  `input_count`, `output_count`, `price_in_usd`, `query_type`,
  `query_source`, `api_key_id`, `isOnline`, `isFreeModel`, and error fields;
  scoped to the calling key unless `all_keys=true` ([API docs](https://ppq.ai/api-docs)).
- **Refunds and KYC.** "Deposited funds are non-withdrawable, except in cases
  of mistaken transactions or service-related issues, subject to review"; on a
  price change you "may discontinue using our Services and withdraw any
  remaining balance"; if PPQ discontinues it gives "refunds for any prepaid,
  unused Services"; accounts inactive over a year "may be deleted"
  ([terms](https://ppq.ai/terms)). No KYC is described anywhere; an account is
  optional and only an email is collected if you make one
  ([privacy](https://ppq.ai/privacy)).
- **Privacy facts.** PPQ stores the user agent and token counts per query,
  not prompts; OpenAI-model requests carry a pseudonymous keyed-hash "safety
  identifier"; ZDR is opt-in per request over the API (`provider.zdr: true`);
  non-chat endpoints (images, video, audio) do not yet run in the enclave
  ([privacy](https://ppq.ai/privacy)).

## 4. 402 payments

### What the docs say

"Pay for individual API requests using the HTTP 402 payment protocol — no
account or API key required. Currently supported via L402 (Bitcoin
Lightning)." Send the request without `Authorization`; get `402` with "a
`WWW-Authenticate: Payment` header containing a challenge and a Lightning
invoice"; pay; "Replay the original request with the payment credential in the
`Authorization` header." The curl example shows the replay as
`Authorization: L402 <token>:<preimage>`. Clients: `lnget` and `mppx`
([API docs](https://ppq.ai/api-docs); [llms.txt](https://ppq.ai/llms.txt)).

Endpoints that accept 402 ([API docs](https://ppq.ai/api-docs)):

| Category | Endpoint | Pricing |
| --- | --- | --- |
| Data enrichment | `/v1/data/api/*`, `/v1/data/x/*` | Fixed per endpoint |
| Image generation | `POST /v1/images/generations` | Dynamic by model, size, quality |
| Image editing | `POST /v1/images/edits` | Same as generation |
| Video generation | `POST /v1/videos` | Dynamic by model, duration, resolution |

Chat, Responses, embeddings, and decisions do **not** take 402 payment: without
a key they return 401 (observed). For those, "Insufficient balance returns
402 Payment Required" with no challenge, i.e. 402 means "top up", not "pay
this invoice" ([API docs](https://ppq.ai/api-docs); observed).

### What the server actually sends (observed)

The live challenge is **not classic L402**. It is the IETF-draft "Payment"
HTTP authentication scheme (Machine Payments Protocol, MPP) with the
`lightning` method and `charge` intent. Verbatim, for an Exa search:

```http
HTTP/1.1 402 Payment Required
content-type: application/problem+json; charset=utf-8
cache-control: no-store
www-authenticate: Payment id="NSjrDPQvkSWeZe1IEpnpIJqBAHZ5-Y08i0jgideX_Eo", realm="e3ea41b5d458", method="lightning", intent="charge", request="eyJhbW91bnQiOiIxNCIs…", description="PPQ Data: Exa Search", expires="2026-10-02T16:49:01.399Z"

{"type":"https://paymentauth.org/problems/payment-required","title":"Payment Required","status":402,"detail":"Payment is required (PPQ Data: Exa Search).","challengeId":"NSjrDPQvkSWeZe1IEpnpIJqBAHZ5-Y08i0jgideX_Eo"}
```

The `request` parameter is base64url JSON:

```json
{"amount":"14","currency":"sat",
 "methodDetails":{"invoice":"lnbc140n1p4tlh63…","network":"mainnet",
   "paymentHash":"5a18eb92e7beb176a6036099dac8f3e94eaf43b4d63dabd38d2afb7e12bcfc1b"}}
```

Other observed challenges: `gpt-image-1` image, 28 sats, detail "PPQ Image:
GPT Image 1 (4o Image)"; `kling-2.5-turbo` 5 s video, 473 sats, "PPQ Video:
Kling 2.5 Turbo Pro". The sat amounts equal the USD API price at spot rounded
up: $0.0115 → 14 sats, $0.023 → 28, $0.4025 → 473 at ~$85,250/BTC.

Notes from the probes:

- **Challenge lifetime is 5 minutes** (`expires` 5 min after the `date` header).
- **`realm` changes between requests** (`e3ea41b5d458`, `9e208d42e4d2`),
  which looks like a container hostname rather than a stable protection space.
  Harmless if the challenge-id HMAC key is shared across instances; we did not
  test cross-instance redemption.
- **No `digest` parameter.** The challenge does not bind the request body
  (the spec says servers "SHOULD include the digest parameter when issuing
  challenges for requests with bodies"). Whether PPQ binds the path or body
  server-side is unverified; nothing in the challenge does.
- **The documented `Authorization: L402 <token>:<preimage>` form is not what
  this server parses.** `Authorization: L402 abc:def` got a fresh
  `payment-required` challenge (treated as no credential), while
  `Authorization: Payment abc` got
  `{"type":"https://paymentauth.org/problems/malformed-credential", … "detail":"Credential is malformed: Invalid base64url or JSON.."}`.
  The docs' L402 wording is stale or aspirational; the working credential is
  `Authorization: Payment <base64url JSON>`.
- **Image/video 402 needs a body the price function understands.** A request
  for `nano-banana-2` with `resolution: "1K"` returned a bare HTML `500`, while
  `gpt-image-1` with `size` returned a proper 402 (observed). The price is
  computed from the request before the challenge.

### The credential and receipt (from the spec and lnget's implementation)

Per [draft-httpauth-payment-01](https://paymentauth.org/draft-httpauth-payment-01.txt)
(Tempo Labs and Stripe, 2026-10-01) and
[draft-lightning-charge-00](https://paymentauth.org/draft-lightning-charge-00.txt)
(Lightspark, 2026-10-01), as implemented in
[lnget `mpp/credential.go`](https://github.com/lightninglabs/lnget/blob/main/mpp/credential.go):

```text
Authorization: Payment base64url({
  "challenge": {"id":"…","realm":"…","method":"lightning","intent":"charge",
                "request":"<request echoed unchanged>","expires":"…","description":"…"},
  "payload":   {"preimage":"<64 lowercase hex>"}})
```

Success carries `Payment-Receipt: base64url({"status":"success","method":"lightning","timestamp":"…","reference":"<paymentHash>","challengeId":"…"})`
([lnget `mpp/receipt.go`](https://github.com/lightninglabs/lnget/blob/main/mpp/receipt.go)).

Spec rules that matter:

- The challenge `id` must be bound to realm, method, intent, request, and
  expires/digest/opaque when present; HMAC-SHA256 over
  `realm|method|intent|request|expires|digest|opaque` is the recommended
  stateless binding (httpauth §5.1.2.1).
- "A payment proof MUST be usable exactly once" (httpauth §11.3). The
  Lightning charge draft: "the server MUST store the issued paymentHash keyed
  by the challenge … A preimage that is valid for one challenge MUST NOT be
  accepted for a different challenge" (lightning-charge §9).
- Unpaid requests must not cause side effects; servers "SHOULD accept an
  Idempotency-Key header" (httpauth §11.4).
- There is also a Lightning **session** intent: prepaid deposit, per-chunk
  deduction during an SSE stream, a `payment-need-topup` event that holds the
  stream open, and a refund of unspent balance to a client-supplied return
  invoice on close, written with "LLM Token Streaming" as its use case
  ([draft-lightning-session-00](https://paymentauth.org/draft-lightning-session-00.txt)).
  PPQ does not use it; it is exactly the shape our per-message pricing lacks.

### Signed status URLs for async jobs

Video submitted through 402 returns `202 {"id":"gen_…","status":"pending","status_url":"https://…"}`;
the `status_url` is signed and pollable without auth. Image responses carry
signed media URLs (`/v1/media/gen_…/0?sig=…&exp=…`). "Signed URLs expire after
24 hours" ([API docs](https://ppq.ai/api-docs)). This is how a keyless payer
gets its result back: the URL itself is the capability. The exact signature
scheme is not documented.

### Bearer key bypass

"If you also have a PPQ API key, you can still use it via
`Authorization: Bearer sk-...` — the 402 flow is only triggered when no API key
is provided" ([API docs](https://ppq.ai/api-docs)). Confirmed: with a key on an
empty account, the data endpoint returned the balance-style 402 with
`"required":"0.0115"`, not a Lightning challenge (observed). So a key holder
with no balance cannot fall back to paying per call on the same request.

### Client tools

- **lnget** ([lightninglabs/lnget](https://github.com/lightninglabs/lnget),
  Go, created 2026-02-05): "wget for the pay-per-request web". It speaks
  classic L402 (`WWW-Authenticate: L402 macaroon="…", invoice="…"`, retry with
  `Authorization: L402 <macaroon>:<preimage>`, tokens cached per domain and
  reused) and, since its `mpp` package (PR #18), the `Payment` scheme with
  `lightning`/`charge`. Pays through lnd, has `--max-cost` in sats, a spending
  dashboard, and a SQLite payment log ([README](https://github.com/lightninglabs/lnget)).
- **mppx** ([wevm/mppx](https://github.com/wevm/mppx), TypeScript): the MPP
  SDK and CLI. Core methods are Tempo, Stripe, and EVM; its CLI can also pay
  x402 offers, EVM only (`--protocol auto|mpp|x402`, "MPP is preferred when
  available") ([README](https://github.com/wevm/mppx)). Lightning comes from
  Spark's `@buildonspark/lightning-mpp-sdk`, "which extends the mppx SDK with
  Lightning Network support", charge and session
  ([mpp.dev Lightning](https://mpp.dev/payment-methods/lightning)).
- **Classic L402** ([lightninglabs/L402](https://github.com/lightninglabs/L402)):
  macaroon + invoice challenge, `Authorization: L402 <base64(macaroon)>:<hex(preimage)>`,
  formerly LSAT (servers "SHOULD send both `LSAT` and `L402`"). The token is
  reusable until a caveat (expiry, usage count) says otherwise.

### Comparison with x402 v2 `lnbtc` (what `crates/x402` implements)

| | PPQ (MPP `Payment`, lightning/charge) | Classic L402 | x402 v2 `exact` on `lnbtc` (ours) |
| --- | --- | --- | --- |
| Challenge header | `WWW-Authenticate: Payment id=, realm=, method="lightning", intent="charge", request=<b64url JSON>, expires=, description=` | `WWW-Authenticate: L402 macaroon="…", invoice="…"` | `PAYMENT-REQUIRED: <base64 JSON>` with `accepts[]` |
| 402 body | RFC 9457 problem JSON | unspecified | free JSON (ours: error + `price_sats`) |
| Credential header | `Authorization: Payment <b64url JSON{challenge echo, payload.preimage}>` | `Authorization: L402 <macaroon>:<preimage>` | `PAYMENT-SIGNATURE: <base64 JSON{accepted, payload.preimage}>` |
| Success header | `Payment-Receipt` | none | `PAYMENT-RESPONSE` |
| Binds the invoice to the request | Challenge `id` HMACs the request/amount/invoice; body only via optional `digest` (RFC 9530); PPQ sends no `digest` | Macaroon caveats, if the server adds them | `requestHash` over method, URL, body bytes, and listed headers (`http:1` profile) |
| Replay protection | Spec: single use per challenge, server stores paymentHash by challenge id | Reusable token by design; limits are caveats | Restart-durable replay store, atomic insert across processes |
| Pricing timing | Upfront, price computed from the request before the challenge | Upfront | Upfront (`paymentFlow: "upfront"`) |
| Metered streaming | `session` intent exists (deposit, per-chunk, top-up event, refund) | No | No Lightning `upto`/`escrow` yet (our gap table) |
| Coexists with API keys | Same `Authorization` header, different scheme (`Bearer` vs `Payment`) | Same header | Separate headers, so a key and a payment can both be bound |
| Off-the-shelf clients | lnget, mppx + Spark plugin | lnget, Fewsats, Alby tools | `openagents x402 fetch`; upstream SDKs have no Lightning mechanism |

### What it would take for OpenAgents to accept their clients

Accepting MPP `Payment`/lightning/charge beside x402 `lnbtc` is a small
addition, because both settle the same thing: one BOLT11 invoice from our
wallet and its preimage.

1. **Offer both in one 402.** Keep `PAYMENT-REQUIRED`, and add
   `WWW-Authenticate: Payment … method="lightning", intent="charge"` with the
   same invoice and payment hash in `request`, an HMAC `id`, a short
   `expires`, and a `digest` of the body. HTTP allows multiple
   `WWW-Authenticate` challenges; x402 clients ignore it.
2. **Accept `Authorization: Payment`.** Decode, recompute the HMAC over the
   echoed fields, check `expires`, check the digest of the replayed body,
   check `sha256(preimage) == paymentHash`, then insert the payment hash into
   the **same** replay store `crates/x402` already uses, so a preimage spent
   one way cannot be spent the other way.
3. **Scheme dispatch on `Authorization`.** `Bearer` is a key, `Payment` is a
   payment. Our design binds `authorization` into the x402 request hash for key
   holders who also pay; with MPP the key and the payment cannot share the
   header, so key + per-call payment stays x402-only.
4. **Return `Payment-Receipt`** next to `PAYMENT-RESPONSE`, and use RFC 9457
   problem bodies with the paymentauth.org `type` URIs for MPP clients.
5. **Bind more than the spec requires.** PPQ omits `digest`; we should not.
   With `digest` plus method and URL in the `opaque` map (HMAC-covered), an MPP
   proof is as narrowly bound as our `requestHash`.
6. **Classic L402 is a separate, later step.** It needs macaroon minting and
   verification, and its tokens are reusable, so we would mint each macaroon
   with a single-use or short-expiry caveat. lnget already speaks MPP, so
   supporting MPP covers lnget without macaroons.
7. **Watch the `session` intent.** It is a ready-made answer to the "a
   message's cost depends on the route" gap in our x402 table: deposit a
   ceiling, deduct actual cost per chunk, refund the rest to a return invoice.

Effort, as an estimate: one module in `crates/x402` (challenge emit, credential
parse/verify, receipt) sharing the replay store and wallet, plus tests against
lnget's `mpp` package as the reference client.

## 5. Pricing

### Chat models: the markup is exactly 5.5%, with Google at half price

Method: every model in `GET https://api.ppq.ai/v1/models` (391) matched by id
against `GET https://openrouter.ai/api/v1/models`; 372 matched. Of the 732
nonzero input/output price ratios, **668 are 1.055** (to rounding) and 44 are
**0.500×** (Google). The remaining exceptions are a few open-weight
models priced from a pinned provider and `private/*` TEE models. Provider list
prices were checked directly for Anthropic
([pricing](https://platform.claude.com/docs/en/about-claude/pricing)), OpenAI
([pricing](https://developers.openai.com/api/docs/pricing)), and Google
([pricing](https://ai.google.dev/gemini-api/docs/pricing)). All prices are USD
per 1M tokens, fetched 2026-10-02.

| PPQ model id | Owner | PPQ in / out | Provider list in / out | OpenRouter in / out | PPQ ÷ list | PPQ ÷ OpenRouter |
| --- | --- | --- | --- | --- | --- | --- |
| `gpt-6.1-sol` | OpenAI | $2.11 / $10.55 | $2 / $10 | $2 / $10 | 1.055× | 1.055× |
| `gpt-5.4-mini` | OpenAI | $0.7913 / $4.7475 | $0.75 / $4.50 | $0.75 / $4.50 | 1.055× | 1.055× |
| `gpt-5.4-nano` | OpenAI | $0.211 / $1.3188 | $0.20 / $1.25 | $0.20 / $1.25 | 1.055× | 1.055× |
| `gpt-5.3-codex` | OpenAI | $1.8462 / $14.77 | $1.75 / $14 | $1.75 / $14 | 1.055× | 1.055× |
| `openai/gpt-6-luna` | OpenAI | $0.1055 / $0.5275 | $0.10 / $0.50 | $0.10 / $0.50 | 1.055× | 1.055× |
| `openai/gpt-6-astra` | OpenAI | $10.55 / $52.75 | $10 / $50 | $10 / $50 | 1.055× | 1.055× |
| `claude-opus-5.5` | Anthropic | $4.22 / $21.10 | $4 / $20 | $4 / $20 | 1.055× | 1.055× |
| `claude-sonnet-5.5` | Anthropic | $2.11 / $10.55 | $2 / $10 | $2 / $10 | 1.055× | 1.055× |
| `claude-fable-5.1` | Anthropic | $10.55 / $52.75 | $10 / $50 | $10 / $50 | 1.055× | 1.055× |
| `claude-haiku-4.5` | Anthropic | $1.055 / $5.275 | $1 / $5 | $1 / $5 | 1.055× | 1.055× |
| `anthropic/claude-opus-4.8` | Anthropic | $5.275 / $26.375 | $5 / $25 | $5 / $25 | 1.055× | 1.055× |
| `anthropic/claude-sonnet-4.5` | Anthropic | $3.165 / $15.825 | $3 / $15 | $3 / $15 | 1.055× | 1.055× |
| `gemini-3.7-flash` | Google | $0.375 / $1.875 | $0.75 / $3.75 | $0.75 / $3.75 | **0.500×** | **0.500×** |
| `google/gemini-3.5-flash` | Google | $0.75 / $4.50 | $1.50 / $9 | $1.50 / $9 | **0.500×** | **0.500×** |
| `google/gemini-2.5-pro` | Google | $0.625 / $5 | $1.25 / $10 | $1.25 / $10 | **0.500×** | **0.500×** |
| `google/gemini-2.5-flash` | Google | $0.15 / $1.25 | $0.30 / $2.50 | $0.30 / $2.50 | **0.500×** | **0.500×** |
| `glm-5.3` | Z.ai | $1.477 / $4.642 | not checked | $1.40 / $4.40 | — | 1.055× |
| `grok-4.6` | SpaceXAI | $2.11 / $6.33 | not checked | $2 / $6 | — | 1.055× |
| `deepseek/deepseek-v4.1-flash` | DeepSeek | $0.3165 / $1.266 | not checked | $0.30 / $1.20 | — | 1.055× |
| `qwen/qwen3.8-max-0902` | Qwen | $2.11 / $6.33 | not checked | $2 / $6 | — | 1.055× |
| `meta/muse-spark-1.3` | Meta | $1.3188 / $4.4837 | not checked | $1.25 / $4.25 | — | 1.055× |
| `moonshotai/kimi-k3` | MoonshotAI | $3.165 / $15.825 | not checked | $2.70 / $13.50 | — | 1.172× |
| `z-ai/glm-5.2` | Z.ai | $1.477 / $4.642 | not checked | $0.41 / $3.99 | — | 3.602× / 1.163× |

Cache reads follow the same rule: Sonnet 5.5 cache read is $0.21 on PPQ
against Anthropic's $0.20 ([pricing](https://ppq.ai/pricing);
[Anthropic](https://platform.claude.com/docs/en/about-claude/pricing)).

**The pattern.**

- **API chat = provider list × 1.055.** The earlier quick look was right, and
  the number is exact, not approximate: 5.5% on input, output, and cache reads,
  identical for OpenAI, Anthropic, xAI, DeepSeek, Qwen, Meta and the long tail.
  OpenRouter states "We pass through the pricing of the underlying providers;
  there is no markup on inference pricing" and charges "5.5% ($0.80 minimum)"
  on card credit purchases, "5%" on crypto
  ([OpenRouter FAQ](https://openrouter.ai/docs/faq)). PPQ's per-token price is
  therefore OpenRouter's price with OpenRouter's card fee folded into every
  token. (That PPQ buys through OpenRouter for many models is suggested by the
  `openrouter:web_search` alias, OpenRouter's routing fields and suffixes, the
  Enclave Router post naming OpenRouter as an upstream, and OpenRouter's `-1`
  sentinel prices for `auto`; it is not stated as a cost basis anywhere.)
- **Web app chat = list × 1.65.** The pricing page embeds both tiers per model,
  e.g. `gpt-6.1-sol` `"api":{"input_per_1M":2.11,"output_per_1M":10.55}` and
  `"ui":{"input_per_1M":3.3,"output_per_1M":16.5}` ([pricing](https://ppq.ai/pricing)
  page source). 362 models have UI/API = 1.564 (= 1.65/1.055). The audio
  catalog says it outright: ElevenLabs v4 `base_price 0.18`, `api_price 0.1899`,
  `ui_price 0.297` per 1k characters; Deepgram Nova-3 `0.006` / `0.00633` /
  `0.0099` per minute ([audio models](https://api.ppq.ai/v1/audio/models)).
- **Google is at half of list, in both the app and the API.** All 22 Google
  models in the chat catalog have UI = API = 0.5× list. Google's own page offers "50% cost
  reduction" only on batch and flex tiers
  ([Gemini pricing](https://ai.google.dev/gemini-api/docs/pricing)). No PPQ page
  explains it. Speculation: cloud credits or a negotiated/flex-tier cost
  passed through as a loss leader; Google models are also among PPQ's most used
  (below).
- **Open-weight models are priced from PPQ's chosen provider, not the
  cheapest.** Kimi K3 at $3.165/$15.825 is $3/$15 × 1.055, and GLM 5.2 at
  $1.477/$4.642 is $1.40/$4.40 × 1.055, while OpenRouter's cheapest listing is
  $2.70/$13.50 and $0.41/$3.99. PPQ made Fireworks "our primary provider for
  open-source models" on 2026-07-07
  ([Fireworks post](https://ppq.ai/blog/fireworks-ai-primary-open-source-provider)).
  That the $3/$15 and $1.40/$4.40 bases are Fireworks' list prices is
  inference, unverified.
- **TEE models carry a larger premium:** `private/kimi-k3` $4.22/$21.10 (1.56×
  OpenRouter's Kimi K3), `private/gpt-oss-120b` 3.7-4.3× (observed catalog).
- **Embeddings and decisions are list × 1.10:** `openai/text-embedding-3-small`
  $0.022 against OpenAI's $0.02; `typesafe/jev-1.13` $0.0462 against Jev's
  $0.042 per million input tokens (our own docs, e.g.
  `docs/gym/terminal-bench-cli.md`).
- **Popularity data is public in the page source.** Each model carries
  `median_cost.api.usd` and `sample_size`. The largest samples on 2026-10-02:
  `google/gemini-2.5-flash` 512,780 (median $0.0005/query), `z-ai/glm-5.3-flash`
  342,990, `deepseek/deepseek-v4-flash-0731` 243,879, `openai/gpt-4o-mini` 91,265,
  `glm-5.3` 86,921 ($0.0334), `anthropic/claude-sonnet-5` 64,511 ($0.0126)
  ([pricing](https://ppq.ai/pricing) page source). Cheap models dominate
  volume; the window the samples cover is not stated.

### Images, video, data: fixed tables, ~15% over base

- **Fixed per generation, varying by variant.** Image and video prices come as
  tables of quality × size × duration, e.g. `gpt-image-1` low/medium/high
  $0.023/$0.0805/$0.2185; `nano-banana-2` 1K/2K/4K $0.092/$0.138/$0.184;
  `veo3` 5 s $2.30, 8 s $3.68; `kling-2.5-turbo` 5 s $0.4025, 10 s $0.805
  (`/v1/models?type=image,video`, observed). The docs call these "dynamic",
  meaning computed from the request parameters before the call, not metered
  after it ([API docs](https://ppq.ai/api-docs)).
- **The API charge is 1.15× the base shown on the pricing page** for video and
  data: Kling 2.5 Turbo 5 s is $0.35 on the [pricing page](https://ppq.ai/pricing)
  and $0.4025 in the catalog and the 402 challenge; FLUX 3 720p 5 s $0.85 →
  $0.9775; Exa search $0.01 → $0.0115; Clado $0.20 → $0.23; Firecrawl scrape
  $0.0126 → $0.01449 (pricing page vs `/v1/data/endpoints`, observed). Some
  image models sit at 1.10× (prices ending .099/.0495/.066). The pricing page
  is inconsistent: it shows `gpt-image-1` at the API price but Kling at base.
  Whether "base" equals the upstream list price (e.g. fal.ai for Kling) is
  unverified.
- **Data enrichment prices** range from $0.0023 to $0.253 per request; the
  March launch post quoted lower round numbers ($0.01-$0.05) for the first set
  ([data post](https://ppq.ai/blog/data-enrichment-endpoints)).
- **Add-ons.** Exa web search $0.02 per request on top of tokens; native
  provider search "varies by provider"; `:online` is a no-op (and free) on free
  models ([API docs](https://ppq.ai/api-docs)). In the web app: document
  uploads 5.75¢ each (Docling in a TEE), People Search "starting at ~25¢ per
  query" ([pricing](https://ppq.ai/pricing)).
- **Lightning bonus.** "Bonus: 5% Lightning fee bonus" on `btc-lightning`
  top-ups ([API docs](https://ppq.ai/api-docs)); applied mechanics unverified.
- **Minimums.** Lightning top-up minimum $0.10 (100 sats); on-chain $10; LTC
  and Liquid $2; Monero $5. No minimum spend or monthly fee
  ([payment-methods](https://api.ppq.ai/topup/payment-methods);
  [llms.txt](https://ppq.ai/llms.txt)). Per-request 402 prices round up to the
  next sat, which is a 3-4% bump on a 1-cent call (14 sats for $0.0115 at
  ~$85,250/BTC, observed).
- **Free models.** Several catalog entries are `:free` or priced 0; free
  models skip web search and memory ([llms.txt](https://ppq.ai/llms.txt)).

## 6. Margin analysis

Evidence first, then labelled speculation.

| Source of margin | Evidence | Size |
| --- | --- | --- |
| Token markup, API | Every non-Google chat price is list × 1.055 (section 5) | 5.5% of revenue gross; net of upstream card/credit fees it may be near zero if PPQ buys OpenRouter credits by card (5.5%) or crypto (5%) **(speculation)** |
| Token markup, web app | UI price = list × 1.65 | ~39% gross margin on app usage (0.65/1.65); this is likely the main profit centre **(inference from published prices; the app/API revenue split is unknown)** |
| Media and data markup | 1.10-1.15× base on images, video, data | 9-13% of revenue gross |
| Embeddings, decisions | 1.10× list | 9% of revenue gross |
| TEE premium | `private/*` at 1.5-4× the standard model | Covers Tinfoil cost plus margin **(cost basis unverified)** |
| Float on unspent balances | Prepaid, "non-withdrawable" except by review; accounts inactive a year "may be deleted"; Credit IDs are bearer strings users lose ("if you lose it, it's gone") ([terms](https://ppq.ai/terms); [account types](https://ppq.ai/blog/payperq-account-types-explained)) | Unknown. Breakage on small, anonymous, unrecoverable balances is plausibly material **(speculation)** |
| Payment-fee arbitrage | Lightning top-ups at spot with no visible spread (observed) and a 5% "fee bonus", i.e. PPQ gives back roughly what a card processor would take; card top-ups go through Stripe | Lightning is cheaper for PPQ than cards, so the 5% bonus steers users to the cheaper rail **(inference)** |
| Rounding | 402 sat prices round up | Pennies; negligible |
| Loss leaders | Google models at 0.5× list in both tiers | Negative unless PPQ's Google cost is below half list **(speculation: credits or flex)** |
| Partnerships | Revenue share with wallets/exchanges ([integrate](https://ppq.ai/integrate)) | A cost, not a margin; buys distribution |

Reading: the API is priced as a near-cost on-ramp (OpenRouter plus its fee),
and the consumer app carries the margin. Images/video/data and the TEE tier add
thicker margins on smaller volume. Float and breakage are structural in a
prepaid, no-account, crypto-funded model. This is speculation built on
published prices; PPQ publishes no financials.

## 7. What OpenAgents should take, and where we differ

Mapped to sections of [the OpenAgents API doc](../api/2026-10-02-openagents-api.md).

### Take

1. **Accept MPP `Payment`/lightning/charge beside x402 `lnbtc`** (§5, gap
   table). It is the scheme lnget and the mppx Lightning plugin actually pay,
   it costs one module and shares our replay store and wallet, and it gives us
   real clients today while upstream x402 has no Lightning mechanism. Bind a
   `digest` where PPQ does not. Section 4 above has the steps.
2. **Adopt the Lightning `session` intent for priced messages** (§5 gap: "A
   message whose cost depends on the route cannot be priced exactly"). Deposit,
   per-chunk deduction, a top-up event mid-stream, refund on close. It is
   already specified and has a client SDK.
3. **Signed, expiring result URLs as the capability for keyless callers**
   (§4.3 Coder runs, §5 "Threads without a key"). PPQ's 24-hour signed
   `status_url` is the same idea as our long random thread id; use it for run
   status, diffs, and generated files too.
4. **A public, unauthenticated price catalog** (§3, §5 "Prices"). PPQ's
   `/v1/models`, `/v1/audio/models`, `/v1/data/endpoints` list every price
   machine-readably, and the 402 `description` names the item ("PPQ Data: Exa
   Search"). Our endpoints and plugin fees (D9) should be listed the same way,
   in sats and USD.
5. **Key spend caps with reset periods and expiry** (§8). `usage_limit_usd`,
   `reset_period`, `expire_at`, show-once keys, soft revoke. Our scoped keys
   should carry the same limits in sats.
6. **A per-call ledger that includes failures** (§8 audit, D11).
   `/queries/history` lists failed calls with an `error_code` and what was
   actually charged. Our `usage` per response should have a queryable twin.
7. **No-signup accounts and NWC auto top-up** (§5 "Who sees a 402", §8). A
   keyless caller who wants to stop paying per call can mint an account in one
   POST and attach NWC to refill at a threshold. Our wallet already speaks NWC
   on the paying side.
8. **`llms.txt` as the agent-facing doc** (§3). PPQ's is the most complete
   description of its API; agents read it first.
9. **Problem-JSON 402 bodies** (§3 errors). Use RFC 9457 with the
   paymentauth.org `type` URIs when we answer an MPP client.
10. **Don't repeat their inconsistencies**: docs that say L402 while the server
    speaks MPP; three different error-body shapes; a pricing page that mixes
    base and charged prices; an HTML 500 when the price function cannot price
    a request. Each is a small trust cost for a payments API.

### Where we differ

- **Routing is the product, not a 5.5% pass-through.** PPQ's auto router is a
  keyword scorer over fixed tiers ("15 weighted dimensions", "zero external
  API calls") and its savings claim is "up to 70%". Our System One routes each
  message with a Jev judgment, and our cost audit measures 61% cheaper and 37%
  faster on eight development tasks and 45% cheaper on 26 Terminal-Bench tasks
  (§9). We should price per call and let the routing savings show in `usage`,
  rather than resell tokens at list plus a fee.
- **Plugins with author payouts.** PPQ resells upstream data APIs at a markup
  and keeps the margin (§5 above). Our plugin authors set their own per-call
  fee and receive all of it over Lightning after settlement (D7, D9). PPQ has
  no third-party author economy; its partners earn a revenue share on
  distribution, not on what they built.
- **Coder runs on granted computers.** PPQ sells inference and media
  generation; nothing like a Coder run on a user's own machine exists there
  (§4.3, D4).
- **Bitcoin only, and per call by default.** PPQ takes five chains plus cards
  and is prepaid-balance first, with 402 only on media and data. We decided
  Lightning only (D10) and a 402 on any priced endpoint (D3), with free tiers
  for keyless callers (D8). PPQ has no keyless free tier; free models still
  need a key.
- **Threads kept by us** (D5). PPQ keeps no API conversation state beyond
  Responses `store` upstream, and its web memory is local-first and
  enclave-extracted.
- **Privacy claims.** PPQ's Enclave Router with signed routing receipts is a
  stronger claim than ours today. If we keep threads server-side (D5) we should
  say plainly what we store, and consider a signed `route/model` receipt to
  back D11's "every response shows route, model, cost".
- **TypeSafe overlap.** PPQ already resells TypeSafe's decision models at
  list × 1.10 through `/v1/decisions` and lists `typesafe/jev-router` as a
  chat model. That is a distribution channel for Jev, and a reminder that our
  routing advantage is visible to anyone who calls Jev directly.

## 8. Sources

All fetched 2026-10-02.

- PPQ: https://ppq.ai/api-docs, https://ppq.ai/pricing, https://ppq.ai/llms.txt,
  https://ppq.ai/faq, https://ppq.ai/terms, https://ppq.ai/privacy,
  https://ppq.ai/integrate, https://ppq.ai/sitemap.xml
- PPQ blog: https://ppq.ai/blog/payperq-account-types-explained,
  https://ppq.ai/blog/data-enrichment-endpoints,
  https://ppq.ai/blog/introducing-the-enclave-router,
  https://ppq.ai/blog/routing-preferences-on-payperq,
  https://ppq.ai/blog/anon-vs-zdr-vs-tee-privacy-tiers,
  https://ppq.ai/blog/introducing-ppq-stats-dashboard,
  https://ppq.ai/blog/fireworks-ai-primary-open-source-provider,
  https://ppq.ai/blog/using-autoclaw-with-payperq
- PPQ live API (observed): `GET https://api.ppq.ai/v1/models` (and `?type=all`,
  `?type=image,video`, `?type=embedding`), `GET https://api.ppq.ai/v1/audio/models`,
  `GET https://api.ppq.ai/v1/decisions/models`, `GET https://api.ppq.ai/v1/data/endpoints`,
  `GET https://api.ppq.ai/topup/payment-methods`, unauthenticated `POST`s to
  `/v1/data/api/exa/search`, `/v1/images/generations`, `/v1/videos`,
  `/chat/completions`, `/v1/embeddings`, `/v1/decisions`, and
  `POST /accounts/create`, `/credits/balance`, `/topup/create/btc-lightning`,
  `GET /queries/history`, `GET /keys` on one zero-balance account
- OpenRouter: https://openrouter.ai/api/v1/models, https://openrouter.ai/docs/faq
- Provider list prices: https://platform.claude.com/docs/en/about-claude/pricing,
  https://developers.openai.com/api/docs/pricing,
  https://ai.google.dev/gemini-api/docs/pricing
- BTC spot: https://api.coinbase.com/v2/prices/BTC-USD/spot,
  https://api.kraken.com/0/public/Ticker?pair=XBTUSD
- Payment specs: https://paymentauth.org/ (draft-httpauth-payment-01,
  draft-lightning-charge-00, draft-lightning-session-00),
  https://github.com/lightninglabs/L402 (protocol-specification.md)
- Clients: https://github.com/lightninglabs/lnget (README, `mpp/`, `l402/`),
  https://github.com/wevm/mppx (README), https://mpp.dev/payment-methods/lightning
- History: https://www.nobsbitcoin.com/payperq-ai/,
  https://lightninglabs.substack.com/p/summer-of-lightning-from-billions
