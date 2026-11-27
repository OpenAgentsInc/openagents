# OpenAgents v1.0.0 — Monday Launch Checklist & Cutover Playbook

**Launch Date:** Monday, November 30, 2026  
**Status Snapshot:** Friday, November 27, 2026 (`964e083668`)

---

## 1. Current Launch Readiness Matrix (as of 2026-11-27)

| Track | Code on `main` | Staging Live & Verified | Production Live & Verified | Remaining Blocker |
| --- | :---: | :---: | :---: | --- |
| **Homepage Login Link Removed (`964e083668`)** | Done | Ready | Deploying now | Complete `release.sh` + `apps/openagents-site/deploy-prod.sh` |
| **Web Workspace + 9 `/books` + `/chat` + `/login`** | Done | Done (`oa-api-staging-00258-lmr`) | Deploying now | Complete `release.sh` |
| **Gemini `thought_signature` + `web_fetch` HTTP 415 (`0ada691a62`)** | Done | Done (live 2-turn verified) | Pending `oa-api` prod deploy | Deploy `oa-api` prod |
| **OpenAgents Cloud Phases 1–5 + GCE `oa-boat` (`#14125`–`#14130`)** | Done | Done (14/14 live GCE VM conformance) | Pending `oa-boat` + `oa-agent` + `oa-api` prod deploy | Deploy `oa-boat` prod & wire `oa-agent` prod |
| **WorkOS Auth Phases 0–4 (`#14097`–`#14101`)** | Done | Done (`OA_WORKOS_ENABLED=1`) | Secrets + mirror + backups done; `OA_WORKOS_ENABLED=0` | **Owner:** enable Prod Google + Apple OAuth creds in WorkOS Dashboard (`#14102`), then flip `OA_WORKOS_ENABLED=1` |
| **iOS 100% Cloud Mode & Tailscale Removal** | In progress (today) | — | Pending TestFlight upload | Finish PR/push, Simulator tests, archive & upload TestFlight, physical iPhone check |
| **Referrals & Promo Credit (`0224`)** | In progress (today) | Pending deploy after merge | Pending `oa-api` + web prod deploy | Finish PR/push, ASan/UBSan E2E, deploy staging → verify → deploy prod |
| **Desktop Unified macOS App (`#14116`)** | Done | Installed locally | Old DMG (2026-11-23) | Build unified DMG, sign + notarize, upload `/downloads/OpenAgents.dmg`, verify update feed |

---

## 2. Owner-Only Actions Required Before Monday Cutover

These items require owner credentials, dashboards, or physical hardware and cannot be completed autonomously by agents:

1. **WorkOS Production Dashboard (`#14102`):**
   - [ ] Enable and configure real production OAuth credentials for **Google OAuth** and **Apple OAuth** in the WorkOS **Production** environment (currently return `404`; `GitHubOAuth` already returns `302`).
   - [ ] Verify **WorkOS Pipes GitHub** provider is enabled for Production with redirect URI `https://openagents.com/api/integrations/github/callback` (or WorkOS Pipes callback matching staging).
   - [ ] Keep **Phone/SMS** and **Radar SMS challenges** explicitly **OFF** (D3). Enable WorkOS Radar (non-SMS) if desired.
2. **macOS Release Signing & Notarization:**
   - [ ] Build & package unified `OpenAgents.app` → `OpenAgents.dmg`, sign with Developer ID, notarize with Apple (`xcrun notarytool`), and upload to `gs://openagentsgemini-openagents-web/downloads/OpenAgents.dmg`.
   - [ ] Publish signed auto-update feed entry for `v1.0.0`.
3. **iOS TestFlight Release & Physical iPhone Sign-Off:**
   - [ ] Archive and upload the Cloud-only (zero-Tailscale) iOS build (build > `1431`) to App Store Connect / TestFlight.
   - [ ] Verify on a physical iPhone with **Tailscale OFF**: Sign in with Apple / Magic Auth → synced Cloud Chat → Cloud Sandbox or enrolled Mac turn → Spark Wallet + Face ID/Passkey backup.
   - [ ] Confirm public TestFlight link `https://testflight.apple.com/join/dvQdns5B` serves the new Cloud-only build.
4. **GitHub Release Tag:**
   - [ ] Tag and publish GitHub Release `v1.0.0` once all production smoke gates pass on Monday morning.

---

## 3. Weekend → Monday Production Cutover Playbook (Ordered)

Execute strictly in this order once all in-flight tracks (iOS Cloud Mode + Referrals) merge to `origin/main` and owner WorkOS Prod OAuth credentials are active:

### Step 1 — Verify Staging End-to-End (Saturday / Sunday)
```bash
# Deploy latest oa-api-staging (applies 0224_referrals_and_promo_credit.sql)
coax/web/api/deploy.sh --staging

# Verify WorkOS staging + live referral lookup + GCE sandbox conformance
coax/web/api/workos-env-check.sh --staging
curl -fsS "https://staging.openagents.com/api/v1/referral/lookup?code=INVALIDTEST" | head -c 300
BOAT_HOSTED=1 coax/web/tool.sh boat conform --lifecycle
```

