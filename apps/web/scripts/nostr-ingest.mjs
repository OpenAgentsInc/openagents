import { SimplePool } from "nostr-tools";
import WebSocket from "ws";

const DEFAULT_RELAYS = [
  "wss://relay.damus.io",
  "wss://nos.lol",
  "wss://relay.nostr.band",
];

const env = process.env;
const relays = (env.NOSTR_RELAYS ?? "")
  .split(",")
  .map((relay) => relay.trim())
  .filter(Boolean);

const relayUrls = relays.length > 0 ? relays : DEFAULT_RELAYS;
const now = Math.floor(Date.now() / 1000);
const sinceOverride = Number.parseInt(env.NOSTR_SINCE ?? "", 10);
const windowSeconds = Number.parseInt(env.NOSTR_WINDOW_SECONDS ?? "", 10);
const windowMinutes = Number.parseInt(env.NOSTR_WINDOW_MINUTES ?? "", 10);
const defaultWindow = 60 * 60;
const window = Number.isFinite(windowSeconds)
  ? windowSeconds
  : Number.isFinite(windowMinutes)
    ? windowMinutes * 60
    : defaultWindow;
const since = Number.isFinite(sinceOverride) ? sinceOverride : Math.max(0, now - window);
const limit = Number.parseInt(env.NOSTR_LIMIT ?? "", 10);
const defaultLimit = Number.isFinite(limit) && limit > 0 ? limit : 500;
const voteLimit = Number.parseInt(env.NOSTR_VOTE_LIMIT ?? "", 10);
const zapLimit = Number.parseInt(env.NOSTR_ZAP_LIMIT ?? "", 10);
const batchSize = Number.parseInt(env.NOSTR_BATCH_SIZE ?? "", 10);
const effectiveBatchSize = Number.isFinite(batchSize) && batchSize > 0 ? batchSize : 250;

const convexSiteUrl = env.CONVEX_SITE_URL ?? env.CONVEX_INGEST_URL ?? "";
const ingestUrl = convexSiteUrl
  ? `${convexSiteUrl.replace(/\/$/, "")}/nostr/ingest`
  : "";
const ingestKey = env.NOSTR_INGEST_KEY ?? "";

if (!ingestUrl) {
  console.error("Missing CONVEX_SITE_URL or CONVEX_INGEST_URL for ingest target.");
  process.exit(1);
}

if (!globalThis.WebSocket) {
  globalThis.WebSocket = WebSocket;
}

function chunk(array, size) {
  const result = [];
  for (let i = 0; i < array.length; i += size) {
    result.push(array.slice(i, i + size));
  }
  return result;
}

async function postBatch(events, relay) {
  const response = await fetch(ingestUrl, {
    method: "POST",
    headers: {
      "content-type": "application/json",
      ...(ingestKey ? { "x-oa-ingest-key": ingestKey } : {}),
    },
    body: JSON.stringify({ events, relay }),
  });

  if (!response.ok) {
    const text = await response.text();
    throw new Error(`Ingest failed: ${response.status} ${text}`);
  }
  return response.json();
}

async function main() {
  const pool = new SimplePool();
  try {
    const filters = [
      {
        kinds: [1111],
        "#K": ["web"],
        since,
        limit: defaultLimit,
      },
      {
        kinds: [0],
        since,
        limit: defaultLimit,
      },
      {
        kinds: [7],
        since,
        limit: Number.isFinite(voteLimit) && voteLimit > 0 ? voteLimit : 2000,
      },
      {
        kinds: [9735],
        since,
        limit: Number.isFinite(zapLimit) && zapLimit > 0 ? zapLimit : 2000,
      },
    ];

    console.log(`Relays: ${relayUrls.join(", ")}`);
    console.log(`Since: ${since} (${new Date(since * 1000).toISOString()})`);
    console.log(`Limit: ${defaultLimit}, Batch: ${effectiveBatchSize}`);

    const events = await pool.list(relayUrls, filters);
    const deduped = new Map(events.map((event) => [event.id, event]));
    const uniqueEvents = [...deduped.values()];

    console.log(`Fetched ${events.length} events (${uniqueEvents.length} unique).`);

    const batches = chunk(uniqueEvents, effectiveBatchSize);
    let inserted = 0;
    let skipped = 0;

    for (const batch of batches) {
      const result = await postBatch(batch, "cron");
      inserted += result.inserted ?? 0;
      skipped += result.skipped ?? 0;
      console.log(`Batch: inserted=${result.inserted ?? 0}, skipped=${result.skipped ?? 0}`);
    }

    console.log(`Done. inserted=${inserted}, skipped=${skipped}`);
  } finally {
    pool.close(relayUrls);
  }
}

main().catch((error) => {
  console.error(error);
  process.exit(1);
});
