import { describe, expect, test } from "bun:test"
import type { DesktopRuntimeGatewayEvent, DesktopRuntimeGatewayResponse, RuntimeInteractionDecisionEnvelope } from "../runtime-gateway-contract.ts"
import { makeDesktopRuntimeInteractionHost } from "./runtime-interactions.ts"

const envelope: RuntimeInteractionDecisionEnvelope = {
  decisionRef: "decision.desktop.1",
  idempotencyKey: "idem.desktop.1",
  decidedAt: "2026-07-11T22:01:00.000Z",
  surface: "desktop",
  decision: { kind: "tool_approval", outcome: "approve" },
}

const interaction = (status: "pending" | "resolved" | "expired" | "revoked", decisionRef?: string) => ({
  schema: "openagents.runtime_interaction_projection.v1" as const,
  interactionRef: "interaction.tool.1",
  threadId: "thread.runtime.1",
  turnId: "turn.runtime.1",
  kind: "tool_approval" as const,
  status,
  displayTitle: "Approve workspaceWrite",
  displayText: "Allow the workspace update?",
  questions: [],
  expiresAt: "2026-07-11T22:05:00.000Z",
  ...(decisionRef === undefined ? {} : { decisionRef }),
  requestedSequence: 7,
  requestedAt: "2026-07-11T22:00:00.000Z",
  version: status === "pending" ? 1 : 2,
})

describe("Desktop canonical runtime interaction host", () => {
  test("sends exact refs and waits for the confirmed matching decision", async () => {
    let current = interaction("pending")
    let listener: ((event: DesktopRuntimeGatewayEvent) => void) | undefined
    const requests: Array<Record<string, any>> = []
    const host = makeDesktopRuntimeInteractionHost({
      request: async value => {
        const request = value as Record<string, any>
        requests.push(request)
        if (request.kind === "command") {
          return {
            kind: "runtime_interaction_decision_outcome",
            commandId: request.commandId,
            interactionRef: request.command.interactionRef,
            threadRef: request.command.threadRef,
            turnRef: request.command.turnRef,
            status: "pending_reconcile",
            mutationId: 9,
          } as DesktopRuntimeGatewayResponse
        }
        return {
          kind: "runtime_interactions",
          requestId: request.requestId,
          threadRef: "thread.runtime.1",
          interactions: [current],
        } as DesktopRuntimeGatewayResponse
      },
      subscribe: next => { listener = next; return () => { listener = undefined } },
      confirmationTimeoutMs: 100,
    })

    const decision = host.decide({
      interactionRef: "interaction.tool.1",
      threadRef: "thread.runtime.1",
      turnRef: "turn.runtime.1",
      envelope,
    })
    await new Promise(resolve => setTimeout(resolve, 0))
    current = interaction("resolved", envelope.decisionRef)
    listener?.({ kind: "runtime.lifecycle", protocolVersion: 9, sequence: 1, phase: "ready" } as DesktopRuntimeGatewayEvent)

    expect(await decision).toMatchObject({ status: "confirmed_resolved", interaction: { version: 2 } })
    expect(requests.find(request => request.kind === "command")?.command).toEqual({
      id: "runtime.decideInteraction",
      interactionRef: "interaction.tool.1",
      threadRef: "thread.runtime.1",
      turnRef: "turn.runtime.1",
      envelope,
    })
  })

  test("does not call an enqueue or a different decision confirmed", async () => {
    let reads = 0
    const host = makeDesktopRuntimeInteractionHost({
      request: async value => {
        const request = value as Record<string, any>
        if (request.kind === "command") return {
          kind: "runtime_interaction_decision_outcome",
          commandId: request.commandId,
          interactionRef: "interaction.tool.1",
          threadRef: "thread.runtime.1",
          turnRef: "turn.runtime.1",
          status: "pending_reconcile",
          mutationId: 10,
        } as DesktopRuntimeGatewayResponse
        reads += 1
        return {
          kind: "runtime_interactions",
          requestId: request.requestId,
          threadRef: "thread.runtime.1",
          interactions: [reads === 1 ? interaction("pending") : interaction("resolved", "decision.other")],
        } as DesktopRuntimeGatewayResponse
      },
    })
    expect(await host.decide({
      interactionRef: "interaction.tool.1",
      threadRef: "thread.runtime.1",
      turnRef: "turn.runtime.1",
      envelope,
    })).toEqual({ status: "pending_reconcile" })
  })

  test("reports confirmed expiry as terminal without claiming resolution", async () => {
    const host = makeDesktopRuntimeInteractionHost({
      request: async value => ({
        kind: "runtime_interactions",
        requestId: (value as any).requestId,
        threadRef: "thread.runtime.1",
        interactions: [interaction("expired")],
      }) as DesktopRuntimeGatewayResponse,
    })
    expect(await host.decide({
      interactionRef: "interaction.tool.1",
      threadRef: "thread.runtime.1",
      turnRef: "turn.runtime.1",
      envelope,
    })).toMatchObject({ status: "confirmed_expired" })
  })
})
