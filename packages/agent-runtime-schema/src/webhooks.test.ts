import { describe, expect, test } from "bun:test"

import {
  agentDefinitionWebhookConditionsMatch,
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
})
