import { STABLE_METHOD_MANIFEST } from "@openagentsinc/agent-client-protocol/stable";

export type StableConformanceCase = Readonly<{
  direction: "client-to-agent" | "agent-to-client" | "protocol";
  method: string;
  kind: "request" | "notification";
  params: unknown;
  result?: unknown;
  supportState: "supported" | "capability-gated";
  capabilityState: "baseline" | "present";
}>;

const sessionId = "fixture-session-1";
const terminalId = "fixture-terminal-1";

export const STABLE_CONFORMANCE_CASES: ReadonlyArray<StableConformanceCase> = [
  {
    direction: "client-to-agent",
    method: "initialize",
    kind: "request",
    params: {
      protocolVersion: 1,
      clientCapabilities: { fs: { readTextFile: true, writeTextFile: true }, terminal: true },
      clientInfo: { name: "openagents-conformance", version: "0.1.0" },
    },
    result: {
      protocolVersion: 1,
      agentCapabilities: {
        loadSession: true,
        promptCapabilities: { image: true, audio: true, embeddedContext: true },
        mcpCapabilities: { http: true, sse: true },
        sessionCapabilities: { list: {}, resume: {}, close: {}, delete: {} },
        auth: { logout: {} },
      },
      authMethods: [{ id: "cached_token", name: "Cached token" }],
      agentInfo: { name: "scripted-peer", version: "1.0.0" },
    },
    supportState: "supported",
    capabilityState: "baseline",
  },
  {
    direction: "client-to-agent",
    method: "authenticate",
    kind: "request",
    params: { methodId: "cached_token", _meta: { headless: true } },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "client-to-agent",
    method: "session/new",
    kind: "request",
    params: { cwd: "/workspace", mcpServers: [] },
    result: { sessionId },
    supportState: "supported",
    capabilityState: "baseline",
  },
  {
    direction: "client-to-agent",
    method: "session/load",
    kind: "request",
    params: { cwd: "/workspace", mcpServers: [], sessionId },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "client-to-agent",
    method: "session/set_mode",
    kind: "request",
    params: { sessionId, modeId: "agent" },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "client-to-agent",
    method: "session/set_config_option",
    kind: "request",
    params: { sessionId, configId: "model", value: "fixture-model" },
    result: { configOptions: [] },
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "client-to-agent",
    method: "session/prompt",
    kind: "request",
    params: { sessionId, prompt: [{ type: "text", text: "fixture prompt" }] },
    result: { stopReason: "end_turn" },
    supportState: "supported",
    capabilityState: "baseline",
  },
  {
    direction: "client-to-agent",
    method: "session/cancel",
    kind: "notification",
    params: { sessionId },
    supportState: "supported",
    capabilityState: "baseline",
  },
  {
    direction: "client-to-agent",
    method: "session/list",
    kind: "request",
    params: {},
    result: { sessions: [] },
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "client-to-agent",
    method: "session/delete",
    kind: "request",
    params: { sessionId },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "client-to-agent",
    method: "session/resume",
    kind: "request",
    params: { sessionId, cwd: "/workspace", mcpServers: [] },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "client-to-agent",
    method: "session/close",
    kind: "request",
    params: { sessionId },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "client-to-agent",
    method: "logout",
    kind: "request",
    params: {},
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "agent-to-client",
    method: "session/request_permission",
    kind: "request",
    params: {
      sessionId,
      toolCall: { toolCallId: "tool-1", title: "Write fixture", kind: "edit", status: "pending" },
      options: [{ optionId: "allow", name: "Allow", kind: "allow_once" }],
    },
    result: { outcome: { outcome: "selected", optionId: "allow" } },
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "agent-to-client",
    method: "session/update",
    kind: "notification",
    params: {
      sessionId,
      update: { sessionUpdate: "agent_message_chunk", content: { type: "text", text: "hello" } },
    },
    supportState: "supported",
    capabilityState: "baseline",
  },
  {
    direction: "agent-to-client",
    method: "fs/write_text_file",
    kind: "request",
    params: { sessionId, path: "/workspace/a.txt", content: "fixture" },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "agent-to-client",
    method: "fs/read_text_file",
    kind: "request",
    params: { sessionId, path: "/workspace/a.txt", line: 1, limit: 20 },
    result: { content: "fixture" },
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "agent-to-client",
    method: "terminal/create",
    kind: "request",
    params: {
      sessionId,
      command: "printf",
      args: ["fixture"],
      env: [],
      cwd: "/workspace",
      outputByteLimit: 1024,
    },
    result: { terminalId },
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "agent-to-client",
    method: "terminal/output",
    kind: "request",
    params: { sessionId, terminalId },
    result: { output: "fixture", truncated: false, exitStatus: { exitCode: 0, signal: null } },
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "agent-to-client",
    method: "terminal/release",
    kind: "request",
    params: { sessionId, terminalId },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "agent-to-client",
    method: "terminal/wait_for_exit",
    kind: "request",
    params: { sessionId, terminalId },
    result: { exitCode: 0, signal: null },
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "agent-to-client",
    method: "terminal/kill",
    kind: "request",
    params: { sessionId, terminalId },
    result: {},
    supportState: "capability-gated",
    capabilityState: "present",
  },
  {
    direction: "protocol",
    method: "$/cancel_request",
    kind: "notification",
    params: { requestId: 7 },
    supportState: "supported",
    capabilityState: "baseline",
  },
];

export const stableCaseKey = (value: Pick<StableConformanceCase, "direction" | "method">) =>
  `${value.direction}:${value.method}`;

export const assertStableManifestCoverage = (): Readonly<{ covered: number; manifest: number }> => {
  const actual = new Set(STABLE_CONFORMANCE_CASES.map(stableCaseKey));
  const expected = new Set(STABLE_METHOD_MANIFEST.members.map(stableCaseKey));
  const missing = [...expected].filter((key) => !actual.has(key));
  const extra = [...actual].filter((key) => !expected.has(key));
  if (missing.length > 0 || extra.length > 0 || actual.size !== STABLE_CONFORMANCE_CASES.length) {
    throw new Error(
      `stable conformance drift: missing=${missing.join(",")} extra=${extra.join(",")}`,
    );
  }
  return { covered: actual.size, manifest: expected.size };
};
