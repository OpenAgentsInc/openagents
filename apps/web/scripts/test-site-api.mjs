#!/usr/bin/env node
/**
 * Full site + API test: Convex backend, Better Auth (user sign-up), and optional site reachability.
 *
 * Usage:
 *   cd apps/web && node scripts/test-site-api.mjs
 *
 * Env (defaults to prod):
 *   CONVEX_URL          - Convex backend (default: https://blessed-warbler-385.convex.cloud)
 *   CONVEX_SITE_URL     - Convex HTTP / auth base (default: https://blessed-warbler-385.convex.site)
 *   SITE_URL            - Deployed frontend (optional; if set, fetches /, /feed, /communities, etc.)
 */
const CONVEX_URL = process.env.CONVEX_URL || "https://blessed-warbler-385.convex.cloud";
const CONVEX_SITE_URL = process.env.CONVEX_SITE_URL || "https://blessed-warbler-385.convex.site";
const SITE_URL = process.env.SITE_URL || ""; // e.g. https://web-ct8.pages.dev

let testsRun = 0;
let testsFailed = 0;

function ok(name, value) {
  testsRun++;
  console.log(`  ✓ ${name}`);
  return value;
}

function fail(name, err) {
  testsFailed++;
  const msg = err?.message ?? String(err);
  console.error(`  ✗ ${name}: ${msg}`);
  return null;
}

async function convexCall(path, args, type = "query") {
  const url = `${CONVEX_URL}/api/${type}`;
  const res = await fetch(url, {
    method: "POST",
    headers: { "Content-Type": "application/json" },
    body: JSON.stringify({ path, args, format: "json" }),
  });
  const data = await res.json();
  if (data.status === "error") {
    throw new Error(data.errorMessage || data.errorData?.message || JSON.stringify(data));
  }
  return data.value;
}

async function main() {
  console.log("=== Site + API test ===\n");
  console.log("Convex backend:", CONVEX_URL);
  console.log("Convex site (auth):", CONVEX_SITE_URL);
  if (SITE_URL) console.log("Frontend:", SITE_URL);
  console.log("");

  // ─── 1. Site reachability (optional) ─────────────────────────────────────
  if (SITE_URL) {
    console.log("1. Site reachability");
    const routes = ["/", "/feed", "/communities", "/kb", "/blog", "/about"];
    for (const path of routes) {
      try {
        const res = await fetch(SITE_URL + path, { redirect: "follow" });
        if (res.ok) ok(`GET ${path}`, res.status);
        else fail(`GET ${path}`, new Error(`status ${res.status}`));
      } catch (e) {
        fail(`GET ${path}`, e);
      }
    }
    console.log("");
  }

  // ─── 2. Better Auth: create new user ──────────────────────────────────────
  console.log("2. Auth (Better Auth)");
  const timestamp = Date.now();
  const testEmail = `api-test+${timestamp}@example.com`;
  const testPassword = "TestPassword123!";
  try {
    const signUpUrl = `${CONVEX_SITE_URL}/api/auth/sign-up/email`;
    const res = await fetch(signUpUrl, {
      method: "POST",
      headers: { "Content-Type": "application/json" },
      body: JSON.stringify({
        name: "API Test User",
        email: testEmail,
        password: testPassword,
      }),
    });
    const data = await res.json().catch(() => ({}));
    if (res.ok && !data?.error) {
      ok("POST /api/auth/sign-up/email (new user)", data);
    } else if (res.status === 400 && (data?.code === "USER_ALREADY_EXISTS" || data?.message?.toLowerCase?.().includes("already exists"))) {
      ok("POST /api/auth/sign-up/email (user exists, skip)", null);
    } else {
      fail("POST /api/auth/sign-up/email", new Error(data?.message || data?.error || `status ${res.status}`));
    }
  } catch (e) {
    fail("POST /api/auth/sign-up/email", e);
  }
  console.log("");

  // ─── 3. Convex: queries (no auth) ─────────────────────────────────────────
  console.log("3. Convex queries");
  let sampleEventId = null;
  let samplePubkey = null;
  try {
    const feed = await convexCall("nostr:listFeed", { limit: 10 }, "query");
    if (!Array.isArray(feed)) throw new Error("Expected array");
    ok("nostr:listFeed", feed);
    sampleEventId = feed.length ? feed[0].event_id : null;
    samplePubkey = feed.length ? feed[0].pubkey : null;
  } catch (e) {
    fail("nostr:listFeed", e);
  }

  try {
    const subclaws = await convexCall("nostr:listSubclaws", { limit: 20 }, "query");
    if (!Array.isArray(subclaws)) throw new Error("Expected array");
    ok("nostr:listSubclaws", subclaws);
  } catch (e) {
    fail("nostr:listSubclaws", e);
  }

  if (sampleEventId) {
    try {
      const post = await convexCall("nostr:getPost", { event_id: sampleEventId }, "query");
      if (!post || post.event_id !== sampleEventId) throw new Error("Expected event");
      ok("nostr:getPost", post);
    } catch (e) {
      fail("nostr:getPost", e);
    }
    try {
      const replies = await convexCall("nostr:listReplies", { event_id: sampleEventId }, "query");
      if (!Array.isArray(replies)) throw new Error("Expected array");
      ok("nostr:listReplies", replies);
    } catch (e) {
      fail("nostr:listReplies", e);
    }
  } else {
    console.log("  (skip nostr:getPost, nostr:listReplies — no events in feed)");
  }

  if (samplePubkey) {
    try {
      const profiles = await convexCall("nostr:getProfiles", { pubkeys: [samplePubkey] }, "query");
      if (!Array.isArray(profiles)) throw new Error("Expected array");
      ok("nostr:getProfiles", profiles);
    } catch (e) {
      fail("nostr:getProfiles", e);
    }
  }
  console.log("");

  // ─── 4. Auth query (unauthenticated) ──────────────────────────────────────
  console.log("4. Auth query");
  try {
    const user = await convexCall("auth:getCurrentUser", {}, "query");
    ok("auth:getCurrentUser (no session)", user); // expect null
  } catch (e) {
    fail("auth:getCurrentUser", e);
  }

  // ─── Summary ──────────────────────────────────────────────────────────────
  console.log("\n=== Summary ===");
  console.log(`  Run: ${testsRun}  Failed: ${testsFailed}`);
  if (testsFailed > 0) {
    process.exit(1);
  }
  console.log("  All tests passed.");
}

main().catch((e) => {
  console.error(e);
  process.exit(1);
});