### Step 2 — Deploy Production Backend Services (`oa-boat`, `oa-agent`, `oa-api`)
```bash
# 1. Deploy first-party GCE sandbox service for production
coax/web/boat/deploy.sh --production

# 2. Deploy production oa-api (runs migrations 0219..0224 automatically; preserves OA_WORKOS_ENABLED)
coax/web/api/deploy.sh production
coax/web/api/schedules.sh workos-events

# 3. Deploy production oa-agent wired to production oa-boat
coax/web/agent/deploy.sh --production

# 4. Run production backup + restore drill before flipping WorkOS on
coax/web/api/backup/deploy.sh --run dump-auth
coax/web/api/backup/deploy.sh --run drill
```

### Step 3 — Promote Production Website (`openagents.com`)
```bash
# 1. Release full apps/openagents-web (chat, login, settings, projects, /r/, /referrals, 9 /books)
coax/web/release.sh --ref origin/main

# 2. Layer minimalist public homepage (no Sign in link on /, logged-in Home Dashboard mount intact)
#    Optionally pass path to freshly notarized OpenAgents.dmg as $1
apps/openagents-site/deploy-prod.sh [/path/to/notarized/OpenAgents.dmg]
```

### Step 4 — Enable WorkOS on Production (`#14102`)
```bash
gcloud run services update oa-api \
  --project=openagentsgemini --region=us-central1 \
  --update-env-vars="OA_WORKOS_ENABLED=1"

coax/web/api/workos-env-check.sh --production
```

### Step 5 — Final Monday Morning Live Smoke Suite (Go / No-Go)

Every box below MUST pass against live `https://openagents.com`, physical iPhone, and clean macOS install before announcing `v1.0.0`:

- [ ] **Homepage (`https://openagents.com/`):** Pure-black minimalist page loads HTTP 200; **zero** `/login` or `Sign in` link in DOM; Download for Mac (`/downloads/OpenAgents.dmg`) returns HTTP 200 and mounts notarized `OpenAgents.app`; TestFlight link works; Build from source copy buttons work.
- [ ] **Auth (`https://openagents.com/login`):** Magic Auth, Google, GitHub, Apple, Email+Password, Enterprise SSO, Passkey, and Nostr render with 0 console/Wasm errors; signing in redirects to `/` showing the Home Dashboard; `gamebuilder.org` remains unaffected (`OA_WORKOS_GAMEBUILDER_ENABLED=0`).
- [ ] **Cloud Chat + Tools + Sandboxes (`https://openagents.com/chat`):** Cold turn on `google/gemini-4-neon` streams cleanly; multi-turn `web_search` / `web_fetch` JSON + GitHub Pipes repo read succeed (`thought_signature` intact); Cloud Sandbox turn boots a real GCE VM (`oa-bx-...`), executes code, and tears down with 0 leaked VMs; Enrolled Computer turn runs on a live Mac workstation.
- [ ] **Referrals & Promo Credit (`/r/<code>`, `/referrals`, `/settings/account`):** `/r/<code>` displays inviter banner and sets 30-day attribution; `/referrals` renders plain-language terms; eligible GitHub connect grants `$5` (or `$15` stacked with referral); promo credit spends oldest-expiry-first on `google/gemini-4-neon` before paid sats; Settings shows separate Paid & Promo lines; kill-switch flags (`openagents admin referral flag`) respond immediately.
- [ ] **Books (`https://openagents.com/books`):** All 9 books render HTTP 200 with 0 KB JS on chapter pages, search works on `/books`, and `.md` alternates return `text/markdown`.
- [ ] **iOS TestFlight (Physical iPhone, Tailscale OFF):** Sign in with Apple / Magic Auth succeeds; synced Cloud chats load; live Cloud turn + enrolled Mac / Sandbox turn stream; Spark Wallet + Passkey backup work; zero Tailscale UI or Local Network prompt appears.
- [ ] **macOS Desktop (`OpenAgents.app` from `/downloads/OpenAgents.dmg`):** Launches cleanly on Apple Silicon without Gatekeeper warning; local Inkling 2 Mini downloads/verifies SHA-256 and serves 32K Metal 4 chat; OpenAgents account sign-in & Cloud computer enrollment succeed; MCP, Subagents, and `/goal` work cleanly.

---

## 4. Instant Rollback Commands (Zero Data Loss)

- **WorkOS Auth Rollback:**
  ```bash
  gcloud run services update oa-api --project=openagentsgemini --region=us-central1 --update-env-vars="OA_WORKOS_ENABLED=0"
  ```
- **Referrals Kill Switches (Granular, Instant, No Redeploy):**
  ```bash
  openagents admin referral flag referral_redeem off
  openagents admin referral flag referral_grants off
  openagents admin referral flag referral_kickback off
  openagents admin referral flag github_connect_grant off
  ```
- **Website Rollback:**
  ```bash
  coax/web/rollback.sh <STAMP_FROM_RELEASE_OUTPUT>
  apps/openagents-site/deploy-prod.sh   # restore minimalist root index.html if needed
  ```
- **Cloud Run Service Rollback (`oa-api`, `oa-agent`, `oa-boat`):**
  ```bash
  gcloud run services update-traffic <SERVICE> --project=openagentsgemini --region=us-central1 --to-revisions=<PREVIOUS_REVISION>=100
  ```
