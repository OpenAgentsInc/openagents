import { describe, expect, it } from "vite-plus/test";

import {
  AcpAuthorityBridge,
  AcpAuthorityFault,
  pendingInteractionDelivery,
  projectAcpClientCapabilities,
  routeAcpInteractionDecision,
  routeAcpUserInput,
  toAcpAuthorityProtocolError,
  type AcpAuthorityBridgeOptions,
  type AcpAuthorityContext,
  type AcpAuthorityReceipt,
  type AcpInteractionBrokerPort,
} from "./authority.js";

const context: AcpAuthorityContext = {
  requestRef: "request.authority.1",
  connectionRef: "connection.acp.1",
  generation: 7,
  sessionId: "session.acp.1",
  scopeRef: "scope.workspace.1",
};

const makeOptions = (input: Partial<AcpAuthorityBridgeOptions> = {}): AcpAuthorityBridgeOptions => {
  let sequence = 0;
  return {
    connectionRef: context.connectionRef,
    generation: context.generation,
    sessions: {
      inspect: async (sessionId) => ({
        sessionId,
        connectionRef: context.connectionRef,
        generation: context.generation,
        scopeRef: context.scopeRef,
        authenticated: true,
        health: "healthy",
      }),
    },
    source: {
      lane: "claude_pylon",
      adapterKind: "grok_cli",
      surface: "server",
      providerRef: "provider.grok",
    },
    threadId: "thread.authority.1",
    turnId: "turn.authority.1",
    now: () => "2026-07-16T13:00:00.000Z",
    nextRef: (kind) => `${kind}.authority.${++sequence}`,
    readiness: {
      permission: ready,
      filesystem: { readTextFile: ready, writeTextFile: ready },
      terminal: ready,
      mcp: { http: ready, sse: ready, stdio: ready },
    },
    ...input,
  };
};

const healthy = async () => "healthy" as const;
const escalatingPermission = {
  health: healthy,
  decide: async () =>
    ({
      kind: "escalate",
      approveOptionId: "allow.once",
      denyOptionId: "reject.once",
      authorityRef: "authority.permission.1",
      policyRef: "policy.permission.1",
      decisionRef: "decision.permission.pending",
    }) as const,
};
const unusedInteractions: AcpInteractionBrokerPort = {
  health: healthy,
  request: async () => ({ kind: "cancelled" }),
};
const ready = Object.freeze({
  handlerInstalled: true,
  authorized: true,
  tested: true,
  healthy: true,
});

