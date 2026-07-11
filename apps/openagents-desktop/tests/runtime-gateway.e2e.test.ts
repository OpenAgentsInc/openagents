import { describe, expect, test } from "bun:test"
import { validateBehaviorContractRegistry } from "@openagentsinc/behavior-contracts"

import {
  decodeDesktopRuntimeGatewayEvent,
  decodeDesktopRuntimeGatewayRequest,
  decodeDesktopRuntimeGatewayResponse,
} from "../src/runtime-gateway-contract.ts"
import { createDesktopRuntimeGateway, desktopRuntimeCapabilities } from "../src/runtime-gateway.ts"
import { openAgentsDesktopUxContractRegistry } from "../src/contracts/ux-contracts.ts"

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
      result: { protocolVersion: 2, lifecycle: "ready" },
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
      result: { kind: "runtime.bootstrap", lifecycle: "ready", protocolVersion: 2 },
    })
    if (response.kind !== "query_result") throw new Error("expected query result")
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
      command: { id: "conversation.interrupt", threadRef: "thread-1" },
    })).toEqual({
      kind: "command_outcome",
      commandId: "command-1",
      status: "unavailable",
      reason: "Conversation interrupt is unavailable until the durable runtime is connected.",
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
      threads: [{ threadRef: "thread.gateway.1", version: 4 }],
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

  test("owns ordered lifecycle delivery and terminal disposal", async () => {
    const gateway = createDesktopRuntimeGateway()
    const events: unknown[] = []
    const unsubscribe = gateway.subscribe(event => events.push(event))
    gateway.start()
    gateway.dispose()
    unsubscribe()
    expect(events).toEqual([
      { kind: "runtime.lifecycle", phase: "ready", protocolVersion: 2, sequence: 1 },
      { kind: "runtime.lifecycle", phase: "disposed", protocolVersion: 2, sequence: 2 },
    ])
    expect(events.every(event => decodeDesktopRuntimeGatewayEvent(event) !== null)).toBe(true)
    expect(await gateway.request({ kind: "query", requestId: "late", query: { id: "runtime.bootstrap" } })).toEqual({
      kind: "request_rejected",
      reason: "gateway_disposed",
    })
  })

  test("rejects unknown operations at the schema boundary", () => {
    expect(decodeDesktopRuntimeGatewayRequest({ kind: "query", requestId: "q", query: { id: "shell.exec" } })).toBeNull()
    expect(decodeDesktopRuntimeGatewayRequest({ kind: "command", commandId: "c", command: { id: "arbitrary", argv: [] } })).toBeNull()
    expect(decodeDesktopRuntimeGatewayRequest({ kind: "command", commandId: "c", command: { id: "session.sign_in", token: "forbidden" } })).toEqual({
      kind: "command",
      commandId: "c",
      command: { id: "session.sign_in" },
    })
    expect(decodeDesktopRuntimeGatewayResponse({ kind: "command_outcome", commandId: "c", status: "completed" })).toBeNull()
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
  })
})
