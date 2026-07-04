import { describe, expect, test } from "bun:test"

import {
  agentDefinitionWebhookConditionsMatch,
  normalizeForumWebhookEvent,
  normalizeGitHubWebhookEvent,
} from "./webhooks.js"

const githubIssuePayload = {
  action: "opened",
  issue: {
    html_url: "https://github.com/OpenAgentsInc/openagents/issues/8195",
    number: 8195,
    state: "open",
    title: "BA-B3 webhook ingress",
  },
  repository: {
    full_name: "OpenAgentsInc/openagents",
    id: 123,
    name: "openagents",
    owner: {
      id: 456,
      login: "OpenAgentsInc",
    },
  },
  sender: {
    id: 789,
    login: "AtlantisPleb",
  },
}

const githubMentionCommentPayload = {
  action: "created",
  comment: {
    author_association: "MEMBER",
    body:
      "@OpenAgents please run the bounded definition.\n\nPrivate-ish freeform text stays out.",
    html_url:
      "https://github.com/OpenAgentsInc/openagents/issues/8209#issuecomment-1001",
    id: 1001,
    user: {
      id: 790,
      login: "AtlantisPleb",
    },
  },
  issue: {
    html_url: "https://github.com/OpenAgentsInc/openagents/issues/8209",
    number: 8209,
    state: "open",
    title: "BA-G2 GitHub @mention runs",
  },
  repository: {
    full_name: "OpenAgentsInc/openagents",
    id: 123,
    name: "openagents",
    owner: {
      id: 456,
      login: "OpenAgentsInc",
    },
  },
  sender: {
    id: 789,
    login: "AtlantisPleb",
  },
}

const forumPostPayload = {
  actorDisplayName: "OpenAgents Operator",
  actorRef: "user:owner_123",
  actorSlug: "openagents-operator",
  forumId: "forum_product_promises",
  forumSlug: "product-promises",
  forumTitle: "Product Promises",
  postId: "post_forum_trigger_001",
  postNumber: 3,
  postState: "visible",
  sourceUrl:
    "https://openagents.com/forum/t/topic_forum_trigger_001#post_forum_trigger_001",
  topicId: "topic_forum_trigger_001",
  topicSlug: "ship-background-agents",
  topicState: "open",
  topicTitle: "Ship background agents",
}

