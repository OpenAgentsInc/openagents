import { describe, expect, test } from "bun:test"

import {
  ConnectorSourceVerifiedEvent,
  connectorEventHasRawProviderMaterial,
  createConnectorDeliveryDedupeKey,
  createGitHubWebhookSignature,
  decideConnectorWritebackToolAuthority,
  normalizeGitHubWebhookEvent,
  projectConnectorEventToWorkspaceLane,
} from "./index.js"

const rawIssueBody = JSON.stringify({
  action: "opened",
  repository: {
    full_name: "OpenAgentsInc/openagents",
  },
  issue: {
    number: 8100,
    html_url: "https://github.com/OpenAgentsInc/openagents/issues/8100",
    title: "BF-6.1 connector sidecar",
    body: "This raw body must not leave the ingress normalizer.",
  },
})

const headersFor = (rawBody: string) => ({
  event: "issues",
  delivery: "delivery-public-8100",
  signature256: createGitHubWebhookSignature("fixture-secret", rawBody),
})

describe("@openagentsinc/connector-sidecar", () => {
  test("normalizes a signed GitHub issue webhook into a bounded source-verified event", () => {
    const result = normalizeGitHubWebhookEvent({
      headers: headersFor(rawIssueBody),
      rawBody: rawIssueBody,
      webhookSecret: "fixture-secret",
      receivedAt: "2026-07-02T00:00:00.000Z",
    })

    expect(result.ok).toBe(true)
    if (!result.ok) {
      throw new Error(result.reasonRef)
    }

    expect(result.event).toMatchObject({
      schema: "openagents.connector_sidecar.v1",
      provider: "github",
      eventKind: "github.issue",
      sourceVerified: true,
      deliveryId: "delivery-public-8100",
      dedupeKey: "connector.github.delivery.delivery-public-8100",
      subject: {
        kind: "issue",
        owner: "OpenAgentsInc",
        repo: "openagents",
        number: 8100,
      },
    })
    expect(() => ConnectorSourceVerifiedEvent.make(result.event)).not.toThrow()
    expect(connectorEventHasRawProviderMaterial(result.event)).toBe(false)
    expect(JSON.stringify(result.event)).not.toContain("This raw body")
  })

  test("rejects unsigned or tampered GitHub webhook bodies", () => {
    const result = normalizeGitHubWebhookEvent({
      headers: headersFor(rawIssueBody),
      rawBody: rawIssueBody.replace("opened", "closed"),
      webhookSecret: "fixture-secret",
      receivedAt: "2026-07-02T00:00:00.000Z",
    })

    expect(result).toMatchObject({
      ok: false,
      reasonRef: "reason.connector.github.bad_signature",
      blockerRefs: ["blocker.connector.github.bad_signature"],
    })
  })

  test("uses app-owned delivery idempotency keys", () => {
    expect(
      createConnectorDeliveryDedupeKey({
        provider: "github",
        deliveryId: "delivery-public-8100",
      }),
    ).toBe("connector.github.delivery.delivery-public-8100")
  })

  test("projects source-verified issue events to a workspace lane with issue writeback only", () => {
    const result = normalizeGitHubWebhookEvent({
      headers: headersFor(rawIssueBody),
      rawBody: rawIssueBody,
      webhookSecret: "fixture-secret",
      receivedAt: "2026-07-02T00:00:00.000Z",
    })
    if (!result.ok) {
      throw new Error(result.reasonRef)
    }

    expect(projectConnectorEventToWorkspaceLane(result.event)).toMatchObject({
      laneRef: "workspace_lane.connector.github.openagentsinc.openagents.issue.8100",
      provider: "github",
      allowedWritebackToolRefs: ["tool.connector.github.issue.comment.create"],
      blockerRefs: [],
    })
  })

  test("authorizes only same-issue bounded writeback tools", () => {
    const result = normalizeGitHubWebhookEvent({
      headers: headersFor(rawIssueBody),
      rawBody: rawIssueBody,
      webhookSecret: "fixture-secret",
      receivedAt: "2026-07-02T00:00:00.000Z",
    })
    if (!result.ok) {
      throw new Error(result.reasonRef)
    }

    expect(
      decideConnectorWritebackToolAuthority({
        event: result.event,
        request: {
          provider: "github",
          toolRef: "tool.connector.github.issue.comment.create",
          owner: "OpenAgentsInc",
          repo: "openagents",
          subjectKind: "issue",
          number: 8100,
        },
      }),
    ).toMatchObject({
      allowed: true,
      status: "allowed",
      blockerRefs: [],
    })

    expect(
      decideConnectorWritebackToolAuthority({
        event: result.event,
        request: {
          provider: "github",
          toolRef: "tool.connector.github.issue.comment.create",
          owner: "OpenAgentsInc",
          repo: "openagents",
          subjectKind: "issue",
          number: 8101,
        },
      }),
    ).toMatchObject({
      allowed: false,
      reasonRef: "reason.connector.writeback_subject_mismatch",
    })

    expect(
      decideConnectorWritebackToolAuthority({
        event: result.event,
        request: {
          provider: "github",
          toolRef: "tool.connector.github.payment.refund",
          owner: "OpenAgentsInc",
          repo: "openagents",
          subjectKind: "issue",
          number: 8100,
        },
      }),
    ).toMatchObject({
      allowed: false,
      reasonRef: "reason.connector.tool_forbidden_authority",
    })
  })
})
