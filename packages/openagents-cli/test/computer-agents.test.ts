import { Effect } from "effect";
import { describe, expect, it, vi } from "vitest";

import {
  buildAgentCatalog,
  ComputerAgentProcess,
  computerAgentProcessNodeLayer,
  permissionAllowed,
  resolveAgent,
  startAgentDelegation,
  type AgentCatalogEntry,
  type AgentProcess,
} from "../src/computer-agents.js";

const localOpenCode = (present = true) => [{ name: "opencode", present, version: "1.2.3" }];

describe("ACP agent catalog and client", () => {
  it("speaks bounded JSON-RPC over a direct argv stdio process", async () => {
    const script = `
      const readline = require("node:readline");
      let promptId;
      readline.createInterface({ input: process.stdin }).on("line", (line) => {
        const request = JSON.parse(line);
        if (request.method === "initialize") {
          process.stdout.write(JSON.stringify({
            jsonrpc: "2.0",
            method: "session/update",
            params: { update: "ready" }
          }) + "\\n");
          process.stdout.write(JSON.stringify({
            jsonrpc: "2.0",
            id: request.id,
            result: { agentCapabilities: {} }
          }) + "\\n");
        } else if (request.method === "session/prompt") {
          promptId = request.id;
          process.stdout.write(JSON.stringify({
            jsonrpc: "2.0",
            id: "permission-1",
            method: "session/request_permission",
            params: { toolCall: { kind: "read", title: "read", rawInput: {} } }
          }) + "\\n");
        } else if (request.id === "permission-1") {
          process.stdout.write(JSON.stringify({
            jsonrpc: "2.0",
            id: promptId,
            result: { stopReason: "end_turn" }
          }) + "\\n");
        }
      });
    `;
    const factory = await Effect.runPromise(
      ComputerAgentProcess.pipe(Effect.provide(computerAgentProcessNodeLayer)),
    );
    const agent = await Effect.runPromise(
      factory.start(
        {
          id: "stub",
          argv: [process.execPath, "-e", script],
          source: "configured",
          version: "",
          env: [],
        },
        process.cwd(),
        { PATH: process.env.PATH ?? "" },
      ),
    );
    const updates: unknown[] = [];
    agent.onNotification("session/update", (params) => updates.push(params));
    agent.onRequest("session/request_permission", () => ({ outcome: "selected" }));
    await expect(agent.request("initialize", {})).resolves.toEqual({
      agentCapabilities: {},
    });
    await expect(agent.request("session/prompt", {})).resolves.toEqual({
      stopReason: "end_turn",
    });
    expect(updates).toEqual([{ update: "ready" }]);
    await agent.terminate();
  });

  it("advertises discovered OpenCode and lets owner configuration override it", () => {
    const catalog = buildAgentCatalog(
      {
        agents: [{ id: "opencode", argv: ["custom-opencode", "--acp"], env: ["XAI_API_KEY"] }],
        registryAgents: false,
      },
      localOpenCode(),
    );
    expect(catalog).toEqual([
      {
        id: "opencode",
        argv: ["custom-opencode", "--acp"],
        source: "configured",
        version: "",
        env: ["XAI_API_KEY"],
      },
    ]);
    expect(resolveAgent(catalog, "missing")).toEqual({
      _tag: "unavailable",
      requestedId: "missing",
      availableIds: ["opencode"],
    });
  });

  it("does not resolve a missing local agent remotely", () => {
    const catalog = buildAgentCatalog({ agents: [], registryAgents: true }, localOpenCode(false));
    expect(catalog).toEqual([]);
    expect(resolveAgent(catalog, "opencode")._tag).toBe("unavailable");
    expect(
      resolveAgent(catalog, "remote", {
        registryAgents: false,
        resolveRemote: () => ({
          id: "remote",
          argv: ["remote-agent"],
          source: "remote",
          version: "",
          env: [],
        }),
      })._tag,
    ).toBe("unavailable");
    expect(
      resolveAgent(catalog, "remote", {
        registryAgents: true,
        resolveRemote: (id) => ({
          id,
          argv: ["remote-agent"],
          source: "remote",
          version: "",
          env: [],
        }),
      }),
    ).toMatchObject({ _tag: "resolved", entry: { source: "remote" } });
  });

  it("maps each local permission tier without selecting a blanket bypass", () => {
    const roots = ["/workspace/project"];
    expect(permissionAllowed("probe", { kind: "read", title: "", rawInput: {} }, roots)).toBe(
      false,
    );
    expect(permissionAllowed("curated", { kind: "read", title: "", rawInput: {} }, roots)).toBe(
      true,
    );
    expect(
      permissionAllowed(
        "curated",
        {
          kind: "edit",
          title: "",
          rawInput: { path: "/workspace/project/a.ts" },
        },
        roots,
      ),
    ).toBe(true);
    expect(
      permissionAllowed(
        "curated",
        {
          kind: "edit",
          title: "",
          rawInput: { path: "/tmp/a.ts" },
        },
        roots,
      ),
    ).toBe(false);
    expect(
      permissionAllowed(
        "curated",
        {
          kind: "edit",
          title: "",
          rawInput: { path: "/workspace/project/a.ts" },
        },
        [],
      ),
    ).toBe(false);
    expect(
      permissionAllowed(
        "curated",
        {
          kind: "execute",
          title: "",
          rawInput: { command: "git status" },
        },
        roots,
      ),
    ).toBe(true);
    expect(
      permissionAllowed(
        "curated",
        {
          kind: "execute",
          title: "",
          rawInput: { command: "curl https://example.com" },
        },
        roots,
      ),
    ).toBe(false);
    expect(permissionAllowed("shell", { kind: "execute", title: "", rawInput: {} }, roots)).toBe(
      true,
    );
    expect(
      permissionAllowed(
        "shell",
        {
          kind: "execute",
          title: "sudo rm -rf /",
          rawInput: {},
        },
        roots,
      ),
    ).toBe(false);
  });

  it("initializes, reports the session before prompting, streams bounded output, and ends once", async () => {
    const calls: string[] = [];
    let promptText = "";
    let permissionHandler: ((params: unknown) => Promise<unknown>) | undefined;
    let resolvePrompt: ((value: unknown) => void) | undefined;
    const process: AgentProcess = {
      request: async (method, params) => {
        calls.push(method);
        if (method === "initialize") return { agentCapabilities: {} };
        if (method === "session/new") return { sessionId: "session-1" };
        if (method === "session/prompt") {
          const value = params as { readonly prompt?: ReadonlyArray<{ readonly text?: string }> };
          promptText = value.prompt?.[0]?.text ?? "";
          return await new Promise((resolve) => {
            resolvePrompt = resolve;
          });
        }
        return {};
      },
      notify: () => undefined,
      onNotification: (_method, handler) => {
        handler({ update: { content: [{ type: "text", text: "hello" }] } });
        return () => undefined;
      },
      onRequest: (_method, handler) => {
        permissionHandler = async (params) =>
          handler(params, {
            method: "session/request_permission",
            requestId: "permission-1",
            signal: new AbortController().signal,
            generation: 1,
          });
        return () => undefined;
      },
      terminate: async () => undefined,
    };
    const entry: AgentCatalogEntry = {
      id: "opencode",
      argv: ["opencode", "acp"],
      source: "local",
      version: "1.2.3",
      env: [],
    };
    const chunks: string[] = [];
    const sessions: string[] = [];
    const permissions: string[] = [];
    const job = startAgentDelegation(
      {
        start: () => Effect.succeed(process),
      },
      {
        entry,
        prompt: "x".repeat(40_000),
        cwd: "/workspace/project",
        tier: "probe",
        roots: ["/workspace/project"],
        curatedExecute: [],
        env: { PATH: "/bin" },
        timeoutMs: 1_000,
        maximumOutputBytes: 3,
        onChunk: (chunk) => chunks.push(chunk),
        onSession: (sessionId) => sessions.push(sessionId),
        onPermission: (allowed, detail) => permissions.push(`${allowed}:${detail}`),
      },
    );
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    await Promise.resolve();
    expect(permissionHandler).toBeDefined();
    expect(
      await permissionHandler?.({
        toolCall: { kind: "read", title: "read", rawInput: {} },
        options: [
          { kind: "allow_once", optionId: "allow" },
          { kind: "reject_once", optionId: "reject" },
        ],
      }),
    ).toEqual({ outcome: { outcome: "selected", optionId: "reject" } });
    resolvePrompt?.({ stopReason: "end_turn" });
    const outcome = await job.done;
    expect(calls).toEqual(["initialize", "session/new", "session/prompt"]);
    expect(sessions).toEqual(["session-1"]);
    expect(chunks).toEqual(["hel"]);
    expect(outcome.status).toBe("truncated");
    expect(outcome.output).toBe("hel");
    expect(promptText).toHaveLength(32_768);
    expect(permissions).toEqual(["false:read: read"]);
  });

  it("loads a resumable session and bounds an impossible reattachment", async () => {
    const methods: string[] = [];
    const process: AgentProcess = {
      request: async (method) => {
        methods.push(method);
        if (method === "initialize") return { agentCapabilities: { loadSession: true } };
        if (method === "session/prompt") return { stopReason: "end_turn" };
        return {};
      },
      notify: () => undefined,
      onNotification: () => () => undefined,
      onRequest: () => () => undefined,
      terminate: async () => undefined,
    };
    const outcome = await startAgentDelegation(
      { start: () => Effect.succeed(process) },
      {
        entry: { id: "opencode", argv: ["opencode", "acp"], source: "local", version: "", env: [] },
        prompt: "continue",
        cwd: "/workspace/project",
        resumeSessionId: "existing",
        tier: "curated",
        roots: ["/workspace/project"],
        curatedExecute: [],
        env: {},
        timeoutMs: 1_000,
        maximumOutputBytes: 100,
        onChunk: () => undefined,
        onSession: () => undefined,
        onPermission: () => undefined,
      },
    ).done;
    expect(methods).toEqual(["initialize", "session/load", "session/prompt"]);
    expect(outcome.sessionId).toBe("existing");

    const failed = await startAgentDelegation(
      {
        start: () =>
          Effect.succeed({
            ...process,
            request: async (method: string) =>
              method === "initialize" ? { agentCapabilities: {} } : {},
          }),
      },
      {
        entry: { id: "opencode", argv: ["opencode", "acp"], source: "local", version: "", env: [] },
        prompt: "continue",
        cwd: "/workspace/project",
        resumeSessionId: "lost",
        tier: "curated",
        roots: ["/workspace/project"],
        curatedExecute: [],
        env: {},
        timeoutMs: 1_000,
        maximumOutputBytes: 100,
        onChunk: () => undefined,
        onSession: () => undefined,
        onPermission: () => undefined,
      },
    ).done;
    expect(failed.status).toBe("failed");
    expect(failed.detail).toContain("reattach");
  });

  it("cancels the ACP session and reports a cancelled terminal result", async () => {
    vi.useFakeTimers();
    const notifications: string[] = [];
    const process: AgentProcess = {
      request: async (method) => {
        if (method === "initialize") return { agentCapabilities: {} };
        if (method === "session/new") return { sessionId: "session-cancel" };
        return await new Promise(() => undefined);
      },
      notify: (method) => notifications.push(method),
      onNotification: () => () => undefined,
      onRequest: () => () => undefined,
      terminate: async () => undefined,
    };
    const job = startAgentDelegation(
      { start: () => Effect.succeed(process) },
      {
        entry: { id: "opencode", argv: ["opencode", "acp"], source: "local", version: "", env: [] },
        prompt: "cancel",
        cwd: "/workspace/project",
        tier: "curated",
        roots: ["/workspace/project"],
        curatedExecute: [],
        env: {},
        timeoutMs: 10_000,
        maximumOutputBytes: 100,
        onChunk: () => undefined,
        onSession: () => undefined,
        onPermission: () => undefined,
      },
    );
    await vi.advanceTimersByTimeAsync(0);
    job.cancel();
    await vi.advanceTimersByTimeAsync(1_000);
    await expect(job.done).resolves.toMatchObject({
      status: "cancelled",
      sessionId: "session-cancel",
    });
    expect(notifications).toContain("session/cancel");
    vi.useRealTimers();
  });
});
