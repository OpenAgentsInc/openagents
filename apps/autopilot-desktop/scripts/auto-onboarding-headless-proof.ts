#!/usr/bin/env bun
// AO-1/AO-2 (#5442/#5443, EPIC #5441) headless convergence proof.
//
// Drives the REAL dev Pylon node through the desktop launcher
// (`superviseManagedNode`) into a fresh, empty managed home with
// `autoOnboarding: true`, pointing onboarding + presence at a local mock of the
// `openagents.com` Worker. It proves, with no GUI and no env vars, that a fresh
// node converges to:
//   1. identity generated (the node writes identity.json),
//   2. agent self-registered (POST /api/agents/register) + token persisted,
//   3. the node restarted with the token + onboarding env injected,
//   4. presence registered (POST /api/pylons/register, bearer path),
//   5. Spark payout target registered (POST .../spark-payout-target, #5305),
//   6. the Tassadar assignment worker polling for claimable work
//      (GET .../assignments).
//
// Anything that needs a real GUI / from-DMG run (rendered window screenshot, a
// real settled Bitcoin receipt) is out of scope here and is AO-6's job.
//
// The mock NEVER receives or echoes a real secret; the only token in play is a
// fake `oa_agent_...` value minted by the mock. The proof asserts the token is
// not printed to stdout.

