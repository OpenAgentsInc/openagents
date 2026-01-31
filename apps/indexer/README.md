# Moltbook Indexer

Systematic long-running indexer for Moltbook: raw snapshots in **R2**, normalized state in **D1**, cursors/backoff in **KV**, comment fetch via **Queues**, triggered by **Cron**. Fetches posts from Moltbook directly (`https://www.moltbook.com/api/v1`); comments are retrieved via **GET /posts/{id}** (single-post response includes a top-level `comments` array; the dedicated comments endpoint returns 405).

**Served at:** `https://openagents.com/api/indexer/*` (no subdomain). R2 bucket name: `moltbook`.

## One-time setup

### 1. Create Cloudflare resources

From `apps/indexer`:

```bash
# D1
npx wrangler d1 create openagents-moltbook-index
# → copy database_id into wrangler.toml [[d1_databases]].database_id

# R2
npx wrangler r2 bucket create openagents-moltbook-raw

# KV
npx wrangler kv namespace create MOLTBOOK_INDEXER_STATE
# → copy id into wrangler.toml [[kv_namespaces]].id

# Queue
npx wrangler queues create moltbook-index-jobs
```

Edit `wrangler.toml`: set `database_id` and KV `id` (replace `REPLACE_ME`).

### 2. Apply D1 migrations

```bash
npx wrangler d1 migrations apply openagents-moltbook-index --remote
```

### 3. Moltbook API key (required for comments)

Posts can be fetched without auth. **Comment ingestion requires a Moltbook API key** (single-post and comment endpoints require `Authorization: Bearer …`):

```bash
npx wrangler secret put MOLTBOOK_API_KEY
```

You can pipe from local credentials: `python3 -c "import json,os; print(json.load(open(os.path.expanduser('~/.config/moltbook/credentials.json')))['api_key'], end='')" | npx wrangler secret put MOLTBOOK_API_KEY`. Then run `POST /api/indexer/ingest/backfill-comments` to fill comments.

Optional: protect `/v1/search` and `/v1/metrics/wallet-adoption` with a bearer token:

```bash
npx wrangler secret put INDEXER_AUTH_HEADER
```

## Develop

```bash
npm install
npm run dev
```

Local: `http://localhost:8787` (path will be `/`; in prod use `/api/indexer/...`).

## Deploy

```bash
npm run deploy
```

Cron runs every 5 minutes. Indexer will:

1. Fetch `posts?sort=new&limit=25` from Moltbook.
2. For each new post: write raw JSON to R2, upsert D1 (author/submolt from nested objects), enqueue comment fetch.
3. Queue consumer fetches **GET /posts/{id}** (single post; response includes `comments`), stores raw + normalized, derives signals.
4. On 429: set KV `backoff_until`, skip work until expiry.

## Endpoints

| Path | Auth | Description |
|------|------|--------------|
| `GET /api/indexer/health` | no | Health check |
| `POST /api/indexer/ingest` | no | Run incremental ingest (posts + enqueue comment jobs) |
| `POST /api/indexer/ingest/backfill-authors` | no | Backfill author_name, author_id, submolt from R2 raw post JSON |
| `POST /api/indexer/ingest/backfill-comments` | no | Fetch comments for up to 20 posts (sync; needs MOLTBOOK_API_KEY) |
| `GET /api/indexer/v1/search?q=...` | optional bearer | Search posts/comments |
| `GET /api/indexer/v1/metrics/wallet-adoption?days=30` | optional bearer | Wallet/adoption metrics from derived_signals |

## Operational rules

- **Rate limits:** 100 req/min upstream; on 429 we respect `retry_after_minutes` and back off in KV.
- **Idempotency:** D1 primary keys; R2 keys by id+date (overwrite ok).
- **Secrets:** Content is scanned before D1; matches go to R2 `quarantine/` and D1 gets redacted text.
- **No subdomain:** Everything lives under `openagents.com/api/indexer/*`.
