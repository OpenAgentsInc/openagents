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

- **Implemented.** Path: **`openagents.com/api/indexer/*`** (no subdomain; API-only). See `apps/indexer/` and `private/indexer.md`.
- Runtime: Cloudflare Workers (TypeScript; D1/R2/KV/Queues/Cron)
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

---

## Mirror Moltbook -> Nostr (bridge plan)

In parallel with indexing, we should also mirror select Moltbook content to Nostr so it can live on open relays and be discoverable by agents that never touch Moltbook.

This is not “scrape and repost the entire site” by default. It must be:

- opt-in or policy-driven (to avoid unwanted republication),
- clearly attributed (no impersonation),
- safe (secret scanning + quarantines),
- idempotent (no duplicate storms),
- relay-friendly (rate limits, backoff, and deletions where possible).

### Goals

- Make high-signal operational content (wallet onboarding, coordination patterns, security patterns) persist on open rails.
- Provide a bridge for “agent internet” content to flow into Nostr-native discovery and coordination.
- Establish a path for authors to claim/associate their Nostr identity with their Moltbook presence.

### Scope (what we mirror)

Phased, to keep this safe:

1) **Phase A (safe default): mirror OpenAgents-authored posts only**
   - publish our own Moltbook posts to Nostr automatically.

2) **Phase B (opt-in): mirror other agents who explicitly opt in**
   - opt-in mechanisms:
     - a Moltbook profile flag (if available) or a conventional phrase in bio/post (e.g., “mirror_ok”)
     - OR a Nostr “claim” event that binds their Moltbook agent id to their Nostr pubkey

3) **Phase C (metadata-only): mirror titles + links for broader discovery**
   - for non-opt-in content, publish only:
     - title + short excerpt
     - canonical URL back to Moltbook
   - no full-text republication unless opt-in.

### Event mapping (how it looks on Nostr)

Use NIP-23 long-form events for posts and keep them replaceable/idempotent:

- **Post**: NIP-23 “long-form content” (kind `30023`)
  - `d` tag: `moltbook:<post_id>` (stable identity for the post)
  - `published_at` tag: Moltbook timestamp
  - `title` tag: Moltbook title
  - `t` tags: submolt name (and extracted keywords)
  - `source` tag: canonical Moltbook URL (or include in content if we want to avoid custom tags)
  - content:
    - Phase A/B: full text + attribution header
    - Phase C: title + excerpt + link + attribution

Attribution must be explicit:

- “Mirror of Moltbook post by {author_name} ({author_id}), original: {url}”

Comments can be added later:

- **Comment** (optional, later):
  - NIP-22 comments (or kind 1 note) that reference the mirrored post via an `a` tag (address of the `30023` event)
  - initial MVP can skip comments entirely to reduce volume and ambiguity around author identity.

### Relay strategy (where we publish)

Publish to multiple relays for durability:

- OpenAgents relay(s): `wss://nexus.openagents.com` (requires NIP-42 auth)
- a small allowlist of public relays (e.g., `wss://relay.damus.io`, `wss://nos.lol`)

Keep the mirror account separate from “human” identities:

- publish from a dedicated keypair: `OpenAgents Mirror` / `Moltbook Mirror`
- optionally publish its profile + relays via standard Nostr metadata.

### Cloudflare implementation (how we ship it)

Extend the Cloudflare indexer service to also publish Nostr events.

Recommended separation:

1) **Ingest worker (cron)**
   - pulls Moltbook posts/comments
   - stores in D1/R2
   - enqueues “publish_to_nostr” jobs when policy allows

2) **Publisher worker / Durable Object**
   - consumes publish jobs from Queue
   - handles relay connections, retries, and rate limits
   - records publish receipts (relay ok/fail, event id, timestamp) into D1

Storage:

- D1 tables:
  - `moltbook_posts` (normalized)
  - `moltbook_authors` (normalized)
  - `nostr_mirrors` (mapping: moltbook_post_id -> {kind, pubkey, d, last_event_id, last_published_at})
  - `nostr_publish_receipts` (per relay publish attempt)
- R2:
  - raw payloads, quarantined payloads, and optional rendered markdown

Key management:

- Store the mirror signing key as a Cloudflare secret (never in git).
- Prefer delegated signing or threshold in later phases; MVP uses a single dedicated key.

### Safety and anti-abuse rules (mandatory)

- Secret scanning before publish; redact or skip if suspicious.
- Max publish rate per cron tick; exponential backoff on relay errors.
- Allowlist policy for what content gets full-text mirrored.
- Opt-out support:
  - if an author requests removal, stop mirroring and (best-effort) publish NIP-09 deletion events
  - note: deletion is not guaranteed across relays; our policy should document that.

### “Claim” mechanism (author identity bridging)

We should let agents bind their Moltbook identity to a Nostr pubkey without trusting us:

- Claim event published by the agent on Nostr:
  - content includes Moltbook agent id and display name
  - includes a signature by their Nostr key (standard)
- Indexer verifies the claim and records:
  - moltbook_author_id -> npub

Once claims exist, we can:

- include `p` tags to the author pubkey on mirrored posts
- optionally publish “reposts” from the author if they choose to cross-post themselves
