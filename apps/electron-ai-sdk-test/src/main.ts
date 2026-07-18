import { createServer, type IncomingMessage, type Server, type ServerResponse } from "node:http";
import { existsSync } from "node:fs";
import { Readable } from "node:stream";
import { homedir } from "node:os";
import { join } from "node:path";
import { app, BrowserWindow, ipcMain, session } from "electron";
import type { HarnessAgent, HarnessAgentResumeSessionState } from "@ai-sdk/harness/agent";
import type { createClaudeCode as CreateClaudeCode } from "@ai-sdk/harness-claude-code";
import type { createCodex as CreateCodex } from "@ai-sdk/harness-codex";
import type { UIMessage } from "ai";
import type { LocalAiSdkSandboxProvider } from "@openagentsinc/ai-sdk-sandbox-local";

type HarnessSession = Awaited<ReturnType<typeof harnessAgent.createSession>>;
type HarnessProvider = "claude" | "codex";
type HarnessRuntime = {
  activeSessions: Set<HarnessSession>;
  agent: HarnessAgent;
  resumeStates: Map<string, HarnessAgentResumeSessionState>;
  sandbox: LocalAiSdkSandboxProvider;
};

let harnessAgent: HarnessAgent;
let createCodex: typeof CreateCodex;
let createClaudeCode: typeof CreateClaudeCode;
let aiSdk: typeof import("ai");

const harnessRuntimes = new Map<HarnessProvider, HarnessRuntime>();
const CHATGPT_CODEX_BIN = "/Applications/ChatGPT.app/Contents/Resources/codex";
let harnessServer: Server | undefined;
let harnessEndpoint: string | undefined;
let mainWindow: BrowserWindow | undefined;
let shuttingDown = false;
let didDispose = false;

void app
  .whenReady()
  .then(async () => {
    denyBrowserPrivileges();
    await initializeHarness();
    harnessEndpoint = await startHarnessServer();

    ipcMain.handle("harness:get-endpoint", () => harnessEndpoint);
    createWindow();

    app.on("activate", () => {
      if (BrowserWindow.getAllWindows().length === 0) createWindow();
    });
  })
  .catch((error: unknown) => {
    console.error("Unable to start the Electron AI SDK test:", error);
    app.quit();
  });

async function initializeHarness(): Promise<void> {
  const load = new Function("specifier", "return import(specifier);") as (
    specifier: string,
  ) => Promise<unknown>;
  const [harnessModule, claudeModule, codexModule, aiModule, sandboxModule] = await Promise.all([
    load("@ai-sdk/harness/agent"),
    load("@ai-sdk/harness-claude-code"),
    load("@ai-sdk/harness-codex"),
    load("ai"),
    load("@openagentsinc/ai-sdk-sandbox-local"),
  ]);
  const { HarnessAgent: HarnessAgentConstructor } = harnessModule as typeof import("@ai-sdk/harness/agent");
  ({ createClaudeCode } = claudeModule as typeof import("@ai-sdk/harness-claude-code"));
  ({ createCodex } = codexModule as typeof import("@ai-sdk/harness-codex"));
  aiSdk = aiModule as typeof import("ai");
  const { LocalAiSdkSandboxProvider: LocalSandboxProvider } = sandboxModule as typeof import("@openagentsinc/ai-sdk-sandbox-local");

  const codexSandbox = createLocalSandbox(LocalSandboxProvider, "codex");
  const claudeSandbox = createLocalSandbox(LocalSandboxProvider, "claude");
  harnessRuntimes.set("codex", {
    activeSessions: new Set(),
    agent: new HarnessAgentConstructor({
      harness: createLocalCodexHarness(),
      id: "openagents-electron-ai-sdk-test-codex",
      instructions:
        "You are a concise coding assistant. Work only in the owner-local harness workspace and explain results briefly.",
      sandbox: codexSandbox,
    }),
    resumeStates: new Map(),
    sandbox: codexSandbox,
  });
  harnessRuntimes.set("claude", {
    activeSessions: new Set(),
    agent: new HarnessAgentConstructor({
      harness: createLocalClaudeHarness(),
      id: "openagents-electron-ai-sdk-test-claude",
      instructions:
        "You are a concise coding assistant. Work only in the owner-local harness workspace and explain results briefly.",
      sandbox: claudeSandbox,
    }),
    resumeStates: new Map(),
    sandbox: claudeSandbox,
  });
}

