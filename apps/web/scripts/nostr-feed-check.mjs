import { SimplePool } from "nostr-tools";
import WebSocket from "ws";

const DEFAULT_RELAYS = [
  "wss://relay.damus.io",
  "wss://nos.lol",
  "wss://relay.nostr.band",
  "wss://relay.primal.net",
  "wss://relay.ditto.pub",
];

const BASE_URLS = ["https://clawstr.com", "https://openagents.com"];

if (!globalThis.WebSocket) {
  globalThis.WebSocket = WebSocket;
}

function getArg(flag) {
  const idx = process.argv.indexOf(flag);
  if (idx === -1) return undefined;
  return process.argv[idx + 1];
}

const subclawArg = getArg("--subclaw");
const showAll = process.argv.includes("--show-all");
const diagnostics = process.argv.includes("--diagnostics");
const limitArg = getArg("--limit");
const sinceArg = getArg("--since");
const relaysArg = getArg("--relays");

const limit = limitArg ? Number.parseInt(limitArg, 10) : 50;
const since = sinceArg ? Number.parseInt(sinceArg, 10) : undefined;
const relays = relaysArg
  ? relaysArg.split(",").map((r) => r.trim()).filter(Boolean)
  : DEFAULT_RELAYS;

const subclaw = subclawArg ? subclawArg.trim().toLowerCase() : undefined;
const identifiers = subclaw
  ? BASE_URLS.flatMap((base) => {
      const id = `${base}/c/${subclaw}`;
      return [id, `${id}/`];
    })
  : [];

const filter = {
  kinds: [1111],
  "#K": ["web"],
  limit: Number.isFinite(limit) ? limit : 50,
};
if (since && Number.isFinite(since)) filter.since = since;
if (!showAll) {
  filter["#l"] = ["ai"];
}
if (identifiers.length > 0) {
  filter["#I"] = identifiers;
}

function getTagValue(tags, name) {
  const tag = tags.find(([t]) => t === name);
  return tag?.[1];
}

function isTopLevel(event) {
  const I = getTagValue(event.tags, "I") ?? getTagValue(event.tags, "i");
  const i = getTagValue(event.tags, "i");
  const e = getTagValue(event.tags, "e");
  const k = getTagValue(event.tags, "k") ?? getTagValue(event.tags, "K");
  if (!I || e) return false;
  if (k !== "web") return false;
  if (i && i !== I) return false;
  return true;
}

function baseFromIdentifier(identifier) {
  if (!identifier) return "unknown";
  const normalized = identifier.replace(/\/+$/, "");
  const match = BASE_URLS.find((base) => normalized.toLowerCase().startsWith(`${base}/c/`));
  return match ?? "unknown";
}

const pool = new SimplePool();
let exitCode = 0;

try {
  console.log(`Relays: ${relays.join(", ")}`);
  if (subclaw) console.log(`Subclaw: ${subclaw}`);
  if (since) console.log(`Since: ${since} (${new Date(since * 1000).toISOString()})`);
  const events = await pool.querySync(relays, filter);
  const deduped = new Map(events.map((event) => [event.id, event]));
  const unique = [...deduped.values()];

  const topLevel = unique.filter((event) => {
    if (!isTopLevel(event)) return false;
    if (!subclaw) return true;
    const identifier = getTagValue(event.tags, "I");
    return identifier && identifiers.includes(identifier);
  });

  if (diagnostics) {
    let missingI = 0;
    let missingi = 0;
    let missingK = 0;
    let missingL = 0;
    let missingl = 0;
    let hasE = 0;
    for (const event of unique) {
      const I = getTagValue(event.tags, "I");
      const i = getTagValue(event.tags, "i");
      const k = getTagValue(event.tags, "k") ?? getTagValue(event.tags, "K");
      const L = getTagValue(event.tags, "L");
      const l = getTagValue(event.tags, "l");
      const e = getTagValue(event.tags, "e");
      if (!I) missingI += 1;
      if (!i) missingi += 1;
      if (!k) missingK += 1;
      if (!L) missingL += 1;
      if (!l) missingl += 1;
      if (e) hasE += 1;
    }
    console.log(`Missing tags: I=${missingI}, i=${missingi}, K/k=${missingK}, L=${missingL}, l=${missingl}`);
    console.log(`Replies (have e tag): ${hasE}`);
  }

  const countsByBase = new Map();
  for (const event of topLevel) {
    const identifier = getTagValue(event.tags, "I");
    const base = baseFromIdentifier(identifier);
    countsByBase.set(base, (countsByBase.get(base) ?? 0) + 1);
  }

  console.log(`Fetched: ${events.length} events (${unique.length} unique)`);
  console.log(`Top-level: ${topLevel.length}`);
  const byBase = [...countsByBase.entries()]
    .map(([base, count]) => `${base}:${count}`)
    .join(", ");
  console.log(`By base: ${byBase || "none"}`);

  const sorted = topLevel.sort((a, b) => b.created_at - a.created_at);
  const preview = sorted.slice(0, 10);
  for (const event of preview) {
    const identifier = getTagValue(event.tags, "I");
    const firstLine = event.content.split("\n").find((line) => line.trim()) ?? "";
    console.log(`- ${event.id} ${new Date(event.created_at * 1000).toISOString()} ${identifier}`);
    console.log(`  ${firstLine.slice(0, 120)}${firstLine.length > 120 ? "…" : ""}`);
  }
} catch (err) {
  exitCode = 1;
  console.error(err);
} finally {
  pool.close(relays);
  setTimeout(() => process.exit(exitCode), 200);
}
