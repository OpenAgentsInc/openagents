# OpenAgents Machine Payments (MPP) — Launch & Operations Runbook

**Date:** 2026-06-23 · **Epic:** [#6049](https://github.com/OpenAgentsInc/openagents/issues/6049)

How the OpenAgents inference API accepts per-call machine payments (MPP / x402),
how it's armed/rolled back, how to prove it, and how to watch for the Stripe
Directory **Machine Payments** badge.

> Authoritative protocol spec is the local mirror at `docs/reference/mpp/`
> (paymentauth specs + mpp.dev SDK). The runtime 402 challenge is always
> authoritative over the discovery doc.

---

## 1. Status (2026-06-23) — all three rails LIVE on prod; badge pending

| Rail | State |
|---|---|
| **⚡ Lightning** (BOLT11 via Spark, local preimage verify) | ✅ **LIVE — leads the 402.** Spark PRIMARY / MDK FALLBACK issuer; real mainnet BOLT11 (see §6). ~0.9–2.4s warm |
| **USDC / crypto** (Tempo/Base/Solana USDC, Stripe deposit-mode) | ✅ **LIVE on prod**, full pay-loop proven end-to-end on staging |
| **Card / SPT** (Stripe Shared Payment Tokens, `profile_…` networkId) | ✅ **LIVE on prod** (unit-tested + fail-closed; no live SPT round-trip yet) |
| **Stripe Directory badge** | ⏳ pending Stripe's async crawl of `/openapi.json` (≤~24h) — our side is complete |

Prod worker: `openagents-autopilot` (verify current deploy version with
`npx wrangler deployments list` after each release).
`POST /api/mpp/v1/chat/completions` (legacy `/mpp/v1/chat/completions` aliases it) →
`402` with a real BOLT11 + deposit address. **402 offer ordering: lightning → base/usdc
→ stripe/card.** `GET /openapi.json` is live and advertises the offers with **lightning
first** (`x-service-info categories:["ai"]`). Profile `@openagents` is live (Network ID
`profile_61Uug9…`); crypto payins enabled (live + test).

**Primary sale model (#6169):** the MPP default is `openai/gpt-oss-20b`, served
under the raw upstream model id. `openagents/khala-*` ids stay supported only for
Khala-specific behavior such as Blueprint/coordinator/verifier/Pylon surfaces.

**Production paid proof (#6169):** deploy
`e66a59cd-7ad4-48bf-801e-1230064a467f` completed a live 1-sat Lightning MPP
payment for `openai/gpt-oss-20b` on 2026-06-24T01:51:12Z. The flow was
`402 Payment` (Lightning mainnet, amount `1 sat`) -> MDK wallet payment ->
`Authorization: Payment ...` retry -> `200` OpenAI-compatible
`chat.completion` with `Payment-Receipt` method `lightning`, status `success`.
The Base USDC and Stripe SPT rails also remain advertised live; USDC test-helper
settlement is sandbox-only because prod issues live-mode Stripe PaymentIntents.

## 2. Architecture (Worker-native, no Node sidecar)

- **Endpoint:** `POST /mpp/v1/chat/completions`. Method-agnostic HMAC challenge
  binding in `workers/api/src/inference/mpp/mpp-canonical.ts` (WebCrypto SHA-256,
  RFC-8785 JCS, base64url) per `draft-httpauth-payment-00`. The crypto deposit
  PaymentIntent id / Lightning paymentHash ride in the challenge `opaque` for
  stateless recovery; verification is fail-closed.
- **Crypto:** Stripe deposit-mode PaymentIntent; deposit addresses are
  `next_action.crypto_display_details.deposit_addresses` — a **network-keyed
  object** `{ "base": { "address": "0x…", "supported_tokens": [...] }, … }` (NOT
  an array). API version `2026-03-04.preview`.
- **Card/SPT:** create+confirm a PaymentIntent with the client's SPT; the
  `profile_…` id is the `networkId`. Single-use SPT replay guard in D1.
- **Lightning:** mint a BOLT11 invoice as the challenge, client returns the
  preimage, verify locally with `sha256(preimage) == paymentHash` (no node call).
  The issuer is **Spark** (`@breeztech/breez-sdk-spark`) — **PRIMARY** per the rail
  invariant — minted through the existing **`MDK_TREASURY` container's
  `/spark/funding-invoice`** endpoint (returns `paymentHash`; `/spark/received/:hash`
  confirms the offline receipt). The **MDK sidecar is the explicit FALLBACK issuer
  only**. Spark supports the offline receives that the agent/MPP rail needs; MDK is
  checkouts-only (`apps/openagents.com/INVARIANTS.md` › Payment Rail Separation). See
  §6 for the live status and the deploy gotcha.
- **Credit semantics (RL-3):** USDC/card mint **USD-origin** credit
  (`agent_balances.usd_credit_msat`, inference-spendable, **NOT**
  Bitcoin-withdrawable). Lightning mints **Bitcoin-origin** `balance_msat`
  (pay-in type `lightning_charge`). Idempotent per PaymentIntent / paymentHash.
- **Discovery:** `GET /openapi.json` (OpenAPI 3.1, `x-service-info
  categories:["ai"]`, `x-payment-info.offers[]`). Offers are **flag-gated** — a
  rail is advertised only when it is actually armed AND can fulfill (honesty
  gate; never advertise an unfulfillable rail).

## 3. Config / arming (Worker secrets — never committed)

Set from `apps/openagents.com/workers/api` (prod = no `--env`; staging =
`--env staging`). Key material lives in `~/work/.secrets/openagents-stripe-mpp.env`
(gitignored) — pipe into wrangler via stdin, never echo:

| Name | Kind | Purpose |
|---|---|---|
| `KHALA_MPP_ENABLED` | secret `true` | master arm |
| `STRIPE_API_KEY` | secret (`rk_live` restricted, PaymentIntents:Write) | crypto + card settle |
| `KHALA_MPP_SIGNING_SECRET` | secret (`openssl rand -hex 32`) | HMAC challenge binding |
| `STRIPE_MPP_NETWORK_PROFILE_ID` | var (`profile_…`) | card/SPT networkId; presence enables the card offer |
| `KHALA_MPP_LIGHTNING_ENABLED` | secret/flag | Lightning rail (Spark mint via the `MDK_TREASURY` container) — set on prod |

All five are **set on prod** (`openagents-autopilot`) — all three rails are LIVE.

Triple-gate: with `KHALA_MPP_ENABLED` OR `STRIPE_API_KEY` OR
`KHALA_MPP_SIGNING_SECRET` absent → `503 mpp_not_configured`, never a charge.

**Deploy (manual — no auto-deploy CI):** from `apps/openagents.com`,
`bun run build:web` (web app builds to `apps/openagents.com/apps/web/dist`), then
`cd workers/api && npx wrangler deploy --containers-rollout=none --assets ../../apps/web/dist`.
Apply D1 migrations: `npx wrangler d1 migrations apply openagents-autopilot --remote`.
Always deploy from a clean `origin/main` worktree.

## 4. Rollback (make a rail inert without a code revert)

- Whole endpoint: `wrangler secret delete KHALA_MPP_ENABLED` → `503` inert.
- A single rail: delete its flag (e.g. `KHALA_MPP_LIGHTNING_ENABLED`) or unset
  `STRIPE_MPP_NETWORK_PROFILE_ID` (card). Confirm with the proof smoke.

## 5. Proving it

- **Inert/armed:** `bun run smoke:khala:billing-mpp-proof -- --base-url <url> --json`
  → `mpp_unauthenticated_safe_state` = `inert` (503) or `armed_402` (never a free
  completion). Sends no payment.
- **Full crypto pay-loop** (staging, test key): `bun run smoke:khala:mpp-payloop`
  with `KHALA_MPP_PAYLOOP_BASE_URL` + `KHALA_MPP_PAYLOOP_STRIPE_TEST_KEY`. Proves
  402 → settle → 200 + `Payment-Receipt` + credit. The settle step calls
  `POST /v1/test_helpers/payment_intents/{id}/simulate_crypto_deposit`
  (`Stripe-Version: 2026-03-04.preview`; params `transaction_hash` (`…testsuccess`),
  `network`, `token_currency=usdc`, `buyer_wallet`). Testnets aren't auto-detected,
  so the simulate helper is required in sandbox; **prod/mainnet uses real on-chain
  settlement — no simulate**.
  Default model: `openai/gpt-oss-20b`. Override with
  `KHALA_MPP_PAYLOOP_MODEL=<model>` only when testing a Khala-specific model.
- **Full Lightning pay-loop** (prod/mainnet, tiny live payment): request
  `openai/gpt-oss-20b` without a credential, pay the returned BOLT11 with the
  MDK agent wallet, then retry with the preimage credential. This was proven on
  2026-06-24T01:51:12Z for 1 sat and returned `200` + `Payment-Receipt`.

## 6. Lightning (LIVE on Spark, leads the 402 — 2026-06-23)

Lightning is armed on prod and **leads the 402** with a real mainnet BOLT11.
- **Issuer:** Spark (`@breeztech/breez-sdk-spark`) is primary — the rail mints via the
  existing **`MDK_TREASURY` container's `/spark/funding-invoice`** (Breez `receivePayment`
  BOLT11; #6152 added the returned `paymentHash` + a `/spark/received/:hash` confirm). The
  **MDK sidecar** is the explicit *fallback* issuer only. Verify is local
  (`sha256(preimage)==paymentHash`). Bounded per-leg timeouts + per-rail isolation (#6149)
  guarantee a slow/failed Lightning leg only drops Lightning — never hangs the endpoint.
- **Latency:** ~0.9–2.4s warm (cold first mint after container idle ~5.6s).
- **Arm:** `KHALA_MPP_LIGHTNING_ENABLED=1` (set on prod) + `KHALA_MPP_ENABLED` + the Stripe
  key + `KHALA_MPP_SIGNING_SECRET`.

> **DEPLOY GOTCHA (this caused a multi-hour saga).** The Spark mint code lives in the
> `MDK_TREASURY` **container**. Prod deploys default to `--containers-rollout=none` because
> **the container image build needs Docker Desktop running locally** — if Docker is down,
> the container can't rebuild, so the running container stays an OLD image (its
> `/spark/funding-invoice` lacked `paymentHash` → the Spark issuer fail-closed → Lightning
> silently dropped). **Fix:** start Docker Desktop, then deploy **without**
> `--containers-rollout=none` (`wrangler deploy --assets ../../apps/web/dist`) so the
> `MdkTreasuryContainer` image rolls. The wallet derives from `MDK_TREASURY_MNEMONIC` and
> re-syncs across the rollout — verify treasury balance/health via
> `GET /api/operator/treasury/status` before and after (custody check). Rolled out at prod
> version `271a3720`; treasury balance was 15 sat and stayed unchanged across the rollout.

Per the rail invariant (`apps/openagents.com/INVARIANTS.md` › Payment Rail Separation):
Spark is the primary agent/MPP rail (offline receives); MDK is checkouts-only + the
fallback issuer only.

## 7. Watching for the Stripe Directory badge

The badge appears when Stripe's directory crawler indexes `/openapi.json` and
associates the Machine Payments capability — **asynchronous, up to ~24h** per the
discovery spec. Our side is done. To check:

```sh
stripe directory search "llm inference api" --mpp-supported   # look for OpenAgents / @openagents with Machine Payments ✅
stripe directory search "openagents"
stripe directory me                                           # see note below
```

> **Known CLI quirk:** `stripe directory me` persistently returns "Your account
> does not have a Stripe profile" even though the Dashboard shows `@openagents`
> live/public (Network ID `profile_61Uug9…`). Treat the Dashboard as truth; this
> is most likely network-index lag. If the badge hasn't appeared well after the
> crawl window, the profile may need a poke (re-save, or contact Stripe).

**Expedite discovery** (broader than the Stripe badge): register on
[MPPScan](https://www.mppscan.com/register) (one-click), submit a PR to
[mpp.dev/services](https://github.com/tempoxyz/mpp), and check the Stripe
Dashboard agentic-commerce/directory area for any submit.

## 8. Gotchas

- `deposit_addresses` is a **network-keyed object**, not an array.
- `simulate_crypto_deposit` lives under `/v1/test_helpers/…`.
- `check-contract-drift.test.ts` **false-fails when run from inside `.worktrees/`**
  (its own SKIP_DIR regex skips `.worktrees`); the file passes 5/5 from a normal
  checkout. Don't chase a contract-drift-only red in a worktree.
- Build outputs to `apps/openagents.com/apps/web/dist`; deploy `--assets
  ../../apps/web/dist` is **mandatory** or you ship stale UI / a 404 `/`.
- The Spark mint code lives in the **`MDK_TREASURY` container**, not the Worker.
  Prod deploys default to `--containers-rollout=none`, which leaves an OLD container
  image — deploy WITHOUT that flag (Docker Desktop must be running locally) to roll
  `MdkTreasuryContainer`. See §6.

## 8a. Open items (2026-06-23)

1. **The badge** — purely external; waiting on Stripe's async crawler indexing
   `/openapi.json` (≤~24h). A 30-min watch loop is running. Our side is complete.
2. **Trivial tracked bug** — the MDK *fallback* sidecar builds a doubled
   `/api/mdk/api/mdk` path. Harmless in practice because the Spark primary issuer
   works; tracked for cleanup.
3. **Future optimization** — a **pre-minted Spark invoice pool** for zero-latency
   402s (eliminates the cold-mint first-hit cost). Tracked on
   [#6049](https://github.com/OpenAgentsInc/openagents/issues/6049).

## 9. References

- Epic [#6049](https://github.com/OpenAgentsInc/openagents/issues/6049); PRs:
  #6131 (profile id), #6132 (deposit-address parser), #6138 (Worker-native HMAC
  verify), #6139 (`/openapi.json`), #6141/#6144 (pay-loop smoke), #6146 (initial
  MDK Lightning — superseded), #6149 (mint timeout + per-rail isolation),
  #6152 (**Spark primary + MDK fallback** + container `/spark/funding-invoice`
  `paymentHash` + `/spark/received`), #6153/#6157 (mint budgets), #6159 (leg
  diagnostics).
- Plan: `docs/stripe/2026-06-22-khala-mpp-integration-plan.md`; survey:
  `docs/stripe/2026-06-22-stripe-directory-mpp-khala.md`.
- Protocol mirror: `docs/reference/mpp/`.
- Production-proof gate: `apps/openagents.com/docs/launch/2026-06-23-khala-billing-mpp-production-proof.md`.
