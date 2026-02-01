#!/usr/bin/env node
/**
 * Test Convex API endpoints against prod (blessed-warbler-385).
 * Uses Convex HTTP API: POST to /api/query, /api/mutation, /api/action.
 *
 * Usage: CONVEX_URL=https://blessed-warbler-385.convex.cloud node scripts/test-api.mjs
 */
const BASE = process.env.CONVEX_URL || "https://blessed-warbler-385.convex.cloud";

async function call(path, args, type = "query") {
  const url = `${BASE}/api/${type}`;
  const body = { path, args, format: "json" };
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify(body),
  });
  const data = await res.json();
  if (data.status === "error") {
    throw new Error(data.errorMessage || JSON.stringify(data));
  }
  return data.value;
}

function ok(name, value) {
  console.log(`  ✓ ${name}`);
  return value;
}
function fail(name, err) {
  console.error(`  ✗ ${name}: ${err.message}`);
  throw err;
}

async function main() {
  console.log("Testing Convex API at", BASE);
  let sampleEventId = null;

  try {
    // ─── Queries (no auth) ─────────────────────────────────────────────────
    console.log("\n1. Queries");
    const feed = await call("nostr:listFeed", { limit: 5 }, "query");
    if (!Array.isArray(feed)) throw new Error("Expected array");
    ok("nostr:listFeed", feed);
    sampleEventId = feed.length ? feed[0].event_id : null;

    const subclaws = await call("nostr:listSubclaws", { limit: 10 }, "query");
    if (!Array.isArray(subclaws)) throw new Error("Expected array");
    ok("nostr:listSubclaws", subclaws);

    if (sampleEventId) {
      const post = await call("nostr:getPost", { event_id: sampleEventId }, "query");
      if (!post || post.event_id !== sampleEventId) throw new Error("Expected event");
      ok("nostr:getPost", post);

      const replies = await call("nostr:listReplies", { event_id: sampleEventId }, "query");
      if (!Array.isArray(replies)) throw new Error("Expected array");
      ok("nostr:listReplies", replies);
    } else {
      console.log("  (skip nostr:getPost and nostr:listReplies — no events in feed)");
    }

    // ─── Auth (optional; may be null when unauthenticated) ──────────────────
    console.log("\n2. Auth");
    const user = await call("auth:getCurrentUser", {}, "query");
    ok("auth:getCurrentUser", user); // often null without session

    console.log("\nAll API endpoints OK.");
  } catch (e) {
    fail("suite", e);
  }
}

main();