import { mkdtempSync, readFileSync, rmSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { superviseManagedNode } from "../src/bun/node-launcher"
import { postForumIntroduction } from "../src/bun/forum-intro"
import { searchForumWork } from "../src/bun/forum-work-search"

const FAKE_TOKEN = "oa_agent_headless_proof_token"

type Hit = { method: string; path: string; bearer: string | null }

const hits: Hit[] = []
let registerBody: Record<string, unknown> | null = null

const server = Bun.serve({
  port: 0,
  async fetch(req) {
    const url = new URL(req.url)
    const bearer = req.headers.get("authorization")
    hits.push({ method: req.method, path: url.pathname, bearer })

    // AO-1: self-serve agent registration -> mint a fake token.
    if (url.pathname === "/api/agents/register" && req.method === "POST") {
      registerBody = (await req.json().catch(() => ({}))) as Record<
        string,
        unknown
      >
      return Response.json(
        {
          user: { id: "user_proof", status: "active" },
          credential: { token: FAKE_TOKEN, tokenPrefix: "oa_agent_hea" },
        },
        { status: 201 },
      )
    }

    // Presence registration (bearer path) + heartbeat.
    if (url.pathname === "/api/pylons/register" && req.method === "POST") {
      return Response.json(
        { registrationRef: "reg_proof", pylonRef: "pylon.proof", stale: false },
        { status: 200 },
      )
    }
    if (url.pathname.endsWith("/heartbeat") && req.method === "POST") {
      return Response.json({ ok: true, stale: false }, { status: 200 })
    }
    // Spark payout-target registration (#5305).
    if (
      url.pathname.endsWith("/spark-payout-target") &&
      req.method === "POST"
    ) {
      return Response.json(
        { payoutTargetRef: "payout_proof", state: "registered" },
        { status: 200 },
      )
    }
    if (url.pathname.endsWith("/wallet-readiness") && req.method === "POST") {
      return Response.json({ ok: true }, { status: 200 })
    }
    // Tassadar assignment worker poll (no work available is fine for the proof).
    if (url.pathname.endsWith("/assignments") && req.method === "GET") {
      return Response.json({ assignments: [] }, { status: 200 })
    }

    // AF-3 (#5900): forum board read for typed intro-lane selection.
    if (url.pathname === "/api/forum" && req.method === "GET") {
      return Response.json(
        {
          boardId: "board_proof",
          slug: "openagents",
          title: "OpenAgents",
          categories: [],
          forums: [
            { slug: "general", title: "General", locked: false },
            { slug: "introductions", title: "Introductions", locked: false },
          ],
          generatedAt: new Date().toISOString(),
          publicProjection: {},
        },
        { status: 200 },
      )
    }
    // AF-3 (#5900): create-topic (the intro post).
    if (
      /^\/api\/forum\/forums\/[^/]+\/topics$/.test(url.pathname) &&
      req.method === "POST"
    ) {
      return Response.json(
        {
          topic: { id: "topic_proof", slug: "intro-proof" },
          firstPost: { id: "post_proof" },
          idempotent: false,
          receiptRefs: [],
        },
        { status: 200 },
      )
    }
    // AF-4 (#5901): read-only work-search over the typed work-requests lane.
    if (url.pathname === "/api/forum/work-requests" && req.method === "GET") {
      return Response.json(
        {
          workRequests: [
            { workRequestId: "wr_1", title: "demo", state: "open" },
            { workRequestId: "wr_2", title: "demo2", state: "running" },
          ],
          pagination: { cursor: null, hasMore: false, limit: 50, nextCursor: null },
        },
        { status: 200 },
      )
    }

    return Response.json({ ok: true }, { status: 200 })
  },
})

const baseUrl = `http://127.0.0.1:${server.port}`
const home = mkdtempSync(join(tmpdir(), "ao-headless-proof-"))

// Repo root: this script lives at apps/autopilot-desktop/scripts.
const repoRoot = join(import.meta.dir, "..", "..", "..")
const cwd = join(repoRoot, "apps", "autopilot-desktop")

const sleep = (ms: number) => new Promise(r => setTimeout(r, ms))

const fail = (message: string): never => {
  console.error(`\nFAIL: ${message}`)
  server.stop(true)
  rmSync(home, { recursive: true, force: true })
  process.exit(1)
}

console.log("== AO-1/AO-2 headless convergence proof ==")
console.log(`mock openagents.com : ${baseUrl}`)
console.log(`fresh managed home  : ${home}`)
console.log("(token is redacted in all output)\n")

const controlPort = 40000 + Math.floor(Math.random() * 10000)
const statuses: string[] = []
const sup = superviseManagedNode({
  cwd,
  // Force the managed home to our fresh temp dir, and point the control server
  // at a unique port so we never collide with a real local node.
  env: {
    PYLON_HOME: home,
    PYLON_CONTROL_PORT: String(controlPort),
  },
  controlBaseUrl: `http://127.0.0.1:${controlPort}`,
  // Force a fresh launch into our temp home: never adopt a real local node that
  // discoverPylonHome would find by walking up from cwd. This is the
  // clean-machine first-run path the proof is about.
  discover: () => null,
  autoOnboarding: true,
  onboardingBaseUrl: baseUrl,
  // Point the launcher's readiness probe at the node's control server. We let
  // the node bring its own control server up; readiness uses controlBaseUrl.
  readinessTimeoutMs: 60_000,
  readinessIntervalMs: 500,
  onStatus: s => {
    statuses.push(s)
    console.log(`[status] ${s}`)
  },
})

// Wait for the chain to converge (registration -> restart -> presence -> poll),
// or time out. We poll the recorded hits.
const deadline = Date.now() + 90_000
const seen = (pred: (h: Hit) => boolean) => hits.some(pred)
while (Date.now() < deadline) {
  const registered = seen(h => h.path === "/api/agents/register")
  const presence = seen(h => h.path === "/api/pylons/register")
  const payout = seen(h => h.path.endsWith("/spark-payout-target"))
  const assignments = seen(
    h => h.path.endsWith("/assignments") && h.method === "GET",
  )
  if (registered && presence && payout && assignments) break
  await sleep(1000)
}

// Capture the managed home BEFORE stopping (stop() clears current -> home()).
// We force PYLON_HOME to our fresh temp `home`, and the launcher honors an
// explicit env PYLON_HOME (node-launcher: `managedHome = env.PYLON_HOME ?? …`),
// so the node writes identity/credential there. `sup.home()` returns that same
// path when a child is current, but can momentarily read null mid token-restart;
// fall back to the temp `home` we forced (never the repo-root `.pylon-local`,
// which is a different node's home).
const managedHome = sup.home() ?? home

sup.stop()
await sleep(500)

// AF-3/AF-4/AF-6 (#5900/#5901/#5903): with the node converged (identity +
// credential persisted in the managed home), drive the forum loop directly
// against the still-running mock — exactly the path index.ts fires on poll.
// The token is read from the home; it is never printed.
let introOutcome = "unrun"
let workOutcome = "unrun"
try {
  const intro = await postForumIntroduction({ home: managedHome, baseUrl })
  introOutcome = intro.outcome
} catch (error) {
  introOutcome = `threw:${error instanceof Error ? error.message : String(error)}`
}
try {
  const work = await searchForumWork({ home: managedHome, baseUrl })
  workOutcome = work.outcome
} catch (error) {
  workOutcome = `threw:${error instanceof Error ? error.message : String(error)}`
}

await sleep(200)
server.stop(true)

console.log("\n-- captured mock hits --")
for (const h of hits) {
  console.log(
    `  ${h.method} ${h.path}${h.bearer ? ` [bearer ${h.bearer.startsWith("Bearer oa_agent_") ? "oa_agent_…" : "other"}]` : ""}`,
  )
}
console.log(`\nmanaged home used: ${managedHome}\n`)

// --- assertions -------------------------------------------------------------

let ok = true
const check = (label: string, cond: boolean) => {
  console.log(`${cond ? "PASS" : "FAIL"}  ${label}`)
  if (!cond) ok = false
}

// 1. identity written by the node.
let identity: Record<string, unknown> | null = null
try {
  identity = JSON.parse(readFileSync(join(managedHome, "identity.json"), "utf8"))
} catch {
  identity = null
}
check("node generated identity (identity.json written)", identity !== null && typeof identity.npub === "string")

// 2. agent self-registered + token persisted.
check("agent self-registered (POST /api/agents/register)", seen(h => h.path === "/api/agents/register" && h.method === "POST"))
check(
  "registration used the node npub as externalId",
  registerBody !== null && registerBody.externalId === identity?.npub,
)
let persisted: Record<string, unknown> | null = null
try {
  persisted = JSON.parse(readFileSync(join(managedHome, "agent-credential.json"), "utf8"))
} catch {
  persisted = null
}
check("token persisted to managed home (agent-credential.json)", persisted !== null && typeof persisted.token === "string" && (persisted.token as string).startsWith("oa_agent_"))

// 3. presence registered using the minted token (bearer path, not NIP-98).
// The first boot (pre-registration) may register presence without a token; the
// post-restart boot registers with the bearer. We assert that at least one
// presence registration carried the bearer token.
const presenceHits = hits.filter(h => h.path === "/api/pylons/register")
check("presence registered (POST /api/pylons/register)", presenceHits.length > 0)
check(
  "presence used the bearer agent token (not NIP-98)",
  presenceHits.some(h => h.bearer === `Bearer ${FAKE_TOKEN}`),
)

// 4. Spark payout target registered (#5305).
check("payout target registered (POST .../spark-payout-target)", seen(h => h.path.endsWith("/spark-payout-target") && h.method === "POST"))

// 5. Tassadar assignment worker polling for claimable work.
check("assignment worker polled for work (GET .../assignments)", seen(h => h.path.endsWith("/assignments") && h.method === "GET"))

// 6. AF-3 (#5900): forum self-introduction posted + receipt persisted.
check("forum intro posted (outcome=posted)", introOutcome === "posted")
check(
  "forum board read for typed lane selection (GET /api/forum)",
  seen(h => h.path === "/api/forum" && h.method === "GET"),
)
check(
  "intro topic created (POST /api/forum/forums/.../topics)",
  seen(h => /^\/api\/forum\/forums\/[^/]+\/topics$/.test(h.path) && h.method === "POST"),
)
let introReceipt: Record<string, unknown> | null = null
try {
  introReceipt = JSON.parse(
    readFileSync(join(managedHome, "forum-intro.json"), "utf8"),
  )
} catch {
  introReceipt = null
}
check(
  "intro receipt persisted with a dereferenceable topic URL",
  introReceipt !== null &&
    typeof introReceipt.topicId === "string" &&
    typeof introReceipt.url === "string" &&
    (introReceipt.url as string).includes("/forum/t/"),
)
check(
  "intro topic used the typed introductions lane",
  introReceipt !== null && introReceipt.forumSlug === "introductions",
)

// 7. AF-4 (#5901): read-only work-search ran + receipt persisted.
check("work-search ran (outcome=searched)", workOutcome === "searched")
check(
  "work-requests lane read (GET /api/forum/work-requests)",
  seen(h => h.path === "/api/forum/work-requests" && h.method === "GET"),
)
let workReceipt: Record<string, unknown> | null = null
try {
  workReceipt = JSON.parse(
    readFileSync(join(managedHome, "forum-work-search.json"), "utf8"),
  )
} catch {
  workReceipt = null
}
check(
  "work-search receipt persisted with the open-item count (typed state)",
  workReceipt !== null && workReceipt.openCount === 1,
)

// Secrets boundary: the token must not appear in this proof's stdout. (We never
// printed it; assert the contract holds for the captured output surfaces.)
check("token never printed to status/log surfaces", !statuses.join("\n").includes(FAKE_TOKEN))

rmSync(home, { recursive: true, force: true })

console.log()
if (ok) {
  console.log("RESULT: a fresh node self-registered, joined, reached earning-ready (presence + payout + assignment poll), posted its forum self-introduction, and ran a read-only work-search — headlessly, no GUI, no env vars.")
  process.exit(0)
} else {
  fail("one or more convergence gates did not pass (see above)")
}
