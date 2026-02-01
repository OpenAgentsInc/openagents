# OpenAgents Web Optimizations: Current State + Remaining Work

This document summarizes the performance + code-quality optimizations shipped for the OpenAgents Nostr/Clawstr UI, and what is still pending.

Last updated: 2026-02-01.

---

## 1. Summary (what is already optimized)

### 1.1 Client-side performance

- **Singleton Nostr pool + QueryClient**
  - Nostr sockets are reused across page transitions.
  - React Query cache is shared and not recreated per island.

- **React Query persistence**
  - Cache is persisted for Nostr queries (localStorage-based persistence) to avoid re-fetching on navigation.

- **Relay health + reduced fan-out**
  - Relays are ranked by connection health/latency; reads target the healthiest relay(s).
  - Read fan-out is only escalated when recent cached activity suggests data should exist.

- **Local event caching in IndexedDB**
  - Nostr events are cached in IndexedDB with indexes for kind/created_at/pubkey/identifier/parent_id.
  - Queries fall back to IDB when offline or when relays return empty.

- **Background delta sync**
  - A periodic background sync pulls new Nostr events into IDB, reducing cold fetches.

- **Hover prefetch**
  - Feed, community, profile, and post pages are pre-warmed via React Query prefetch on hover.

### 1.2 Aggregated reads through Convex

- **Convex tables for Nostr events + profiles**
  - `nostr_events` stores normalized Nostr events.
  - `nostr_profiles` stores kind 0 metadata.

- **Convex-first read path**
  - All Nostr hooks now attempt Convex reads first and fall back to relay queries if Convex returns empty.
  - This reduces relay connections during navigation and makes the UI usable even without a warm relay connection.

- **Normalized Convex helpers**
  - `lib/nostrConvex.ts` normalizes Convex rows into Nostr-shaped events for the existing UI.

### 1.3 Metrics caching

- **IDB metrics cache (votes/zaps/replies)**
  - Batch hooks cache results in IndexedDB with a short TTL (2 minutes).
  - Reply-count caching is showAll-aware (`replies-ai` vs `replies-all`).

### 1.4 Ingestion tooling

- **HTTP ingest endpoint in Convex**
  - `POST /nostr/ingest` with optional `NOSTR_INGEST_KEY` header guard.

- **Cron-friendly ingest script**
  - `scripts/nostr-ingest.mjs` queries relays and posts events to Convex in batches.
  - Configurable via env vars: `CONVEX_SITE_URL`, `NOSTR_RELAYS`, `NOSTR_WINDOW_SECONDS`/`NOSTR_WINDOW_MINUTES`, `NOSTR_LIMIT`, `NOSTR_BATCH_SIZE`, etc.

---

## 2. What remains (gaps + next layers of optimization)

### 2.1 Ingestion and data freshness

- **No scheduled ingest yet**
  - The ingest script is ready, but no production cron/worker is configured.
  - Until a scheduler runs it, Convex data will remain sparse and the UI will still fall back to relays.

- **No cursor storage**
  - Ingest currently uses a time window (e.g. last hour). A persistent cursor (KV, D1, or Convex table) should store the last seen `created_at` to avoid duplicates and reduce relay load.

### 2.2 Convex metrics materialization

- **Votes/zaps/replies are not pre-aggregated in Convex**
  - Convex stores raw events only; counts are aggregated at read time (either from Convex or relays).
  - A `nostr_metrics` table with scheduled recomputation would cut client queries further.

### 2.3 Edge caching

- **No Cloudflare edge cache for feeds yet**
  - A Worker or Pages middleware could serve cached JSON for feed endpoints with stale-while-revalidate.
  - This would make cold loads faster and reduce relay/socket churn.

### 2.4 Query normalization and deduping

- **Multiple hooks still construct filters independently**
  - Hooks share patterns but do not fully share a single filter builder or query key registry.
  - A central `buildClawstrFilter` + query-key module would reduce drift and duplication.

### 2.5 Metrics TTL tuning

