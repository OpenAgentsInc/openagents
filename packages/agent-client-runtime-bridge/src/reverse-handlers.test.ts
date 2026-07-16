import { describe, expect, it } from "vite-plus/test";
import type { AgentStdioReverseHandler } from "@openagentsinc/agent-stdio-transport";
import { AcpAuthorityBridge } from "./authority.ts";
import { registerAcpAuthorityReverseHandlers } from "./reverse-handlers.ts";

describe("ACP production reverse-handler adapter", () => {
  it("registers every authority method and routes validated requests through the scoped bridge", async () => {
    const handlers = new Map<string, AgentStdioReverseHandler>();
    const bridge = new AcpAuthorityBridge({
      connectionRef: "connection.1",
      generation: 4,
      sessions: {
        inspect: async (sessionId) => ({
          sessionId,
          connectionRef: "connection.1",
          generation: 4,
          scopeRef: "scope.1",
          authenticated: true,
          health: "healthy",
        }),
      },
      filesystem: {
        health: async () => "healthy",
        readTextFile: async (_request, lease) => ({
          value: { content: "brokered" },
          evidenceRefs: [`evidence.${lease.requestRef}`],
        }),
      },
      readiness: {
        filesystem: {
          readTextFile: { handlerInstalled: true, authorized: true, tested: true, healthy: true },
        },
      },
      source: { lane: "agent_client_protocol", surface: "server" },
      threadId: "thread.1",
      turnId: "turn.1",
      now: () => "2026-07-16T12:00:00.000Z",
      nextRef: (kind) => `${kind}.1`,
    });
    const unregister = registerAcpAuthorityReverseHandlers({
      transport: {
        registerReverseHandler(method, handler) {
          handlers.set(method, handler);
          return () => {
            handlers.delete(method);
          };
        },
      },
      bridge,
      contextFor: (params) => ({
        connectionRef: "connection.1",
        sessionId: (params as { sessionId: string }).sessionId,
        scopeRef: "scope.1",
      }),
    });
    expect(handlers.size).toBe(8);
    const result = await handlers.get("fs/read_text_file")!(
      { sessionId: "session.1", path: "/workspace/a" },
      {
        method: "fs/read_text_file",
        requestId: 17,
        generation: 4,
        signal: new AbortController().signal,
      },
    );
    expect(result).toEqual({ content: "brokered" });
    unregister();
    expect(handlers.size).toBe(0);
  });

  it("rejects malformed broker responses before they reach the wire", async () => {
    const handlers = new Map<string, AgentStdioReverseHandler>();
    const bridge = new AcpAuthorityBridge({
      connectionRef: "connection.1",
      generation: 4,
      sessions: {
        inspect: async (sessionId) => ({
          sessionId,
          connectionRef: "connection.1",
          generation: 4,
          scopeRef: "scope.1",
          authenticated: true,
          health: "healthy",
        }),
      },
      filesystem: {
        health: async () => "healthy",
        readTextFile: async () => ({ value: {} as never }),
      },
      readiness: {
        filesystem: {
          readTextFile: { handlerInstalled: true, authorized: true, tested: true, healthy: true },
        },
      },
      source: { lane: "agent_client_protocol", surface: "server" },
      threadId: "thread.1",
      turnId: "turn.1",
      now: () => "2026-07-16T12:00:00.000Z",
      nextRef: (kind) => `${kind}.1`,
    });
    registerAcpAuthorityReverseHandlers({
      transport: {
        registerReverseHandler(method, handler) {
          handlers.set(method, handler);
          return () => handlers.delete(method);
        },
      },
      bridge,
      contextFor: (params) => ({
        connectionRef: "connection.1",
        sessionId: (params as { sessionId: string }).sessionId,
        scopeRef: "scope.1",
      }),
    });
    await expect(
      handlers.get("fs/read_text_file")!(
        { sessionId: "session.1", path: "/workspace/a" },
        {
          method: "fs/read_text_file",
          requestId: 18,
          generation: 4,
          signal: new AbortController().signal,
        },
      ),
    ).rejects.toMatchObject({ code: -32603, data: { reason: "broker_failure" } });
  });
});
