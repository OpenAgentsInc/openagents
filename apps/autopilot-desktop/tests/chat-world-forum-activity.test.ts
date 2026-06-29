import { describe, expect, it } from "bun:test"
import {
  FORUM_ACTIVITY_WORLD_RUN_REF,
  forumMessagesByEntityRef,
  projectForumPylonMessages,
  withForumPylonMessages,
  type ChatWorldWorldEventRow,
} from "../src/shared/chat-world-forum-activity"
import {
  PUBLIC_FORUM_ACTIVITY_WORLD_RUN_REF,
  chatWorldMultiplayerSubscriptionQueries,
} from "../src/shared/chat-world-multiplayer"

const summary = (extra: Record<string, unknown>) =>
  JSON.stringify({
    schema: "openagents.world.forum_activity_event_summary.v1",
    text: "Posted: Introducing my node",
    topicRef: "topic_1",
    ...extra,
  })

const row = (over: Partial<ChatWorldWorldEventRow> = {}): ChatWorldWorldEventRow => ({
  eventRef: "world_event.forum_activity.abc",
  runRef: FORUM_ACTIVITY_WORLD_RUN_REF,
  eventKind: "forum_post",
  entityRef: "agent:autopilot_abc",
  sourceRef: "topic_1",
  sourceGeneratedAt: "2026-06-21T18:00:00.000Z",
  summary: summary({}),
  ...over,
})

describe("projectForumPylonMessages (BF-3)", () => {
  it("maps a forum_post into a message with a dereferenceable topic URL", () => {
    const [message] = projectForumPylonMessages([row()])
    expect(message?.entityRef).toBe("agent:autopilot_abc")
    expect(message?.eventKind).toBe("forum_post")
    expect(message?.summary).toContain("Introducing")
    expect(message?.topicRef).toBe("topic_1")
    expect(message?.sourceUrl).toBe("https://openagents.com/forum/t/topic_1")
  })

  it("deep-links a forum_reply to the specific post", () => {
    const [message] = projectForumPylonMessages([
      row({
        eventKind: "forum_reply",
        sourceRef: "post_9",
        summary: summary({ text: "Replied: welcome", topicRef: "topic_1" }),
      }),
    ])
    expect(message?.sourceUrl).toBe(
      "https://openagents.com/forum/t/topic_1#post-post_9",
    )
  })

  it("ignores non-forum world_event kinds", () => {
    expect(
      projectForumPylonMessages([
        row({ eventKind: "tassadar_tick", entityRef: "x" }),
      ]).length,
    ).toBe(0)
  })

  it("keeps the most recent forum event per entity", () => {
    const messages = projectForumPylonMessages([
      row({ sourceGeneratedAt: "2026-06-21T10:00:00.000Z", sourceRef: "old" }),
      row({ sourceGeneratedAt: "2026-06-21T20:00:00.000Z", sourceRef: "new" }),
    ])
    expect(messages.length).toBe(1)
    expect(messages[0]?.sourceRef).toBe("new")
  })

  it("falls back to raw summary text and no URL when topicRef is missing", () => {
    const [message] = projectForumPylonMessages([
      row({ summary: "plain text summary" }),
    ])
    expect(message?.summary).toBe("plain text summary")
    expect(message?.topicRef).toBeNull()
    expect(message?.sourceUrl).toBeNull()
  })

  it("respects a custom base URL", () => {
    const [message] = projectForumPylonMessages([row()], {
      baseUrl: "https://staging.openagents.com/",
    })
    expect(message?.sourceUrl).toBe("https://staging.openagents.com/forum/t/topic_1")
  })
})

describe("withForumPylonMessages (BF-3 attach)", () => {
  it("attaches a forumMessage to the entity whose actorRef matches", () => {
    const byRef = forumMessagesByEntityRef(projectForumPylonMessages([row()]))
    const avatars = [
      { id: "a1", actorRef: "agent:autopilot_abc" },
      { id: "a2", actorRef: "agent:someone_else" },
    ]
    const marked = withForumPylonMessages(avatars, byRef)
    expect(marked[0]?.forumMessage?.eventKind).toBe("forum_post")
    expect(marked[1]?.forumMessage).toBeUndefined()
  })

  it("leaves entities without actorRef untouched", () => {
    const byRef = forumMessagesByEntityRef(projectForumPylonMessages([row()]))
    const marked = withForumPylonMessages([{ id: "s1" }], byRef)
    expect(marked[0]).toEqual({ id: "s1" })
  })
})

describe("forum world_event subscription (BF-3)", () => {
  it("subscribes to the forum activity run ref", () => {
    const queries = chatWorldMultiplayerSubscriptionQueries("run.tassadar.x")
    expect(
      queries.some(q => q.includes(PUBLIC_FORUM_ACTIVITY_WORLD_RUN_REF)),
    ).toBe(true)
    expect(PUBLIC_FORUM_ACTIVITY_WORLD_RUN_REF).toBe(FORUM_ACTIVITY_WORLD_RUN_REF)
  })
})