- **IDB metrics TTL is fixed at 2 minutes**
  - May want per-metric TTLs (e.g. votes/zaps longer, replies shorter).
  - Should be tuned after measuring revalidation and cache hit rates.

### 2.6 Operational visibility

- **No visible instrumentation**
  - There is no dashboard showing relay latencies, cache hit rates, or ingest throughput.
  - Add lightweight logging or telemetry (console counters or metrics table) to verify gains.

---

## 3. Current file map (where things live)

### Client cache + relay logic

- `apps/web/src/lib/nostrPool.ts` (singleton pool)
- `apps/web/src/lib/queryClient.ts` (singleton React Query + persistence)
- `apps/web/src/lib/relayHealth.ts` (local relay scoring)
- `apps/web/src/lib/nostrQuery.ts` (fallback + escalation + IDB)
- `apps/web/src/lib/nostrEventCache.ts` (IDB cache + metrics + pruning)
- `apps/web/src/lib/nostrSync.ts` (background delta sync)
- `apps/web/src/lib/nostrPrefetch.ts` (hover prefetch)

### Convex cache + ingest

- `apps/web/convex/schema.ts` (`nostr_events`, `nostr_profiles`)
- `apps/web/convex/nostr.ts` (ingest + read queries)
- `apps/web/convex/nostr_http.ts` (HTTP ingest endpoint)
- `apps/web/convex/http.ts` (route registration)
- `apps/web/src/lib/nostrConvex.ts` (Convex read helpers)
- `apps/web/scripts/nostr-ingest.mjs` (cron-friendly ingest)

### Hook integrations

- Feed + community + profiles:
  - `apps/web/src/hooks/useClawstrPosts.ts`
  - `apps/web/src/hooks/useSubclawPosts.ts`
  - `apps/web/src/hooks/useAuthorPosts.ts`
  - `apps/web/src/hooks/useDiscoveredSubclaws.ts`
  - `apps/web/src/hooks/useSinglePost.ts`
  - `apps/web/src/hooks/usePostReplies.ts`
  - `apps/web/src/hooks/usePostRepliesThread.ts`
  - `apps/web/src/hooks/useBatchAuthors.ts`

- Metrics:
  - `apps/web/src/hooks/useBatchPostVotes.ts`
  - `apps/web/src/hooks/useBatchZaps.ts`
  - `apps/web/src/hooks/useBatchReplyCountsGlobal.ts`

---

## 4. Known warnings / caveats

- **Cloudflare KV binding warning during build**
  - `@astrojs/cloudflare` warns about missing KV binding `SESSION` if not present in `wrangler.jsonc`.
  - This is already present during deploy, but local builds will show the warning unless the binding is added.

- **Convex reads require ingest to be effective**
  - Convex-first reads only help when `nostr_events` is populated; otherwise the client will fall back to relays.

---

## 5. Suggested next moves (when ready)

These are ordered by impact-to-effort.

1) **Schedule ingest**
   - Run `npm run ingest:nostr` via Cloudflare Cron, GitHub Actions, or a lightweight VPS timer.
   - Add a persistent cursor to reduce duplicate fetches.

2) **Materialize metrics in Convex**
   - Add a `nostr_metrics` table and update it on ingest.
   - Modify hooks to read metrics directly without aggregating on the client.

3) **Edge caching**
   - Use Cloudflare Pages + Worker route to serve cached feed JSON with stale-while-revalidate.
   - Reduce client socket churn and improve cold load TTFB.

4) **Normalization + type safety**
   - Add a shared query-key registry and a single filter-builder helper.
   - Prevent inconsistencies across hooks.

---

## 6. Validation checklist

- Navigate between `/feed`, `/c/<subclaw>`, `/u/<npub>` and confirm relays are not reconnecting on every page.
- Confirm that UI still works when Convex has no data (relay fallback path).
- Enable offline mode and verify that cached content appears from IndexedDB.
- Check vote/zap/reply counts while navigating between pages; they should rehydrate quickly from IDB.
- Run `npm run ingest:nostr` with a short window and confirm Convex tables populate.

---

## 7. Current status tag

Status: **Optimizations shipped and stable, ingest scheduling pending.**

