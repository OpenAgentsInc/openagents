# OpenAgents v1.0.0 — Cloud Execution, Auth, Money & Referrals Specification

**Backend Services (`us-central1`):** `oa-api`, `oa-agent`, `oa-boat`, `oa-spark`, `oa-voice`  
**Database:** Cloud SQL Postgres (`oa_analytics`), migrations `0001` through `0224`

---

## 1. OpenAgents Cloud Execution Fabric (Phases 1–5)

Migrations `0219`–`0223` implement the three-tier execution model so every user gets real tool execution and coding power from Web, Mobile, and Desktop without Tailscale:

| Tier | Mechanism | Implementation | Status as of 2026-11-27 |
| --- | --- | --- | --- |
| **Tier 1 — Cloud Tools** | Direct server-side tools during streaming chat (`web_search`, `web_fetch`, `github_list_repos`, `github_read_file`, `github_search_code`, `github_list_issues`, `github_get_pr`) | `coax/web/api/cloud-tools.c`, `coax/web/api/integrations.c` (WorkOS Pipes GitHub — zero stored GitHub tokens), `coax/lib/gateway/` (`thought_signature`) | **Shipped & verified live on staging** (`oa-api-staging-00258-lmr`, `#14126`, `#14127`, `0ada691a62`) |
| **Tier 2 — Enrolled Computers** | Outbound-only workstation enrollment + repo-matched project auto-binding + durable `turns_v2` execution on user's Mac/Linux machine | `0220_cloud_projects_environments.sql`, `coax/web/api/environment.c`, `apps/openagents-cli/environments.coax`, `oa-agent` | **Shipped & verified on staging** (`#14125`, `#14127`) |
| **Tier 3 — First-Party Cloud Sandboxes** | Ephemeral Debian 12 GCE VMs (`oa-bx-...`, isolated VPC egress, `block-project-ssh-keys=TRUE`, passwordless sudo `/workspace`) managed by `oa-boat` | `coax/web/boat/oa-boat.c`, `0221_cloud_sandbox_compute.sql`, `0223_cloud_sandbox_metering.sql`, `oa-agent` sandbox driver | **Live on staging & verified 14/14 GCE VM lifecycle conformance** (`oa-boat-staging-00003-tvd`, `oa-agent-staging-00024-d7v`, `#14128`, `#14129`, `#14130`) |
| **Automatic Placement & Metering** | `POST /api/chat/placement/v1` chooses Local vs Enrolled Computer vs Cloud Sandbox; `0223` meters sandbox compute seconds into account balance | `3d05daab3b` (`#14129`) | **Shipped on `main` & staging** |

### Production Promotion Needed for Monday
- Deploy `oa-boat` (production Cloud Run service, Direct VPC Egress `default/default`, `--no-cpu-throttling`, least-privilege `oa-boat@openagentsgemini.iam.gserviceaccount.com` SA) and wire `OA_BOAT_API_BASE` + `OA_BOAT_API_KEY` on production `oa-agent`.
- Deploy production `oa-api` (applying migrations `0219`–`0224`) and production `oa-agent`.
- Enable WorkOS Pipes GitHub provider in WorkOS **Production** environment (already verified live in Staging & Production callback checks).

---

## 2. WorkOS Authentication & Option A1 Security (`0218_workos_auth.sql`)

Reference specs: [docs/auth/2026-11-26-workos-coax-sdk.md](../auth/2026-11-26-workos-coax-sdk.md), [docs/auth/2026-11-26-workos-cutover-runbook.md](../auth/2026-11-26-workos-cutover-runbook.md).

- **Identity Spine & Trust Levels:**
  - `acct_` in Postgres (`auth.accounts`) is the permanent canonical account ID.
  - WorkOS login mints an `L1` session (`__Host-oa_session` HttpOnly cookie on web; device-bound `oa_at_`/`oa_rt_` with `OA-Device-Proof` on iOS/Desktop/Android/CLI).
  - Verified-email (`email_verified: true`) WorkOS logins auto-link to existing `acct_`, log `workos.auto_link`, emit a Resend security notice (`workos.link`), and sync `external_id = acct_...` back to WorkOS.
  - Sensitive operations (`POST /api/wallet/backup`, `POST /api/agents`, recovery, key rotation) ALWAYS enforce first-party `L2` / `L2-fresh` Passkey or Nostr step-up.
  - Freezing an account revokes local + WorkOS sessions and causes `/api/auth/workos/action` to return HMAC-signed `Deny`.
- **Auth Methods Enabled:** Magic Auth (6-digit email), Google OAuth, GitHub OAuth, Apple OAuth (iOS Guideline 4.8), Email + Password, Enterprise SSO + Optional Organizations (`LG_ORG_SELECT` & org switcher), plus first-party Passkeys and Nostr. Phone/SMS and Radar SMS challenges stay **OFF** (D3).
- **Live Postgres Mirror & Verified Backups:**
  - 1-minute Cloud Scheduler poller (`oa-api-workos-events`) mirrors `workos.*` tables into `oa_analytics`.
  - 6-hourly Cloud Run backups (`OA_BACKUP_SCHEMA=auth:workos`) and automated restore drills are live and verified (`"ok":true,"tables":229,"rows":504747,"restore_errors":0`).