describe("agent definition webhook normalization", () => {
  test("normalizes GitHub issue events into a typed bounded payload", () => {
    const event = normalizeGitHubWebhookEvent({
      deliveryId: "delivery-123",
      eventName: "issues",
      payload: githubIssuePayload,
      receivedAt: "2026-07-03T16:00:00.000Z",
    })

    expect(event).toMatchObject({
      schema: "openagents.agent_definition_webhook_event.v1",
      source: "github",
      eventType: "issues.opened",
      deliveryId: "delivery-123",
      subjectRef: "github.repository.OpenAgentsInc/openagents.issue.8195",
      payload: {
        action: "opened",
        event: "issues",
        repository: {
          full_name: "OpenAgentsInc/openagents",
          owner: {
            login: "OpenAgentsInc",
          },
        },
        issue: {
          number: 8195,
          state: "open",
        },
      },
    })
    expect(event?.sourceRefs).toContain(
      "github.issue.OpenAgentsInc/openagents.8195",
    )
  })

  test("normalizes GitHub issue-comment mentions without raw comment bodies", () => {
    // background_agents.integrations.github_mention_callback.v1
    const event = normalizeGitHubWebhookEvent({
      deliveryId: "delivery-8209",
      eventName: "issue_comment",
      mentionLogins: ["openagents"],
      payload: githubMentionCommentPayload,
      receivedAt: "2026-07-04T00:00:00.000Z",
    })

    expect(event).toMatchObject({
      schema: "openagents.agent_definition_webhook_event.v1",
      source: "github",
      eventType: "issue_comment.created.mention",
      deliveryId: "delivery-8209",
      subjectRef: "github.repository.OpenAgentsInc/openagents.issue.8209",
      payload: {
        action: "created",
        comment: {
          html_url:
            "https://github.com/OpenAgentsInc/openagents/issues/8209#issuecomment-1001",
          id: 1001,
          user: {
            login: "AtlantisPleb",
          },
        },
        mention: {
          present: true,
          source: "comment",
          target_login: "openagents",
        },
        repository: {
          full_name: "OpenAgentsInc/openagents",
        },
        subject: {
          kind: "issue",
          number: 8209,
        },
      },
      sourceRefs: [
        "github.delivery.delivery-8209",
        "github.repository.OpenAgentsInc/openagents",
        "github.issue.OpenAgentsInc/openagents.8209",
        "github.comment.OpenAgentsInc/openagents.1001",
      ],
    })
    expect(JSON.stringify(event?.payload)).not.toContain(
      "Private-ish freeform text stays out",
    )
  })

  test("keeps unmentioned GitHub issue comments out of mention triggers", () => {
    const event = normalizeGitHubWebhookEvent({
      deliveryId: "delivery-8210",
      eventName: "issue_comment",
      mentionLogins: ["openagents"],
      payload: {
        ...githubMentionCommentPayload,
        comment: {
          ...githubMentionCommentPayload.comment,
          body: "ordinary discussion comment",
          id: 1002,
        },
      },
      receivedAt: "2026-07-04T00:05:00.000Z",
    })

    expect(event?.eventType).toBe("issue_comment.created")
    expect(agentDefinitionWebhookConditionsMatch(event!, [
      {
        kind: "event_type",
        equals: "issue_comment.created.mention",
      },
    ])).toBe(false)
  })

  test("evaluates event type and bounded JSON-path conditions", () => {
    const event = normalizeGitHubWebhookEvent({
      deliveryId: "delivery-123",
      eventName: "issues",
      payload: githubIssuePayload,
      receivedAt: "2026-07-03T16:00:00.000Z",
    })

    expect(event).toBeDefined()
    expect(agentDefinitionWebhookConditionsMatch(event!, [
      {
        kind: "event_type",
        equals: "issues.opened",
      },
      {
        kind: "json_path_equals",
        path: "$.repository.full_name",
        equals: "OpenAgentsInc/openagents",
      },
      {
        kind: "json_path_in",
        path: "$.issue.state",
        values: ["open", "reopened"],
      },
      {
        kind: "json_path_matches",
        path: "$.sender.login",
        pattern: "^Atlantis",
      },
    ])).toBe(true)
    expect(agentDefinitionWebhookConditionsMatch(event!, [
      {
        kind: "json_path_equals",
        path: "$.repository.full_name",
        equals: "OpenAgentsInc/other",
      },
    ])).toBe(false)
  })

  test("normalizes Forum post events into bounded public source refs", () => {
    // background_agents.integrations.forum_trigger_callback.v1
    const event = normalizeForumWebhookEvent({
      deliveryId: "forum-delivery-123",
      eventType: "forum.post.created",
      payload: forumPostPayload,
      receivedAt: "2026-07-03T17:00:00.000Z",
    })

    expect(event).toMatchObject({
      schema: "openagents.agent_definition_webhook_event.v1",
      source: "forum",
      eventType: "forum.post.created",
      deliveryId: "forum-delivery-123",
      subjectRef:
        "forum.topic.topic_forum_trigger_001.post.post_forum_trigger_001",
      payload: {
        event: "forum.post.created",
        actor: {
          ref: "user:owner_123",
        },
        forum: {
          id: "forum_product_promises",
          slug: "product-promises",
        },
        post: {
          id: "post_forum_trigger_001",
          number: 3,
          state: "visible",
        },
        topic: {
          id: "topic_forum_trigger_001",
          title: "Ship background agents",
        },
      },
    })
    expect(event?.sourceRefs).toEqual([
      "forum.delivery.forum-delivery-123",
      "forum.forum.forum_product_promises",
      "forum.slug.product-promises",
      "forum.topic.topic_forum_trigger_001",
      "forum.post.post_forum_trigger_001",
    ])
    expect(agentDefinitionWebhookConditionsMatch(event!, [
      {
        kind: "event_type",
        equals: "forum.post.created",
      },
      {
        kind: "json_path_equals",
        path: "$.forum.slug",
        equals: "product-promises",
      },
      {
        kind: "json_path_matches",
        path: "$.topic.title",
        pattern: "background agents",
      },
    ])).toBe(true)
  })
})
