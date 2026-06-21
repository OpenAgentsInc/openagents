import { describe, expect, it } from "bun:test"
import { mkdtempSync, rmSync, writeFileSync } from "node:fs"
import { tmpdir } from "node:os"
import { join } from "node:path"
import { persistCredential, type NodeIdentity } from "../src/bun/agent-onboarding"
import {
  composeIntroPost,
  hasPostedForumIntro,
  loadIntroReceipt,
  postForumIntroduction,
  selectIntroForumSlug,
  type IntroFetch,
} from "../src/bun/forum-intro"

const NPUB =
  "npub1examplepubkey000000000000000000000000000000000000000000000abc"
const identity: NodeIdentity = {
  npub: NPUB,
  nodeLabel: "Studio Mac",
  pylonRef: "pylon.abc123",
}

const seedHome = (opts: { credential?: boolean; identity?: boolean } = {}) => {
  const home = mkdtempSync(join(tmpdir(), "fi-"))
  if (opts.identity !== false) {
    writeFileSync(
      join(home, "identity.json"),
      JSON.stringify({ npub: NPUB, nodeLabel: "Studio Mac" }),
    )
  }
  if (opts.credential !== false) {
    persistCredential(home, {
      token: "oa_agent_introToken123",
      tokenPrefix: "oa_agent_int",
      userId: "u1",
      externalId: NPUB,
      registeredAt: "2026-06-21T00:00:00.000Z",
    })
  }
  return home
}

const board = (slugs: Array<{ slug: string; locked?: boolean }>) => ({
  forums: slugs.map(s => ({ slug: s.slug, title: s.slug, locked: s.locked ?? false })),
})

const okTopicResponse = (topicId = "topic_1", postId = "post_1") =>
  ({
    status: 200,
    json: async () => ({
      topic: { id: topicId, slug: "intro-slug" },
      firstPost: { id: postId },
      idempotent: false,
    }),
  }) as const

describe("selectIntroForumSlug (AF-3 typed lane selection)", () => {
  it("prefers the highest-priority canonical lane present (exact match)", () => {
    expect(
      selectIntroForumSlug(
        board([{ slug: "general" }, { slug: "introductions" }, { slug: "agents" }]),
      ),
    ).toBe("introductions")
  })

  it("falls through the priority order to the next present lane", () => {
    expect(
      selectIntroForumSlug(board([{ slug: "general" }, { slug: "agents" }])),
    ).toBe("agents")
  })

  it("excludes the feedback-only release-candidates lane", () => {
    expect(
      selectIntroForumSlug(
        board([{ slug: "release-candidates" }, { slug: "show-and-tell" }]),
      ),
    ).toBe("show-and-tell")
  })

  it("skips locked forums and falls back to the first writable public forum", () => {
    expect(
      selectIntroForumSlug(
        board([
          { slug: "announcements", locked: true },
          { slug: "lounge" },
        ]),
      ),
    ).toBe("lounge")
  })

  it("returns null when there is no writable lane", () => {
    expect(selectIntroForumSlug(board([{ slug: "x", locked: true }]))).toBeNull()
    expect(selectIntroForumSlug({ forums: [] })).toBeNull()
  })
})

describe("composeIntroPost (AF-3 honest copy)", () => {
  it("produces honest, public-safe copy with no filler and within length bounds", () => {
    const { title, bodyText } = composeIntroPost(identity)
    expect(title.length).toBeGreaterThanOrEqual(3)
    expect(title.length).toBeLessThanOrEqual(160)
    expect(bodyText).toContain("on behalf of my owner")
    expect(bodyText).toContain("cannot spend money")
    expect(bodyText).toContain("Bitcoin-earning work")
    // No "help with anything" filler (explicitly called bad in AGENTS.md).
    expect(bodyText.toLowerCase()).not.toContain("anything")
    // Never leaks identity material beyond the public npub suffix.
    expect(bodyText).not.toContain(NPUB)
  })

  it("mentions tip-readiness only when tips are claimable", () => {
    expect(composeIntroPost(identity, { tipReady: false }).bodyText).not.toContain(
      "receive Bitcoin tips",
    )
    expect(composeIntroPost(identity, { tipReady: true }).bodyText).toContain(
      "receive Bitcoin tips",
    )
  })
})