describe("AcpAuthorityBridge", () => {
  it("maps structured faults to bounded protocol errors", () => {
    expect(toAcpAuthorityProtocolError(new AcpAuthorityFault("unsupported"))).toMatchObject({
      code: -32601,
      data: { reason: "unsupported", retryable: false },
    });
    expect(toAcpAuthorityProtocolError(new AcpAuthorityFault("generation_mismatch"))).toMatchObject(
      { code: -32002 },
    );
    expect(toAcpAuthorityProtocolError(new AcpAuthorityFault("timed_out"))).toMatchObject({
      code: -32001,
      data: { retryable: true },
    });
    expect(toAcpAuthorityProtocolError(new AcpAuthorityFault("overloaded"))).toMatchObject({
      code: -32005,
    });
    expect(toAcpAuthorityProtocolError(new AcpAuthorityFault("cancelled"))).toMatchObject({
      code: -32800,
    });
    expect(
      JSON.stringify(toAcpAuthorityProtocolError(new AcpAuthorityFault("broker_failure"))),
    ).not.toContain("secret");
  });
  it("captures one truthful immutable capability snapshot from installed ports", () => {
    const filesystem = {
      health: healthy,
      readTextFile: async () => ({ value: { content: "fixture" } }),
    };
    const terminal = {
      health: healthy,
      create: async () => ({ value: { terminalId: "terminal.1" } }),
      output: async () => ({ value: { output: "", truncated: false } }),
      waitForExit: async () => ({ value: { exitCode: 0 } }),
      kill: async () => ({ value: {} }),
      release: async () => ({ value: {} }),
    };
    const mcp: NonNullable<AcpAuthorityBridgeOptions["mcp"]> = {
      health: healthy,
      supportedTransports: ["http"],
      materializeForSessionNew: async (_request, _lease, launch) => ({ value: await launch([]) }),
    };
    const bridge = new AcpAuthorityBridge(
      makeOptions({
        filesystem,
        terminal,
        mcp,
      }),
    );

    expect(bridge.capabilities).toEqual({
      connectionRef: context.connectionRef,
      generation: 7,
      permission: false,
      filesystem: { readTextFile: true, writeTextFile: false },
      terminal: true,
      mcpTransports: ["http"],
    });
    expect(Object.isFrozen(bridge.capabilities)).toBe(true);
    expect(Object.isFrozen(bridge.capabilities.filesystem)).toBe(true);
    expect(Object.isFrozen(bridge.capabilities.mcpTransports)).toBe(true);
    expect(projectAcpClientCapabilities(bridge.capabilities)).toEqual({
      fs: { readTextFile: true, writeTextFile: false },
      terminal: true,
    });

    const noProof = new AcpAuthorityBridge(
      makeOptions({ filesystem, terminal, mcp, readiness: {} }),
    );
    expect(noProof.capabilities).toMatchObject({
      filesystem: { readTextFile: false, writeTextFile: false },
      terminal: false,
      mcpTransports: [],
    });
    const partialProof = new AcpAuthorityBridge(
      makeOptions({
        filesystem,
        readiness: {
          filesystem: {
            readTextFile: { ...ready, tested: false },
          },
        },
      }),
    );
    expect(partialProof.capabilities.filesystem.readTextFile).toBe(false);
  });

  it("queues permission interactions and returns only the exact validated ACP option", async () => {
    const deliveries: string[] = [];
    const interactions: AcpInteractionBrokerPort = {
      health: healthy,
      request: async (interaction, { delivery }) => {
        deliveries.push(delivery);
        if (interaction.payload.kind !== "tool_approval") throw new Error("wrong payload");
        return {
          kind: "decision",
          envelope: {
            decisionRef: "decision.permission.1",
            idempotencyKey: "idem.permission.1",
            decidedAt: "2026-07-16T13:00:00.000Z",
            surface: "desktop",
            decision: {
              kind: "tool_approval",
              outcome: "approve",
            },
          },
          evidenceRefs: ["evidence.private.permission.1"],
        };
      },
    };
    const bridge = new AcpAuthorityBridge(
      makeOptions({ interactions, permissionPolicy: escalatingPermission }),
    );
    const response = await bridge.requestPermission(
      {
        sessionId: context.sessionId,
        toolCall: { toolCallId: "tool.1", title: "Write selected file" },
        options: [
          { optionId: "allow.once", name: "Allow", kind: "allow_once" },
          { optionId: "reject.once", name: "Reject", kind: "reject_once" },
        ],
      },
      context,
    );

    expect(response).toEqual({ outcome: { outcome: "selected", optionId: "allow.once" } });
    expect(deliveries).toEqual(["queue"]);
    expect(pendingInteractionDelivery("provider_question", true)).toBe("steer");
    expect(pendingInteractionDelivery("provider_question", false)).toBe("queue");
    expect(pendingInteractionDelivery("permission", true)).toBe("queue");
    expect(routeAcpUserInput(true)).toBe("queue-next-prompt");
    expect(routeAcpUserInput(false)).toBe("start-prompt");
    expect(routeAcpInteractionDecision()).toBe("steer-active-interaction");
  });

  it("rejects an unknown permission selection through RuntimeInteraction validation", async () => {
    const interactions: AcpInteractionBrokerPort = {
      health: healthy,
      request: async (interaction) => ({
        kind: "decision",
        envelope: {
          decisionRef: "decision.permission.invalid",
          idempotencyKey: "idem.permission.invalid",
          decidedAt: "2026-07-16T13:00:00.000Z",
          surface: "desktop",
          decision: {
            kind: "provider_question",
            answers: [
              {
                questionRef:
                  interaction.payload.kind === "provider_question"
                    ? interaction.payload.questions[0]!.questionRef
                    : "question.invalid",
                optionRefs: ["option.not_offered"],
              },
            ],
          },
        },
      }),
    };
    const bridge = new AcpAuthorityBridge(
      makeOptions({ interactions, permissionPolicy: escalatingPermission }),
    );
    await expect(
      bridge.requestPermission(
        {
          sessionId: context.sessionId,
          toolCall: { toolCallId: "tool.1", title: "Write selected file" },
          options: [{ optionId: "allow.once", name: "Allow", kind: "allow_once" }],
        },
        context,
      ),
    ).rejects.toEqual(new AcpAuthorityFault("invalid_decision"));
  });

  it("returns cancellation exactly without manufacturing a selected outcome", async () => {
    const bridge = new AcpAuthorityBridge(
      makeOptions({
        interactions: {
          health: healthy,
          request: async () => ({ kind: "cancelled" }),
        },
        permissionPolicy: escalatingPermission,
      }),
    );
    await expect(
      bridge.requestPermission(
        {
          sessionId: context.sessionId,
          toolCall: { toolCallId: "tool.1", title: "Run command" },
          options: [
            { optionId: "allow.once", name: "Allow", kind: "allow_once" },
            { optionId: "reject.once", name: "Reject", kind: "reject_once" },
          ],
        },
        context,
      ),
    ).resolves.toEqual({ outcome: { outcome: "cancelled" } });
  });

  it("gates filesystem, terminal, and MCP calls by session, generation, scope, and health", async () => {
    const leases: Array<Readonly<{ method: string; scopeRef: string }>> = [];
    const bridge = new AcpAuthorityBridge(
      makeOptions({
        filesystem: {
          health: healthy,
          readTextFile: async (_request, lease) => {
            leases.push({ method: "read", scopeRef: lease.scopeRef });
            return { value: { content: "private file body" } };
          },
          writeTextFile: async (_request, lease) => {
            leases.push({ method: "write", scopeRef: lease.scopeRef });
            return { value: {} };
          },
        },
        terminal: {
          health: healthy,
          create: async (_request, lease) => {
            leases.push({ method: "terminal", scopeRef: lease.scopeRef });
            return { value: { terminalId: "terminal.1" } };
          },
          output: async () => ({ value: { output: "", truncated: false } }),
          waitForExit: async () => ({ value: { exitCode: 0 } }),
          kill: async () => ({ value: {} }),
          release: async () => ({ value: {} }),
        },
        mcp: {
          health: healthy,
          supportedTransports: ["http"],
          materializeForSessionNew: async (_request, lease, launch) => {
            leases.push({ method: "mcp", scopeRef: lease.scopeRef });
            return {
              value: await launch([
                { type: "http", name: "brokered", url: "https://mcp.invalid", headers: [] },
              ]),
            };
          },
        },
      }),
    );

    await expect(
      bridge.readTextFile({ sessionId: context.sessionId, path: "/workspace/a.txt" }, context),
    ).resolves.toEqual({ content: "private file body" });
    await expect(
      bridge.writeTextFile(
        { sessionId: context.sessionId, path: "/workspace/a.txt", content: "private" },
        context,
      ),
    ).resolves.toEqual({});
    await expect(
      bridge.createTerminal({ sessionId: context.sessionId, command: "printf" }, context),
    ).resolves.toEqual({ terminalId: "terminal.1" });
    await expect(
      bridge.createSessionWithMcp(
        {
          cwdRef: "workspace.cwd.1",
          servers: [
            {
              transport: "http",
              serverRef: "mcp.server.brokered",
              expiresAt: "2026-07-16T14:00:00.000Z",
            },
          ],
        },
        context,
        async (material) => ({ sessionId: `session.${material[0]!.name}` }),
      ),
    ).resolves.toEqual({ sessionId: "session.brokered" });
    expect(leases).toEqual([
      { method: "read", scopeRef: context.scopeRef },
      { method: "write", scopeRef: context.scopeRef },
      { method: "terminal", scopeRef: context.scopeRef },
      { method: "mcp", scopeRef: context.scopeRef },
    ]);

    await expect(
      bridge.readTextFile(
        { sessionId: context.sessionId, path: "/workspace/a.txt" },
        { ...context, generation: 6 },
      ),
    ).rejects.toEqual(new AcpAuthorityFault("generation_mismatch"));
    await expect(
      bridge.readTextFile(
        { sessionId: context.sessionId, path: "/workspace/a.txt" },
        { ...context, scopeRef: "scope.other" },
      ),
    ).rejects.toEqual(new AcpAuthorityFault("scope_mismatch"));
  });

  it("uses canonical policy outcomes and records rejection as refused", async () => {
    const receipts: AcpAuthorityReceipt[] = [];
    const bridge = new AcpAuthorityBridge(
      makeOptions({
        receipts: {
          record: (receipt) => {
            receipts.push(receipt);
          },
        },
        permissionPolicy: {
          health: healthy,
          decide: async () => ({
            kind: "selected",
            optionId: "reject.once",
            allowed: false,
            authorityRef: "authority.deny",
            policyRef: "policy.default_deny",
            decisionRef: "decision.deny",
          }),
        },
        interactions: unusedInteractions,
      }),
    );
    await expect(
      bridge.requestPermission(
        {
          sessionId: context.sessionId,
          toolCall: { toolCallId: "tool.1", title: "Denied" },
          options: [{ optionId: "reject.once", name: "Reject", kind: "reject_once" }],
        },
        context,
      ),
    ).resolves.toEqual({ outcome: { outcome: "selected", optionId: "reject.once" } });
    expect(receipts.at(-1)).toMatchObject({ outcome: "refused" });
  });

  it("keeps MCP credentials callback-scoped and rejects expired references", async () => {
    const receipts: AcpAuthorityReceipt[] = [];
    let materialized = 0;
    const bridge = new AcpAuthorityBridge(
      makeOptions({
        receipts: {
          record: (receipt) => {
            receipts.push(receipt);
          },
        },
        mcp: {
          health: healthy,
          supportedTransports: ["http"],
          materializeForSessionNew: async (_request, _lease, launch) => {
            materialized += 1;
            return {
              value: await launch([
                {
                  type: "http",
                  name: "private",
                  url: "https://mcp.invalid",
                  headers: [{ name: "Authorization", value: "Bearer secret-canary" }],
                },
              ]),
              evidenceRefs: ["evidence.mcp.1"],
            };
          },
        },
      }),
    );
    await expect(
      bridge.createSessionWithMcp(
        {
          cwdRef: "cwd.1",
          servers: [
            { serverRef: "mcp.server.1", transport: "http", expiresAt: "2026-07-16T14:00:00.000Z" },
          ],
        },
        { ...context, requestRef: "request.mcp.1" },
        async (material) => ({ sessionId: `session.${material[0]!.name}` }),
      ),
    ).resolves.toEqual({ sessionId: "session.private" });
    expect(JSON.stringify(receipts)).not.toContain("secret-canary");
    await expect(
      bridge.createSessionWithMcp(
        {
          cwdRef: "cwd.1",
          servers: [
            {
              serverRef: "mcp.server.expired",
              transport: "http",
              expiresAt: "2026-07-16T12:00:00.000Z",
            },
          ],
        },
        { ...context, requestRef: "request.mcp.expired" },
        async () => ({ sessionId: "no" }),
      ),
    ).rejects.toEqual(new AcpAuthorityFault("interaction_expired"));
    await expect(
      bridge.createSessionWithMcp(
        {
          cwdRef: "cwd.1",
          servers: [
            { serverRef: "mcp.server.invalid-date", transport: "http", expiresAt: "not-a-date" },
          ],
        },
        { ...context, requestRef: "request.mcp.invalid-date" },
        async () => ({ sessionId: "no" }),
      ),
    ).rejects.toEqual(new AcpAuthorityFault("interaction_expired"));
    expect(materialized).toBe(1);
  });

  it("fails closed on broker health and absent capability", async () => {
    const unhealthy = new AcpAuthorityBridge(
      makeOptions({
        filesystem: {
          health: async () => "unhealthy",
          readTextFile: async () => ({ value: { content: "must not run" } }),
        },
      }),
    );
    await expect(
      unhealthy.readTextFile({ sessionId: context.sessionId, path: "/private" }, context),
    ).rejects.toEqual(new AcpAuthorityFault("authority_unhealthy"));

    const absent = new AcpAuthorityBridge(makeOptions());
    await expect(
      absent.writeTextFile(
        { sessionId: context.sessionId, path: "/private", content: "secret" },
        context,
      ),
    ).rejects.toEqual(new AcpAuthorityFault("capability_not_advertised"));
  });

  it("binds wire session IDs and makes replayed effects idempotent", async () => {
    let writes = 0;
    const bridge = new AcpAuthorityBridge(
      makeOptions({
        filesystem: {
          health: healthy,
          writeTextFile: async () => {
            writes += 1;
            return { value: {} };
          },
        },
      }),
    );
    const request = { sessionId: context.sessionId, path: "/workspace/a", content: "one" };
    await bridge.writeTextFile(request, context);
    await bridge.writeTextFile(request, context);
    expect(writes).toBe(1);
    await expect(
      bridge.writeTextFile({ ...request, content: "conflict" }, context),
    ).rejects.toEqual(new AcpAuthorityFault("decision_conflict"));
    await expect(
      bridge.writeTextFile(
        { ...request, sessionId: "session.foreign" },
        { ...context, requestRef: "request.foreign" },
      ),
    ).rejects.toEqual(new AcpAuthorityFault("session_not_found"));
  });

  it("bounds reverse replay state and returns overload", async () => {
    const bridge = new AcpAuthorityBridge(
      makeOptions({
        maxReplayEntries: 1,
        filesystem: { health: healthy, readTextFile: async () => ({ value: { content: "one" } }) },
      }),
    );
    await bridge.readTextFile(
      { sessionId: context.sessionId, path: "/workspace/a" },
      { ...context, requestRef: "request.bound.1" },
    );
    await expect(
      bridge.readTextFile(
        { sessionId: context.sessionId, path: "/workspace/b" },
        { ...context, requestRef: "request.bound.2" },
      ),
    ).rejects.toEqual(new AcpAuthorityFault("overloaded"));
    bridge.close();
    await expect(
      bridge.readTextFile(
        { sessionId: context.sessionId, path: "/workspace/b" },
        { ...context, requestRef: "request.bound.2" },
      ),
    ).resolves.toEqual({ content: "one" });
  });

  it("emits structured receipts without request bodies, paths, output, or broker errors", async () => {
    const receipts: AcpAuthorityReceipt[] = [];
    const canaries = ["/secret/path", "secret-file-body", "broker-secret-error"];
    const bridge = new AcpAuthorityBridge(
      makeOptions({
        receipts: {
          record: (receipt) => {
            receipts.push(receipt);
          },
        },
        filesystem: {
          health: healthy,
          readTextFile: async () => ({
            value: { content: canaries[1]! },
            evidenceRefs: ["evidence.safe.1", "/unsafe evidence"],
          }),
          writeTextFile: async () => {
            throw new Error(canaries[2]);
          },
        },
      }),
    );
    await bridge.readTextFile({ sessionId: context.sessionId, path: canaries[0]! }, context);
    await expect(
      bridge.writeTextFile(
        { sessionId: context.sessionId, path: canaries[0]!, content: canaries[1]! },
        context,
      ),
    ).rejects.toEqual(new AcpAuthorityFault("broker_failure"));

    expect(receipts).toHaveLength(2);
    expect(receipts[0]).toMatchObject({
      method: "fs/read_text_file",
      outcome: "allowed",
      evidenceRefs: ["evidence.safe.1", "evidence.redacted"],
      redaction: "safe-metadata-only",
    });
    expect(receipts[1]).toMatchObject({
      method: "fs/write_text_file",
      outcome: "refused",
      faultCode: "broker_failure",
    });
    const encoded = JSON.stringify(receipts);
    for (const canary of canaries) expect(encoded).not.toContain(canary);
  });

  it("records invalid broker responses as refused and preserves cancellation", async () => {
    const receipts: AcpAuthorityReceipt[] = [];
    const invalid = new AcpAuthorityBridge(
      makeOptions({
        receipts: {
          record: (receipt) => {
            receipts.push(receipt);
          },
        },
        filesystem: { health: healthy, readTextFile: async () => ({ value: {} as never }) },
      }),
    );
    await expect(
      invalid.readTextFile(
        { sessionId: context.sessionId, path: "/workspace/a" },
        { ...context, requestRef: "request.invalid.result" },
      ),
    ).rejects.toEqual(new AcpAuthorityFault("broker_failure"));
    expect(receipts).toMatchObject([{ outcome: "refused", faultCode: "broker_failure" }]);
    const aborted = Object.assign(new Error("private abort"), {
      name: "NodeBrokerFault",
      code: "aborted",
    });
    const cancelled = new AcpAuthorityBridge(
      makeOptions({
        filesystem: {
          health: healthy,
          readTextFile: async () => {
            throw aborted;
          },
        },
      }),
    );
    await expect(
      cancelled.readTextFile(
        { sessionId: context.sessionId, path: "/workspace/a" },
        { ...context, requestRef: "request.aborted" },
      ),
    ).rejects.toEqual(new AcpAuthorityFault("cancelled"));
  });
});