function createLocalSandbox(
  LocalSandboxProvider: typeof LocalAiSdkSandboxProvider,
  provider: HarnessProvider,
): LocalAiSdkSandboxProvider {
  return new LocalSandboxProvider({
    accountHomes: {
      ...(provider === "claude"
        ? {
            claudeConfigDir: join(homedir(), ".claude"),
            // Claude Code's login state includes files at the account home;
            // reuse it for this owner-local development harness.
            home: homedir(),
          }
        : {}),
      codexHome: join(homedir(), ".codex"),
    },
    // Port zero lets each bridge bind an available localhost port. This keeps
    // Electron dev restarts from colliding with a bridge from the prior process.
    defaultPorts: [0],
    inheritClaudeConfig: provider === "claude",
    env: {
      // This deliberately uses the user's installed, signed-in Codex CLI. The
      // adapter's old bundled executable is not used by this proof of concept.
      ...(provider === "codex" ? { OPENAGENTS_CODEX_BIN: resolveCodexBin() } : {}),
      // The Claude bridge installs its pinned dependencies on first use. Its
      // bootstrap runs without a terminal, so Corepack must not wait for a
      // download confirmation.
      ...(provider === "claude"
        ? {
            COREPACK_ENABLE_DOWNLOAD_PROMPT: "0",
            OPENAGENTS_CLAUDE_BIN: resolveClaudeBin(),
          }
        : {}),
    },
    rootDirectory: join(app.getPath("userData"), "harness-workspaces", provider),
  });
}

function resolveCodexBin(): string {
  if (process.env.CODEX_BIN !== undefined) return process.env.CODEX_BIN;
  if (process.platform === "darwin" && existsSync(CHATGPT_CODEX_BIN)) {
    return CHATGPT_CODEX_BIN;
  }
  return "codex";
}

function resolveClaudeBin(): string {
  if (process.env.CLAUDE_BIN !== undefined) return process.env.CLAUDE_BIN;
  const localBin = join(homedir(), ".local", "bin", "claude");
  return existsSync(localBin) ? localBin : "claude";
}

app.on("window-all-closed", () => {
  if (process.platform !== "darwin") app.quit();
});

app.on("before-quit", (event) => {
  if (didDispose) return;
  event.preventDefault();
  if (shuttingDown) return;
  shuttingDown = true;
  void dispose().finally(() => {
    didDispose = true;
    app.quit();
  });
});

function createWindow(): void {
  mainWindow = new BrowserWindow({
    width: 980,
    height: 760,
    minWidth: 680,
    minHeight: 520,
    titleBarStyle: "hiddenInset",
    backgroundColor: "#111522",
    webPreferences: {
      contextIsolation: true,
      nodeIntegration: false,
      sandbox: true,
      webSecurity: true,
      webviewTag: false,
      preload: join(__dirname, "preload.js"),
    },
  });

  mainWindow.webContents.setWindowOpenHandler(() => ({ action: "deny" }));
  mainWindow.webContents.on("will-navigate", (event) => event.preventDefault());

  if (MAIN_WINDOW_VITE_DEV_SERVER_URL) {
    void mainWindow.loadURL(MAIN_WINDOW_VITE_DEV_SERVER_URL);
  } else {
    void mainWindow.loadFile(join(__dirname, `../renderer/${MAIN_WINDOW_VITE_NAME}/index.html`));
  }
}

function denyBrowserPrivileges(): void {
  session.defaultSession.setPermissionRequestHandler((_webContents, _permission, callback) => {
    callback(false);
  });
  session.defaultSession.setPermissionCheckHandler(() => false);
}

