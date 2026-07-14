import { describe, expect, test } from "vite-plus/test"

import {
  applyRuntimeInteractionDecision,
  decodeRuntimeInteraction,
  decodeRuntimeInteractionDecisionEnvelope,
  projectRuntimeInteraction,
  type RuntimeInteraction,
  type RuntimeInteractionPayload,
} from "./index.js"

const requestedAt = "2026-07-11T22:00:00.000Z"
const expiresAt = "2026-07-11T22:05:00.000Z"

const pending = (payload: RuntimeInteractionPayload): RuntimeInteraction =>
  decodeRuntimeInteraction({
    schema: "openagents.runtime_interaction.v1",
    interactionRef: `interaction.${payload.kind}.1`,
    threadId: "thread.runtime.1",
    turnId: "turn.runtime.1",
    requestedSequence: 7,
    requestedAt,
    expiresAt,
    source: {
      lane: "claude_pylon",
      adapterKind: "claude_code",
      surface: "server",
      providerRef: "provider.anthropic",
      modelRef: "model.claude",
    },
    visibility: "private",
    redactionClass: "private_ref",
    causalityRefs: ["event.runtime.6"],
    payload,
    lifecycle: { status: "pending" },
  })

const questionPayload: RuntimeInteractionPayload = {
  kind: "provider_question",
  displayTitle: "Choose a verification path",
  questions: [
    {
      questionRef: "question.runtime.1",
      displayText: "Which verification should run?",
      multiSelect: false,
      options: [
        { optionRef: "option.tests", label: "Tests", description: "Run the focused suite" },
        { optionRef: "option.smoke", label: "Smoke", description: "Run the host receipt" },
      ],
    },
  ],
}

const envelope = (decision: unknown, input: Readonly<{
  decisionRef?: string
  idempotencyKey?: string
}> = {}) => decodeRuntimeInteractionDecisionEnvelope({
  decisionRef: input.decisionRef ?? "decision.runtime.1",
  idempotencyKey: input.idempotencyKey ?? "idem.decision.runtime.1",
  decidedAt: "2026-07-11T22:01:00.000Z",
  surface: "mobile",
  decision,
})

describe("openagents.runtime_interaction.v1", () => {
  test("decodes and projects provider questions, tool approvals, and plan reviews", () => {
    const authority = {
      authorityRef: "authority.tool.1",
      policyRef: "policy.tool.1",
      decisionRef: "decision.tool.pending.1",
      toolRef: "tool.workspace.write",
      status: "operator_escalation_required" as const,
      allowed: false,
      blockerRefs: ["blocker.owner_approval"],
    }
    const interactions = [
      pending(questionPayload),
      pending({
        kind: "tool_approval",
        displayText: "Allow the agent to update the selected workspace?",
        toolCallId: "tool_call.runtime.1",
        toolName: "workspaceWrite",
        authority,
      }),
      pending({
        kind: "plan_review",
        displayText: "Review the proposed three-step implementation plan.",
        planRef: "plan.runtime.1",
      }),
    ]
    expect(interactions.map(interaction => interaction.payload.kind)).toEqual([
      "provider_question",
      "tool_approval",
      "plan_review",
    ])
    expect(interactions.map(projectRuntimeInteraction)).toMatchObject([
      {
        kind: "provider_question",
        status: "pending",
        displayTitle: "Choose a verification path",
        questions: [{
          questionRef: "question.runtime.1",
          options: [{ optionRef: "option.tests" }, { optionRef: "option.smoke" }],
        }],
      },
      {
        kind: "tool_approval",
        status: "pending",
        displayTitle: "Approve workspaceWrite",
        questions: [],
      },
      {
        kind: "plan_review",
        status: "pending",
        displayTitle: "Review plan",
        questions: [],
      },
    ])
  })

  test("applies one exact question decision, deduplicates its retry, and rejects conflicting reuse", () => {
    const interaction = pending(questionPayload)
    const answer = envelope({
      kind: "provider_question",
      answers: [{ questionRef: "question.runtime.1", optionRefs: ["option.tests"] }],
    })
    const applied = applyRuntimeInteractionDecision(
      interaction,
      answer,
      "2026-07-11T22:01:00.000Z",
    )
    expect(applied.state).toBe("applied")
    expect(applied.interaction.lifecycle.status).toBe("resolved")
    expect(projectRuntimeInteraction(applied.interaction)).toMatchObject({
      status: "resolved",
      decisionRef: "decision.runtime.1",
    })

    expect(applyRuntimeInteractionDecision(
      applied.interaction,
      answer,
      "2026-07-11T22:02:00.000Z",
    ).state).toBe("duplicate")
    expect(applyRuntimeInteractionDecision(
      applied.interaction,
      envelope({
        kind: "provider_question",
        answers: [{ questionRef: "question.runtime.1", optionRefs: ["option.smoke"] }],
      }),
      "2026-07-11T22:02:00.000Z",
    ).state).toBe("conflict")
  })

  test("rejects wrong-kind, unknown-option, incomplete, and invalid multi-select decisions", () => {
    const interaction = pending(questionPayload)
    const invalid = [
      envelope({ kind: "tool_approval", outcome: "approve" }),
      envelope({
        kind: "provider_question",
        answers: [{ questionRef: "question.runtime.1", optionRefs: ["option.unknown"] }],
      }),
      envelope({
        kind: "provider_question",
        answers: [{ questionRef: "question.runtime.1", optionRefs: [] }],
      }),
      envelope({
        kind: "provider_question",
        answers: [{
          questionRef: "question.runtime.1",
          optionRefs: ["option.tests", "option.smoke"],
        }],
      }),
    ]
    expect(invalid.map(decision => applyRuntimeInteractionDecision(
      interaction,
      decision,
      "2026-07-11T22:01:00.000Z",
    ).state)).toEqual([
      "invalid_decision",
      "invalid_decision",
      "invalid_decision",
      "invalid_decision",
    ])
  })

  test("expires from server time and refuses both expired and revoked decisions", () => {
    const interaction = pending(questionPayload)
    const answer = envelope({
      kind: "provider_question",
      answers: [{ questionRef: "question.runtime.1", optionRefs: ["option.tests"] }],
    })
    const late = applyRuntimeInteractionDecision(
      interaction,
      answer,
      "2026-07-11T22:05:00.000Z",
    )
    expect(late.state).toBe("expired")
    expect(late.interaction.lifecycle).toEqual({
      status: "expired",
      terminalAt: "2026-07-11T22:05:00.000Z",
      reasonRef: "reason.interaction_deadline_elapsed",
    })
    expect(applyRuntimeInteractionDecision(
      late.interaction,
      answer,
      "2026-07-11T22:06:00.000Z",
    ).state).toBe("expired")

    const revoked = decodeRuntimeInteraction({
      ...interaction,
      lifecycle: {
        status: "revoked",
        terminalAt: "2026-07-11T22:02:00.000Z",
        reasonRef: "reason.owner_authority_revoked",
      },
    })
    expect(applyRuntimeInteractionDecision(
      revoked,
      answer,
      "2026-07-11T22:03:00.000Z",
    ).state).toBe("revoked")
  })

  test("bounds display-safe content and requires private visibility", () => {
    const interaction = pending(questionPayload)
    expect(() => decodeRuntimeInteraction({
      ...interaction,
      visibility: "public",
    })).toThrow()
    expect(() => decodeRuntimeInteraction({
      ...interaction,
      payload: {
        ...questionPayload,
        questions: [{
          ...questionPayload.questions[0],
          displayText: "x".repeat(2_001),
        }],
      },
    })).toThrow()
  })
})
