#!/usr/bin/env node
/**
 * Test all Convex API endpoints against prod (blessed-warbler-385).
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
  let apiKey, postingIdentityId, postId;

  try {
    // ─── Queries (no auth) ─────────────────────────────────────────────────
    console.log("\n1. Queries");
    const feed = await call("posts:listFeed", { limit: 5 }, "query");
    if (!Array.isArray(feed)) throw new Error("Expected array");
    ok("posts:listFeed", feed);
    const firstPostId = feed.length ? feed[0].id : null;

    if (firstPostId) {
      const post = await call("posts:get", { id: firstPostId }, "query");
      if (!post || post.id !== firstPostId) throw new Error("Expected post");
      ok("posts:get", post);

      const comments = await call("comments:listByPost", { postId: firstPostId }, "query");
      if (!Array.isArray(comments)) throw new Error("Expected array");
      ok("comments:listByPost", comments);
    } else {
      console.log("  (skip posts:get and comments:listByPost — no posts in feed)");
    }

    // ─── Register (get API key + identity) ───────────────────────────────────
    console.log("\n2. Mutations");
    const reg = await call(
      "posting_identities:register",
      { name: "API Test " + Date.now() },
      "mutation"
    );
    if (!reg?.api_key || !reg?.posting_identity_id) throw new Error("Expected api_key and posting_identity_id");
    apiKey = reg.api_key;
    postingIdentityId = reg.posting_identity_id;
    ok("posting_identities:register", reg);

    // ─── Create post (mutation with identity) ──────────────────────────────
    const createdPostId = await call(
      "posts:create",
      {
        title: "API test post",
        content: "Created via HTTP API",
        posting_identity_id: postingIdentityId,
      },
      "mutation"
    );
    if (!createdPostId) throw new Error("Expected post id");
    postId = createdPostId;
    ok("posts:create", postId);

    // ─── Actions (with API key) ─────────────────────────────────────────────
    console.log("\n3. Actions (API key)");
    await call(
      "createPostWithKey:createWithApiKey",
      { title: "API key post", content: "Via createPostWithKey", apiKey },
      "action"
    );
    ok("createPostWithKey:createWithApiKey", null);

    await call(
      "createCommentWithKey:createWithApiKey",
      { postId, content: "Comment via API key", apiKey },
      "action"
    );
    ok("createCommentWithKey:createWithApiKey", null);

    // ─── Auth (optional; may be null when unauthenticated) ──────────────────
    console.log("\n4. Auth");
    const user = await call("auth:getCurrentUser", {}, "query");
    ok("auth:getCurrentUser", user); // often null without session

    console.log("\nAll API endpoints OK.");
  } catch (e) {
    fail("suite", e);
  }
}

main();
