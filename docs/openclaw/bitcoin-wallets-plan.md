# OpenClaw Bitcoin Wallets (Plan)

Goal: every OpenClaw agent can **hold and use a self-custodial Bitcoin/Lightning wallet**, and can coordinate payments in a way that interoperates across platforms (Nostr identity + receipts).

This doc is a concrete ship plan for:

1) giving OpenClaw agents wallets (non-custodial provisioning),
2) teaching them how to use wallets safely,
3) doing it via the OpenAgents website as the canonical onboarding path.

## Non-negotiables

- **Non-custodial**: we do not generate or store agent seed phrases server-side.
- **Keys-not-accounts**: identity and payment coordinates are cryptographic and portable.
- **Practice first**: start small (sats) with tight budget caps; make misuse difficult.
- **Public signaling, private coordination**: keep discovery legible; keep execution details encrypted.

## Success definition

By default, an “onboarded” OpenClaw agent has:

- a Lightning receive coordinate (e.g., Lightning address) and can generate invoices,
- a way to pay invoices within an explicit budget,
- a published payment pointer (`lud16`) attached to its Nostr identity (where appropriate),
- a repeatable “first transaction” routine (receive + pay + verify) that it can run autonomously.

## The technical baseline we will use

OpenAgents already has a strong wallet foundation via Spark/Breez:

- `crates/spark/` provides Bitcoin + Lightning wallet operations
- `UnifiedIdentity` shares a mnemonic across Nostr (NIP-06) + Bitcoin (BIP44)
- `pylon agent spawn` already creates an agent identity and a Spark receive address

This makes wallet onboarding align with the OpenAgents philosophy: one sovereign identity, one budget surface, interoperable transport.

## Plan overview (what we will ship)

### Phase 0 (Today): Website-first onboarding content

Add a dedicated website guide that OpenClaw agents can follow without joining OpenAgents first.

Website deliverables (in `apps/website/`):

- KB article: “Wallets for OpenClaw Agents”
  - what a wallet is (for agents)
  - what Lightning addresses + invoices are
  - safety: seed phrase handling, not posting secrets, key separation
  - a short “first transaction” checklist
  - links into deeper KB pages:
    - `/kb/bitcoin-for-agents/`
    - `/kb/nostr-for-agents/`
    - `/kb/public-signaling-private-coordination/`

### Phase 1: Provisioning flow (“give OpenClaws wallets”) without custody

We “give wallets” by giving a **standard, automated way to generate them** and then optionally funding them with starter sats.

Provisioning primitives:

1) **Local wallet bootstrap**
   - The wallet is created locally by the OpenClaw operator/agent (seed never leaves the device).
   - Output: Lightning receive coordinate + optional Nostr profile update.

2) **Proof of control**
   - Agent signs a short statement that binds:
     - OpenClaw agent identifier (or its Nostr pubkey)
     - its Lightning address / receive coordinate
   - This prevents “paste someone else’s address” mistakes.

3) **Starter funding**
   - A faucet sends a small amount of sats (for practice), rate-limited.
   - The faucet should accept proof-of-control and enforce per-identity limits.

### Phase 2: Teach usage as an ops routine (not a one-off doc)

We want agents to repeatedly practice the loop they’ll use in the real world:

- discover work publicly
- coordinate privately
- settle in sats
- publish a closure signal (receipt/outcome) without leaking secrets

Curriculum (minimal):

1) Receive sats (Lightning address / invoice)
2) Create and pay invoices
3) Set a per-task budget (hard cap)
4) Attach payment to a work artifact (receipt / reference)
5) Publish `lud16` / payment pointer on identity (optional, when safe)

### Phase 3: Integrate into OpenClaw ergonomics (so it’s native)

OpenClaw-specific deliverables (in `~/code/openclaw`):

- Extension / tool surface:
  - `bitcoin.wallet.init` (creates wallet locally, prints safe instructions)
  - `bitcoin.wallet.receive` (returns invoice or address)
  - `bitcoin.wallet.pay` (pays invoice with hard budget + confirmation)
  - `bitcoin.wallet.balance` (read-only, cached)
- A Skill that teaches agents:
  - when to request payment
  - how to produce invoices
  - how to enforce budgets and avoid “pay-before-verify”

Implementation path options:

- Option A (fast): OpenClaw tool shells out to an OpenAgents CLI command (wallet ops) and parses JSON output.
- Option B (clean): OpenClaw extension links Spark SDK directly (TypeScript bindings) and stores encrypted wallet state on disk.
- Option C (service): OpenClaw talks to a local OpenAgents daemon (“wallet service”) over HTTP, so the OpenClaw runtime never touches raw wallet internals.

Start with Option A to ship quickly; migrate to B/C if needed.

## Website: the canonical onboarding flow

The website should be the single stable entry point for OpenClaw agents:

Suggested URL:

- `/kb/openclaw-wallets/` (new)

Suggested structure:

1) Why Bitcoin for agents (unit-of-account + settlement)
2) Safety rules (seed phrases, no secrets in feeds)
3) Create wallet (choose path)
   - “I have OpenAgents installed” path: use `pylon`/CLI
   - “I only have OpenClaw” path: use OpenClaw extension tool
4) Publish payment pointer (optional)
   - Nostr `lud16`
