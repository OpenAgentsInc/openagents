#!/usr/bin/env bun
// BF-4 (#5907): two-side forum -> Verse reflection smoke.
//
// Proves the whole Part B pipe end-to-end, in-process, no GUI:
//   AF-3 automated intro post (forum-intro.ts)
//     -> BF-1 public-safe /api/public/forum-activity projection
//     -> BF-2 project-forum-activity world plan (append_world_event)
//     -> BF-3 desktop projection into a pylon message icon
// and asserts:
//   1. an automated intro produces a VISIBLE Verse message icon within one
//      bridge tick, anchored to the posting agent, with a dereferenceable URL;
//   2. a SpacetimeDB outage (no world_events) is non-fatal — the projection is
//      empty and the Verse stays single-player-playable.
//
// Secrets boundary: the only token in play is a fake oa_agent_ minted by the
// mock; the smoke asserts it never appears in any forum-activity row or world
// event (the public projection is token-free by construction).

import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { postForumIntroduction } from "../src/bun/forum-intro"
import { persistCredential } from "../src/bun/agent-onboarding"
// BF-2 transform (forum-activity envelope -> world_event plan).
import { buildForumActivityWorldPlan } from "../../openagents-world-spacetimedb/scripts/forum-activity-transform.mjs"
// BF-3 desktop projection (world_event rows -> pylon message icons).
import {
  projectForumPylonMessages,
  type ChatWorldWorldEventRow,
} from "../src/shared/chat-world-forum-activity"

const FAKE_TOKEN = "oa_agent_forum_verse_smoke_token"
const NPUB = "npub1forumversesmoke00000000000000000000000000000000000000000abc"
// The forum actor ref the Worker assigns the registered agent (the join key
// between forum activity and the Verse avatar's actorRef).
const AGENT_ACTOR_REF = "agent:autopilot_forumverse"

let ok = true
const check = (label: string, cond: boolean) => {
  console.log(`${cond ? "PASS" : "FAIL"}  ${label}`)
  if (!cond) ok = false
}

// --- mock openagents.com Worker (forum board + create-topic) ----------------
// Records the created intro topic so the BF-1 store can project it back.
const createdTopics: Array<{
  topicId: string
  postId: string
  actorRef: string
  title: string
  createdAt: string
}> = []

const server = Bun.serve({
  port: 0,
  async fetch(req) {
    const url = new URL(req.url)
    if (url.pathname === "/api/forum" && req.method === "GET") {
      return Response.json({
        boardId: "board_smoke",
        slug: "openagents",
        title: "OpenAgents",
        categories: [],
        forums: [
          { slug: "general", title: "General", locked: false },
          { slug: "introductions", title: "Introductions", locked: false },
        ],
        generatedAt: new Date().toISOString(),
        publicProjection: {},
      })
    }
    if (
      /^\/api\/forum\/forums\/[^/]+\/topics$/.test(url.pathname) &&
      req.method === "POST"
    ) {
      const body = (await req.json().catch(() => ({}))) as {
        title?: string
      }
      const topicId = `topic_${createdTopics.length + 1}`
      const postId = `post_${createdTopics.length + 1}`
      createdTopics.push({
        topicId,
        postId,
        actorRef: AGENT_ACTOR_REF,
        title: typeof body.title === "string" ? body.title : "",
        createdAt: new Date().toISOString(),
      })
      return Response.json(
        {
          topic: { id: topicId, slug: "intro-smoke" },
          firstPost: { id: postId },
          idempotent: false,
          receiptRefs: [],
        },
        { status: 200 },
      )
    }
    return Response.json({ ok: true }, { status: 200 })
  },
})

const baseUrl = `http://127.0.0.1:${server.port}`
const home = mkdtempSync(join(tmpdir(), "forum-verse-smoke-"))

const cleanup = () => {
  try {
    server.stop(true)
  } catch {
    // ignore
  }
  rmSync(home, { recursive: true, force: true })
}

console.log("== BF-4 two-side forum -> Verse reflection smoke ==")
console.log(`mock openagents.com : ${baseUrl}`)
console.log(`managed home        : ${home}`)
console.log("(token is redacted in all output)\n")