async function startHarnessServer(): Promise<string> {
  harnessServer = createServer((request, response) => {
    void handleHarnessRequest(request, response);
  });

  await new Promise<void>((resolve, reject) => {
    harnessServer?.once("error", reject);
    harnessServer?.listen(0, "127.0.0.1", () => resolve());
  });

  const address = harnessServer.address();
  if (address === null || typeof address === "string") {
    throw new Error("The local harness server did not receive a TCP address.");
  }
  return `http://127.0.0.1:${address.port}`;
}

async function handleHarnessRequest(
  request: IncomingMessage,
  response: ServerResponse,
): Promise<void> {
  let requestProvider: HarnessProvider | undefined;
  try {
    setCorsHeaders(response);
    if (request.method === "OPTIONS") {
      response.writeHead(204);
      response.end();
      return;
    }

    const path = new URL(request.url ?? "/", "http://127.0.0.1").pathname;
    if (request.method !== "POST" || (path !== "/api/chat" && !path.startsWith("/api/chat/"))) {
      sendJson(response, 404, { error: "not_found" });
      return;
    }

    const body = await readChatRequest(request);
    requestProvider =
      path === "/api/chat"
        ? requireProvider(body.provider ?? "codex")
        : requireProvider(path.slice("/api/chat/".length));
    const runtime = getRuntime(requestProvider);
    const chatId = requireChatId(body.id);
    const messages = await aiSdk.convertToModelMessages(body.messages);
    const harnessSession = await resumeOrCreateSession(runtime, chatId);
    runtime.activeSessions.add(harnessSession);

    const result = await runtime.agent.stream({ session: harnessSession, messages });
    const uiResponse = aiSdk.createUIMessageStreamResponse({
      stream: aiSdk.toUIMessageStream({
        stream: result.stream,
        onEnd: async () => {
          try {
            runtime.resumeStates.set(chatId, await harnessSession.detach());
          } finally {
            runtime.activeSessions.delete(harnessSession);
          }
        },
      }),
    });

    response.writeHead(
      uiResponse.status,
      Object.fromEntries(uiResponse.headers.entries()),
    );
    if (uiResponse.body === null) {
      response.end();
      return;
    }
    Readable.fromWeb(uiResponse.body as import("node:stream/web").ReadableStream).pipe(response);
  } catch (error) {
    console.error("AI SDK harness request failed:", error);
    if (!response.headersSent) {
      sendJson(response, error instanceof RequestError ? 400 : 502, {
        error:
          error instanceof RequestError
            ? error.message
            : `The ${requestProvider === "claude" ? "Claude Code" : "Codex"} harness failed.`,
      });
    } else {
      response.destroy(error instanceof Error ? error : undefined);
    }
  }
}

function getRuntime(provider: HarnessProvider): HarnessRuntime {
  const runtime = harnessRuntimes.get(provider);
  if (runtime === undefined) throw new Error(`Harness runtime ${provider} has not initialized.`);
  return runtime;
}

async function resumeOrCreateSession(
  runtime: HarnessRuntime,
  chatId: string,
): Promise<HarnessSession> {
  const resumeFrom = runtime.resumeStates.get(chatId);
  return runtime.agent.createSession(
    resumeFrom === undefined ? { sessionId: chatId } : { sessionId: chatId, resumeFrom },
  );
}

function requireProvider(value: unknown): HarnessProvider {
  if (value === "codex" || value === "claude") return value;
  throw new RequestError("Unknown harness provider.");
}

function createLocalCodexHarness() {
  const harness = createCodex({
    model: process.env.CODEX_MODEL ?? "gpt-5.6-sol",
  });
  const getBootstrap = harness.getBootstrap;
  if (getBootstrap === undefined) {
    throw new Error("The installed Codex harness does not expose a bootstrap recipe.");
  }

  return {
    ...harness,
    getBootstrap: async () => {
      const bootstrap = await getBootstrap();
      return {
        ...bootstrap,
        files: bootstrap.files.map((file) =>
          file.path.endsWith("/bridge.mjs")
            ? { ...file, content: patchBridgeForLocalCodex(file.content) }
            : file,
        ),
      };
    },
  };
}

