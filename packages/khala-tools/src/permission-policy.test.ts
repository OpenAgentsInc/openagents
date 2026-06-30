import { describe, expect, test } from "bun:test"
import { Effect } from "effect"
import {
  makeKhalaPermissionPolicyService,
  nonInteractiveKhalaInteractionService,
  type KhalaInteractionAskInput,
  type KhalaInteractionService,
  type KhalaPermissionRequest,
} from "./index.js"

function request(input: Partial<KhalaPermissionRequest> = {}): KhalaPermissionRequest {
  return {
    action: "edit",
    authorityMode: "local",
    publicSafety: "private",
    resources: ["src/app.ts"],
    saveScope: "session",
    sessionId: "session-a",
    toolCallId: "call-a",
    toolName: "edit",
    workingDirectory: "/workspace/project",
    ...input,
  }
}

function scriptedInteraction(answers: ReadonlyArray<string>): KhalaInteractionService & {
  readonly prompts: ReadonlyArray<KhalaInteractionAskInput>
} {
  const prompts: KhalaInteractionAskInput[] = []
  const queue = [...answers]
  return {
    askUser: input =>
      Effect.sync(() => {
        prompts.push(input)
        const choiceId = queue.shift() ?? "deny"
        return {
          answer: {
            choiceId,
            kind: "choice" as const,
            text: choiceId,
          },
          events: [],
          requestId: `request.${prompts.length}`,
          status: "answered" as const,
        }
      }),
    marker: "khala.interaction_service",
    prompts,
  }
}

describe("Khala permission policy", () => {
  test("denies when the approval prompt is unavailable", async () => {
    const permission = makeKhalaPermissionPolicyService({ interaction: nonInteractiveKhalaInteractionService })

    const decision = await Effect.runPromise(permission.decide(request()))

    expect(decision).toBe("deny")
  })

  test("caches explicit always approvals for the same session action and resource", async () => {
    const interaction = scriptedInteraction(["always"])
    const permission = makeKhalaPermissionPolicyService({ interaction })

    const first = await Effect.runPromise(permission.decide(request()))
    const second = await Effect.runPromise(permission.decide(request({ toolCallId: "call-b" })))

    expect(first).toBe("allow")
    expect(second).toBe("allow")
    expect(interaction.prompts).toHaveLength(1)
    expect(interaction.prompts[0]?.choices.map(choice => choice.id)).toEqual(["allow", "deny", "always"])
  })

  test("denies product-policy blocked authorities without prompting", async () => {
    const interaction = scriptedInteraction(["always"])
    const permission = makeKhalaPermissionPolicyService({ interaction })

    const decision = await Effect.runPromise(permission.decide(request({
      action: "owner_full_access",
      resources: ["danger-full-access"],
      toolName: "exec_command",
    })))

    expect(decision).toBe("deny")
    expect(interaction.prompts).toHaveLength(0)
  })

  test("does not leak session approvals across resources, actions, or sessions", async () => {
    const interaction = scriptedInteraction(["always", "deny", "deny", "deny"])
    const permission = makeKhalaPermissionPolicyService({ interaction })

    const allowed = await Effect.runPromise(permission.decide(request()))
    const otherResource = await Effect.runPromise(permission.decide(request({
      resources: ["src/other.ts"],
      toolCallId: "call-resource",
    })))
    const otherAction = await Effect.runPromise(permission.decide(request({
      action: "shell",
      resources: ["src/app.ts"],
      toolCallId: "call-action",
      toolName: "exec_command",
    })))
    const otherSession = await Effect.runPromise(permission.decide(request({
      sessionId: "session-b",
      toolCallId: "call-session",
    })))

    expect(allowed).toBe("allow")
    expect(otherResource).toBe("deny")
    expect(otherAction).toBe("deny")
    expect(otherSession).toBe("deny")
    expect(interaction.prompts).toHaveLength(4)
  })

  test("requires every resource in a multi-resource request to be cached", async () => {
    const interaction = scriptedInteraction(["always", "deny"])
    const permission = makeKhalaPermissionPolicyService({ interaction })

    const first = await Effect.runPromise(permission.decide(request()))
    const widened = await Effect.runPromise(permission.decide(request({
      resources: ["src/app.ts", "src/other.ts"],
      toolCallId: "call-widened",
    })))

    expect(first).toBe("allow")
    expect(widened).toBe("deny")
    expect(interaction.prompts).toHaveLength(2)
  })

  test("project approvals are shared across sessions only for the same project action and resource", async () => {
    const interaction = scriptedInteraction(["always", "deny"])
    const permission = makeKhalaPermissionPolicyService({ interaction })

    const first = await Effect.runPromise(permission.decide(request({ saveScope: "project" })))
    const sameProject = await Effect.runPromise(permission.decide(request({
      saveScope: "project",
      sessionId: "session-b",
      toolCallId: "call-b",
    })))
    const otherProject = await Effect.runPromise(permission.decide(request({
      saveScope: "project",
      sessionId: "session-c",
      toolCallId: "call-c",
      workingDirectory: "/workspace/other",
    })))

    expect(first).toBe("allow")
    expect(sameProject).toBe("allow")
    expect(otherProject).toBe("deny")
    expect(interaction.prompts).toHaveLength(2)
  })
})