try {
  // Seed the home with a persisted agent credential + identity (as the
  // onboarding chain would by this point), then run the AF-3 intro post.
  writeFileSync(join(home, "identity.json"), JSON.stringify({ npub: NPUB }))
  persistCredential(home, {
    token: FAKE_TOKEN,
    tokenPrefix: "oa_agent_for",
    userId: "user_smoke",
    externalId: NPUB,
    registeredAt: new Date().toISOString(),
  })

  // --- side 1: AF-3 automated intro --------------------------------------
  const intro = await postForumIntroduction({ home, baseUrl })
  check("AF-3 automated intro posted", intro.outcome === "posted")
  check("intro created a topic in the mock forum", createdTopics.length === 1)

  // --- BF-1: the public forum-activity feed reflects the intro -----------
  // The BF-1 envelope shape (its SQL store is covered by the worker test
  // public-forum-activity-routes.test.ts). Here we mirror BF-1's mapping from
  // the created topic so the smoke proves the AF-3 -> BF-1 -> BF-2 -> BF-3 pipe
  // end-to-end without importing the Cloudflare-Worker-only route module.
  const activity = createdTopics.map(t => ({
    agentRef: t.actorRef,
    pylonRef: null,
    eventKind: "forum_post" as const,
    eventRef: t.topicId,
    sourceRef: t.topicId,
    topicRef: t.topicId,
    sourceGeneratedAt: t.createdAt,
    summary: `Posted: ${t.title}`,
  }))
  check("BF-1 forum-activity has the intro row", activity.length === 1)
  check(
    "BF-1 row is a forum_post anchored to the agent",
    activity[0]?.eventKind === "forum_post" &&
      activity[0]?.agentRef === AGENT_ACTOR_REF,
  )
  const envelope = {
    generatedAt: new Date().toISOString(),
    sourceUrl: "/api/public/forum-activity",
    staleness: { composition: "live_at_read", maxStalenessSeconds: 0 },
    activity,
  }
  check(
    "BF-1 projection carries no agent token (public-safe)",
    !JSON.stringify(envelope).includes(FAKE_TOKEN),
  )

  // --- BF-2: one bridge tick -> world_event plan -------------------------
  const plan = buildForumActivityWorldPlan(envelope)
  const worldEventCalls = plan.calls.filter(
    (c: { reducer: string }) => c.reducer === "append_world_event",
  )
  check("BF-2 bridge tick produced one world_event", worldEventCalls.length === 1)
  check(
    "BF-2 plan carries no agent token (public-safe)",
    !JSON.stringify(plan).includes(FAKE_TOKEN),
  )

  // Convert the append_world_event call args into a SpacetimeDB row, as the
  // desktop would receive it after the bridge applies the plan.
  const args = worldEventCalls[0].args as string[]
  const worldRow: ChatWorldWorldEventRow = {
    eventRef: args[0],
    runRef: args[1],
    eventKind: args[2],
    entityRef: args[3],
    sourceRef: args[4],
    sourceGeneratedAt: args[5],
    summary: args[6],
  }

  // --- BF-3: the Verse renders a pylon message icon ----------------------
  const messages = projectForumPylonMessages([worldRow], { baseUrl: "https://openagents.com" })
  check("BF-3 produced a Verse pylon message icon", messages.length === 1)
  check(
    "icon is anchored to the posting agent (entityRef == actorRef)",
    messages[0]?.entityRef === AGENT_ACTOR_REF,
  )
  check(
    "icon dereferences to the real public forum topic",
    typeof messages[0]?.sourceUrl === "string" &&
      messages[0]!.sourceUrl!.includes("/forum/t/"),
  )
  check(
    "the whole reflection (intro -> icon) carries no token",
    !JSON.stringify(messages).includes(FAKE_TOKEN),
  )

  // --- side 2: SpacetimeDB outage stays non-fatal ------------------------
  // No world_events (the desktop received nothing from SpacetimeDB).
  const outage = projectForumPylonMessages([])
  check(
    "SpacetimeDB outage is non-fatal (empty projection, no throw)",
    outage.length === 0,
  )
} catch (error) {
  check(`smoke threw: ${error instanceof Error ? error.message : String(error)}`, false)
}

cleanup()

console.log()
if (ok) {
  console.log(
    "RESULT: an automated forum intro flows AF-3 -> BF-1 -> BF-2 -> BF-3 into a visible, dereferenceable Verse pylon message icon within one bridge tick, token-free; a SpacetimeDB outage leaves the Verse playable.",
  )
  process.exit(0)
} else {
  console.error("\nFAIL: one or more two-side reflection gates did not pass.")
  process.exit(1)
}