- **Zero-Data-Loss Rollback:** Single env var (`OA_WORKOS_ENABLED=0`) reverts all clients cleanly to first-party auth with zero dropped sessions.

---

## 3. Bitcoin-Only Money Rail (`inference.credit`)

- Paid inference, Exa search metering (`0222`), and Cloud Sandbox compute (`0223`) settle strictly in **Bitcoin / Lightning sats** (`inference.credit` stored in millisatoshis via `oa-spark` / Breez Spark).
- No fiat wallets, stablecoins (USDC/USDT), EVM, Solana, Tron, or Cashu exist anywhere in the stack.
- Self-custodial Spark Bitcoin wallets on iOS keep wallet seed custody isolated from account keys; Desktop v1 does not bundle a Bitcoin/Spark wallet or Nostr signer and instead displays read-only Paid + Promo Cloud credit balances fetched from `/api/auth/life`.

---

## 4. Referrals & Promo Credit System (`0224_referrals_and_promo_credit.sql`)

Full specification: [docs/business/2026-11-27-referrals-and-promo-credit-spec.md](../business/2026-11-27-referrals-and-promo-credit-spec.md).

### 4.1 One Unified System, Zero Naming Collisions
- Uses `referral.*` schema, `/api/v1/referral/*` endpoints, and `/r/<code>` public links (never reusing realm `invite` paths/tables).
- Glossary entries (`Referral`, `Referral code`, `Promo credit`, `Kickback`) documented in [docs/books/glossary.md](../books/glossary.md).
- Every rule lives in `#authority` `pay/referral-policy.coax` (mutated to **0 surviving mutants**) with closed wire codecs in `coax/web/api/referral-wire.coax` registered in `proto/registry.json` and **0 new PL/pgSQL route calls** (`sql-rules-routes` unchanged).

### 4.2 Reward Schedule & Rules (Default Config in `referral.config`)

| Reward | Amount | Trigger & Guardrails | Expiry |
| --- | ---: | --- | --- |
| **`github_connect`** | **$5** (`5_000_000` µUSD) | Connect an eligible GitHub account (age ≥ 90 days AND ≥ 3 non-fork public repos OR ≥ 5 followers OR ≥ 1 public org). Permanent uniqueness in `referral.github_claims` (keyed hash retained after deletion). | 90 days |
| **`referral_invitee`** | **$10** (`10_000_000` µUSD) | Redeem a valid referral code within 7 days of signup + connect eligible GitHub (**stacks with `$5` GitHub connect = `$15` total**). Blocks self-referral, shared GitHub, reciprocal referral, and revoked/expired codes. | 90 days |
| **`referral_inviter`** | **$5** (`5_000_000` µUSD) | Awarded to inviter once invitee completes model turns on **2 distinct UTC days** within 30 days of redeeming. Rolling **20/30-day** inviter cap (per-account admin override supported). | 90 days |
| **`kickback`** | **1%** (`100` bps) | `floor(topup_micros * 100 / 10000)` credited to inviter in the **exact same Postgres transaction** as invitee's settled paid Lightning top-up (`inference.topups`). 1-level only, uncapped, forever. | 12 months inactivity |
| **`milestone`** | **$10 / $50 / $250** | One-time bonus at **5 / 25 / 100** activated invitees. | 90 days |
| **`campaign`** | Custom | Admin-issued codes (`openagents admin referral new`) with optional custom bonus, max uses, and expiry. | 90 days |

### 4.3 Promo Credit Spend Order & Safety Controls
- **Separate Balance:** `inference.promo_credit` (USD micros) is strictly separate from paid `inference.credit` (msat), non-withdrawable, non-transferable, and shown on its own line in Settings & Home Dashboard with nearest expiry date.
- **Spend Order (`rp_spend_order`):** Promo credit spends **before** paid sats, **oldest expiry first**, ONLY on config-listed promo-eligible models (default: `google/gemini-4-neon`, `gemini-4-neon`, `openagents/router-decision`), capped at **$5 (`5_000_000` µUSD) per UTC day**. Non-eligible models bypass promo credit cleanly and draw from paid sats.
- **Budget Ceilings & Independent Kill Switches:**
  - Total budget & daily ceiling in `referral.config` govern flat grants (kickbacks are exempt); exceeding ceiling leaves new grants `pending` without erroring.
  - Four independent kill-switch flags (`referral_redeem`, `referral_grants`, `referral_kickback`, `github_connect_grant`) toggleable via `openagents admin referral flag` with full audit trail in `referral.admin_audit`.
  - Admin `hold`, `void`, and `clawback` actions operate exclusively on promo credit and **never touch paid Bitcoin/Lightning sats**.
