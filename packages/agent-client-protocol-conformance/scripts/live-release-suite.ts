import { execFile } from "node:child_process";
import { createHash, randomBytes } from "node:crypto";
import { mkdir, mkdtemp, readFile, readdir, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { isAbsolute, join, resolve, sep } from "node:path";
import { promisify } from "node:util";

import {
  createCursorAcpPeerRuntime,
  probeCursorAcpExecutable,
} from "@openagentsinc/cursor-agent-runtime";
import { CURSOR_ACP_PROFILE } from "@openagentsinc/agent-client-protocol/extensions/cursor";
import { GROK_ACP_PROFILE } from "@openagentsinc/agent-client-protocol/extensions/grok";
import { createGrokAcpPeerRuntime, probeGrokAcpExecutable } from "@openagentsinc/grok-harness";
import { AgentStdioTransport } from "@openagentsinc/agent-stdio-transport";

import {
  buildAcpLiveReleaseArtifact,
  validateAcpLiveReleaseArtifact,
  type AcpLiveReleasePeerReceipt,
  type AcpLiveReleaseScenarioReceipt,
} from "../src/live-release.ts";

const execFileAsync = promisify(execFile);
const arm = process.env.ACP_RELEASE_LIVE;
if (arm !== "1") {
  process.stderr.write(
    "Set ACP_RELEASE_LIVE=1 to run real Grok/Cursor processes in disposable Git workspaces.\n",
  );
  process.exit(2);
}

const selection = process.env.ACP_RELEASE_PEER ?? "both";
if (selection !== "grok" && selection !== "cursor" && selection !== "both")
  throw new TypeError("ACP_RELEASE_PEER must be grok, cursor, or both");

const revision = (
  await execFileAsync("git", ["rev-parse", "HEAD"], { maxBuffer: 4_096 })
).stdout.trim();
const recordedAt = new Date().toISOString();
const platform = `${process.platform}-${process.arch}-node-${process.versions.node}`;

const scenario = (
  id: AcpLiveReleaseScenarioReceipt["id"],
  result: AcpLiveReleaseScenarioReceipt["result"],
  safeDetail: string,
): AcpLiveReleaseScenarioReceipt => ({ id, result, safeDetail });

const failedPeer = (peer: "grok" | "cursor"): AcpLiveReleasePeerReceipt => ({
  peer,
  result: "fail",
  binary: {
    reportedVersion: "unavailable",
    executableSha256: "0".repeat(64),
    ...(peer === "cursor" ? { installationClosureSha256: "0".repeat(64) } : {}),
  },
  negotiation: { wireVersion: 1, authMethodIds: [], capabilityKeys: [] },
  scenarios: [
    scenario("initialize", "fail", "Live release runner failed before a complete receipt"),
  ],
  counters: {
    updateCount: 0,
    updateKinds: [],
    promptCount: 0,
    updateMetadataCount: 0,
    completionMetadataCount: 0,
    usageMetadataCount: 0,
  },
});

const capabilityKeys = (value: Readonly<Record<string, boolean>>): string[] =>
  Object.entries(value)
    .filter(([, enabled]) => enabled)
    .map(([key]) => key)
    .toSorted();

const boundedCanaryScan = async (
  roots: ReadonlyArray<string>,
  canary: string,
  modifiedSince: number,
): Promise<Readonly<{ complete: boolean; files: number; bytes: number; matches: number }>> => {
  const pending = [...new Set(roots)];
  const needle = Buffer.from(canary);
  const deadlineAt = Date.now() + 30_000;
  let files = 0;
  let bytes = 0;
  let matches = 0;
  const inspectFile = async (candidate: string): Promise<void> => {
    const metadata = await stat(candidate).catch(() => undefined);
    if (metadata === undefined || !metadata.isFile() || metadata.mtimeMs < modifiedSince) return;
    files += 1;
    if (files >= 20_000 || Date.now() >= deadlineAt) return;
    const content = await readFile(candidate).catch(() => undefined);
    if (content === undefined) return;
    bytes += content.byteLength;
    if (content.includes(needle)) matches += 1;
  };
  while (
    pending.length > 0 &&
    files < 20_000 &&
    bytes < 256 * 1_048_576 &&
    Date.now() < deadlineAt
  ) {
    const directory = pending.pop()!;
    const rootMetadata = await stat(directory).catch(() => undefined);
    if (rootMetadata?.isFile()) {
      await inspectFile(directory);
      continue;
    }
    const entries = await readdir(directory, { withFileTypes: true }).catch(() => []);
    for (const entry of entries) {
      const candidate = join(directory, entry.name);
      if (entry.isDirectory()) pending.push(candidate);
      else if (entry.isFile()) {
        await inspectFile(candidate);
        if (files >= 20_000 || bytes >= 256 * 1_048_576 || Date.now() >= deadlineAt) break;
      }
    }
  }
  return {
    complete:
      pending.length === 0 && files < 20_000 && bytes < 256 * 1_048_576 && Date.now() < deadlineAt,
    files,
    bytes,
    matches,
  };
};

const workspace = async (peer: "grok" | "cursor"): Promise<string> => {
  const root = await mkdtemp(join(tmpdir(), `openagents-acp-release-${peer}-`));
  await execFileAsync("git", ["init", "--quiet", root], { maxBuffer: 4_096 });
  await writeFile(join(root, "README.md"), "# Disposable ACP release workspace\n", {
    mode: 0o600,
  });
  return root;
};

const object = (value: unknown): Record<string, unknown> =>
  value !== null && typeof value === "object" && !Array.isArray(value)
    ? (value as Record<string, unknown>)
    : {};

const qualifyCursorExtensions = async (
  root: string,
  probe: Awaited<ReturnType<typeof probeCursorAcpExecutable>>,
): Promise<
  Readonly<{
    questions: number;
    plans: number;
    todos: number;
    models: number;
  }>
> => {
  let transport: AgentStdioTransport | undefined;
  let questions = 0;
  let plans = 0;
  let todos = 0;
  try {
    transport = await AgentStdioTransport.start({
      executable: probe.realPath,
      args: ["acp"],
      cwd: root,
      env: { HOME: process.env.HOME, PATH: "/usr/bin:/bin" },
      identityPin: { realPath: probe.realPath, sha256: probe.sha256 },
      methodKinds: CURSOR_ACP_PROFILE.methods
        .filter((entry) => entry.direction === "agent-to-client")
        .map((entry) => ({ method: entry.method, kind: entry.kind })),
      limits: { requestTimeoutMs: 120_000 },
    });
    transport.registerReverseHandler("cursor/ask_question", async (params) => {
      questions += 1;
      const answers: Record<string, ReadonlyArray<string>> = {};
      const requested = object(params).questions;
      if (Array.isArray(requested)) {
        for (const rawQuestion of requested) {
          const question = object(rawQuestion);
          if (typeof question.id !== "string") continue;
          const firstOption = Array.isArray(question.options) ? object(question.options[0]) : {};
          answers[question.id] = [
            typeof firstOption.label === "string" ? firstOption.label : "Continue",
          ];
        }
      }
      return { answers };
    });
    transport.registerReverseHandler("cursor/create_plan", async () => {
      plans += 1;
      return { accepted: true };
    });
    transport.onNotification("cursor/update_todos", () => {
      todos += 1;
    });
    const initialized = object(
      await transport.request("initialize", {
        protocolVersion: 1,
        clientCapabilities: {
          fs: { readTextFile: false, writeTextFile: false },
          terminal: false,
        },
        clientInfo: { name: "openagents-release-qualification", version: "0.1.0" },
      }),
    );
    const authMethods = Array.isArray(initialized.authMethods)
      ? initialized.authMethods.map(object)
      : [];
    if (!authMethods.some((method) => method.id === "cursor_login"))
      throw new Error("Cursor did not advertise cursor_login");
    await transport.request("authenticate", {
      methodId: "cursor_login",
      _meta: { headless: true },
    });
    const attached = object(await transport.request("session/new", { cwd: root, mcpServers: [] }));
    if (typeof attached.sessionId !== "string") throw new Error("Cursor returned no session id");
    const listedModels = object(
      await transport.request("cursor/list_available_models", {}).catch(() => ({})),
    );
    const models = Array.isArray(listedModels.models) ? listedModels.models.length : 0;
    for (const [modeId, text] of [
      [
        "agent",
        "Call AskQuestion before answering. Ask one multiple-choice clarification question, accept the answer, then reply briefly.",
      ],
      [
        "plan",
        "Create a short reviewed plan and maintain a todo list for adding one sentence to README.md. Do not edit any file.",
      ],
      [
        "agent",
        "Call TodoWrite to create and then complete one harmless planning todo. Do not edit any file.",
      ],
    ] as const) {
      await transport
        .request("session/set_mode", { sessionId: attached.sessionId, modeId })
        .catch(() => undefined);
      await transport
        .request("session/prompt", {
          sessionId: attached.sessionId,
          prompt: [{ type: "text", text }],
        })
        .catch(() => undefined);
    }
    return { questions, plans, todos, models };
  } finally {
    await transport?.dispose();
  }
};

const qualifyCursorPermission = async (
  root: string,
  probe: Awaited<ReturnType<typeof probeCursorAcpExecutable>>,
  decision: "approve" | "refuse",
): Promise<number> => {
  let transport: AgentStdioTransport | undefined;
  let selections = 0;
  try {
    await mkdir(join(root, ".cursor"), { recursive: true });
    await writeFile(
      join(root, ".cursor", "cli.json"),
      `${JSON.stringify({ permissions: { allow: [], deny: [] } }, null, 2)}\n`,
      { mode: 0o600 },
    );
    transport = await AgentStdioTransport.start({
      executable: probe.realPath,
      args: ["acp"],
      cwd: root,
      env: { HOME: process.env.HOME, PATH: "/usr/bin:/bin" },
      identityPin: { realPath: probe.realPath, sha256: probe.sha256 },
      methodKinds: [{ method: "session/request_permission", kind: "request" }],
      limits: { requestTimeoutMs: 120_000 },
    });
    transport.registerReverseHandler("session/request_permission", async (params) => {
      const rawOptions = object(params).options;
      const options: ReadonlyArray<Record<string, unknown>> = Array.isArray(rawOptions)
        ? rawOptions.map(object)
        : [];
      const preferred = options.find((option) =>
        decision === "approve"
          ? typeof option.kind === "string" && option.kind.startsWith("allow")
          : typeof option.kind === "string" &&
            (option.kind.startsWith("reject") || option.kind.startsWith("deny")),
      );
      if (typeof preferred?.optionId !== "string") return { outcome: { outcome: "cancelled" } };
      selections += 1;
      return { outcome: { outcome: "selected", optionId: preferred.optionId } };
    });
    const initialized = object(
      await transport.request("initialize", {
        protocolVersion: 1,
        clientCapabilities: {
          fs: { readTextFile: false, writeTextFile: false },
          terminal: false,
        },
        clientInfo: { name: "openagents-release-qualification", version: "0.1.0" },
      }),
    );
    const authMethods = Array.isArray(initialized.authMethods)
      ? initialized.authMethods.map(object)
      : [];
    if (!authMethods.some((method) => method.id === "cursor_login"))
      throw new Error("Cursor did not advertise cursor_login");
    await transport.request("authenticate", {
      methodId: "cursor_login",
      _meta: { headless: true },
    });
    const attached = object(await transport.request("session/new", { cwd: root, mcpServers: [] }));
    if (typeof attached.sessionId !== "string") throw new Error("Cursor returned no session id");
    await transport
      .request("session/prompt", {
        sessionId: attached.sessionId,
        prompt: [
          {
            type: "text",
            text:
              decision === "approve"
                ? "Run the shell command mkdir CURSOR_PERMISSION_ALLOW_PROOF now, then reply briefly."
                : "Run the shell command mkdir CURSOR_PERMISSION_REFUSE_PROOF now, then acknowledge if permission is rejected.",
          },
        ],
      })
      .catch(() => undefined);
    return selections;
  } finally {
    await transport?.dispose();
  }
};

const qualifyGrokReverse = async (
  root: string,
  probe: Awaited<ReturnType<typeof probeGrokAcpExecutable>>,
): Promise<
  Readonly<{
    questionMethods: ReadonlyArray<string>;
    permissionApprovals: number;
    permissionRefusals: number;
    filesystemCalls: number;
    terminalCalls: number;
  }>
> => {
  let transport: AgentStdioTransport | undefined;
  const questionMethods = new Set<string>();
  let permissionApprovals = 0;
  let permissionRefusals = 0;
  let filesystemCalls = 0;
  let terminalCalls = 0;
  let preferApproval = true;
  try {
    transport = await AgentStdioTransport.start({
      executable: probe.realPath,
      args: ["agent", "stdio"],
      cwd: root,
      env: { HOME: process.env.HOME },
      identityPin: { realPath: probe.realPath, sha256: probe.sha256 },
      methodKinds: GROK_ACP_PROFILE.methods.map((entry) => ({
        method: entry.method,
        kind: entry.kind,
      })),
      limits: { requestTimeoutMs: 120_000 },
    });
    for (const method of ["x.ai/ask_user_question", "_x.ai/ask_user_question"] as const) {
      transport.registerReverseHandler(method, async (params) => {
        questionMethods.add(method);
        const answers: Record<string, ReadonlyArray<string>> = {};
        const requested = object(params).questions;
        if (Array.isArray(requested)) {
          for (const rawQuestion of requested) {
            const question = object(rawQuestion);
            if (typeof question.question !== "string") continue;
            const firstOption = Array.isArray(question.options) ? object(question.options[0]) : {};
            answers[question.question] = [
              typeof firstOption.label === "string" ? firstOption.label : "Continue",
            ];
          }
        }
        return { outcome: "accepted", answers };
      });
    }
    transport.registerReverseHandler("session/request_permission", async (params) => {
      const rawOptions = object(params).options;
      const options: ReadonlyArray<Record<string, unknown>> = Array.isArray(rawOptions)
        ? rawOptions.map(object)
        : [];
      const preferred = options.find((option) =>
        preferApproval
          ? typeof option.kind === "string" && option.kind.startsWith("allow")
          : typeof option.kind === "string" &&
            (option.kind.startsWith("reject") || option.kind.startsWith("deny")),
      );
      if (typeof preferred?.optionId !== "string") return { outcome: { outcome: "cancelled" } };
      if (preferApproval) permissionApprovals += 1;
      else permissionRefusals += 1;
      return { outcome: { outcome: "selected", optionId: preferred.optionId } };
    });
    transport.registerReverseHandler("fs/read_text_file", async () => {
      filesystemCalls += 1;
      return { content: "# Disposable ACP qualification workspace\n" };
    });
    transport.registerReverseHandler("fs/write_text_file", async () => {
      filesystemCalls += 1;
      return {};
    });
    transport.registerReverseHandler("terminal/create", async () => {
      terminalCalls += 1;
      return { terminalId: "terminal.release.proof" };
    });
    transport.registerReverseHandler("terminal/output", async () => {
      terminalCalls += 1;
      return { output: "", truncated: false };
    });
    transport.registerReverseHandler("terminal/wait_for_exit", async () => {
      terminalCalls += 1;
      return { exitCode: 0 };
    });
    for (const method of ["terminal/kill", "terminal/release"] as const)
      transport.registerReverseHandler(method, async () => {
        terminalCalls += 1;
        return {};
      });
    const initialized = object(
      await transport.request("initialize", {
        protocolVersion: 1,
        clientCapabilities: {
          fs: { readTextFile: true, writeTextFile: true },
          terminal: true,
        },
        clientInfo: { name: "openagents-release-qualification", version: "0.1.0" },
      }),
    );
    const authMethods = Array.isArray(initialized.authMethods)
      ? initialized.authMethods.map(object)
      : [];
    if (!authMethods.some((method) => method.id === "cached_token"))
      throw new Error("Grok did not advertise cached_token");
    await transport.request("authenticate", {
      methodId: "cached_token",
      _meta: { headless: true },
    });
    const attached = object(
      await transport.request("session/new", {
        cwd: root,
        mcpServers: [],
        _meta: {
          yoloMode: false,
          autoMode: false,
          clientIdentifier: "openagents-release-qualification",
        },
      }),
    );
    if (typeof attached.sessionId !== "string") throw new Error("Grok returned no session id");
    for (const text of [
      "Before answering, call ask_user_question with one multiple-choice question, accept the answer, then reply briefly.",
      "Use the ACP client filesystem read capability to read README.md, then reply briefly.",
      "Use the ACP client terminal capability for one harmless printf command, then reply briefly.",
    ])
      await transport
        .request("session/prompt", {
          sessionId: attached.sessionId,
          prompt: [{ type: "text", text }],
        })
        .catch(() => undefined);
    preferApproval = true;
    await transport
      .request("session/prompt", {
        sessionId: attached.sessionId,
        prompt: [{ type: "text", text: "Create PERMISSION_ALLOW_GROK.txt with one word." }],
      })
      .catch(() => undefined);
    preferApproval = false;
    const refusalSession = object(
      await transport.request("session/new", {
        cwd: root,
        mcpServers: [],
        _meta: {
          yoloMode: false,
          autoMode: false,
          clientIdentifier: "openagents-release-qualification",
        },
      }),
    );
    if (typeof refusalSession.sessionId !== "string")
      throw new Error("Grok returned no refusal-session id");
    await transport
      .request("session/prompt", {
        sessionId: refusalSession.sessionId,
        prompt: [
          {
            type: "text",
            text: "Run the shell command mkdir PERMISSION_REFUSE_GROK, then acknowledge if permission is rejected.",
          },
        ],
      })
      .catch(() => undefined);
    return {
      questionMethods: [...questionMethods].toSorted(),
      permissionApprovals,
      permissionRefusals,
      filesystemCalls,
      terminalCalls,
    };
  } finally {
    await transport?.dispose();
  }
};

const runGrok = async (): Promise<AcpLiveReleasePeerReceipt> => {
  const root = await workspace("grok");
  const canary = `oa-mcp-${randomBytes(24).toString("hex")}`;
  const marker = join(root, ".openagents-mcp-release-marker");
  const markerDigest = createHash("sha256").update(canary).digest("hex");
  const updates = new Set<string>();
  let updateCount = 0;
  let updateMetadataCount = 0;
  let completionMetadataCount = 0;
  let usageMetadataCount = 0;
  let assistantText = "";
  let promptCount = 0;
  let peer: Awaited<ReturnType<typeof createGrokAcpPeerRuntime>> | undefined;
  let authCancelPeer: Awaited<ReturnType<typeof createGrokAcpPeerRuntime>> | undefined;
  try {
    const probe = await probeGrokAcpExecutable();
    let authDecisionCount = 0;
    authCancelPeer = await createGrokAcpPeerRuntime({
      cwd: root,
      probe,
      environment: { HOME: process.env.HOME },
      requestedInteractiveAuthMethod: "grok.com",
      authorizeLogin: async () => {
        authDecisionCount += 1;
        return "cancel";
      },
      requestTimeoutMs: 30_000,
    });
    const authCancelled = await authCancelPeer.start();
    const authCancelPassed =
      authDecisionCount === 1 && !authCancelled.ok && authCancelled.reason === "auth_required";
    await authCancelPeer.shutdown();
    authCancelPeer = undefined;
    peer = await createGrokAcpPeerRuntime({
      cwd: root,
      probe,
      environment: { HOME: process.env.HOME },
      requestTimeoutMs: 60_000,
      materializeMcp: async (refs) => ({
        servers: refs.map((ref) => ({
          type: "stdio" as const,
          name: ref.serverRef,
          command: process.execPath,
          args: [resolve(import.meta.dirname, "live-mcp-server.mjs")],
          env: [
            { name: "OPENAGENTS_MCP_CANARY", value: canary },
            { name: "OPENAGENTS_MCP_MARKER", value: marker },
          ],
        })),
        resolvedRefs: refs.map((ref) => ({ serverRef: ref.serverRef, transport: ref.transport })),
        receiptRefs: ["receipt.acp.release.mcp"],
        dispose: () => undefined,
      }),
      onUpdate: (record) => {
        updateCount += 1;
        if (record.notificationMeta !== undefined) {
          updateMetadataCount += 1;
          if (
            typeof record.notificationMeta.totalTokens === "number" ||
            record.notificationMeta.usage !== undefined
          )
            usageMetadataCount += 1;
        }
        const update = record.update as {
          sessionUpdate?: unknown;
          content?: { text?: unknown };
        };
        if (typeof update.sessionUpdate === "string") updates.add(update.sessionUpdate);
        if (
          update.sessionUpdate === "agent_message_chunk" &&
          typeof update.content?.text === "string"
        )
          assistantText += update.content.text;
      },
    });
    const started = await peer.start();
    if (!started.ok) throw new Error(`start:${started.reason}`);
    const scenarios: AcpLiveReleaseScenarioReceipt[] = [
      scenario("identity-version", "live-pass", "Exact Grok version and executable digest probed"),
      scenario("initialize", "live-pass", "Wire version 1 initialize completed"),
      scenario("auth-primary", "live-pass", "Advertised cached authentication completed"),
      scenario(
        "auth-cancel",
        authCancelPassed ? "live-pass" : "fail",
        authCancelPassed
          ? "Client cancelled explicitly requested Grok login before authenticate"
          : "Client-side Grok login cancellation did not return auth required",
      ),
    ];
    const attached = await peer.newSession({
      cwd: root,
      canonicalThreadSeed: "release-grok",
      mcpRefs: [
        {
          serverRef: "release-proof",
          transport: "stdio",
          expiresAt: new Date(Date.now() + 300_000).toISOString(),
        },
      ],
    });
    if (!attached.ok) throw new Error(`session-new:${attached.reason}`);
    scenarios.push(scenario("session-new", "live-pass", "Disposable repository session created"));
    for (const promptText of [
      "Reply with exactly GROK_RELEASE_ONE and do not use tools.",
      "Reply with exactly GROK_RELEASE_TWO and do not use tools.",
    ]) {
      const prompted = await peer.prompt(attached.value.peerSessionId, [
        { type: "text", text: promptText },
      ]);
      promptCount += 1;
      if (!prompted.ok || prompted.value.terminal !== "completed")
        throw new Error(`prompt:${prompted.ok ? prompted.value.terminal : prompted.reason}`);
      if (prompted.value.completionMeta !== undefined) {
        completionMetadataCount += 1;
        if (
          typeof prompted.value.completionMeta.totalTokens === "number" ||
          prompted.value.completionMeta.usage !== undefined
        )
          usageMetadataCount += 1;
      }
    }
    scenarios.push(
      scenario(
        "real-repo-text",
        assistantText.length > 0 ? "live-pass" : "fail",
        assistantText.length > 0
          ? "Real disposable repository produced assistant text"
          : "No assistant text was observed",
      ),
      scenario("sequential-turns", "live-pass", "Two sequential prompts completed"),
    );
    if (started.value.capabilities.list) {
      const listed = await peer.listSessions({ cwd: root });
      scenarios.push(
        scenario(
          "session-list",
          listed.ok ? "live-pass" : "fail",
          listed.ok ? "Advertised session list completed" : `Session list failed ${listed.reason}`,
        ),
      );
    } else {
      scenarios.push(
        scenario("session-list", "not-observed", "Peer did not advertise session list"),
      );
    }
    const beforeCancel = updateCount;
    const pending = peer.prompt(attached.value.peerSessionId, [
      { type: "text", text: "Write a detailed twenty paragraph explanation of binary trees." },
    ]);
    promptCount += 1;
    await new Promise((resolve) => setTimeout(resolve, 250));
    const cancelled = await peer.cancel(attached.value.peerSessionId, "user");
    const cancelledPrompt = await pending;
    const cancelPassed =
      cancelled.ok &&
      ((!cancelledPrompt.ok && cancelledPrompt.reason === "cancelled") ||
        (cancelledPrompt.ok && cancelledPrompt.value.terminal === "cancelled"));
    scenarios.push(
      scenario(
        "stream-cancel",
        cancelPassed ? "live-pass" : "not-observed",
        cancelPassed
          ? `Streaming prompt cancelled after ${Math.max(0, updateCount - beforeCancel)} updates`
          : "Prompt completed before cancellation could be observed",
      ),
    );
    const observedMarker = (await readFile(marker, "utf8").catch(() => "")).trim();
    scenarios.push(
      scenario(
        "mcp-authorized",
        observedMarker === markerDigest ? "live-pass" : "fail",
        observedMarker === markerDigest
          ? "Brokered MCP canary reached only the disposable stdio server"
          : "Brokered MCP disposable server did not observe its canary",
      ),
    );
    await peer.shutdown();
    peer = undefined;
    const home = process.env.HOME;
    const scan = await boundedCanaryScan(
      [
        root,
        ...(home === undefined
          ? []
          : [
              ...[
                "active_sessions.json",
                "auth.json",
                "config.toml",
                "models_cache.json",
                "slash-mru.json",
                "tip_cursor.json",
                "worktrees.db",
              ].map((name) => join(home, ".grok", name)),
              join(home, ".grok", "sessions", encodeURIComponent(root)),
              join(home, ".grok", "logs", "mcp"),
              join(home, ".config", "grok"),
              join(home, "Library", "Application Support", "Grok"),
            ]),
      ],
      canary,
      Date.parse(recordedAt) - 5_000,
    );
    scenarios.push(
      scenario(
        "mcp-no-durable-secret",
        scan.complete && scan.matches === 0 ? "live-pass" : "fail",
        scan.complete && scan.matches === 0
          ? `Bounded post-shutdown scan found zero canary matches in ${scan.files} files`
          : `Bounded post-shutdown scan incomplete after ${scan.files} files or found ${scan.matches} matches`,
      ),
      scenario("cleanup-bounds", "live-pass", "Runtime completed bounded shutdown before scanning"),
    );
    const reverse = await qualifyGrokReverse(root, probe);
    scenarios.push(
      scenario(
        "grok-question-extensions",
        reverse.questionMethods.length === 2 ? "live-pass" : "not-observed",
        `Pinned question methods observed: ${reverse.questionMethods.length}`,
      ),
      scenario(
        "permission-approval",
        reverse.permissionApprovals > 0 ? "live-pass" : "not-observed",
        `Pinned permission approval selections: ${reverse.permissionApprovals}`,
      ),
      scenario(
        "permission-refusal",
        reverse.permissionRefusals > 0 ? "live-pass" : "not-observed",
        `Pinned permission refusal selections: ${reverse.permissionRefusals}`,
      ),
      scenario(
        "fs-terminal-enabled",
        reverse.filesystemCalls > 0 && reverse.terminalCalls > 0 ? "live-pass" : "not-observed",
        `Pinned reverse calls: filesystem ${reverse.filesystemCalls}, terminal ${reverse.terminalCalls}`,
      ),
    );
    return {
      peer: "grok",
      result: scenarios.some((entry) => entry.result === "fail") ? "fail" : "partial",
      binary: {
        reportedVersion: probe.reportedVersion,
        executableSha256: probe.sha256,
      },
      negotiation: {
        wireVersion: 1,
        authMethodIds: started.value.authMethodIds,
        capabilityKeys: capabilityKeys(started.value.capabilities),
      },
      scenarios,
      counters: {
        updateCount,
        updateKinds: [...updates].toSorted(),
        promptCount,
        updateMetadataCount,
        completionMetadataCount,
        usageMetadataCount,
      },
    };
  } finally {
    await authCancelPeer?.shutdown().catch(() => undefined);
    await peer?.shutdown().catch(() => undefined);
    await rm(root, { recursive: true, force: true });
  }
};

const runCursor = async (): Promise<AcpLiveReleasePeerReceipt> => {
  const root = await workspace("cursor");
  const updates = new Set<string>();
  let updateCount = 0;
  let updateMetadataCount = 0;
  let completionMetadataCount = 0;
  let usageMetadataCount = 0;
  let assistantText = "";
  let promptCount = 0;
  let peer: Awaited<ReturnType<typeof createCursorAcpPeerRuntime>> | undefined;
  let authCancelPeer: Awaited<ReturnType<typeof createCursorAcpPeerRuntime>> | undefined;
  try {
    const probe = await probeCursorAcpExecutable();
    let authDecisionCount = 0;
    authCancelPeer = await createCursorAcpPeerRuntime({
      cwd: root,
      probe,
      environment: { HOME: process.env.HOME },
      authorizeLogin: async () => {
        authDecisionCount += 1;
        return "cancel";
      },
      requestTimeoutMs: 30_000,
    });
    const authCancelled = await authCancelPeer.start();
    const authCancelPassed =
      authDecisionCount === 1 && !authCancelled.ok && authCancelled.reason === "auth_required";
    await authCancelPeer.shutdown();
    authCancelPeer = undefined;
    peer = await createCursorAcpPeerRuntime({
      cwd: root,
      probe,
      environment: { HOME: process.env.HOME },
      authorizeLogin: async () => "continue",
      requestTimeoutMs: 60_000,
      onUpdate: (record) => {
        updateCount += 1;
        if (record.notificationMeta !== undefined) {
          updateMetadataCount += 1;
          if (
            typeof record.notificationMeta.totalTokens === "number" ||
            record.notificationMeta.usage !== undefined
          )
            usageMetadataCount += 1;
        }
        const update = record.update as {
          sessionUpdate?: unknown;
          content?: { text?: unknown };
        };
        if (typeof update.sessionUpdate === "string") updates.add(update.sessionUpdate);
        if (
          update.sessionUpdate === "agent_message_chunk" &&
          typeof update.content?.text === "string"
        )
          assistantText += update.content.text;
      },
    });
    const started = await peer.start();
    if (!started.ok) throw new Error(`start:${started.reason}`);
    const scenarios: AcpLiveReleaseScenarioReceipt[] = [
      scenario(
        "identity-version",
        "live-pass",
        "Exact Cursor version and installation closure probed",
      ),
      scenario("initialize", "live-pass", "Wire version 1 initialize completed"),
      scenario("auth-primary", "live-pass", "Advertised Cursor login completed"),
      scenario(
        "auth-cancel",
        authCancelPassed ? "live-pass" : "fail",
        authCancelPassed
          ? "Client cancelled advertised login before authenticate and received auth required"
          : "Client-side login cancellation did not return the typed auth-required outcome",
      ),
    ];
    const attached = await peer.newSession({ cwd: root, canonicalThreadSeed: "release-cursor" });
    if (!attached.ok) throw new Error(`session-new:${attached.reason}`);
    scenarios.push(scenario("session-new", "live-pass", "Disposable repository session created"));
    for (const promptText of [
      "Reply with exactly CURSOR_RELEASE_ONE and do not use tools.",
      "Reply with exactly CURSOR_RELEASE_TWO and do not use tools.",
    ]) {
      const prompted = await peer.prompt(attached.value.peerSessionId, [
        { type: "text", text: promptText },
      ]);
      promptCount += 1;
      if (!prompted.ok || prompted.value.terminal !== "completed")
        throw new Error(`prompt:${prompted.ok ? prompted.value.terminal : prompted.reason}`);
      if (prompted.value.completionMeta !== undefined) {
        completionMetadataCount += 1;
        if (
          typeof prompted.value.completionMeta.totalTokens === "number" ||
          prompted.value.completionMeta.usage !== undefined
        )
          usageMetadataCount += 1;
      }
    }
    scenarios.push(
      scenario(
        "real-repo-text",
        assistantText.length > 0 ? "live-pass" : "fail",
        assistantText.length > 0
          ? "Real disposable repository produced assistant text"
          : "No assistant text was observed",
      ),
      scenario("sequential-turns", "live-pass", "Two sequential prompts completed"),
    );
    if (started.value.capabilities.list) {
      const listed = await peer.listSessions({ cwd: root });
      scenarios.push(
        scenario(
          "session-list",
          listed.ok ? "live-pass" : "fail",
          listed.ok ? "Advertised session list completed" : `Session list failed ${listed.reason}`,
        ),
      );
    }
    const modes = attached.value.modes?.availableModes ?? [];
    const alternateMode = modes.find((mode) => mode.id !== attached.value.modes?.currentModeId);
    if (alternateMode !== undefined) {
      const changed = await peer.setMode(attached.value.peerSessionId, alternateMode.id);
      scenarios.push(
        scenario(
          "model-mode-config",
          changed.ok ? "live-pass" : "fail",
          changed.ok ? "Advertised mode change completed" : `Mode change failed ${changed.reason}`,
        ),
      );
    } else {
      scenarios.push(
        scenario("model-mode-config", "not-observed", "No alternate advertised mode was available"),
      );
    }
    const beforeCancel = updateCount;
    const pending = peer.prompt(attached.value.peerSessionId, [
      { type: "text", text: "Write a detailed twenty paragraph explanation of binary trees." },
    ]);
    promptCount += 1;
    await new Promise((resolve) => setTimeout(resolve, 250));
    const cancelled = await peer.cancel(attached.value.peerSessionId, "user");
    const cancelledPrompt = await pending;
    const cancelPassed =
      cancelled.ok &&
      ((!cancelledPrompt.ok && cancelledPrompt.reason === "cancelled") ||
        (cancelledPrompt.ok && cancelledPrompt.value.terminal === "cancelled"));
    scenarios.push(
      scenario(
        "stream-cancel",
        cancelPassed ? "live-pass" : "not-observed",
        cancelPassed
          ? `Streaming prompt cancelled after ${Math.max(0, updateCount - beforeCancel)} updates`
          : "Prompt completed before cancellation could be observed",
      ),
      scenario(
        "cleanup-bounds",
        "not-observed",
        "Shutdown ran in finally; this runner does not retain process leak counters",
      ),
    );
    await peer.shutdown();
    peer = undefined;
    const extensions = await qualifyCursorExtensions(root, probe);
    const permissionApprovals = await qualifyCursorPermission(root, probe, "approve");
    const permissionRefusals = await qualifyCursorPermission(root, probe, "refuse");
    const extensionsPassed =
      extensions.questions > 0 &&
      extensions.plans > 0 &&
      extensions.todos > 0 &&
      extensions.models > 0;
    scenarios.push(
      scenario(
        "cursor-extensions-models",
        extensionsPassed ? "live-pass" : "not-observed",
        `Pinned extension counts: questions ${extensions.questions}, plans ${extensions.plans}, todos ${extensions.todos}, models ${extensions.models}`,
      ),
      scenario(
        "permission-approval",
        permissionApprovals > 0 ? "live-pass" : "not-observed",
        `Pinned permission approval selections: ${permissionApprovals}`,
      ),
      scenario(
        "permission-refusal",
        permissionRefusals > 0 ? "live-pass" : "not-observed",
        `Pinned permission refusal selections: ${permissionRefusals}`,
      ),
    );
    return {
      peer: "cursor",
      result: scenarios.some((entry) => entry.result === "fail") ? "fail" : "partial",
      binary: {
        reportedVersion: probe.reportedVersion,
        executableSha256: probe.sha256,
        ...(probe.closureSha256 === undefined
          ? {}
          : { installationClosureSha256: probe.closureSha256 }),
      },
      negotiation: {
        wireVersion: 1,
        authMethodIds: started.value.authMethodIds,
        capabilityKeys: capabilityKeys(started.value.capabilities),
      },
      scenarios,
      counters: {
        updateCount,
        updateKinds: [...updates].toSorted(),
        promptCount,
        updateMetadataCount,
        completionMetadataCount,
        usageMetadataCount,
      },
    };
  } finally {
    await authCancelPeer?.shutdown().catch(() => undefined);
    await peer?.shutdown().catch(() => undefined);
    await rm(root, { recursive: true, force: true });
  }
};

const peers: AcpLiveReleasePeerReceipt[] = [];
if (selection === "grok" || selection === "both") {
  try {
    peers.push(await runGrok());
  } catch {
    peers.push(failedPeer("grok"));
  }
}
if (selection === "cursor" || selection === "both") {
  try {
    peers.push(await runCursor());
  } catch {
    peers.push(failedPeer("cursor"));
  }
}
const artifact = buildAcpLiveReleaseArtifact({
  recordedAt,
  openAgentsRevision: revision,
  platform,
  peers,
});
const validation = validateAcpLiveReleaseArtifact(artifact);
const output = process.env.ACP_RELEASE_OUTPUT;
if (output !== undefined) {
  const liveDirectory = resolve(process.cwd(), "compatibility", "live");
  if (
    !isAbsolute(output) ||
    !resolve(output).startsWith(`${liveDirectory}${sep}`) ||
    !output.endsWith(".json")
  )
    throw new TypeError(
      "ACP_RELEASE_OUTPUT must be an absolute .json path beneath compatibility/live",
    );
  await writeFile(output, `${JSON.stringify(artifact, null, 2)}\n`, { mode: 0o600 });
}
process.stdout.write(`${JSON.stringify({ artifact, validation }, null, 2)}\n`);
if (!validation.valid || peers.some((peer) => peer.result === "fail")) process.exitCode = 1;
