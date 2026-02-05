# Wallet considerations (OpenAgents)

**Purpose:** Single reference for all wallet-related considerations in the OpenAgents ecosystem: wallet stacks (Spark SDK / Breez vs MoneyDevKit), OpenAgents.com wallet today, OpenClaw integration (connect your own + wallet visibility), multi-tenant OpenClaw server, and mechanics of running a node locally.

**Sources:** docs/local/openclaw-openagents-website-integration.md, docs/local/openclaw-moneydevkit-wallets-on-openagents.md, docs/local/multi-tenant-openclaw-server.md, docs/local/spark-sdk-wallet-stack-comparison.md, apps/website/public/NODE.md, OpenClaw chris/ docs, MoneyDevKit README and ARCHITECTURE.md, spark-sdk README and docs.

---

## Table of contents

1. [Wallet stacks: Spark SDK (Breez) vs MoneyDevKit](#1-wallet-stacks-spark-sdk-breez-vs-moneydevkit)
2. [OpenAgents.com wallet today](#2-openagentscom-wallet-today)
3. [OpenClaw ↔ OpenAgents.com integration](#3-openclaw--openagentscom-integration)
4. [OpenClaw + MoneyDevKit: wallets visible on OpenAgents.com](#4-openclaw--moneydevkit-wallets-visible-on-openagentscom)
5. [Multi-tenant OpenClaw server](#5-multi-tenant-openclaw-server)
6. [Running a node locally](#6-running-a-node-locally)
7. [Summary tables](#7-summary-tables)

---

## 1. Wallet stacks: Spark SDK (Breez) vs MoneyDevKit

### 1.1 What is the Spark SDK (Breez)?

**Breez SDK – Nodeless (Spark Implementation)** is a **nodeless** Lightning + Bitcoin L2 SDK. Your app does **not** run a Lightning node.

- **Spark** is a Bitcoin-native Layer 2 built on a **shared signing protocol**. Spark Operators facilitate transfers but **cannot move funds without the user**. Funds are self-custodial: you hold the keys; you can exit Spark and reclaim bitcoin on the Bitcoin main chain.
- **In your app:** You integrate the SDK (Rust core; bindings for Go, C#, React Native, Python, JS/TS, Flutter, Kotlin, Swift, **WebAssembly**). You initialize with: **mnemonic**, **Breez API key** (free, request from Breez), and a **storage directory** (or IndexedDB in the browser). No separate node process.
- **Hosted services:** Breez provides the API (for Spark coordination), an optional real-time sync server (multi-device), and optional LNURL server.

**Key features:** Send/receive Bolt11, LNURL-Pay, Lightning address, BTC address, Spark address, BTKN; on-chain interoperability; multi-device sync; payments persistency and restore; automatic on-chain claims; WebAssembly (browser); keys held only by the user.

**Credentials:** Mnemonic + Breez API key. No node to run.

### 1.2 What is MoneyDevKit (MDK)?

**MoneyDevKit** is a **hybrid** stack:

- **Hosted API** (moneydevkit.com): Checkout sessions, product catalog, customer records, onboarding (device flow → API key), VSS (wallet state backup), Esplora, LSP. Auth: `x-api-key` (MDK_ACCESS_TOKEN).
- **Self-hosted Lightning node** (in your app): You run **ldk-node** (via lightning-js bindings or native). Your app creates Lightning invoices, receives payments on your node, and stores channel/wallet state (locally or via MDK VSS). **You** operate the node; funds are self-custodial.
- **Checkout flow:** Your app calls MDK API to create a checkout → your **local node** creates an invoice → your app registers the invoice with MDK API → customer pays via Lightning → your node receives payment and notifies MDK API.

**Credentials:** MDK_ACCESS_TOKEN + MDK_MNEMONIC (for the node). The node runs in your process (or subprocess); you must persist node state (or use VSS).

**Key point:** MDK is **not** nodeless. The hosted API coordinates checkouts and infrastructure; the **node is yours**. MDK API does **not** expose balance — balance lives in the node.

### 1.3 Side-by-side comparison

| Dimension | **Spark SDK (Breez)** | **MoneyDevKit (MDK)** |
|-----------|------------------------|------------------------|
| **Architecture** | Nodeless. SDK + Breez API + Spark L2. No Lightning node in your app. | Hybrid. Hosted API + **self-hosted Lightning node** (ldk-node) in your app. |
| **Where funds live** | Spark L2 (shared signing); you hold keys; can exit to Bitcoin main chain. | Your Lightning node (channels + on-chain wallet via ldk-node). |
| **What you run** | Your app + SDK (init with mnemonic, API key, storage). No node process. | Your app + **Lightning node** (ldk-node) + MDK API client. Node must run and persist state. |
| **Credentials** | Mnemonic + Breez API key. | MDK_ACCESS_TOKEN + MDK_MNEMONIC (node seed). |
| **Hosted services** | Breez API, optional sync server, optional LNURL server. | MDK API (checkout, products, VSS, Esplora, LSP). |
| **Protocols** | Bolt11, LNURL-Pay, Lightning address, BTC address, Spark address, BTKN. | Lightning (Bolt11, etc.) via your node; checkout/product flow via MDK API. |
| **Checkout / products** | Not built in. You build your own or use something else. | Built-in: create checkout, product catalog, register invoice, payment received webhooks. |
| **Multi-device** | Real-time sync server (Breez or custom). | VSS (MDK or self-hosted) for node state backup/recovery. |
| **Browser / WASM** | Yes. SDK has WebAssembly bindings; OpenAgents website uses it (IndexedDB). | Node runs server-side or in OpenClaw; browser talks to backend/bridge. |
| **AI agents** | Can use SDK with API key + mnemonic in backend or WASM. No node ops. | Can use MDK + node (e.g. in OpenClaw). Node persistence and ops required. |

### 1.4 How Spark SDK works (mechanics)

1. **Init:** Call SDK `connect` (or Builder) with: network, mnemonic, Breez API key, storage path (or IndexedDB for WASM). SDK loads or creates state.
2. **No node:** SDK talks to Breez/Spark services. Your keys never leave your app (derived from mnemonic).
3. **Receive:** Request an invoice (Bolt11) or Spark address/invoice; SDK returns payment request. When someone pays, you get events and can call `get_info` for balance.
4. **Send:** Pass an invoice or address; SDK executes payment via Spark/Bolt11.
5. **Sync:** Optional real-time sync server keeps multiple SDK instances in sync.
6. **Exit:** SDK supports on-chain claims and config (e.g. max deposit claim fee).

**Limitations (Spark SDK):** Depends on Breez/Spark; API key required (free but mandatory); no built-in checkout/products; WASM needs cross-origin isolation (e.g. SharedArrayBuffer); mnemonic in browser is a tradeoff.

### 1.5 How MoneyDevKit works (mechanics)

1. **Account:** Get MDK_ACCESS_TOKEN from moneydevkit.com or `npx @moneydevkit/create`.
2. **Node:** Start ldk-node with MDK_MNEMONIC and config (Esplora, LSP, optionally VSS). State stored locally or in VSS.
3. **Checkout:** Call MDK API to create checkout; your **node** creates invoice; register it with MDK API (with nodeId); customer pays; your node receives; call MDK API `paymentReceived`.
4. **Balance / history:** Come from **your node**, not from MDK API. So “wallet visible on OpenAgents.com” with MDK means: something that runs the node (e.g. OpenClaw) must expose balance/receive/send via a bridge.

**Limitations (MDK):** You run a node (persistence, backups, channel management); no first-class WASM/browser node; checkout-centric.

### 1.6 When to use which

| Use case | Spark SDK (Breez) | MoneyDevKit |
|----------|-------------------|-------------|
| Wallet in the browser (no backend node) | ✅ Good fit. WASM, IndexedDB. OpenAgents website uses it. | ❌ Node runs server-side or in OpenClaw; browser talks to bridge. |
| E‑commerce checkout (session, products, webhooks) | ❌ Not built in. | ✅ Good fit. MDK API + your node; mdk-checkout for Next.js. |
| Agent/bot that receives and sends sats | ✅ SDK in backend/headless. No node ops. | ✅ MDK + node in OpenClaw; agent uses tools. |
| “Wallet visible on OpenAgents.com” minimal setup | ✅ Already there: Breez SDK on site. | ✅ Via OpenClaw: OpenClaw runs node; bridge exposes wallet summary. |
| Full control over channels (run your own node) | ❌ You don’t run a node. | ✅ You run ldk-node; you manage channels, LSP. |

**One line:** Spark SDK = nodeless wallet (app + Breez API + Spark L2); MDK = app + **your Lightning node** + MDK API for checkout and infra.

---

## 2. OpenAgents.com wallet today

- **Wallet UI:** Exists (WalletApp, WalletHomePage, ReceivePaymentDialog, SendPaymentDialog, wallet routes). Implemented with **Breez Spark SDK** (`@breeztech/breez-sdk-spark`).
- **Stack:** WASM SDK, IndexedDB for storage (`openagents-spark-wallet`), env `VITE_BREEZ_API_KEY`. Receive (Lightning/Spark/Bitcoin), send, balance, Lightning address, deposits/claims. No node runs on the server; nodeless in the browser.
- **Identity:** Nostr (npub), Convex/auth where used. Wallet is per-browser/session (mnemonic in SDK storage).
- **Other wallet paths:** openclaw-wallets.mdx describes Pylon-based agent wallets (Bitcoin/Lightning for agents). No MDK integration on the website yet; planned via OpenClaw + bridge (see §4).

---

## 3. OpenClaw ↔ OpenAgents.com integration

### 3.1 Do we want people to connect their OpenClaw?

**Recommendation:** **Optional only.** Do not require connecting OpenClaw to use the site (browse, feed, KB, wallet, Nostr). Support an optional “Link your OpenClaw” for users who run OpenClaw locally (or on a tailnet) and want the site to show status or trigger limited actions. Connecting is a trust/security decision; keep it opt-in and scope-limited.

### 3.2 How does it work?

1. **You run OpenClaw** on your own machine (or server). Gateway listens locally (e.g. `ws://127.0.0.1:18789`).
2. **You run a bridge** (plugin or separate service) that talks to your Gateway and exposes HTTPS endpoints (e.g. Tailscale Funnel). You paste the **bridge URL** (and optionally a token) into OpenAgents.com settings. The **site never gets your Gateway password or token**; it only calls your bridge URL.
3. **The site calls your bridge** for opted-in features: status, chat, “send this to OpenClaw,” wallet summary (if OpenClaw has MDK wallet), etc. Bridge forwards to Gateway and returns the response.
4. **Identity:** You sign in on OpenAgents.com with Nostr (or Convex). “Linked OpenClaw” = we store the bridge URL (and token) for your account. We don’t store OpenClaw credentials.

**Multi-tenant path (§5):** If you use a hosted OpenClaw bot from the site, you don’t run anything; “connect” is “this is my bot”; no bridge needed.

### 3.3 What can you do?

**Without connecting OpenClaw:** Everything today: feed, KB, Nostr, wallet (Breez), API key, communities.

**If you connect your own OpenClaw (via bridge):**

| You can … | How |
|-----------|-----|
| See “OpenClaw status” on the site | Bridge returns minimal status (e.g. “3 channels, 2 sessions” or “unreachable”). |
| Chat with your OpenClaw from the site | Site → bridge → Gateway `agent` → streamed reply. |
| Send a KB/article “to my OpenClaw” | Click “Send to OpenClaw”; site POSTs title + URL to your webhook; OpenClaw receives as message or reading list. |
| Run a task (e.g. Autopilot) from the site | Site asks bridge to run `autopilot.run`; you see “started” then “finished” with summary/link. |
| Set a reminder delivered via OpenClaw (Telegram/WhatsApp) | Site sends “remind me at X” to bridge; OpenClaw cron/reminder sends you a message. |
| Publish to Nostr “via” your OpenClaw | Draft on site; site sends to bridge; OpenClaw (Nostr channel) publishes; keys stay in OpenClaw. |
| See recent OpenClaw activity | Bridge returns last N runs (one-line summaries only). |
| See your OpenClaw wallet on the site | If OpenClaw has MDK-backed wallet, bridge returns balance, receive invoice, recent payments; site shows “OpenClaw wallet” card. |
| Fund your agent from the site | “Pay this agent” can use MDK checkout or show agent’s Lightning address. |

**From OpenClaw toward the site:** Share to feed, save to “Saved from OpenClaw,” cross-post to community, invite OpenClaw as community bot (with the right plugins/skills).

### 3.4 Limitations (OpenClaw connection)

- **Site never sees your Gateway.** It only talks to your bridge. If the bridge is down, “connected” features won’t work from the site.
- **You run and expose the bridge.** We don’t host it. You need HTTPS (Tailscale, tunnel, or VPS) for the site to reach it.
- **No Gateway credentials on the site.** Trust is narrow but site can’t talk to Gateway directly; everything goes through your bridge.
- **No live message content or channel list** on the site (by design unless we add explicit consent flow). Status is minimal; activity is summaries only.
- **No full Gateway control panel** on the site; only a limited set of actions via the bridge.
- **Optional only.** Connecting is never required.
- **Reachability:** If Gateway and bridge are only on your home network without a tunnel, the site can’t reach them when you’re away.
- **Features are “could have,” not all shipped.** Each depends on bridge contract and site UI being implemented.

### 3.5 How “connect” works technically

- **Option A (recommended):** User runs a bridge that talks to Gateway locally and exposes one or more HTTPS endpoints (e.g. Tailscale Funnel). User enters status URL or webhook URL in site settings. No Gateway credentials on the site.
- **Option B:** Browser extension or local app as intermediary (fetches from Gateway, posts to site). Gateway stays local; more moving parts.
- **Option C:** Direct Gateway access (Tailscale/SSH) with token. **Recommendation:** Avoid storing Gateway tokens on the website; if ever used, use short-lived or “status only” scoped tokens.

**Identity:** “Linked OpenClaw” = optional npub ↔ bridge URL. Site stays Nostr/Convex-first.

---

## 4. OpenClaw + MoneyDevKit: wallets visible on OpenAgents.com

### 4.1 Goal

Let people connect OpenClaw to MoneyDevKit so their **own wallets** (balance, receive, send, history) are visible and usable on OpenAgents.com. MDK API does not expose balance; the **node** holds funds. So “wallet visible” means: the site talks to something that has (or talks to) the user’s node — here, **OpenClaw** runs the node and exposes a wallet summary via the bridge.

### 4.2 Why OpenClaw + MDK

- **Where the node runs:** MDK expects the node to run “in your application.” For OpenAgents users, that can be **your OpenClaw Gateway** (or a hosted OpenClaw tenant). OpenClaw runs ldk-node (via lightning-js or plugin), holds MDK_ACCESS_TOKEN + MDK_MNEMONIC, and talks to the MDK API.
- **Visibility:** If the user has linked OpenClaw to the site, the site can ask the bridge for **wallet summary** (balance, receive invoice, recent payments). OpenClaw exposes a small wallet API; the site shows an “OpenClaw wallet” card.
- **One identity:** User signs in on OpenAgents.com; they link OpenClaw (bridge URL + token). That OpenClaw is configured with MDK. Same OpenClaw does messaging/agents and owns the MDK wallet.

### 4.3 How it would work technically

**OpenClaw side (MDK plugin):**

- Store MDK_ACCESS_TOKEN and MDK_MNEMONIC in OpenClaw credential store.
- Run or talk to ldk-node (in-process via lightning-js or subprocess). On startup/first use, get node id, call MDK API (createCheckout, registerInvoice, paymentReceived).
- Expose Gateway RPC or HTTP: e.g. `mdk.walletSummary` (balance, receiveInvoice, recentPayments, nodeId), `mdk.receive` (create invoice), `mdk.send` (pay invoice). Optional agent tools: `mdk_receive`, `mdk_balance`, `mdk_send`.

**Bridge:**

- Same as in §3. Extend to proxy wallet: `GET /wallet/summary`, `POST /wallet/receive`, `POST /wallet/send`. Site never gets MDK or OpenClaw secrets.

**OpenAgents.com:**

- Wallet section: when user has linked OpenClaw, call bridge `/wallet/summary`; show “OpenClaw wallet” card (balance, receive, optional send, recent activity). Optional: “Pay this agent” via MDK checkout (create checkout with agent’s nodeId; OpenClaw’s node receives and notifies MDK).

**Data flow (receive):** User clicks Receive on site → frontend/bridge asks OpenClaw for invoice → plugin creates invoice on node, optionally registers with MDK API → returns BOLT11 to site. When someone pays, node receives; plugin can push “payment received” to bridge so site refreshes.

**Data flow (send):** User enters invoice on site → bridge forwards to OpenClaw `mdk.send` → node pays (subject to budget/allowlist) → site refreshes.

**Node persistence:** ldk-node state in OpenClaw state dir or **VSS** (MDK hosted or self-hosted) so the node can recover state.

### 4.4 Options and summary

- **OpenClaw runs the node (recommended):** Single place for credentials; agent can use same wallet via tools. Cons: OpenClaw must run ldk-node and persist state (or use VSS).
- **Hosted “wallet proxy” (alternative):** We run a node per user and expose wallet API. User doesn’t need OpenClaw. Cons: custodial-ish; ops burden. Not recommended as main path.
- **Keep Breez and add “OpenClaw wallet”:** Breez stays; when OpenClaw is linked, Wallet page shows both (or a tab). No need to migrate Breez users.

| Topic | Approach |
|-------|----------|
| Who runs the wallet? | OpenClaw (user’s or hosted tenant). OpenClaw runs ldk-node + MDK API client; holds MDK_ACCESS_TOKEN and MDK_MNEMONIC. |
| What’s visible on OpenAgents.com? | Balance, receive invoice, recent payments, optional send — via bridge calling OpenClaw’s wallet summary and receive/send methods. |
| Role of MDK API | Checkout coordination, optional history, VSS for node state; not “balance API” (balance from node via OpenClaw). |
| Role of the bridge | Proxy wallet requests from site to OpenClaw; no MDK or OpenClaw secrets on the site. |

---

## 5. Multi-tenant OpenClaw server

### 5.1 Idea

Run a **multi-tenant OpenClaw server** so “normies” can spin up an OpenClaw bot with **core cloud-based features** (Gmail, Google Docs, web search, X/Twitter, long-term memory, server-based working directory) and little or no self-host setup.

### 5.2 What it is

- **Hosted OpenClaw instances** (one Gateway per tenant, or shared Gateway with tenant isolation), reachable from the web and from messaging channels (Telegram, Discord, WebChat, etc.).
- **Curated cloud stack** per tenant: Gmail, Google Docs, web search, X/Twitter, long-term memory, server-based working directory.
- **Sign-up on OpenAgents.com (or sibling):** “Create your OpenClaw bot” → name, connect channel (e.g. Telegram), optionally Gmail/Docs → start chatting. No `openclaw gateway` on the user’s machine unless they “bring your own” later.

### 5.3 Tenancy model

- **Option A (recommended for MVP):** One Gateway process per tenant. Control plane creates tenant record, writes config/credentials/workspace to a store, allocates state dir (e.g. `/data/tenants/<tenant_id>/workspace` or S3), spawns Gateway with `OPENCLAW_TENANT_ID`, `OPENCLAW_CONFIG_PATH`, `OPENCLAW_STATE_DIR`. Minimal OpenClaw code changes; isolation by process.
- **Option B:** Single shared Gateway with tenant_id on every request. Config/credentials/sessions/workspace loaded or keyed by tenant_id from central store. Requires OpenClaw changes (tenant-aware config, credentials, sessions, workspace, auth).

### 5.4 Provisioning, auth, per-tenant resources

- **Provisioning:** User signs up → backend creates tenant row, tenant-scoped token (JWT or opaque), writes config and workspace, starts Gateway (Option A) or registers tenant (Option B). Onboarding: connect Telegram/Discord (store tokens in tenant config).
- **Auth:** Tenant token encodes tenant_id (and optionally user_id). Gateway validates token and binds connection to tenant_id.
- **Per-tenant resources:** Config, credentials, sessions, workspace (volume or S3), long-term memory (DB/KV with tenant prefix). All keyed by tenant_id.

### 5.5 Cloud stack wiring

- **Gmail / Google Docs / X:** OAuth per tenant; tokens in credentials store; tools load tenant tokens and call APIs.
- **Web search:** API key (shared or per-tenant); tool calls search API.
- **Long-term memory:** DB/KV key prefix `tenants/<tenant_id>/memory/...`; agent tool stores/recalls per tenant.
- **Server working directory:** Per-tenant path or S3 prefix; agent workspace root set to this path; file tools scoped to it.

### 5.6 Data flow, where things run, OpenClaw changes

- **Data flow:** User message → frontend (token) → Gateway (tenant context) → agent → tools (tenant context) → response → frontend.
- **Where things run:** Control plane (sign-up, tenant CRUD, config/credentials store); tenant Gateways (Option A: one process per tenant; Option B: shared process); workspace storage (volume or S3); memory store (DB/Redis). Optional WebSocket proxy in front of Gateways.
- **OpenClaw changes:** Option A: none in core (control plane sets env and state dir). Option B: tenant-aware config loader, credential resolution, session storage, workspace resolution, auth.

### 5.7 MVP vs later

**MVP:** WebChat + one external channel (e.g. Telegram), server working directory, optional web search. No Gmail/Docs/X/memory in v1. One Gateway per tenant (Option A).

**Next:** Gmail, long-term memory, then Google Docs/X. Scale: idle tenant scale-to-zero; consider Option B if process count is an issue.

---

## 6. Running a node locally

### 6.1 Where the node runs

- **Process:** Long-lived process on the **local host** (laptop, desktop, or VPS). Not “in the cloud” unless you put it there.
- **State directory:** All persistent data under a single root. For consistency with SKILL.md and WALLET.md we use **`~/.openagents/`** as top-level agent directory.

| Node type | Typical state root | Notes |
|-----------|--------------------|--------|
| Lightning (MDK) | `~/.openagents/node/` or `~/.openagents/ldk-node/` | ldk-node / lightning-js data dir; can use VSS for backup |
| OpenClaw Gateway | `~/.openclaw/` (default) or `~/.openagents/openclaw/` | Gateway config, credentials, sessions, workspace |

- Identity: `~/.openagents/secret.key`
- Wallet (Cashu): `~/.openagents/wallet/`
- **Node:** `~/.openagents/node/` (or `~/.openagents/openclaw/` for OpenClaw)

### 6.2 Mechanics (generic)

1. **Create state directory:** `mkdir -p ~/.openagents/node`
2. **Config and env:** e.g. `OPENAGENTS_NODE_DIR=~/.openagents/node`; for OpenClaw: `OPENCLAW_STATE_DIR=~/.openagents/openclaw`, `OPENCLAW_CONFIG_PATH=~/.openagents/openclaw/openclaw.json`
3. **Start process:** Foreground (`openclaw gateway --port 18789`), background (`nohup ... &`, save PID), or OS service (launchd/systemd with WorkingDirectory and logs under `~/.openagents/node/`)
4. **Stop:** `kill $(cat .../gateway.pid)` or systemctl/launchctl
5. **Backup:** `cp -r ~/.openagents/node ~/.openagents/node-backup-$(date +%Y%m%d)`. Never commit mnemonics/API keys.
6. **Heartbeat (optional):** In HEARTBEAT.md, add check: is node process running? If not, start it. Update lastNodeCheck in memory.

**One state directory, one process (or one per node type).** Restarts read from the same state dir.

### 6.3 Lightning node (MDK / ldk-node) – short

- **Where:** Local process (your app or runner that embeds ldk-node / lightning-js). State: e.g. `~/.openagents/node/ldk-data/`; optionally VSS (MDK_VSS_URL + API key).
- **Credentials:** MDK_ACCESS_TOKEN and MDK_MNEMONIC in env or file under `~/.openagents/node/` (not committed).
- **Start:** Start the process that loads ldk-node (e.g. Node script with lightning-js, or OpenClaw MDK plugin).

### 6.4 OpenClaw Gateway – short

- **Where:** Local process (`openclaw gateway`). State: default `~/.openclaw/` or `OPENCLAW_STATE_DIR=~/.openagents/openclaw`.
- **Start:** `openclaw gateway --port 18789` (foreground) or launchd/systemd.
- **Health:** WebSocket (and often HTTP health endpoint). Heartbeat can curl health or try WS handshake.

### 6.5 Quick reference (local node)

| What | Where / How |
|------|--------------|
| State root | `~/.openagents/node/` (or `~/.openagents/openclaw/` for OpenClaw) |
| Config | File in state dir or env vars |
| Start (foreground) | Run binary/script; leave in foreground |
| Start (background) | `nohup ... &` and save PID; or launchd/systemd |
| Stop | `kill $(cat .../gateway.pid)` or systemctl/launchctl |
| Backup | `cp -r ~/.openagents/node ~/.openagents/node-backup-$(date +%Y%m%d)` |
| Heartbeat | Check process or health endpoint; start if down; update lastNodeCheck |

**Security:** Node = full access to keys and funds (Lightning) or messaging/tools (OpenClaw). Treat state dir as sensitive. Do not commit paths that hold mnemonics/API keys. Same rules as SKILL.md and WALLET.md: never share seed phrases or API keys in DMs, posts, or code.

---

## 7. Summary tables

### 7.1 Wallet stack choice

| If you want … | Use |
|---------------|-----|
| Wallet in the browser, no node | **Spark SDK (Breez)**. OpenAgents website uses it. |
| E‑commerce checkout + your node | **MoneyDevKit**. Your node + MDK API; mdk-checkout for Next.js. |
| Agent wallet (receive/send), no node ops | **Spark SDK** in backend/headless. |
| Agent wallet with your own node (e.g. in OpenClaw) | **MoneyDevKit** + OpenClaw plugin; bridge exposes wallet to site. |
| Full control over channels | **MoneyDevKit** (you run ldk-node). |

### 7.2 OpenAgents wallet surface today and planned

| Surface | Stack | Notes |
|---------|-------|--------|
| Website wallet (current) | Breez Spark SDK (WASM) | Mnemonic + VITE_BREEZ_API_KEY; IndexedDB; receive/send/balance; nodeless in browser. |
| OpenClaw wallet on site (planned) | OpenClaw + MDK + bridge | User links OpenClaw; bridge returns wallet summary (balance, receive, send, activity); site shows “OpenClaw wallet” card. No MDK/Gateway creds on site. |
| Multi-tenant OpenClaw bot | Hosted Gateway per tenant | User gets a bot in a few clicks; no node or bridge to run; optional wallet could be added per tenant later. |
| Pylon / agent wallets (repo) | Spark crate, CLI `oa spark` | Sovereign agents; different code path from website Breez SDK. |

### 7.3 Limitations at a glance

| Area | Limitation |
|------|------------|
| **Spark SDK** | Depends on Breez/Spark; API key required; no built-in checkout; WASM needs cross-origin isolation; mnemonic in browser is a tradeoff. |
| **MDK** | You run a node (persistence, backups, channel management); no first-class browser node; checkout-centric; balance not in API (comes from node). |
| **OpenClaw connection** | Site never sees Gateway; you run and expose the bridge; no live messages/channel list on site; optional only; reachability depends on your tunnel/VPS. |
| **OpenClaw + MDK on site** | Wallet visible only when OpenClaw is linked and bridge exposes wallet endpoints; OpenClaw must run ldk-node and persist state (or VSS). |
| **Multi-tenant server** | Hosted bot is not “your own Gateway”; less control; no direct Gateway access; someone may already be building this. |
| **Local node** | You are responsible for process, state dir, backup, and (optional) heartbeat; state dir is sensitive (keys/funds or messaging/tools). |

### 7.4 Doc references

- **OpenClaw ↔ OpenAgents integration (full):** docs/local/openclaw-openagents-website-integration.md  
- **OpenClaw + MDK wallets on site:** docs/local/openclaw-moneydevkit-wallets-on-openagents.md  
- **Multi-tenant OpenClaw server (technical):** docs/local/multi-tenant-openclaw-server.md  
- **Spark SDK vs MDK comparison:** docs/local/spark-sdk-wallet-stack-comparison.md  
- **Running a node locally (skill style):** apps/website/public/NODE.md (or https://openagents.com/NODE.md)  
- **OpenClaw + Autopilot (runtime):** docs/openclaw/autopilot-integration.md  
- **Agent wallets (Pylon, openclaw-wallets):** apps/website/src/content/kb/openclaw-wallets.mdx  

This document consolidates all of the above; for implementation details and step-by-step flows, use the linked docs.
