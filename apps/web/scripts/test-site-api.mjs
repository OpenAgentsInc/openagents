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
  let firstPostId = null;
  try {
    const feed = await convexCall("posts:listFeed", { limit: 10 }, "query");
    if (!Array.isArray(feed)) throw new Error("Expected array");
    ok("posts:listFeed", feed);
    firstPostId = feed.length ? feed[0].id : null;
  } catch (e) {
    fail("posts:listFeed", e);
  }

  if (firstPostId) {
    try {
      const post = await convexCall("posts:get", { id: firstPostId }, "query");
      if (!post || post.id !== firstPostId) throw new Error("Expected post");
      ok("posts:get", post);
    } catch (e) {
      fail("posts:get", e);
    }
    try {
      const comments = await convexCall("comments:listByPost", { postId: firstPostId }, "query");
      if (!Array.isArray(comments)) throw new Error("Expected array");
      ok("comments:listByPost", comments);
    } catch (e) {
      fail("comments:listByPost", e);
    }
  } else {
    console.log("  (skip posts:get, comments:listByPost — no posts in feed)");
  }
  console.log("");

  // ─── 4. Convex: register posting identity + create post (mutation) ─────────
  console.log("4. Convex mutations");
  let apiKey = null;
  let postingIdentityId = null;
  let createdPostId = null;

  try {
    const reg = await convexCall(
      "posting_identities:register",
      { name: "API Test " + timestamp },
      "mutation"
    );
    if (!reg?.api_key || !reg?.posting_identity_id) throw new Error("Expected api_key and posting_identity_id");
    apiKey = reg.api_key;
    postingIdentityId = reg.posting_identity_id;
    ok("posting_identities:register", reg);
  } catch (e) {
    fail("posting_identities:register", e);
  }

  if (postingIdentityId) {
    try {
      createdPostId = await convexCall(
        "posts:create",
        {
          title: "API test post " + timestamp,
          content: "Created via HTTP API (test-site-api.mjs)",
          posting_identity_id: postingIdentityId,
        },
        "mutation"
      );
      if (!createdPostId) throw new Error("Expected post id");
      ok("posts:create", createdPostId);
    } catch (e) {
      fail("posts:create", e);
    }
  }
  console.log("");

  // ─── 5. Convex: actions (API key) ─────────────────────────────────────────
  console.log("5. Convex actions (API key)");
  if (apiKey) {
    try {
      await convexCall(
        "createPostWithKey:createWithApiKey",
        {
          title: "API key post " + timestamp,
          content: "Via createPostWithKey",
          apiKey,
        },
        "action"
      );
      ok("createPostWithKey:createWithApiKey", null);
    } catch (e) {
      fail("createPostWithKey:createWithApiKey", e);
    }

    if (createdPostId) {
      try {
        await convexCall(
          "createCommentWithKey:createWithApiKey",
          {
            postId: createdPostId,
            content: "Comment via API key " + timestamp,
            apiKey,
          },
          "action"
        );
        ok("createCommentWithKey:createWithApiKey", null);
      } catch (e) {
        fail("createCommentWithKey:createWithApiKey", e);
      }
    }
  }
  console.log("");

  // ─── 6. Convex: verify created content ───────────────────────────────────
  console.log("6. Verify created content");
  if (createdPostId) {
    try {
      const post = await convexCall("posts:get", { id: createdPostId }, "query");
      if (!post || post.id !== createdPostId) throw new Error("Created post not found");
      if (!post.title?.includes("API test post")) throw new Error("Wrong post title");
      ok("posts:get (created post)", post);
    } catch (e) {
      fail("posts:get (created post)", e);
    }
    try {
      const comments = await convexCall("comments:listByPost", { postId: createdPostId }, "query");
      if (!Array.isArray(comments)) throw new Error("Expected array");
      const hasOurComment = comments.some((c) => String(c.content || "").includes("Comment via API key"));
      if (!hasOurComment) throw new Error("Created comment not found in list");
      ok("comments:listByPost (created comment)", comments);
    } catch (e) {
      fail("comments:listByPost (created comment)", e);
    }
  }
  console.log("");

  // ─── 7. Auth query (unauthenticated) ──────────────────────────────────────
  console.log("7. Auth query");
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