describe("postForumIntroduction (AF-3)", () => {
  it("is not_registered until an agent credential is persisted", async () => {
    const home = seedHome({ credential: false })
    try {
      const res = await postForumIntroduction({
        home,
        fetchImpl: (async () => okTopicResponse()) as IntroFetch,
      })
      expect(res.outcome).toBe("not_registered")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("reads the board, posts to the resolved lane with bearer + idempotency key, persists a receipt", async () => {
    const home = seedHome()
    try {
      const calls: Array<{ url: string; method: string; headers: Record<string, string>; body?: string }> = []
      const res = await postForumIntroduction({
        home,
        baseUrl: "https://openagents.com",
        fetchImpl: (async (url, init) => {
          calls.push({ url, method: init.method, headers: init.headers, body: init.body })
          if (init.method === "GET") {
            return {
              status: 200,
              json: async () => board([{ slug: "general" }, { slug: "introductions" }]),
            }
          }
          return okTopicResponse("topic_42", "post_42")
        }) as IntroFetch,
      })
      expect(res.outcome).toBe("posted")
      // GET board first, then POST to the resolved intro lane.
      expect(calls[0]?.method).toBe("GET")
      expect(calls[0]?.url).toContain("/api/forum")
      const post = calls.find(c => c.method === "POST")!
      expect(post.url).toContain("/api/forum/forums/introductions/topics")
      expect(post.headers.authorization).toBe("Bearer oa_agent_introToken123")
      expect(post.headers["idempotency-key"]).toBeTruthy()
      const sentBody = JSON.parse(post.body ?? "{}") as Record<string, unknown>
      expect(typeof sentBody.title).toBe("string")
      expect(typeof sentBody.bodyText).toBe("string")
      // Receipt persisted with a dereferenceable URL.
      const receipt = loadIntroReceipt(home)
      expect(receipt?.topicId).toBe("topic_42")
      expect(receipt?.url).toBe("https://openagents.com/forum/t/topic_42")
      expect(hasPostedForumIntro(home)).toBe(true)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("is idempotent: a persisted receipt short-circuits without any network call", async () => {
    const home = seedHome()
    try {
      await postForumIntroduction({
        home,
        fetchImpl: (async (_url, init) =>
          init.method === "GET"
            ? { status: 200, json: async () => board([{ slug: "introductions" }]) }
            : okTopicResponse()) as IntroFetch,
      })
      const res = await postForumIntroduction({
        home,
        fetchImpl: (async () => {
          throw new Error("must not re-post")
        }) as IntroFetch,
      })
      expect(res.outcome).toBe("reused")
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("defers when no writable forum lane is available", async () => {
    const home = seedHome()
    try {
      const res = await postForumIntroduction({
        home,
        fetchImpl: (async () => ({
          status: 200,
          json: async () => board([{ slug: "locked-only", locked: true }]),
        })) as IntroFetch,
      })
      expect(res.outcome).toBe("no_forum")
      expect(hasPostedForumIntro(home)).toBe(false)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })

  it("is offline-tolerant and never logs the agent token", async () => {
    const home = seedHome()
    try {
      const logs: string[] = []
      const res = await postForumIntroduction({
        home,
        log: m => logs.push(m),
        fetchImpl: (async () => {
          throw new Error("offline")
        }) as IntroFetch,
      })
      expect(res.outcome).toBe("deferred")
      expect(logs.join("\n")).not.toContain("oa_agent_introToken123")
      expect(hasPostedForumIntro(home)).toBe(false)
    } finally {
      rmSync(home, { recursive: true, force: true })
    }
  })
})
