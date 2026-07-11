import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  PylonRuntimeInteractionBridgeError,
  requestPylonRuntimeInteraction,
  type PylonRuntimeInteractionAuthority,
  type PylonRuntimeInteractionRequest,
} from "./runtime-interaction-bridge.js"

const request: PylonRuntimeInteractionRequest = {
  interactionRef: "interaction.pylon.1",
  threadRef: "thread.pylon.1",
  turnRef: "turn.pylon.1",
  requestedSequence: 4,
  requestedAt: "2026-07-11T23:00:00.000Z",
  expiresAt: "2026-07-11T23:05:00.000Z",
  source: {
    lane: "claude_pylon",
    adapterKind: "claude_code",
    surface: "server",
    providerRef: "provider.anthropic",
  },
  causalityRefs: ["event.pylon.3"],
  payload: {
    kind: "tool_approval",
    displayText: "Allow the bounded workspace edit?",
    toolCallId: "tool_call.pylon.1",
    toolName: "workspaceWrite",
    authority: {
      authorityRef: "authority.pylon.1",
      policyRef: "policy.pylon.supervised",
      decisionRef: "decision.pylon.pending",
      toolRef: "tool.workspace.write",
      status: "operator_escalation_required",
      allowed: false,
      blockerRefs: ["blocker.pylon.owner_approval"],
    },
  },
}

const terminal = (lifecycle: unknown, input: Partial<PylonRuntimeInteractionRequest> = {}) => ({
  schema: "openagents.runtime_interaction.v1",
  interactionRef: input.interactionRef ?? request.interactionRef,
  threadId: input.threadRef ?? request.threadRef,
  turnId: input.turnRef ?? request.turnRef,
  requestedSequence: request.requestedSequence,
  requestedAt: request.requestedAt,
  expiresAt: request.expiresAt,
  source: request.source,
  visibility: "private",
  redactionClass: "private_ref",
  causalityRefs: request.causalityRefs,
  payload: input.payload ?? request.payload,
  lifecycle,
})

describe("Pylon runtime interaction bridge", () => {
  test("requests canonical pending authority and returns one exact resolved decision", async () => {
    const requested: unknown[] = []
    const authority: PylonRuntimeInteractionAuthority = {
      request: interaction => Effect.sync(() => { requested.push(interaction) }),
      awaitTerminal: () => Effect.succeed(terminal({
        status: "resolved",
        envelope: {
          decisionRef: "decision.pylon.1",
          idempotencyKey: "idem.decision.pylon.1",
          decidedAt: "2026-07-11T23:01:00.000Z",
          surface: "desktop",
          decision: { kind: "tool_approval", outcome: "approve" },
        },
      })),
    }
    expect(await Effect.runPromise(requestPylonRuntimeInteraction(authority, request))).toEqual({
      state: "resolved",
      interactionRef: request.interactionRef,
      decisionRef: "decision.pylon.1",
      decision: { kind: "tool_approval", outcome: "approve" },
    })
    expect(requested).toEqual([expect.objectContaining({
      interactionRef: request.interactionRef,
      visibility: "private",
      lifecycle: { status: "pending" },
      payload: request.payload,
    })])
  })

  test("returns expired and revoked outcomes without inventing a provider decision", async () => {
    for (const state of ["expired", "revoked"] as const) {
      const authority: PylonRuntimeInteractionAuthority = {
        request: () => Effect.void,
        awaitTerminal: () => Effect.succeed(terminal({
          status: state,
          terminalAt: "2026-07-11T23:05:00.000Z",
          reasonRef: `reason.pylon.${state}`,
        })),
      }
      expect(await Effect.runPromise(requestPylonRuntimeInteraction(authority, request))).toEqual({
        state,
        interactionRef: request.interactionRef,
        terminalAt: "2026-07-11T23:05:00.000Z",
        reasonRef: `reason.pylon.${state}`,
      })
    }
  })

  test("fails public-safe on pending, foreign, malformed, or kind-mismatched terminal rows", async () => {
    const cases: ReadonlyArray<readonly [unknown, PylonRuntimeInteractionBridgeError["reason"]]> = [
      [terminal({ status: "pending" }), "not_terminal"],
      [terminal({ status: "expired", terminalAt: request.expiresAt, reasonRef: "reason.expired" }, { turnRef: "turn.foreign" }), "identity_mismatch"],
      [{ malformed: true }, "invalid_terminal"],
      [terminal({ status: "resolved", envelope: { decisionRef: "decision.bad", idempotencyKey: "idem.bad", decidedAt: request.requestedAt, surface: "desktop", decision: { kind: "plan_review", outcome: "accept" } } }), "kind_mismatch"],
    ]
    for (const [value, reason] of cases) {
      const error = await Effect.runPromise(Effect.flip(requestPylonRuntimeInteraction({
        request: () => Effect.void,
        awaitTerminal: () => Effect.succeed(value),
      }, request)))
      expect(error).toBeInstanceOf(PylonRuntimeInteractionBridgeError)
      expect(error.reason).toBe(reason)
      expect(String(error)).not.toContain("Allow the bounded workspace edit?")
    }
  })
})
