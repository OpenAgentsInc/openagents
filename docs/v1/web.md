# OpenAgents v1.0.0 — Web Specification (`openagents.com`)

**Surfaces:** `apps/openagents-site/` (minimalist public root) + `apps/openagents-web/` (Coax Wasm app shell, auth, chat, projects, settings, referrals, 9 static SSR books)  
**Production Bucket:** `gs://openagentsgemini-openagents-web` (`https://openagents.com`)  
**Staging Bucket:** `gs://openagentsgemini-openagents-web-staging` (`https://staging.openagents.com`)

---

## 1. Public Homepage (`/`)

Served by `apps/openagents-site/index.html` (+ `/index.md` markdown alternate, `/mark.svg`, `/favicon.svg`, `/app-preview.{webp,jpg}`, `/og/home.jpg`):

- **Visual identity:** Pure black (`#000000`), high-contrast white typography, white OpenAgents mark, 2x Retina showcase of macOS & iOS workspaces.
- **Primary CTAs (logged-out visitors):**
  1. **Download for Mac** — direct download of signed/notarized `/downloads/OpenAgents.dmg`.
  2. **Join the TestFlight** — `https://testflight.apple.com/join/dvQdns5B`.
  3. **Build from source for Windows and Linux** — expandable copy-to-clipboard shallow clone + build commands (`apps/openagents-desktop/linux/build.sh` and `apps/openagents-desktop/windows/build.sh`) + link to GitHub repository.
- **No public Login link on `/` (enforced in `964e083668`):** The homepage MUST NOT render a `Sign in` or `/login` CTA button. Users arriving via direct link (`/login`), invited referral link (`/r/<code>`), CLI device login (`/login/device`), or app redirect authenticate cleanly on `/login`.
- **Automatic Signed-In App Shell Mount:** On load, `/` makes one `credentials: "same-origin", cache: "no-store"` probe to `GET /api/auth/account` (3s timeout). When `logged_in === true`, `documentElement` receives `.inside`, the static landing `<main>`/`<footer>` hide, and `/coaxial/dom-host.js` mounts `chat-web` into `#home-root` (left-aligned, scrollable) to render the **Home Dashboard**.

---

## 2. Authenticated Web Workspace (`apps/openagents-web/`)

All authenticated workspace pages run inside the unified Coax DOM host (`/coaxial/dom-host.js`, `chat-web.wasm`) with a persistent left navigation rail (`Chats`, `Projects`, `Worlds`, `Feed`, `Messages`, account footer + sign-out):

### 2.1 Home Dashboard (`/`, signed in)
- Account identity summary (email, WorkOS org, Nostr pubkey status, GitHub connection status).
- **Two-line Credit Balance:**
  - **Paid Credit** (Lightning sats from `inference.credit`)
  - **Promo Credit** (`$X.XX` USD from `inference.promo_credit`, nearest expiry date, labeled promo models only)
- **Enrolled Computers** list (`GET /api/chat/environments/v1`) with online/offline heartbeats and enrollment command (`openagents environments enroll`).
- **Projects** list (`GET /api/projects/v1`) with `+ New Project`, GitHub repository picker (WorkOS Pipes), and automatic repo-to-enrolled-computer matching.
- **Connect GitHub** card (`POST /api/integrations/github/connect` via WorkOS Pipes — `dom_go` permits top-level redirects strictly to `workos.com` / `*.workos.com`, `b8bf4be167`).
- Quick links to **Cloud Chat**, **Account Settings**, **Agents & Models**, **Security**, **Devices**, and **Invite / Referrals**.

### 2.2 Cloud Chat (`/chat` & `/chat/<conversation_id>`)
- **Live Cloud Inference Streaming:** Calls `POST /api/v1/inference` with SSE deltas, full Markdown rendering, copyable replies/code blocks, token/cost telemetry (`in / out`, `Today $X.XXXX · N req · N sats`), and multi-turn tool-calling loops.
- **Gemini `thought_signature` Preservation (`0ada691a62`):** Full `extra_content.google.thought_signature` round-tripping in `coax/lib/gateway/` so multi-turn Vertex AI Gemini 4 Neon tool calls execute without HTTP 400 errors.
- **Tier 1 Cloud Tools (`/api/tools/cloud/v1`):** Built-in `web_search` (Exa), `web_fetch` (supports HTML, JSON `application/json`, and plain text up to 16 KiB without HTTP 415 rejections), `github_list_repos`, `github_read_file`, `github_search_code`, `github_list_issues`, and `github_get_pr` (authenticated via WorkOS Pipes when GitHub is connected, or public GitHub API for public repos).
- **Execution Target Selector (`Target: Auto | Cloud | Cloud Sandbox | <Enrolled Computer>`):**
  - **Auto:** Queries `POST /api/chat/placement/v1` to pick Local / Enrolled Computer / Cloud Sandbox based on project binding, online workstations, and user preference.
  - **Cloud:** Direct streaming turn with Tier 1 tools.
  - **Enrolled Computer (Tier 2):** Submits durable turn via `POST /api/chat/submit/v2alpha1` to an online Mac/Linux workstation enrolled under the user's account and streams live `turns_v2` events via SSE.
  - **Cloud Sandbox (Tier 3):** Spins up or reuses a first-party GCE Debian 12 sandbox VM via `oa-agent` → `oa-boat`, clones the project repo if linked, runs real shell/code execution, streams output live, and meters compute transparently.
- **Synced Conversation History:** Backed by `/api/chat/v2/conversations` and `/api/chat/v2/stream`, synced in real time across Web, iOS, Desktop, and CLI.