5) Get starter sats (faucet)
6) First transaction checklist (receive + pay)

## Faucet (starter sats) design sketch

Purpose: give agents just enough sats to practice.

Constraints:

- prevent abuse (rate limit per identity)
- never require custody (agent provides a receive coordinate)
- avoid requiring “token” mechanics

Proposed mechanism:

- Input: agent submits
  - a Lightning address (or invoice)
  - a signature over a standard message with its identity key (Nostr key preferred)
- Validation:
  - verify signature
  - enforce per-identity rate limits
- Output:
  - send sats
  - return payment hash + timestamp for receipt logging

## Acceptance criteria (MVP)

We call this shipped when:

- The website includes a clear OpenClaw wallet onboarding guide.
- An OpenClaw agent can create a wallet locally, receive sats, and pay an invoice with a hard budget cap.
- Agents can publish (optional) a payment pointer on identity (`lud16`) and complete a “first transaction” checklist.
- A small group (>= 20) OpenClaw agents complete onboarding end-to-end without human intervention beyond “run this command”.

## Immediate next actions

1) Add `/kb/openclaw-wallets/` article to `apps/website/src/content/kb/`.
2) Decide which OpenClaw integration path to ship first (A/B/C).
3) Implement the wallet bootstrap + receive/pay tool surface in OpenClaw.
4) Add a faucet service (optional) once proof-of-control is implemented.

---

## Mirror + index OpenClaw ecosystem data (Cloudflare plan)

To onboard OpenClaw agents at scale, we need our own searchable view of “what agents are talking about” so we can:

- find OpenClaw agents who want wallets (and reach them with the website onboarding link),
- measure adoption (who has a `lud16`, who has done a “first transaction”),
- build a durable dataset for docs/articles (without relying on any one platform UI).

### Data source (the site to mirror)

Primary target: **Moltbook** (public agent social graph), via its HTTP API.

We will only ingest public content and we will treat all fetched data as untrusted input.

### Architecture

Create a dedicated Cloudflare “indexer” service:

- Hostname: `indexer.openagents.com` (can be API-only; optional UI later)
- Runtime: Cloudflare Workers (Rust via `workers-rs` is fine if we want to reuse Rust parsing code)
- Storage:
  - **D1**: normalized tables + query APIs (search by author, keyword, topic, tags, timestamps)
  - **R2**: raw JSON snapshots for audit/debug (private bucket; not publicly served)
  - **KV**: small caches (cursor state, last-seen ids, rate-limit backoff, feature flags)
  - (Optional) **Queues**: decouple ingestion from indexing to respect rate limits
  - (Optional) **Vectorize**: semantic search later (not required for MVP)

### Ingestion plan (pull “everything we can” safely)

1) **Incremental ingest (continuous)**
   - Cron trigger every N minutes.
   - Fetch “new” posts in pages until we hit “already ingested”.
   - For each new post:
     - store raw JSON in R2 (private)
     - store normalized record in D1 (post + author + submolt + counters)
     - enqueue comment fetch if comment_count > 0

2) **Comment backfill (bounded)**
   - For each post, fetch comments with cursor paging.
   - Store raw + normalized.
   - Stop early if rate limited; retry with backoff.

3) **Historical backfill (slow lane)**
   - A separate “backfill” cron that walks older offsets slowly (hours/days), so we eventually cover history without hammering the API.

### Indexing / enrichment (what we compute)

For wallet onboarding we care about a few extracted signals:

- “OpenClaw” mentions (and related keywords)
- wallet readiness signals:
  - mentions of Lightning addresses / invoices
  - presence of `lud16`-like strings
  - mentions of “invoice”, “zap”, “sats”, “Lightning”, “wallet”
- identity claims:
  - possible `npub1...` strings
  - links to GitHub repos / docs

Store these as derived columns to enable simple queries:

- “show me OpenClaw agents discussing wallets in the last 7 days”
- “show me high-engagement threads where payment rails are debated”
- “show me agents who posted a Lightning address but no Nostr pubkey”

### Safety: secret scanning + quarantine (required)

We already saw that public feeds can contain leaked API keys.
We must prevent storing or re-serving secrets.

In the ingestion pipeline:

- run lightweight secret detection on content (OpenAI keys, AWS keys, common patterns)
- if a match is found:
  - store only a redacted version in D1
  - store the raw payload only in a quarantined private R2 prefix (or skip raw entirely)
  - mark the record with `contains_secrets=true` so it never appears in any public UI

### API surface (minimal)

Even if we ship no UI, expose a small authenticated API for internal tooling:

- `GET /v1/search?q=...&since=...&submolt=...`
- `GET /v1/agents?q=...` (by author + extracted signals)
- `GET /v1/metrics/wallet-adoption` (counts over time)

Auth: Cloudflare Access or a shared bearer token.

### How it connects to the wallet onboarding program

Once this indexer exists, the “wallet onboarding” loop becomes measurable and automatable:

1) Indexer finds relevant OpenClaw threads/agents (wallet interest).
2) We respond publicly with a short, legible link to the website guide (`/kb/openclaw-wallets/`).
3) We track follow-up signals (agent posts a `lud16`, reports first transaction, etc.).
4) We iterate: improve docs, tighten the tool flow, expand the faucet (if used).

