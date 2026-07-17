import { describe, expect, test } from "vite-plus/test"
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"
import { emptyLiveAgentGraphEntity } from "@openagentsinc/khala-sync"
import type { KhalaConversationLiveUpdate } from "@openagentsinc/khala-sync-client"
import { readFileSync } from "node:fs"

import {
  decodeDesktopRuntimeGatewayEvent,
  decodeDesktopRuntimeGatewayRequest,
  decodeDesktopRuntimeGatewayResponse,
} from "../src/runtime-gateway-contract.ts"
import { createDesktopRuntimeGateway, desktopRuntimeCapabilities } from "../src/runtime-gateway.ts"
import { openAgentsDesktopUxContractRegistry } from "../src/contracts/ux-contracts.ts"
import type { DesktopRuntimeLiveSubscriptions } from "../src/runtime-live-subscriptions.ts"

const contractId = "openagents_desktop.seam.runtime_gateway_closed_protocol.v1"

describe("Desktop Runtime Gateway", () => {
  test("registers the enforced seam contract", () => {
    expect(validateBehaviorContractRegistry(openAgentsDesktopUxContractRegistry).ok).toBe(true)
    expect(openAgentsDesktopUxContractRegistry.contracts.find(contract => contract.contractId === contractId)?.state).toBe("enforced")
  })

  test("round-trips the renderer request through both schema boundaries", async () => {
    const gateway = createDesktopRuntimeGateway()
    gateway.start()
    const rendererValue: unknown = {
      kind: "query",
      requestId: "renderer-bootstrap",
      query: { id: "runtime.bootstrap" },
    }
    const mainRequest = decodeDesktopRuntimeGatewayRequest(rendererValue)
    if (mainRequest === null) throw new Error("preload rejected a valid request")
    const rendererResponse = decodeDesktopRuntimeGatewayResponse(await gateway.request(mainRequest))
    expect(rendererResponse).toMatchObject({
      kind: "query_result",
      requestId: "renderer-bootstrap",
      result: { protocolVersion: 12, lifecycle: "ready" },
    })
  })

  test("bootstraps a versioned truthful capability projection", async () => {
    const gateway = createDesktopRuntimeGateway()
    gateway.start()
    const response = await gateway.request({
      kind: "query",
      requestId: "query-1",
      query: { id: "runtime.bootstrap" },
    })
    expect(decodeDesktopRuntimeGatewayResponse(response)).toEqual(response)
    expect(response).toMatchObject({
      kind: "query_result",
      requestId: "query-1",
      result: { kind: "runtime.bootstrap", lifecycle: "ready", protocolVersion: 12,identityTier:"local_unavailable" },
    })
    if (response.kind !== "query_result") throw new Error("expected query result")
    expect(response.result.capabilities).toContainEqual({
      id:"local-identity",state:"unavailable",reason:"Device-local identity persistence is unavailable.",
    })
    expect(response.result.capabilities).toContainEqual({
      id: "khala-sync",
      state: "unavailable",
      reason: "Local Sync persistence is unavailable.",
    })
    expect(response.result.capabilities).toContainEqual({
      id: "openagents-session",
      state: "unavailable",
      reason: "OS-encrypted OpenAgents session custody is unavailable.",
    })
  })

  test("returns a durable-shaped unavailable outcome instead of optimistic command success", async () => {
    const gateway = createDesktopRuntimeGateway()
    gateway.start()
    expect(await gateway.request({
      kind: "command",
      commandId: "command-1",
      command: {
        id: "conversation.interrupt",
        commandRef: "interrupt.test.1",
        threadRef: "thread-1",
        runRef: "run-1",
      },
    })).toEqual({
      kind: "runtime_command_outcome",
      commandId: "command-1",
      threadRef: "thread-1",
      runRef: "run-1",
      status: "unavailable",
      reason: "Authenticated runtime Sync is unavailable.",
    })
  })

  // Oracle for openagents_desktop.session.loopback_pkce_entry_exit.v1.
  test("routes bounded session commands without credential-bearing arguments", async () => {
    const calls: Array<string> = []
    const gateway = createDesktopRuntimeGateway(undefined, {
      signIn: async () => { calls.push("sign-in"); return { state: "verified" } },
      signOut: async () => { calls.push("sign-out"); return { state: "signed_out" } },
    })
    gateway.start()
    await expect(gateway.request({
      kind: "command",
      commandId: "sign-in-1",
      command: { id: "session.sign_in" },
    })).resolves.toEqual({
      kind: "session_outcome",
      commandId: "sign-in-1",
      status: "completed",
      phase: "session_ready",
    })
    await expect(gateway.request({
      kind: "command",
      commandId: "sign-out-1",
      command: { id: "session.sign_out" },
    })).resolves.toEqual({
      kind: "session_outcome",
      commandId: "sign-out-1",
      status: "completed",
      phase: "signed_out",
    })
    expect(calls).toEqual(["sign-in", "sign-out"])
  })

  // Also enforces openagents_desktop.session.effect_native_controls.v1.
  test("keeps native-session entry and exit single-flight", async () => {
    let finish: ((value: { state: "verified" }) => void) | undefined
    const firstAction = new Promise<{ state: "verified" }>(resolve => { finish = resolve })
    const gateway = createDesktopRuntimeGateway(undefined, {
      signIn: () => firstAction,
      signOut: async () => ({ state: "signed_out" }),
    })
    gateway.start()
    const first = gateway.request({
      kind: "command",
      commandId: "first",
      command: { id: "session.sign_in" },
    })
    expect(await gateway.request({
      kind: "command",
      commandId: "overlap",
      command: { id: "session.sign_out" },
    })).toEqual({
      kind: "session_outcome",
      commandId: "overlap",
      status: "unavailable",
      phase: "unavailable",
    })
    finish?.({ state: "verified" })
    expect(await first).toMatchObject({ status: "completed", phase: "session_ready" })
  })

  test("aborts one in-flight session action exactly once on gateway disposal", async () => {
    let aborts = 0
    const gateway = createDesktopRuntimeGateway(undefined, {
      signIn: signal => new Promise(resolve => {
        signal?.addEventListener("abort", () => {
          aborts++
          resolve({ state: "unavailable" })
        }, { once: true })
      }),
      signOut: async () => ({ state: "signed_out" }),
    })
    gateway.start()
    const pending = gateway.request({
      kind: "command",
      commandId: "session-abort",
      command: { id: "session.sign_in" },
    })
    gateway.dispose()
    gateway.dispose()
    expect(await pending).toMatchObject({ kind: "session_outcome", status: "unavailable" })
    expect(aborts).toBe(1)
  })

  test("preserves structured operation context through gateway, Sync command, response, and observer", async () => {
    const observed: string[] = []
    const contexts: unknown[] = []
    const gateway = createDesktopRuntimeGateway(
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      () => ({
        outcome: () => null,
        start: (_input, context) => { contexts.push(context); return 7 },
        interrupt: () => 8,
      }),
      (stage, context) => {
        observed.push(stage)
        contexts.push(context)
      },
    )
    const context = {
      operationRef: "operation.desktop.test",
      sessionRef: "session.desktop.test",
      correlationRef: "correlation.desktop.test",
      runRef: "run.desktop.test",
    } as const
    gateway.start()
    const response = await gateway.request({
      kind: "command",
      commandId: context.operationRef,
      context,
      command: {
        id: "conversation.start",
        threadRef: "thread.desktop.test",
        messageRef: "message.desktop.test",
        runRef: context.runRef,
      },
    }, context)
    expect(response).toMatchObject({
      kind: "runtime_command_outcome",
      status: "unknown_pending_reconcile",
      context,
    })
    expect(observed).toEqual(["gateway.received"])
    expect(contexts).toEqual([context, context])
    expect(decodeDesktopRuntimeGatewayResponse(response)).toEqual(response)
  })

  // Oracle for openagents_desktop.session.recovered_validation_rotation.v1.
  test("projects only bounded session readiness after host verification", async () => {
    const gateway = createDesktopRuntimeGateway(() => desktopRuntimeCapabilities({
      sessionLocalState: "session_ready",
      syncLocalState: "ready",
      syncNetworkPhase: "live",
    }))
    gateway.start()
    const response = await gateway.request({
      kind: "query",
      requestId: "verified-session",
      query: { id: "runtime.bootstrap" },
    })
    if (response.kind !== "query_result") throw new Error("expected query result")
    expect(response.result.capabilities).toContainEqual({
      id: "openagents-session",
      state: "available",
      reason: undefined,
    })
    expect(response.result.capabilities).toContainEqual({
      id: "khala-sync",
      state: "available",
      reason: undefined,
    })
    expect(response.result.capabilities).toContainEqual({
      id: "agent-graph",
      state: "available",
      reason: undefined,
    })
    expect(response.result.capabilities).toContainEqual({
      id: "agent-timeline",
      state: "available",
      reason: undefined,
    })
    const serialized = JSON.stringify(response)
    expect(serialized).not.toContain("ownerUserId")
    expect(serialized).not.toContain("accessToken")
    expect(serialized).not.toContain("refreshToken")
  })

  test("queries confirmed conversations and enqueues mutations as pending reconcile", async () => {
    const calls: Array<string> = []
    const status = { phase: "live" as const, cursor: 5, pendingMutationCount: 0 }
    const gateway = createDesktopRuntimeGateway(undefined, undefined, undefined, () => ({
      catalog: () => ({
        status,
        threads: [{
          threadRef: "thread.gateway.1",
          title: "Gateway thread",
          status: "active",
          messageCount: 1,
          lastMessageAt: "2026-07-10T20:00:00.000Z",
          updatedAt: "2026-07-10T20:00:00.000Z",
          version: 4,
        }],
      }),
      thread: threadRef => {
        calls.push(`thread:${threadRef}`)
        return {
          status,
          messages: [{
            messageRef: "message.gateway.1",
            threadRef,
            body: "Confirmed only",
            createdAt: "2026-07-10T20:00:00.000Z",
            updatedAt: "2026-07-10T20:00:00.000Z",
            version: 5,
          }],
        }
      },
      create: (threadRef, title) => {
        calls.push(`create:${threadRef}:${title}`)
        return 6
      },
      append: (threadRef, messageRef, body) => {
        calls.push(`append:${threadRef}:${messageRef}:${body}`)
        return 7
      },
    }))
    gateway.start()
    const catalog = await gateway.request({
      kind: "query",
      requestId: "catalog",
      query: { id: "conversation.catalog" },
    })
    expect(decodeDesktopRuntimeGatewayResponse(catalog)).toEqual(catalog)
    expect(catalog).toMatchObject({
      kind: "conversation_catalog",
      status,
      threads: [{ threadRef: "thread.gateway.1", status: "active", version: 4 }],
    })
    const thread = await gateway.request({
      kind: "query",
      requestId: "thread",
      query: { id: "conversation.thread", threadRef: "thread.gateway.1" },
    })
    expect(decodeDesktopRuntimeGatewayResponse(thread)).toEqual(thread)
    expect(thread).toMatchObject({
      kind: "conversation_thread",
      status,
      messages: [{ messageRef: "message.gateway.1", version: 5 }],
    })
    const create = await gateway.request({
      kind: "command",
      commandId: "create",
      command: { id: "conversation.create", threadRef: "thread.gateway.2", title: "New" },
    })
    expect(decodeDesktopRuntimeGatewayResponse(create)).toEqual(create)
    expect(create).toEqual({
      kind: "conversation_mutation_outcome",
      commandId: "create",
      status: "pending_reconcile",
      mutationId: 6,
    })
    expect(await gateway.request({
      kind: "command",
      commandId: "append",
      command: {
        id: "conversation.append",
        threadRef: "thread.gateway.1",
        messageRef: "message.gateway.2",
        body: "Follow-up",
      },
    })).toEqual({
      kind: "conversation_mutation_outcome",
      commandId: "append",
      status: "pending_reconcile",
      mutationId: 7,
    })
    expect(calls).toEqual([
      "thread:thread.gateway.1",
      "create:thread.gateway.2:New",
      "append:thread.gateway.1:message.gateway.2:Follow-up",
    ])
  })

  test("conversation requests fail closed while the host service is not live", async () => {
    const gateway = createDesktopRuntimeGateway()
    gateway.start()
    expect(await gateway.request({
      kind: "query",
      requestId: "catalog-offline",
      query: { id: "conversation.catalog" },
    })).toEqual({
      kind: "conversation_unavailable",
      requestId: "catalog-offline",
      reason: "not_live",
    })
    expect(await gateway.request({
      kind: "command",
      commandId: "append-offline",
      command: {
        id: "conversation.append",
        threadRef: "thread.gateway.1",
        messageRef: "message.gateway.2",
        body: "Follow-up",
      },
    })).toEqual({
      kind: "conversation_mutation_outcome",
      commandId: "append-offline",
      status: "unavailable",
    })
  })

  test("submits exact runtime refs and projects the same thread timeline", async () => {
    const calls: Array<Record<string, unknown>> = []
    const status = { phase: "live" as const, cursor: 21, pendingMutationCount: 0 }
    const gateway = createDesktopRuntimeGateway(
      undefined,
      undefined,
      undefined,
      undefined,
      () => ({
        snapshot: () => ({ status, run: null, events: [] }),
        snapshotForThread: threadRef => ({
          status,
          run: {
            runRef: "run.gateway.1",
            routeRef: threadRef,
            status: "running",
            createdAt: "2026-07-11T12:00:00.000Z",
            updatedAt: "2026-07-11T12:00:01.000Z",
            startedAt: "2026-07-11T12:00:01.000Z",
            completedAt: null,
            failedAt: null,
            canceledAt: null,
            version: 2,
          },
          events: [{
            eventRef: "event.gateway.1",
            runRef: "run.gateway.1",
            sequence: 1,
            eventType: "turn.started",
            summary: "Connected",
            status: "running",
            artifactRefs: [],
            item: { kind: "connected", lane: "codex_app_server", turnRef: "run.gateway.1" },
            createdAt: "2026-07-11T12:00:01.000Z",
            version: 3,
          }],
        }),
      }),
      undefined,
      undefined,
      () => ({
        outcome: input => ({
          commandRef: input.intentId,
          mutationId: null,
          runRef: "run.gateway.1",
          status: "accepted",
          threadRef: input.threadRef,
          updatedAt: "2026-07-11T12:00:01.000Z",
          version: 4,
        }),
        start: input => { calls.push({ id: "start", ...input }); return 31 },
        interrupt: input => { calls.push({ id: "interrupt", ...input }); return 32 },
        continue: input => { calls.push({ id: "continue", ...input }); return 33 },
        retry: input => { calls.push({ id: "retry", ...input }); return 34 },
        close: input => { calls.push({ id: "close", ...input }); return 35 },
      }),
    )
    gateway.start()

    expect(await gateway.request({
      kind: "command",
      commandId: "start-1",
      command: {
        id: "conversation.start",
        threadRef: "thread.gateway.1",
        messageRef: "message.gateway.1",
        runRef: "run.gateway.1",
      },
    })).toEqual({
      kind: "runtime_command_outcome",
      commandId: "start-1",
      threadRef: "thread.gateway.1",
      messageRef: "message.gateway.1",
      runRef: "run.gateway.1",
      status: "unknown_pending_reconcile",
      mutationId: 31,
    })
    expect(await gateway.request({
      kind: "query",
      requestId: "command-status",
      query: {
        id: "conversation.commandOutcome",
        intentId: "intent.start.run.gateway.1",
        threadRef: "thread.gateway.1",
      },
    })).toEqual({
      kind: "runtime_command_status",
      requestId: "command-status",
      commandRef: "intent.start.run.gateway.1",
      mutationId: null,
      runRef: "run.gateway.1",
      status: "accepted",
      threadRef: "thread.gateway.1",
      updatedAt: "2026-07-11T12:00:01.000Z",
      version: 4,
    })
    expect(await gateway.request({
      kind: "query",
      requestId: "thread-timeline",
      query: { id: "conversation.timeline", threadRef: "thread.gateway.1" },
    })).toMatchObject({
      kind: "conversation_timeline",
      threadRef: "thread.gateway.1",
      run: { runRef: "run.gateway.1", routeRef: "thread.gateway.1" },
      events: [{ item: { kind: "connected" } }],
    })
    expect(await gateway.request({
      kind: "command",
      commandId: "start-2",
      command: {
        id: "conversation.start",
        threadRef: "thread.gateway.1",
        messageRef: "message.gateway.2",
        runRef: "run.gateway.2",
        lane: "claude_pylon",
      },
    })).toMatchObject({
      kind: "runtime_command_outcome",
      commandId: "start-2",
      status: "unknown_pending_reconcile",
    })
    await gateway.request({
      kind: "command",
      commandId: "interrupt-1",
      command: {
        id: "conversation.interrupt",
        commandRef: "control.gateway.1",
        threadRef: "thread.gateway.1",
        runRef: "run.gateway.1",
        // CUT-16: the exact confirmed run lane rides the control command so
        // the durable lane fence admits Claude/hosted turn controls.
        lane: "claude_pylon",
      },
    })
    for (const [id, commandId, lane] of [
      ["conversation.continue", "continue-1", "claude_pylon"],
      ["conversation.retry", "retry-1", "hosted_khala"],
      ["conversation.close", "close-1", undefined],
    ] as const) {
      expect(await gateway.request({
        kind: "command",
        commandId,
        command: {
          id,
          commandRef: `control.gateway.${commandId}`,
          threadRef: "thread.gateway.1",
          runRef: "run.gateway.1",
          ...(lane === undefined ? {} : { lane }),
          expectedVersion: 2,
        },
      })).toMatchObject({
        kind: "runtime_command_outcome",
        commandId,
        status: "unknown_pending_reconcile",
      })
    }
    expect(calls).toEqual([
      { id: "conversation.start", threadRef: "thread.gateway.1", messageRef: "message.gateway.1", runRef: "run.gateway.1" },
      { id: "conversation.start", threadRef: "thread.gateway.1", messageRef: "message.gateway.2", runRef: "run.gateway.2", lane: "claude_pylon" },
      { id: "conversation.interrupt", commandRef: "control.gateway.1", threadRef: "thread.gateway.1", runRef: "run.gateway.1", lane: "claude_pylon" },
      { id: "conversation.continue", commandRef: "control.gateway.continue-1", threadRef: "thread.gateway.1", runRef: "run.gateway.1", lane: "claude_pylon", expectedVersion: 2 },
      { id: "conversation.retry", commandRef: "control.gateway.retry-1", threadRef: "thread.gateway.1", runRef: "run.gateway.1", lane: "hosted_khala", expectedVersion: 2 },
      { id: "conversation.close", commandRef: "control.gateway.close-1", threadRef: "thread.gateway.1", runRef: "run.gateway.1", expectedVersion: 2 },
    ])
  })

  test("control-command lane decodes only exact known lanes and main threads it into the intent context (CUT-16)", () => {
    const base = {
      kind: "command" as const,
      commandId: "lane-decode-1",
      command: {
        id: "conversation.interrupt" as const,
        commandRef: "control.lane.1",
        threadRef: "thread.gateway.1",
        runRef: "run.gateway.1",
      },
    }
    // Additive optional field: present-and-known decodes, absent decodes,
    // unknown lane literals reject at the schema boundary.
    expect(decodeDesktopRuntimeGatewayRequest(base)).not.toBeNull()
    for (const lane of ["codex_app_server", "claude_pylon", "hosted_khala"]) {
      expect(decodeDesktopRuntimeGatewayRequest({
        ...base,
        command: { ...base.command, lane },
      })).not.toBeNull()
    }
    expect(decodeDesktopRuntimeGatewayRequest({
      ...base,
      command: { ...base.command, lane: "another_owner_lane" },
    })).toBeNull()
    expect(decodeDesktopRuntimeGatewayRequest({
      ...base,
      command: {
        id: "conversation.retry",
        commandRef: "control.lane.2",
        threadRef: "thread.gateway.1",
        runRef: "run.gateway.1",
        lane: "hosted_khala",
        expectedVersion: 2,
      },
    })).not.toBeNull()

    // Source oracle: main's interrupt/continue/retry/close adapters thread the
    // caller-derived lane into the shared control-intent context instead of
    // the hard-coded Codex default (the durable lane fence rejects mismatches).
    // Five adapters: start plus the four turn controls all pass input.lane.
    const mainSource = readFileSync(new URL("../src/main.ts", import.meta.url), "utf8")
    expect(mainSource.match(/context: context\(input\.lane\)/g) ?? []).toHaveLength(5)
    expect(mainSource).not.toContain("context: context(),")
  })

  test("queries confirmed interactions and queues exact decisions without optimistic resolution", async () => {
    const decisions: unknown[] = []
    const interaction = {
      schema: "openagents.runtime_interaction_projection.v1" as const,
      interactionRef: "interaction.gateway.1",
      threadId: "thread.gateway.1",
      turnId: "turn.gateway.1",
      kind: "provider_question" as const,
      status: "pending" as const,
      displayTitle: "Choose a test lane",
      displayText: "Which lane should run?",
      questions: [{
        questionRef: "question.gateway.1",
        displayText: "Which lane should run?",
        options: [{ optionRef: "option.gateway.unit", label: "Unit tests" }],
        multiSelect: false,
      }],
      expiresAt: "2026-07-11T23:30:00.000Z",
      requestedSequence: 7,
      requestedAt: "2026-07-11T23:00:00.000Z",
      version: 4,
    }
    const gateway = createDesktopRuntimeGateway(
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      () => ({
        list: async threadRef => threadRef === interaction.threadId ? [interaction] : [],
        decide: command => {
          decisions.push(command)
          return 41
        },
      }),
    )
    gateway.start()

    const listed = await gateway.request({
      kind: "query",
      requestId: "interactions-1",
      query: { id: "runtime.interactions", threadRef: interaction.threadId },
    })
    expect(decodeDesktopRuntimeGatewayResponse(listed)).toEqual(listed)
    expect(listed).toEqual({
      kind: "runtime_interactions",
      requestId: "interactions-1",
      threadRef: interaction.threadId,
      interactions: [interaction],
    })

    const envelope = {
      decisionRef: "decision.gateway.1",
      idempotencyKey: "idem.gateway.1",
      decidedAt: "2026-07-11T23:01:00.000Z",
      surface: "desktop" as const,
      decision: {
        kind: "provider_question" as const,
        answers: [{
          questionRef: "question.gateway.1",
          optionRefs: ["option.gateway.unit"],
        }],
      },
    }
    const decided = await gateway.request({
      kind: "command",
      commandId: "decide-1",
      command: {
        id: "runtime.decideInteraction",
        interactionRef: interaction.interactionRef,
        threadRef: interaction.threadId,
        turnRef: interaction.turnId,
        envelope,
      },
    })
    expect(decodeDesktopRuntimeGatewayResponse(decided)).toEqual(decided)
    expect(decided).toEqual({
      kind: "runtime_interaction_decision_outcome",
      commandId: "decide-1",
      interactionRef: interaction.interactionRef,
      threadRef: interaction.threadId,
      turnRef: interaction.turnId,
      status: "pending_reconcile",
      mutationId: 41,
    })
    expect(decisions).toEqual([{
      interactionRef: interaction.interactionRef,
      threadId: interaction.threadId,
      turnId: interaction.turnId,
      envelope,
    }])
  })

  test("fails closed for unavailable or malformed runtime interaction traffic", async () => {
    const gateway = createDesktopRuntimeGateway()
    gateway.start()
    expect(await gateway.request({
      kind: "query",
      requestId: "interactions-offline",
      query: { id: "runtime.interactions", threadRef: "thread.gateway.1" },
    })).toEqual({
      kind: "runtime_interactions_unavailable",
      requestId: "interactions-offline",
      reason: "not_live",
    })
    expect(await gateway.request({
      kind: "command",
      commandId: "decide-offline",
      command: {
        id: "runtime.decideInteraction",
        interactionRef: "interaction.gateway.1",
        threadRef: "thread.gateway.1",
        turnRef: "turn.gateway.1",
        envelope: {
          decisionRef: "decision.gateway.1",
          idempotencyKey: "idem.gateway.1",
          decidedAt: "2026-07-11T23:01:00.000Z",
          surface: "desktop",
          decision: { kind: "tool_approval", outcome: "deny" },
        },
      },
    })).toMatchObject({
      kind: "runtime_interaction_decision_outcome",
      commandId: "decide-offline",
      status: "unavailable",
    })
    expect(decodeDesktopRuntimeGatewayRequest({
      kind: "command",
      commandId: "malformed-decision",
      command: {
        id: "runtime.decideInteraction",
        interactionRef: "interaction.gateway.1",
        threadRef: "thread.gateway.1",
        turnRef: "turn.gateway.1",
        envelope: {
          decisionRef: "decision.gateway.1",
          idempotencyKey: "idem.gateway.1",
          decidedAt: "not-an-iso-timestamp",
          surface: "desktop",
          decision: { kind: "tool_approval", outcome: "approve" },
        },
      },
    })).toBeNull()
  })

  // Oracle for openagents_desktop.seam.runtime_gateway_agent_timeline.v1.
  test("queries a confirmed bounded agent timeline and preserves only the server route binding", async () => {
    const status = { phase: "live" as const, cursor: 12, pendingMutationCount: 0 }
    const gateway = createDesktopRuntimeGateway(
      undefined,
      undefined,
      undefined,
      undefined,
      () => ({
        snapshot: runRef => ({
          status,
          run: {
            runRef,
            routeRef: "route.server-confirmed.42",
            status: "running",
            createdAt: "2026-07-10T20:00:00.000Z",
            updatedAt: "2026-07-10T20:01:00.000Z",
            startedAt: "2026-07-10T20:00:01.000Z",
            completedAt: null,
            failedAt: null,
            canceledAt: null,
            version: 11,
          },
          events: [{
            eventRef: "event.confirmed.1",
            runRef,
            sequence: 1,
            eventType: "runtime.activity",
            summary: "Confirmed activity",
            status: "running",
            artifactRefs: ["artifact.confirmed.1"],
            createdAt: "2026-07-10T20:00:02.000Z",
            version: 12,
          }],
        }),
      }),
    )
    gateway.start()
    const response = await gateway.request({
      kind: "query",
      requestId: "timeline",
      query: { id: "agent.timeline", runRef: "run.requested.1" },
    })

    expect(decodeDesktopRuntimeGatewayResponse(response)).toEqual(response)
    expect(response).toMatchObject({
      kind: "agent_timeline",
      requestId: "timeline",
      runRef: "run.requested.1",
      status,
      run: {
        runRef: "run.requested.1",
        routeRef: "route.server-confirmed.42",
        version: 11,
      },
      events: [{ eventRef: "event.confirmed.1", sequence: 1, version: 12 }],
    })
    expect(JSON.stringify(response)).not.toContain("ownerUserId")
    expect(JSON.stringify(response)).not.toContain("payloadJson")
    expect(JSON.stringify(response)).not.toContain("externalEventId")
  })

  test("agent timeline queries fail closed without a live confirmed snapshot", async () => {
    const offline = createDesktopRuntimeGateway()
    offline.start()
    expect(await offline.request({
      kind: "query",
      requestId: "timeline-offline",
      query: { id: "agent.timeline", runRef: "run.requested.1" },
    })).toEqual({
      kind: "agent_timeline_unavailable",
      requestId: "timeline-offline",
      reason: "not_live",
    })

    const catchingUp = createDesktopRuntimeGateway(
      undefined,
      undefined,
      undefined,
      undefined,
      () => ({
        snapshot: () => ({
          status: { phase: "catching_up", cursor: 4, pendingMutationCount: 0 },
          run: null,
          events: [],
        }),
      }),
    )
    catchingUp.start()
    expect(await catchingUp.request({
      kind: "query",
      requestId: "timeline-catching-up",
      query: { id: "agent.timeline", runRef: "run.requested.1" },
    })).toEqual({
      kind: "agent_timeline_unavailable",
      requestId: "timeline-catching-up",
      reason: "not_live",
    })

    const crossRun = createDesktopRuntimeGateway(
      undefined,
      undefined,
      undefined,
      undefined,
      () => ({
        snapshot: () => ({
          status: { phase: "live", cursor: 4, pendingMutationCount: 0 },
          run: {
            runRef: "run.other",
            routeRef: "route.other",
            status: "running",
            createdAt: "2026-07-10T20:00:00.000Z",
            updatedAt: "2026-07-10T20:00:00.000Z",
            startedAt: null,
            completedAt: null,
            failedAt: null,
            canceledAt: null,
            version: 4,
          },
          events: [],
        }),
      }),
    )
    crossRun.start()
    expect(await crossRun.request({
      kind: "query",
      requestId: "timeline-cross-run",
      query: { id: "agent.timeline", runRef: "run.requested.1" },
    })).toEqual({
      kind: "agent_timeline_unavailable",
      requestId: "timeline-cross-run",
      reason: "read_failed",
    })
  })

  test("owns ordered lifecycle delivery and terminal disposal", async () => {
    const gateway = createDesktopRuntimeGateway()
    const events: unknown[] = []
    const unsubscribe = gateway.subscribe(event => events.push(event))
    gateway.start()
    gateway.dispose()
    unsubscribe()
    expect(events).toEqual([
      { kind: "runtime.lifecycle", phase: "ready", protocolVersion: 12, sequence: 1 },
      { kind: "runtime.lifecycle", phase: "disposed", protocolVersion: 12, sequence: 2 },
    ])
    expect(events.every(event => decodeDesktopRuntimeGatewayEvent(event) !== null)).toBe(true)
    expect(await gateway.request({ kind: "query", requestId: "late", query: { id: "runtime.bootstrap" } })).toEqual({
      kind: "request_rejected",
      reason: "gateway_disposed",
    })
  })

  test("routes cursor-aware live subscribe, update, unsubscribe, and disposal", async () => {
    const calls: Array<string> = []
    let publish: ((update: KhalaConversationLiveUpdate) => void | Promise<void>) | undefined
    const service: DesktopRuntimeLiveSubscriptions = {
      subscribe: async (request, listener) => {
        calls.push(`subscribe:${request.subscriptionRef}:${request.generation}:${request.afterCursor}`)
        publish = listener
        return { status: "subscribed" }
      },
      unsubscribe: async (subscriptionRef, generation) => {
        calls.push(`unsubscribe:${subscriptionRef}:${generation}`)
        return true
      },
      metrics: () => null,
      activeCount: () => 1,
      reset: async () => { calls.push("reset") },
      dispose: async () => { calls.push("dispose") },
    }
    const gateway = createDesktopRuntimeGateway(
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      undefined,
      () => service,
    )
    const events: Array<unknown> = []
    gateway.subscribe(event => events.push(event))
    gateway.start()

    const subscribeRequest = decodeDesktopRuntimeGatewayRequest({
      kind: "command",
      commandId: "subscribe-1",
      command: {
        id: "conversation.subscribe",
        subscriptionRef: "subscription.gateway.1",
        generation: 2,
        threadRef: "thread.gateway.1",
        afterCursor: 7,
      },
    })
    if (subscribeRequest === null) throw new Error("subscribe request did not decode")
    expect(await gateway.request(subscribeRequest)).toEqual({
      kind: "conversation_subscription_outcome",
      commandId: "subscribe-1",
      subscriptionRef: "subscription.gateway.1",
      generation: 2,
      status: "subscribed",
    })

    const update: KhalaConversationLiveUpdate = {
      kind: "conversation.live.update",
      envelope: {
        kind: "conversation.live",
        delivery: "confirmed",
        subscriptionRef: "subscription.gateway.1",
        generation: 2,
        sequence: 1,
        threadRef: "thread.gateway.1",
        cursor: 8,
        recovery: "resumed",
        messageRefs: ["message.gateway.1"],
        eventRefs: [],
        graphRefs: ["graph.runtime.run.gateway.1"],
      },
      snapshot: {
        status: { phase: "live", cursor: 8, pendingMutationCount: 0 },
        thread: {
          threadRef: "thread.gateway.1",
          title: "Gateway live",
          status: "active",
          messageCount: 1,
          lastMessageAt: "2026-07-11T16:00:00.000Z",
          updatedAt: "2026-07-11T16:00:00.000Z",
          version: 8,
        },
        messages: [{
          messageRef: "message.gateway.1",
          threadRef: "thread.gateway.1",
          body: "Confirmed",
          createdAt: "2026-07-11T16:00:00.000Z",
          updatedAt: "2026-07-11T16:00:00.000Z",
          version: 8,
        }],
        timeline: null,
        graphs: [emptyLiveAgentGraphEntity({
          graphRef: "graph.runtime.run.gateway.1",
          sessionRef: "session.runtime.thread.gateway.1",
          threadRef: "thread.gateway.1",
          attachmentGeneration: 1,
          updatedAt: "2026-07-11T16:00:00.000Z",
        })],
      },
    }
    await publish?.(update)
    expect(events.at(-1)).toEqual(update)
    expect(decodeDesktopRuntimeGatewayEvent(events.at(-1))).toEqual(update)

    expect(await gateway.request({
      kind: "command",
      commandId: "unsubscribe-1",
      command: {
        id: "conversation.unsubscribe",
        subscriptionRef: "subscription.gateway.1",
        generation: 2,
      },
    })).toEqual({
      kind: "conversation_subscription_outcome",
      commandId: "unsubscribe-1",
      subscriptionRef: "subscription.gateway.1",
      generation: 2,
      status: "unsubscribed",
    })
    gateway.dispose()
    await Promise.resolve()
    expect(calls).toEqual([
      "subscribe:subscription.gateway.1:2:7",
      "unsubscribe:subscription.gateway.1:2",
      "dispose",
    ])
  })

  test("live subscription operations fail closed without a host registry", async () => {
    const gateway = createDesktopRuntimeGateway()
    gateway.start()
    expect(await gateway.request({
      kind: "command",
      commandId: "subscribe-unavailable",
      command: {
        id: "conversation.subscribe",
        subscriptionRef: "subscription.gateway.unavailable",
        generation: 1,
        threadRef: "thread.gateway.unavailable",
      },
    })).toEqual({
      kind: "conversation_subscription_outcome",
      commandId: "subscribe-unavailable",
      subscriptionRef: "subscription.gateway.unavailable",
      generation: 1,
      status: "unavailable",
    })
    expect(decodeDesktopRuntimeGatewayRequest({
      kind: "command",
      commandId: "subscribe-invalid",
      command: {
        id: "conversation.subscribe",
        subscriptionRef: "subscription.gateway.invalid",
        generation: 0,
        threadRef: "thread.gateway.invalid",
      },
    })).toBeNull()
  })

  test("rejects unknown operations at the schema boundary", () => {
    expect(decodeDesktopRuntimeGatewayRequest({ kind: "query", requestId: "q", query: { id: "shell.exec" } })).toBeNull()
    expect(decodeDesktopRuntimeGatewayRequest({ kind: "query", requestId: "q", query: { id: "agent.timeline", runRef: "../private" } })).toBeNull()
    expect(decodeDesktopRuntimeGatewayRequest({ kind: "command", commandId: "c", command: { id: "arbitrary", argv: [] } })).toBeNull()
    expect(decodeDesktopRuntimeGatewayRequest({ kind: "command", commandId: "c", command: { id: "session.sign_in", token: "forbidden" } })).toEqual({
      kind: "command",
      commandId: "c",
      command: { id: "session.sign_in" },
    })
    expect(decodeDesktopRuntimeGatewayResponse({ kind: "command_outcome", commandId: "c", status: "completed" })).toBeNull()
    expect(decodeDesktopRuntimeGatewayRequest({
      kind: "command",
      commandId: "start-no-lane",
      command: { id: "conversation.start", threadRef: "thread.gateway.1", messageRef: "message.gateway.1", runRef: "run.gateway.1" },
    })).toEqual({
      kind: "command",
      commandId: "start-no-lane",
      command: { id: "conversation.start", threadRef: "thread.gateway.1", messageRef: "message.gateway.1", runRef: "run.gateway.1" },
    })
    expect(decodeDesktopRuntimeGatewayRequest({
      kind: "command",
      commandId: "start-claude",
      command: { id: "conversation.start", threadRef: "thread.gateway.1", messageRef: "message.gateway.1", runRef: "run.gateway.1", lane: "claude_pylon" },
    })).toMatchObject({ command: { lane: "claude_pylon" } })
    expect(decodeDesktopRuntimeGatewayRequest({
      kind: "command",
      commandId: "start-codex",
      command: { id: "conversation.start", threadRef: "thread.gateway.1", messageRef: "message.gateway.1", runRef: "run.gateway.1", lane: "codex_app_server" },
    })).toMatchObject({ command: { lane: "codex_app_server" } })
    expect(decodeDesktopRuntimeGatewayRequest({
      kind: "command",
      commandId: "start-bogus-lane",
      command: { id: "conversation.start", threadRef: "thread.gateway.1", messageRef: "message.gateway.1", runRef: "run.gateway.1", lane: "gemini_pylon" },
    })).toBeNull()
    expect(decodeDesktopRuntimeGatewayRequest({
      kind: "command",
      commandId: "oversized",
      command: {
        id: "conversation.append",
        threadRef: "thread.gateway.1",
        messageRef: "message.gateway.1",
        body: "x".repeat(20_001),
      },
    })).toBeNull()
    expect(decodeDesktopRuntimeGatewayResponse({
      kind: "agent_timeline",
      requestId: "oversized-artifacts",
      runRef: "run.1",
      status: { phase: "live", cursor: 1, pendingMutationCount: 0 },
      run: {
        runRef: "run.1",
        routeRef: "route.1",
        status: "running",
        createdAt: "2026-07-10T20:00:00.000Z",
        updatedAt: "2026-07-10T20:00:00.000Z",
        startedAt: null,
        completedAt: null,
        failedAt: null,
        canceledAt: null,
        version: 1,
      },
      events: [{
        eventRef: "event.1",
        runRef: "run.1",
        sequence: 1,
        eventType: "activity",
        summary: "bounded",
        status: null,
        artifactRefs: Array.from({ length: 101 }, (_, index) => `artifact.${index}`),
        createdAt: "2026-07-10T20:00:00.000Z",
        version: 1,
      }],
    })).toBeNull()
  })

  test("projects provider-native Codex history through protocol v10 only", async () => {
    const agent = { threadRef: "root", parentThreadRef: null, title: "Root", status: "completed" as const, createdAt: "2026-07-10T00:00:00Z", updatedAt: "2026-07-10T00:00:00Z", depth: 0, descendantCount: 0, model: null, role: null, nickname:null,agentPath:null,sourceVersion:null,reasoning:null, source: "codex" as const }
    const page = { rootThreadRef: "root", selectedThreadRef: "root", agents: [agent], items: [{ itemRef: "root:0", threadRef: "root", sequence: 0, timestamp: "2026-07-10T00:00:00Z", kind: "session" as const, label: "Session", summary: "Started", status: null, fields: [], redacted: false, sourceType: "session_meta/session_meta" }], offset: 0, limit: 200, totalItems: 1, hasPrevious: false, hasNext: false, completeness: { source: 1, rendered: 1, redactions: 0, gaps: 0, complete: true } }
    const gateway = createDesktopRuntimeGateway(undefined, undefined, undefined, undefined, undefined, () => ({ catalog: () => ({ roots: [agent], agents: [agent] }), page: () => page, search: () => ({ query: "", results: [], indexedSessions: 0, truncated: false }) }))
    gateway.start()
    const catalog = await gateway.request({ kind: "query", requestId: "history-catalog", query: { id: "codex.history.catalog" } })
    const detail = await gateway.request({ kind: "query", requestId: "history-page", query: { id: "codex.history.page", threadRef: "root", offset: 0, limit: 200 } })
    expect(decodeDesktopRuntimeGatewayResponse(catalog)?.kind).toBe("codex_history_catalog")
    expect(decodeDesktopRuntimeGatewayResponse(detail)).toMatchObject({ kind: "codex_history_page", page: { completeness: { gaps: 0 }, items: [{ sourceType: "session_meta/session_meta" }] } })
  })
})