function createLocalClaudeHarness() {
  const harness = createClaudeCode({
    maxTurns: 10,
    thinking: { display: "summarized", type: "adaptive" },
  });
  const getBootstrap = harness.getBootstrap;
  if (getBootstrap === undefined) {
    throw new Error("The installed Claude Code harness does not expose a bootstrap recipe.");
  }

  return {
    ...harness,
    getBootstrap: async () => {
      const bootstrap = await getBootstrap();
      return {
        ...bootstrap,
        files: [
          ...bootstrap.files.map((file) =>
            file.path.endsWith("/bridge.mjs")
              ? { ...file, content: patchBridgeForLocalClaude(file.content) }
              : file,
          ),
          {
            path: "/tmp/harness/claude-code/pnpm-workspace.yaml",
            content: 'allowBuilds:\n  "@anthropic-ai/claude-code": true\n',
          },
        ],
      };
    },
  };
}

function patchBridgeForLocalClaude(bridge: string): string {
  const executableNeedle = "      includePartialMessages: true,";
  if (!bridge.includes(executableNeedle)) {
    throw new Error("The installed Claude Code harness bridge no longer supports this POC patch.");
  }
  return bridge.replace(
    executableNeedle,
    `${executableNeedle}\n      ...process.env.OPENAGENTS_CLAUDE_BIN\n        ? { pathToClaudeCodeExecutable: process.env.OPENAGENTS_CLAUDE_BIN }\n        : {},`,
  );
}

function patchBridgeForLocalCodex(bridge: string): string {
  const needle = "const codex = new codexSdk.Codex({";
  if (!bridge.includes(needle)) {
    throw new Error("The installed Codex harness bridge no longer supports this POC patch.");
  }
  return bridge.replace(
    needle,
    'const codex = new codexSdk.Codex({ codexPathOverride: process.env.OPENAGENTS_CODEX_BIN ?? "codex",',
  );
}

async function readChatRequest(
  request: IncomingMessage,
): Promise<{ id: unknown; messages: UIMessage[]; provider: unknown }> {
  const chunks: Buffer[] = [];
  let length = 0;
  for await (const chunk of request) {
    const bytes = Buffer.from(chunk);
    length += bytes.length;
    if (length > 256 * 1024) throw new RequestError("Request body is too large.");
    chunks.push(bytes);
  }

  try {
    const value: unknown = JSON.parse(Buffer.concat(chunks).toString("utf8"));
    if (value === null || typeof value !== "object" || Array.isArray(value)) {
      throw new RequestError("Request body must be a JSON object.");
    }
    const body = value as { id?: unknown; messages?: unknown; provider?: unknown };
    if (!Array.isArray(body.messages)) throw new RequestError("messages must be an array.");
    return { id: body.id, messages: body.messages as UIMessage[], provider: body.provider };
  } catch (error) {
    if (error instanceof RequestError) throw error;
    throw new RequestError("Request body must be valid JSON.");
  }
}

function requireChatId(value: unknown): string {
  if (typeof value !== "string" || !/^[a-zA-Z0-9._-]{1,64}$/.test(value)) {
    throw new RequestError("Chat id must use only letters, numbers, dots, underscores, or hyphens.");
  }
  return value;
}

function setCorsHeaders(response: ServerResponse): void {
  response.setHeader("access-control-allow-origin", "*");
  response.setHeader("access-control-allow-methods", "POST, OPTIONS");
  response.setHeader("access-control-allow-headers", "content-type");
}

function sendJson(response: ServerResponse, status: number, body: unknown): void {
  response.writeHead(status, { "content-type": "application/json; charset=utf-8" });
  response.end(JSON.stringify(body));
}

async function dispose(): Promise<void> {
  await Promise.allSettled(
    [...harnessRuntimes.values()].flatMap((runtime) => [
      ...runtime.activeSessions,
    ]).map((harnessSession) => harnessSession.destroy()),
  );
  await Promise.all([...harnessRuntimes.values()].map((runtime) => runtime.sandbox.destroyAllSessions()));
  if (harnessServer !== undefined) {
    await new Promise<void>((resolve) => harnessServer?.close(() => resolve()));
  }
}

class RequestError extends Error {}
