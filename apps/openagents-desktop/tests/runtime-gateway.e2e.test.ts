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
      result: { protocolVersion: 1, lifecycle: "ready" },
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
      result: { kind: "runtime.bootstrap", lifecycle: "ready", protocolVersion: 1 },
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
    const serialized = JSON.stringify(response)
    expect(serialized).not.toContain("ownerUserId")
    expect(serialized).not.toContain("accessToken")
    expect(serialized).not.toContain("refreshToken")
  })

  test("owns ordered lifecycle delivery and terminal disposal", async () => {
    const gateway = createDesktopRuntimeGateway()
    const events: unknown[] = []
    const unsubscribe = gateway.subscribe(event => events.push(event))
    gateway.start()
    gateway.dispose()
    unsubscribe()
    expect(events).toEqual([
      { kind: "runtime.lifecycle", phase: "ready", protocolVersion: 1, sequence: 1 },
      { kind: "runtime.lifecycle", phase: "disposed", protocolVersion: 1, sequence: 2 },
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
  })
})