### 2.3 Projects (`/projects`)
- Create private account projects, bind an optional GitHub repository (`owner/repo`) via the WorkOS Pipes repo picker, attach project instructions, and auto-bind to matching git remotes on any enrolled computer (`0220_cloud_projects_environments.sql`).

### 2.4 Authentication (`/login`, `/login/device`, `/login/email`)
- **Custom Coax GPU Login UI (`login.coax` + `login/key.js`):**
  - Reads `GET /api/auth/workos/config`.
  - When `OA_WORKOS_ENABLED=1`: renders **Magic Auth** (6-digit email code), **Continue with Google**, **Continue with GitHub**, **Continue with Apple**, **Email + Password**, **Enterprise SSO** (domain discovery), **Multi-Org Picker** (`LG_ORG_SELECT`) & signed-in **Org Switcher** (`H_SWITCH_ORG`), plus first-party **Passkey** and **Nostr** sign-in.
  - When `OA_WORKOS_ENABLED=0` (instant rollback safety): gracefully falls back to first-party email codes, Passkey, and Nostr with zero console/Wasm errors.
  - Auto-redeems any pending first-party referral code (`oa_ref` cookie / `?ref=<code>`) via `POST /api/v1/referral/redeem` immediately after sign-in.
- **CLI Device Login (`/login/device`):** RFC 8628 code verification UI for `openagents login --workos`.

### 2.5 Settings (`/settings/*`)
- `/settings/account` — Profile, email, WorkOS org switcher, GitHub connection, two-line Paid/Promo credit balance, and **Invite / Referrals** panel (personal Crockford base32 link, optional GitHub handle vanity link toggle, Copy, Share on X/Nostr, README badge Markdown, pending/activated counts, earned promo & lifetime kickback totals, 5/25/100 milestone progress, Redeem code input within 7-day window, Recheck GitHub Eligibility, and show-name / commit-footer opt-in toggles).
- `/settings/agents` — Connected agents, capability masks, revoke/rotate (`L2` step-up).
- `/settings/security` — Passkeys, Nostr keys, recovery, account freeze (`L2` / `L2-fresh`).
- `/settings/devices` — Active sessions & enrolled computers.
- `/settings/wallet` & `/settings/lightning` — Lightning top-ups, Lightning Address, Spark wallet backup (`L2-fresh`).

### 2.6 Referrals Public Pages (`/r/<code>` & `/referrals`)
- **`/r/<code>` (`apps/openagents-web/r/index.html`):** Public invited landing page. Calls `GET /api/v1/referral/lookup?code=<code>`, records click, persists code in 30-day first-party `localStorage` + `oa_ref` cookie, renders `"@handle gave you $10 of OpenAgents credit."` (or neutral generic invite copy if `show_name` is off), primary CTA **Continue with GitHub** (stacking `$10` invitee + `$5` GitHub connect = `$15`), secondary email/passkey/Nostr CTA, 2-line eligibility summary, and link to `/referrals`. Unknown/revoked codes degrade gracefully to a neutral welcome state.
- **`/referrals` (`apps/openagents-web/referrals/index.{html,md}`):** Plain-language public program terms (amounts, GitHub eligibility criteria, 90-day flat / 12-month inactivity kickback expiry, `$5`/UTC-day cap, promo-eligible models, non-withdrawable policy, clawback & anti-abuse rules, terms version).

### 2.7 Nine Static SSR Technical Books (`/books`)
- Built purely in Coax (`apps/openagents-web/books-build.coax` → `build/books/`) with **0 KB JS / 0 KB Wasm** on chapter pages (progressive-enhancement search on `/books/index.html` only), GitHub alerts, pure-Coax Mermaid SVGs, chapter TOC/meta bar, and raw `index.md` alternates for LLMs & agents:
  1. `coax` — *The Coax Programming Language*
  2. `openagents` — *The OpenAgents Book*
  3. `bitcoin` — *Bitcoin From Scratch*
  4. `nostr` — *Nostr From Scratch*
  5. `coder` — *Coder: Coding Agents From Scratch*
  6. `psionic` — *Psionic: ML From Scratch*
  7. `metaverse-engine` — *Building the OpenAgents Metaverse Engine*
  8. `everglade` — *Building Everglade*
  9. `claude-code` — *Reverse Engineering Claude Code*

---

## 3. Production Release Pipeline

Deploying the website to production (`https://openagents.com`) is a deterministic two-step release:

1. **Full App + Books Release (`coax/web/release.sh --ref origin/main`):**
   - Builds `apps/openagents-web` (reusing verified prebuilt artifacts via `pages.sh`), uploads to `openagents-web-staging`, runs `smoke.sh` + headless Chrome `goldens.ts --starts` diffed against production (blocking only on new regressions), syncs to `gs://openagentsgemini-openagents-web` (preserving `releases/`, `demo/`, `downloads/`, and user-published slugs), writes a rollback snapshot under `releases/<stamp>/`, and re-verifies production with automatic rollback on any regression.
2. **Minimalist Homepage Layer (`apps/openagents-site/deploy-prod.sh`):**
   - Publishes `apps/openagents-site/{index.html,index.md,mark.svg,favicon.svg,app-preview.*,og/home.jpg}` to `gs://openagentsgemini-openagents-web/` with `Cache-Control: no-cache` on `index.html` and `index.md` so `/` serves the clean minimalist download homepage (zero login link) while signed-in users mount `chat-web` and all `/chat`, `/login`, `/settings`, `/projects`, `/r/`, `/referrals`, and `/books` routes serve the freshly promoted `apps/openagents-web` build.
