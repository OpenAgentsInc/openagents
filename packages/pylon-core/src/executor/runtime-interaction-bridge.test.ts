import { describe, expect, test } from "bun:test"
import { Effect } from "effect"

import {
  createClaudeCanUseToolInteractionController,
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

  test("maps only confirmed Claude tool approval to allow and keeps raw input out of authority", async () => {
    const requested: unknown[] = []
    const controller = createClaudeCanUseToolInteractionController({
      authority: {
        request: interaction => Effect.sync(() => { requested.push(interaction) }),
        awaitTerminal: () => Effect.succeed(terminal({
          status: "resolved",
          envelope: {
            decisionRef: "decision.pylon.claude.1",
            idempotencyKey: "idem.pylon.claude.1",
            decidedAt: "2026-07-11T23:01:00.000Z",
            surface: "desktop",
            decision: { kind: "tool_approval", outcome: "approve" },
          },
        })),
      },
      requestFor: tool => ({
        ...request,
        payload: {
          ...request.payload,
          toolCallId: tool.toolUseId,
          toolName: tool.toolName,
        },
      }),
    })
    const rawInput = { command: "private command must stay provider-local" }
    expect(await controller("Bash", rawInput, {
      signal: new AbortController().signal,
      toolUseID: "tool_call.claude.1",
    })).toEqual({ behavior: "allow", updatedInput: rawInput })
    const encoded = JSON.stringify(requested)
    expect(encoded).toContain("tool_call.claude.1")
    expect(encoded).toContain('"toolName":"Bash"')
    expect(encoded).not.toContain("private command")
  })

  test("maps denial, expiry, and authority failure to bounded Claude deny results", async () => {
    const cases: ReadonlyArray<readonly [PylonRuntimeInteractionAuthority, string]> = [
      [{
        request: () => Effect.void,
        awaitTerminal: () => Effect.succeed(terminal({
          status: "resolved",
          envelope: { decisionRef: "decision.deny", idempotencyKey: "idem.deny", decidedAt: request.requestedAt, surface: "mobile", decision: { kind: "tool_approval", outcome: "deny" } },
        })),
      }, "Denied by confirmed OpenAgents authority."],
      [{
        request: () => Effect.void,
        awaitTerminal: () => Effect.succeed(terminal({ status: "expired", terminalAt: request.expiresAt, reasonRef: "reason.expired" })),
      }, "OpenAgents approval is no longer actionable."],
      [{
        request: () => Effect.fail("offline"),
        awaitTerminal: () => Effect.never,
      }, "OpenAgents approval authority is unavailable."],
    ]
    for (const [authority, message] of cases) {
      const controller = createClaudeCanUseToolInteractionController({
        authority,
        requestFor: () => request,
      })
      expect(await controller("Write", {}, {
        signal: new AbortController().signal,
        toolUseID: "tool_call.claude.deny",
      })).toEqual({ behavior: "deny", message })
    }
  })

  test("preserves Claude cancellation instead of converting interruption into denial", async () => {
    const controller = createClaudeCanUseToolInteractionController({
      authority: {
        request: () => Effect.void,
        awaitTerminal: () => Effect.never,
      },
      requestFor: () => request,
    })
    const abort = new AbortController()
    abort.abort()
    await expect(controller("Bash", {}, {
      signal: abort.signal,
      toolUseID: "tool_call.claude.cancelled",
    })).rejects.toBeDefined()
  })
})
